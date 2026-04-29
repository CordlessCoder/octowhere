use core::{cell::UnsafeCell, sync::atomic::AtomicBool};

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
    thread1_wants_val2: AtomicBool,
    thread2_wants_val1: AtomicBool,
    waker1: AtomicWaker,
    waker2: AtomicWaker,
}

impl<T> Swap<T> {
    pub const fn new(val1: T, val2: T) -> Self {
        Self {
            val1: UnsafeCell::new(val1),
            val2: UnsafeCell::new(val2),
            thread1_wants_val2: AtomicBool::new(false),
            thread2_wants_val1: AtomicBool::new(false),
            waker1: AtomicWaker::new(),
            waker2: AtomicWaker::new(),
        }
    }
    pub fn split<'s>(&'s mut self) -> (SwapThread<'s, T>, SwapThread<'s, T>) {
        self.thread1_wants_val2 = AtomicBool::new(false);
        self.thread2_wants_val1 = AtomicBool::new(false);
        (
            SwapThread {
                swap: self,
                has_val1: true,
            },
            SwapThread {
                swap: self,
                has_val1: false,
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
}

impl<'s, T> SwapThread<'s, T> {
    pub fn get(&mut self) -> &mut T {
        let ptr = if self.has_val1 {
            self.swap.val1.get()
        } else {
            self.swap.val2.get()
        };
        unsafe { &mut *ptr }
    }
    pub async fn swap(&mut self) {}
}

unsafe impl<T: Send> Send for Swap<T> {}
unsafe impl<T: Sync> Sync for Swap<T> {}
unsafe impl<T: Sync> Send for SwapThread<'_, T> {}
unsafe impl<T: Sync> Sync for SwapThread<'_, T> {}
