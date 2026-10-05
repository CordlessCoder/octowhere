//! Paints core 0's stack at boot and reports the deepest it has been used since.

use core::ptr::{addr_of, read_volatile, write_volatile};

/// What a word of stack that was never used holds.
const PAINT: u32 = 0x5AA5_C33C;
/// Left unpainted below the painter's own frame, for the register windows spilled there.
const MARGIN: usize = 512;

unsafe extern "C" {
    /// The stack's lowest address, which it grows down towards.
    static _stack_end_cpu0: u32;
    /// The stack's highest address, where it starts.
    static _stack_start_cpu0: u32;
}

/// Where this frame is on the stack, near enough to the stack pointer.
#[inline(always)]
fn stack_pointer() -> usize {
    let marker = 0u8;
    core::hint::black_box(addr_of!(marker)) as usize
}

/// Paints the stack below the caller's frame. Call it first in `main`, before anything else
/// has used the stack's depth.
#[inline(never)]
pub fn paint() {
    let bottom = (addr_of!(_stack_end_cpu0) as usize + 3) & !3;
    let top = stack_pointer() - MARGIN;
    let mut at = bottom;
    while at < top {
        // SAFETY: the words between the stack's end and below this frame are unused.
        unsafe { write_volatile(at as *mut u32, PAINT) };
        at += 4;
    }
}

/// The deepest the stack has been used since [`paint`], and its size, in bytes.
pub fn deepest() -> (usize, usize) {
    let bottom = (addr_of!(_stack_end_cpu0) as usize + 3) & !3;
    let top = addr_of!(_stack_start_cpu0) as usize;
    let mut at = bottom;
    // SAFETY: reads words inside core 0's stack.
    while at < top && unsafe { read_volatile(at as *const u32) } == PAINT {
        at += 4;
    }
    (top - at, top - bottom)
}
