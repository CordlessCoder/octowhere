use core::{
    cell::UnsafeCell,
    sync::atomic::{AtomicBool, Ordering},
    task::{Poll, Waker},
};

use embassy_sync::waitqueue::AtomicWaker;

pub use octowhere_ui::framebuffer::fill_buf_repeat;

pub fn widening_copy<const FACTOR: usize>(buf: &mut [u8], data: &[u8], width: usize) {
    const {
        assert!(FACTOR != 0);
    }
    if const { FACTOR == 1 } {
        buf.copy_from_slice(data);
    } else {
        buf.chunks_exact_mut(width * FACTOR)
            .zip(data.chunks_exact(width))
            .for_each(|(chunk, source)| {
                fill_buf_repeat(chunk, source, FACTOR);
            });
    }
}

/// Allows for efficiently implementing double-buffering accross tasks and even cores.
pub struct Swap<T> {
    val1: UnsafeCell<T>,
    val2: UnsafeCell<T>,
    thread1_has_val1: AtomicBool,
    thread1_wants_val1: AtomicBool,
    thread2_wants_val1: AtomicBool,
    waker1: AtomicWaker,
    waker2: AtomicWaker,
}

impl<T> Swap<T> {
    #[must_use]
    pub const fn new(val1: T, val2: T) -> Self {
        Self {
            val1: UnsafeCell::new(val1),
            val2: UnsafeCell::new(val2),
            thread1_has_val1: AtomicBool::new(true),
            thread1_wants_val1: AtomicBool::new(true),
            thread2_wants_val1: AtomicBool::new(false),
            waker1: AtomicWaker::new(),
            waker2: AtomicWaker::new(),
        }
    }
    #[must_use]
    pub fn split<'s>(&'s mut self) -> (SwapThread<'s, T>, SwapThread<'s, T>) {
        self.thread1_has_val1 = AtomicBool::new(true);
        self.thread1_wants_val1 = AtomicBool::new(true);
        self.thread2_wants_val1 = AtomicBool::new(false);
        (
            SwapThread {
                swap: self,
                has_val1: true,
                is_thread1: true,
                poisoned: false,
            },
            SwapThread {
                swap: self,
                has_val1: false,
                is_thread1: false,
                poisoned: false,
            },
        )
    }
    #[must_use]
    pub fn release(self) -> (T, T) {
        (self.val1.into_inner(), self.val2.into_inner())
    }
}

pub struct SwapThread<'s, T> {
    swap: &'s Swap<T>,
    has_val1: bool,
    is_thread1: bool,
    // Indicates a SwapThreadFuture was dropped or forgotten, leaving the inter-thread
    // synchronization logic in a possibly invalid state.
    poisoned: bool,
}

pub struct SwapThreadFuture<'r, 's, T> {
    inner: &'r mut SwapThread<'s, T>,
    state: SwapThreadFutureState,
}

enum SwapThreadFutureState {
    Started,
    WaitingForOtherThread,
    Completed,
}

// Sequence of  operations:
// - Set wants_val1
// - Check if other thread already set its wants_val1 to the inverse, if so wake it, set
// swap.thread1_has_val1 appropriately and complete
// - Register waker and repeat the above
impl<T> Future for SwapThreadFuture<'_, '_, T> {
    type Output = ();

    fn poll(
        mut self: core::pin::Pin<&mut Self>,
        cx: &mut core::task::Context<'_>,
    ) -> core::task::Poll<Self::Output> {
        self.register_waker(cx.waker());
        match self.state {
            SwapThreadFutureState::Started => {
                self.inner.poisoned = true;
                self.declare_wants_val1(self.wants_val1());
                self.state = SwapThreadFutureState::WaitingForOtherThread;
                self.check_for_completion_once()
            }
            SwapThreadFutureState::WaitingForOtherThread => self.check_for_completion_once(),
            SwapThreadFutureState::Completed => {
                unreachable!("Cannot poll SwapThreadFuture after completion")
            }
        }
    }
}

