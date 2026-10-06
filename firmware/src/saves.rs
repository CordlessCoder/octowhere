//! The writes `settings_task` saves, settings and the mesh's state, kept so that powering off
//! can wait for them. Every write goes through here.

use core::{
    cell::RefCell,
    sync::atomic::{AtomicU32, Ordering},
};

use embassy_futures::select::{Either, select};
use embassy_sync::{
    blocking_mutex::{Mutex, raw::CriticalSectionRawMutex},
    channel::Channel,
    signal::Signal,
};
use octowhere::{settings, settings_queue::SettingsQueue};

/// The settings changes waiting, and whether one taken is still being saved.
static SETTINGS: Mutex<CriticalSectionRawMutex, RefCell<(SettingsQueue, bool)>> =
    Mutex::new(RefCell::new((SettingsQueue::new(), false)));
/// Signalled as a settings change is queued.
static SETTINGS_QUEUED: Signal<CriticalSectionRawMutex, ()> = Signal::new();
static GROUP: Channel<CriticalSectionRawMutex, settings::GroupWrite, 2> = Channel::new();
/// How many group writes were queued, and how many `settings_task` has finished, saved or not.
static QUEUED: AtomicU32 = AtomicU32::new(0);
static DONE: AtomicU32 = AtomicU32::new(0);
/// Group writes queued and done, numbered in the order they go, so that a waiter can tell its
/// own write's result from one queued ahead of it. One task queues them at a time.
static GROUP_QUEUED: AtomicU32 = AtomicU32::new(0);
static GROUP_DONE: AtomicU32 = AtomicU32::new(0);
/// Whether each of the last 32 group writes reached the flash, at its number's bit.
static GROUP_RESULTS: AtomicU32 = AtomicU32::new(0);
/// Signalled as each group write is done.
static GROUP_SAVED: Signal<CriticalSectionRawMutex, ()> = Signal::new();

/// Queues a change to the settings, which replaces one of the same setting not yet saved.
pub fn queue(write: settings::Write) {
    SETTINGS.lock(|settings| settings.borrow_mut().0.push(write));
    SETTINGS_QUEUED.signal(());
}

/// Queues a change to the mesh's state without waiting, and returns its number for
/// [`group_result`], or `None` when the queue is full.
pub fn queue_group(write: settings::GroupWrite) -> Option<u32> {
    GROUP.try_send(write).ok()?;
    QUEUED.fetch_add(1, Ordering::Relaxed);
    Some(GROUP_QUEUED.fetch_add(1, Ordering::Relaxed).wrapping_add(1))
}

/// Queues a change to the mesh's state, waiting for room, and returns its number for
/// [`group_saved`].
pub async fn send_group(write: settings::GroupWrite) -> u32 {
    QUEUED.fetch_add(1, Ordering::Relaxed);
    GROUP.send(write).await;
    GROUP_QUEUED.fetch_add(1, Ordering::Relaxed).wrapping_add(1)
}

/// Whether the group write numbered `number` reached the flash, once it is done. Only the last
/// 32 writes' results are kept.
pub fn group_result(number: u32) -> Option<bool> {
    // Wrapping: a write up to half the range behind is done.
    (GROUP_DONE.load(Ordering::Acquire).wrapping_sub(number) < u32::MAX / 2)
        .then(|| GROUP_RESULTS.load(Ordering::Relaxed) & 1 << (number % 32) != 0)
}

/// Waits for the group write numbered `number` and says whether it reached the flash.
pub async fn group_saved(number: u32) -> bool {
    loop {
        if let Some(saved) = group_result(number) {
            return saved;
        }
        GROUP_SAVED.wait().await;
    }
}

/// Whether every write queued has been done.
pub fn all_done() -> bool {
    SETTINGS.lock(|settings| {
        let (waiting, saving) = &*settings.borrow();
        waiting.is_empty() && !saving
    }) && DONE.load(Ordering::Acquire) == QUEUED.load(Ordering::Relaxed)
}

/// A write taken to save, which [`Taken::done`] reports on.
#[must_use]
pub struct Taken(Either<settings::Write, settings::GroupWrite>);

impl Taken {
    pub fn write(&self) -> &Either<settings::Write, settings::GroupWrite> {
        &self.0
    }

    /// Reports the write done, `saved` saying whether it reached the flash.
    pub fn done(self, saved: bool) {
        if let Either::First(_) = self.0 {
            SETTINGS.lock(|settings| settings.borrow_mut().1 = false);
            return;
        }
        let number = GROUP_DONE.load(Ordering::Relaxed).wrapping_add(1);
        let bit = 1 << (number % 32);
        if saved {
            GROUP_RESULTS.fetch_or(bit, Ordering::Relaxed);
        } else {
            GROUP_RESULTS.fetch_and(!bit, Ordering::Relaxed);
        }
        GROUP_DONE.store(number, Ordering::Release);
        GROUP_SAVED.signal(());
        DONE.fetch_add(1, Ordering::Release);
    }
}

/// Waits for the next write to save.
pub async fn take() -> Taken {
    loop {
        let next = SETTINGS.lock(|settings| {
            let (waiting, saving) = &mut *settings.borrow_mut();
            let next = waiting.pop();
            *saving = next.is_some();
            next
        });
        if let Some(write) = next {
            return Taken(Either::First(write));
        }
        if let Either::Second(write) = select(SETTINGS_QUEUED.wait(), GROUP.receive()).await {
            return Taken(Either::Second(write));
        }
    }
}
