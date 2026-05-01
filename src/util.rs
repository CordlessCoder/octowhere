use core::{
    cell::UnsafeCell,
    iter::FusedIterator,
    sync::atomic::{AtomicBool, Ordering},
    task::{Poll, Waker},
};

use embassy_sync::waitqueue::AtomicWaker;

const CHUNK_SIZE: usize = 32;
#[inline]
pub fn fill_buf_repeat<'b>(mut buf: &'b mut [u8], mut data: &[u8], mut n: usize) -> &'b mut [u8] {
    if n == 0 {
        return &mut [];
    }
    let bytes = data.len() * n;
    assert!(buf.len() >= bytes);
    if bytes >= 4 * CHUNK_SIZE && CHUNK_SIZE.is_multiple_of(data.len()) {
        let mut arr = [0; CHUNK_SIZE];
        arr.chunks_exact_mut(data.len())
            .for_each(|chunk| chunk.copy_from_slice(data));
        let mut chunks = buf.chunks_exact_mut(arr.len());
        chunks
            .by_ref()
            .take(n / (CHUNK_SIZE / data.len()))
            .for_each(|chunk| {
                chunk.copy_from_slice(&arr);
            });
        n %= CHUNK_SIZE / data.len();
        buf = chunks.into_remainder();
    }
    buf.chunks_exact_mut(data.len()).take(n).for_each(|chunk| {
        chunk.copy_from_slice(data);
    });

    &mut buf[..data.len() * n]
}

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

// Shared:
// - Val1
// - Val2
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
    pub fn release(self) -> (T, T) {
        (self.val1.into_inner(), self.val2.into_inner())
    }
}

pub struct SwapThread<'s, T> {
    swap: &'s Swap<T>,
    has_val1: bool,
    is_thread1: bool,
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
unsafe impl<T: Sync> Sync for Swap<T> {}
unsafe impl<T: Sync> Send for SwapThread<'_, T> {}
unsafe impl<T: Sync> Sync for SwapThread<'_, T> {}