impl<T> SwapThreadFuture<'_, '_, T> {
    fn check_for_completion_once(&mut self) -> Poll<()> {
        if self.is_swap_complete() {
            self.finish_handoff();
            self.wake_other_thread();
            return Poll::Ready(());
        }
        if !self.is_other_thread_done() {
            return Poll::Pending;
        }
        self.finish_handoff();
        self.wake_other_thread();
        Poll::Ready(())
    }
    #[inline(always)]
    fn wants_val1(&self) -> bool {
        !self.inner.has_val1
    }
    fn finish_handoff(&mut self) {
        self.state = SwapThreadFutureState::Completed;
        let new_thread1_has_val1 = if self.inner.is_thread1 {
            !self.inner.has_val1
        } else {
            self.inner.has_val1
        };
        self.inner
            .swap
            .thread1_has_val1
            .store(new_thread1_has_val1, Ordering::Release);
        self.inner.has_val1 = self.wants_val1();
        self.inner.poisoned = false;
    }
    fn is_other_thread_done(&self) -> bool {
        let other_thread = if self.inner.is_thread1 {
            &self.inner.swap.thread2_wants_val1
        } else {
            &self.inner.swap.thread1_wants_val1
        };
        other_thread.load(Ordering::Acquire) == self.inner.has_val1
    }
    fn is_swap_complete(&self) -> bool {
        let old_thread1_has_val1 = if self.inner.is_thread1 {
            self.inner.has_val1
        } else {
            !self.inner.has_val1
        };
        self.inner.swap.thread1_has_val1.load(Ordering::Acquire) != old_thread1_has_val1
    }
    fn declare_wants_val1(&self, wants_val1: bool) {
        let val = if self.inner.is_thread1 {
            &self.inner.swap.thread1_wants_val1
        } else {
            &self.inner.swap.thread2_wants_val1
        };
        val.store(wants_val1, Ordering::Release);
    }
    fn register_waker(&self, waker: &Waker) {
        let storage = if self.inner.is_thread1 {
            &self.inner.swap.waker1
        } else {
            &self.inner.swap.waker2
        };
        storage.register(waker);
    }
    fn wake_other_thread(&self) {
        let waker = if self.inner.is_thread1 {
            &self.inner.swap.waker2
        } else {
            &self.inner.swap.waker1
        };
        waker.wake();
    }
}

impl<'s, T> SwapThread<'s, T> {
    pub fn get(&mut self) -> &mut T {
        assert!(
            !self.poisoned,
            "Cannot access value through a poisoned SwapThread,\
            a SwapThread is poisoned if its SwapThreadFuture is dropped before completion"
        );
        let ptr = if self.has_val1 {
            self.swap.val1.get()
        } else {
            self.swap.val2.get()
        };
        unsafe { &mut *ptr }
    }
    pub fn swap<'r>(&'r mut self) -> SwapThreadFuture<'r, 's, T> {
        SwapThreadFuture {
            inner: self,
            state: SwapThreadFutureState::Started,
        }
    }
}

unsafe impl<T: Send> Send for Swap<T> {}
unsafe impl<T: Send> Sync for Swap<T> {}
// Each handoff transfers exclusive access to T across threads.
unsafe impl<T: Send> Send for SwapThread<'_, T> {}
unsafe impl<T: Send> Sync for SwapThread<'_, T> {}

/// A touch read, as the queue to the frame loop weighs it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TouchKind {
    /// Fingers down.
    Contact,
    /// A lift, a cover or a gesture: a report the frame loop has to see.
    Kept,
    /// No report since the last read, or a failed read.
    Stale,
}

/// Where a touch read goes in the queue to the frame loop.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Place {
    Push,
    /// In place of the newest read.
    Replace,
    Drop,
}

