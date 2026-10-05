//! What the start-up leaves on the heap, in a binary of its own: the counting allocator sees
//! every thread, so no other test may run beside it.

use std::{
    alloc::{GlobalAlloc, Layout, System},
    sync::atomic::{AtomicIsize, Ordering},
};

use embedded_graphics::prelude::Point;
use octowhere_ui::{
    chrome::FB,
    ui::{
        script::Driver,
        startup::{Outcome, Part},
    },
};

/// The bytes allocated and not yet freed.
static LIVE: AtomicIsize = AtomicIsize::new(0);

struct Counting;

// SAFETY: every call goes to `System` unchanged; the count is a side effect.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        LIVE.fetch_add(layout.size() as isize, Ordering::Relaxed);
        // SAFETY: the caller's obligations are `System`'s.
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        LIVE.fetch_sub(layout.size() as isize, Ordering::Relaxed);
        // SAFETY: as for `alloc`.
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

/// Plays the start-up through, drawing every frame, with a touch `skip_after` µs past the last
/// report if given, and returns what it left on the heap once the clock face has shown a while.
fn held_after_startup(skip_after: Option<u64>) -> isize {
    let mut fb = FB::boxed();
    let before = LIVE.load(Ordering::Relaxed);
    let mut driver = Driver::starting();
    driver.observe(|stage, _| stage.draw(&mut *fb));
    for part in Part::ALL {
        driver.wait(100_000);
        driver.boot(part, Outcome::Answered);
    }
    if let Some(after) = skip_after {
        driver.wait(after);
        driver.touch(Some(Point::new(400, 233)));
        assert!(
            !driver.stage.starting_up(),
            "the touch skipped the identity"
        );
        driver.stroke(&[]);
    }
    while driver.stage.starting_up() {
        driver.wait(100_000);
    }
    driver.wait(2_000_000);
    LIVE.load(Ordering::Relaxed) - before
}

#[test]
fn a_skipped_start_up_leaves_no_more_on_the_heap_than_one_played_through() {
    let played = held_after_startup(None);
    let skipped = held_after_startup(Some(1_500_000));
    assert!(
        skipped <= played + 1_024,
        "skipped {skipped} bytes, played through {played}"
    );
}
