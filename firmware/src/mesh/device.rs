//! The board around the node: the fix and the GNSS and RTC times the other tasks set, the
//! commands the screens send, and the view they are shown.

use alloc::boxed::Box;
use core::{
    cell::{Cell, RefCell},
    sync::atomic::{AtomicU32, Ordering},
};

use embassy_sync::{
    blocking_mutex::{Mutex as BlockingMutex, raw::CriticalSectionRawMutex},
    channel::Channel,
    signal::Signal,
};
use embassy_time::Instant;
use lc76g::FixQuality;
use octowhere::ui::group::view::{MeshView, MessagesView, Request};
use octowhere_mesh::packet::Quality;
use octowhere_node::{Command, Commands, Device, Fix, GpsTime, GroupStore, GroupWrite, blank_view};

/// The latest fix and the UTC second it was made in, from `gnss_task`.
pub static FIX: BlockingMutex<CriticalSectionRawMutex, Cell<Option<Fix>>> =
    BlockingMutex::new(Cell::new(None));
/// The RTC's UTC seconds and when they were read, from `sensor_task`, while its oscillator has
/// not stopped.
pub static RTC_TIME: BlockingMutex<CriticalSectionRawMutex, Cell<Option<(i64, Instant)>>> =
    BlockingMutex::new(Cell::new(None));
/// What the user asks of the mesh.
pub static COMMANDS: Channel<CriticalSectionRawMutex, Command, 4> = Channel::new();
/// What the screens show of the mesh, as last published, and a signal that it changed.
static VIEW: BlockingMutex<CriticalSectionRawMutex, RefCell<Option<Box<MeshView>>>> =
    BlockingMutex::new(RefCell::new(None));
pub static VIEW_CHANGED: Signal<CriticalSectionRawMutex, ()> = Signal::new();
/// Counts the views published, so a reader can tell when there is a new one.
static VIEWS: AtomicU32 = AtomicU32::new(0);
/// The messages the screens are shown, as last published, in memory [`lend_messages`] gave,
/// and how many times they have been.
static MESSAGES: BlockingMutex<
    CriticalSectionRawMutex,
    RefCell<Option<&'static mut MessagesView>>,
> = BlockingMutex::new(RefCell::new(None));
static MESSAGE_VIEWS: AtomicU32 = AtomicU32::new(0);

/// Gives the messages the screens are shown somewhere to be kept: PSRAM, since they are tens of
/// kilobytes. Until then the mesh's messages are not shown.
pub fn lend_messages(buffer: &'static mut MessagesView) {
    MESSAGES.lock(|held| *held.borrow_mut() = Some(buffer));
}

/// Copies the messages as last published into `into`, if they changed after the time counted
/// `seen`, and returns that count. The copy holds a critical section for as long as the
/// messages held take to copy, a millisecond or two when the store is full.
pub fn messages_since(seen: u32, into: &mut MessagesView) -> Option<u32> {
    let count = MESSAGE_VIEWS.load(Ordering::Acquire);
    if count == seen {
        return None;
    }
    MESSAGES.lock(|held| held.borrow().as_deref().map(|held| into.copy_from(held)))?;
    Some(count)
}

/// How the protocol grades a fix the GNSS module made.
#[must_use]
pub fn quality(fix: FixQuality) -> Quality {
    match fix {
        FixQuality::Autonomous => Quality::Autonomous,
        FixQuality::Differential | FixQuality::Pps | FixQuality::Rtk | FixQuality::FloatRtk => {
            Quality::Differential
        }
        _ => Quality::Estimated,
    }
}

/// Passes what the screens asked on to the mesh. Returns false when the queue is full.
pub fn request(request: Request) -> bool {
    COMMANDS.try_send(request.into()).is_ok()
}

/// Copies the mesh as last published into `into`, if it changed after the view counted `seen`,
/// and returns its count.
pub fn view_since(seen: u32, into: &mut MeshView) -> Option<u32> {
    let count = VIEWS.load(Ordering::Acquire);
    if count == seen {
        return None;
    }
    VIEW.lock(|view| view.borrow().as_deref().map(|view| into.copy_from(view)))?;
    Some(count)
}

/// Publishes `view` when it differs from what the screens were last shown. It trades places
/// with the view it replaces, so it holds an older one after.
pub fn publish(view: &mut Box<MeshView>) {
    let changed = VIEW.lock(|current| {
        let mut current = current.borrow_mut();
        match &mut *current {
            Some(current) if **current == **view => return false,
            Some(current) => core::mem::swap(current, view),
            None => *current = Some(core::mem::replace(view, blank_view())),
        }
        VIEWS.fetch_add(1, Ordering::Release);
        true
    });
    if changed {
        VIEW_CHANGED.signal(());
    }
}

pub struct BoardDevice;

impl Device for BoardDevice {
    fn fix(&self) -> Option<Fix> {
        FIX.lock(Cell::get)
    }

    fn gps_time(&self) -> Option<GpsTime> {
        crate::GPS_TIME.lock(Cell::get).map(|gps| GpsTime {
            offset: gps.offset,
            updated: gps.updated.as_micros() as i64,
        })
    }

    fn rtc_utc(&self, now: i64) -> Option<i64> {
        RTC_TIME
            .lock(Cell::get)
            .map(|(seconds, read)| seconds * 1_000_000 + now - read.as_micros() as i64)
    }

    fn publish(&self, view: &mut Box<MeshView>) {
        publish(view);
    }

    fn publish_messages(&self, messages: &MessagesView) -> bool {
        let shown = MESSAGES.lock(|held| match &mut *held.borrow_mut() {
            Some(held) => {
                held.copy_from(messages);
                true
            }
            None => false,
        });
        if shown {
            MESSAGE_VIEWS.fetch_add(1, Ordering::Release);
            VIEW_CHANGED.signal(());
        }
        shown
    }
}

/// The commands the screens and the debugger queue in [`COMMANDS`].
pub struct BoardCommands;

impl Commands for BoardCommands {
    async fn receive(&self) -> Command {
        COMMANDS.receive().await
    }
}

pub struct BoardGroupStore;

impl GroupStore for BoardGroupStore {
    fn queue(&self, write: GroupWrite) -> Option<u32> {
        crate::queue_group_write(write)
    }

    async fn send(&self, write: GroupWrite) -> u32 {
        crate::send_group_write(write).await
    }

    fn result(&self, number: u32) -> Option<bool> {
        crate::group_result(number)
    }

    async fn saved(&self, number: u32) -> bool {
        crate::group_saved(number).await
    }
}
