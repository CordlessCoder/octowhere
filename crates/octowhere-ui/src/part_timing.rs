//! Accumulates time per numbered part of a draw, from a clock the firmware installs.

use core::sync::atomic::{AtomicU32, AtomicUsize, Ordering};

pub const PARTS: usize = 14;
pub const NAMES: [&str; PARTS] = [
    "clear",
    "field",
    "dashes",
    "parts",
    "band",
    "name",
    "strip",
    "line",
    "micro",
    "hatch",
    "reason",
    "barcode",
    "name_raster",
    "line_raster",
];
pub static TOTALS: [AtomicU32; PARTS] = [const { AtomicU32::new(0) }; PARTS];
static TIMER: AtomicUsize = AtomicUsize::new(0);
static LAST: AtomicU32 = AtomicU32::new(0);

pub fn install(timer: fn() -> u32) {
    TIMER.store(timer as usize, Ordering::Relaxed);
}

fn now() -> u32 {
    match TIMER.load(Ordering::Relaxed) {
        0 => 0,
        f => unsafe { core::mem::transmute::<usize, fn() -> u32>(f)() },
    }
}

/// Starts timing from now.
pub fn start() {
    LAST.store(now(), Ordering::Relaxed);
}

/// Charges the time since the last mark to `part`.
pub fn mark(part: usize) {
    let t = now();
    let last = LAST.swap(t, Ordering::Relaxed);
    TOTALS[part].fetch_add(t.wrapping_sub(last), Ordering::Relaxed);
}

/// Runs `f` and charges its time to `part`, apart from the marks.
pub fn charge<T>(part: usize, f: impl FnOnce() -> T) -> T {
    let t = now();
    let result = f();
    TOTALS[part].fetch_add(now().wrapping_sub(t), Ordering::Relaxed);
    result
}

/// Takes and zeroes the totals.
pub fn take() -> [u32; PARTS] {
    core::array::from_fn(|i| TOTALS[i].swap(0, Ordering::Relaxed))
}
