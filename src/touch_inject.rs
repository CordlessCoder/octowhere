//! Lets a debugger touch the screen, cover it and press the power key, and find the framebuffers
//! to read the screen back, for driving the screens on a board nobody is holding;
//! `tools/touch-inject.py` does.

use core::sync::atomic::{AtomicU32, Ordering};

use embassy_time::{Duration, Instant};
use embedded_graphics::prelude::Point;
use octowhere::ui::stage::{Key, Touch};

/// Where a stroke starts and ends, `x | y << 16` each.
#[unsafe(no_mangle)]
static OCTOWHERE_TOUCH_PATH: [AtomicU32; 2] = [const { AtomicU32::new(0) }; 2];
/// What to do in the low byte, and a stroke's length in milliseconds above it. The debugger
/// writes it last, and the firmware clears it as it takes it.
#[unsafe(no_mangle)]
static OCTOWHERE_TOUCH_GO: AtomicU32 = AtomicU32::new(0);
/// The two framebuffers' addresses, and the one drawn last.
#[unsafe(no_mangle)]
static OCTOWHERE_FRAMEBUFFERS: [AtomicU32; 3] = [const { AtomicU32::new(0) }; 3];

const STROKE: u32 = 1;
const COVER: u32 = 2;
const SHORT_PRESS: u32 = 3;
const LONG_PRESS: u32 = 4;

struct Stroke {
    from: Point,
    to: Point,
    start: Instant,
    duration: Duration,
}

#[derive(Default)]
pub struct Injector {
    stroke: Option<Stroke>,
}

fn point(word: u32) -> Point {
    Point::new((word & 0xffff) as i32, (word >> 16) as i32)
}

impl Injector {
    /// The report and key press a debugger asked for, as of now. A stroke reports a contact on
    /// every call until its time is up, then its lift.
    pub fn next(&mut self) -> (Option<Touch>, Option<Key>) {
        let now = Instant::now();
        let go = OCTOWHERE_TOUCH_GO.swap(0, Ordering::Acquire);
        match go & 0xff {
            STROKE => {
                self.stroke = Some(Stroke {
                    from: point(OCTOWHERE_TOUCH_PATH[0].load(Ordering::Relaxed)),
                    to: point(OCTOWHERE_TOUCH_PATH[1].load(Ordering::Relaxed)),
                    start: now,
                    duration: Duration::from_millis(u64::from(go >> 8)),
                });
            }
            COVER => return (Some(Touch::Cover), None),
            SHORT_PRESS => return (None, Some(Key::Short)),
            LONG_PRESS => return (None, Some(Key::Long)),
            _ => {}
        }
        let Some(stroke) = &self.stroke else {
            return (None, None);
        };
        let elapsed = now - stroke.start;
        // A tap's single contact counts as its first report, so a stroke always lifts later.
        if elapsed > stroke.duration && elapsed.as_millis() > 0 {
            let to = stroke.to;
            self.stroke = None;
            return (Some(Touch::Lifted(to)), None);
        }
        let (done, whole) = (
            elapsed.as_micros().min(stroke.duration.as_micros()) as i64,
            stroke.duration.as_micros().max(1) as i64,
        );
        let along = |from: i32, to: i32| from + (i64::from(to - from) * done / whole) as i32;
        let at = Point::new(
            along(stroke.from.x, stroke.to.x),
            along(stroke.from.y, stroke.to.y),
        );
        (Some(Touch::Contacts([Some(at), None])), None)
    }

    /// Whether a stroke is under way, which needs steps a frame apart.
    pub fn is_stroking(&self) -> bool {
        self.stroke.is_some()
    }
}

/// Notes the framebuffer about to be drawn into, for the debugger to read the screen from.
pub fn drawing(buffer: &[u8]) {
    let address = buffer.as_ptr() as u32;
    let [first, second, last] = &OCTOWHERE_FRAMEBUFFERS;
    if first.load(Ordering::Relaxed) == 0 {
        first.store(address, Ordering::Relaxed);
    } else if first.load(Ordering::Relaxed) != address {
        second.store(address, Ordering::Relaxed);
    }
    last.store(address, Ordering::Release);
}