/// Where a read of kind `read` goes, `back` being the newest read the frame loop has not taken
/// and `full` whether the queue is full. A newer report replaces a contact or a stale read, and
/// a stale read never displaces a report. A lift, cover or gesture stays, and the next report
/// queues behind it, or is dropped when there is no room: the frame loop is that far behind,
/// and the read it keeps already ends the touch.
#[must_use]
pub fn place_touch(read: TouchKind, back: Option<TouchKind>, full: bool) -> Place {
    match (read, back) {
        (_, None) => Place::Push,
        (TouchKind::Stale, Some(TouchKind::Contact | TouchKind::Kept)) => Place::Drop,
        (_, Some(TouchKind::Contact | TouchKind::Stale)) => Place::Replace,
        (_, Some(TouchKind::Kept)) if full => Place::Drop,
        (_, Some(TouchKind::Kept)) => Place::Push,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::{cell::Cell, pin::Pin, task::Context};

    fn assert_send_sync<T: Send + Sync>() {}

    /// The touch queue, two reads deep as the firmware's, after `reads` in turn.
    fn queued(reads: &[TouchKind]) -> heapless::Vec<TouchKind, 2> {
        let mut queue = heapless::Vec::new();
        for &read in reads {
            match place_touch(read, queue.last().copied(), queue.is_full()) {
                Place::Push => queue.push(read).unwrap(),
                Place::Replace => *queue.last_mut().unwrap() = read,
                Place::Drop => {}
            }
        }
        queue
    }

    #[test]
    fn a_full_touch_queue_keeps_its_lifts_and_covers() {
        use TouchKind::{Contact, Kept};
        assert_eq!(queued(&[Kept, Kept, Contact]), [Kept, Kept]);
        // A tap and then a touch, while the frame loop takes nothing.
        assert_eq!(
            queued(&[Contact, Kept, Contact, Kept, Contact]),
            [Kept, Kept]
        );
    }

    #[test]
    fn newer_reports_replace_contacts_and_stale_reads_never_displace_reports() {
        use TouchKind::{Contact, Kept, Stale};
        assert_eq!(queued(&[Contact, Contact, Contact]), [Contact]);
        assert_eq!(queued(&[Contact, Kept]), [Kept]);
        assert_eq!(queued(&[Kept, Contact, Contact]), [Kept, Contact]);
        assert_eq!(queued(&[Contact, Stale]), [Contact]);
        assert_eq!(queued(&[Stale, Stale, Contact]), [Contact]);
    }

    #[test]
    fn swap_accepts_send_values_that_are_not_sync() {
        assert_send_sync::<Swap<Cell<u8>>>();
        assert_send_sync::<SwapThread<'static, Cell<u8>>>();
    }

    #[test]
    fn handoff_repeats_between_both_threads() {
        let mut swap = Swap::new(1, 2);
        let (mut first, mut second) = swap.split();
        let waker = Waker::noop();
        let mut cx = Context::from_waker(waker);

        let mut first_swap = first.swap();
        let mut second_swap = second.swap();
        assert!(Pin::new(&mut first_swap).poll(&mut cx).is_pending());
        assert!(Pin::new(&mut second_swap).poll(&mut cx).is_ready());
        assert!(Pin::new(&mut first_swap).poll(&mut cx).is_ready());
        drop((first_swap, second_swap));
        assert_eq!(*first.get(), 2);
        assert_eq!(*second.get(), 1);

        let mut first_swap = first.swap();
        let mut second_swap = second.swap();
        assert!(Pin::new(&mut second_swap).poll(&mut cx).is_pending());
        assert!(Pin::new(&mut first_swap).poll(&mut cx).is_ready());
        assert!(Pin::new(&mut second_swap).poll(&mut cx).is_ready());
        drop((first_swap, second_swap));
        assert_eq!(*first.get(), 1);
        assert_eq!(*second.get(), 2);
    }

    #[test]
    #[should_panic(expected = "poisoned SwapThread")]
    fn cancelling_a_started_handoff_poisoned_thread() {
        let mut swap = Swap::new(1, 2);
        let (mut first, _second) = swap.split();
        let waker = Waker::noop();
        let mut cx = Context::from_waker(waker);
        let mut handoff = first.swap();
        assert!(Pin::new(&mut handoff).poll(&mut cx).is_pending());
        drop(handoff);
        let _ = first.get();
    }

    #[test]
    fn scoped_threads_exchange_repeatedly() {
        let mut swap = Swap::new(0u32, 100u32);
        let (mut first, mut second) = swap.split();
        std::thread::scope(|scope| {
            scope.spawn(move || {
                for _ in 0..16 {
                    *first.get() += 1;
                    let mut handoff = first.swap();
                    let waker = Waker::noop();
                    let mut cx = Context::from_waker(waker);
                    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
                    while Pin::new(&mut handoff).poll(&mut cx).is_pending() {
                        assert!(std::time::Instant::now() < deadline, "handoff stalled");
                        std::thread::yield_now();
                    }
                }
                assert_eq!(*first.get(), 16);
            });
            scope.spawn(move || {
                for _ in 0..16 {
                    *second.get() += 1;
                    let mut handoff = second.swap();
                    let waker = Waker::noop();
                    let mut cx = Context::from_waker(waker);
                    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
                    while Pin::new(&mut handoff).poll(&mut cx).is_pending() {
                        assert!(std::time::Instant::now() < deadline, "handoff stalled");
                        std::thread::yield_now();
                    }
                }
                assert_eq!(*second.get(), 116);
            });
        });
    }
}
