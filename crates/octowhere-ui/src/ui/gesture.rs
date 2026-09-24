//! Single-contact gestures from raw touch samples: taps, and drags with their velocity.

use embedded_graphics_core::geometry::Point;

/// Microseconds on any monotonic clock.
pub type Micros = u64;

/// Travel on either axis, in pixels, beyond which a contact is a drag rather than a tap.
pub const TAP_SLOP: i32 = 16;
/// Velocity is measured over the samples in this trailing window.
pub const VELOCITY_WINDOW: Micros = 80_000;
/// Consecutive samples without contact before the contact counts as lifted. The controller
/// drops single samples mid-contact.
pub const LIFT_SAMPLES: u8 = 3;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Drag {
    pub start: Point,
    pub current: Point,
    /// Pixels per second over [`VELOCITY_WINDOW`], x then y.
    pub velocity: (f32, f32),
}

impl Drag {
    #[must_use]
    pub fn offset(&self) -> Point {
        self.current - self.start
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GestureEvent {
    None,
    /// Contact landed. Every contact starts with this, and ends with `Tap` or `DragEnd`.
    Down(Point),
    /// Contact left the tap slop. Later movement is reported as `DragMove`.
    DragStart(Drag),
    DragMove(Drag),
    /// Contact lifted after a drag, at the last position it touched.
    DragEnd(Drag),
    /// Contact lifted without leaving the slop, reported where it landed.
    Tap(Point),
}

const HISTORY: usize = 8;

#[derive(Clone, Copy, Debug, Default)]
struct Contact {
    start: Point,
    dragging: bool,
    /// The most recent samples with a position, newest at `history[newest]`.
    history: [(Point, Micros); HISTORY],
    newest: usize,
    len: usize,
    empty_samples: u8,
}

impl Contact {
    fn push(&mut self, position: Point, now: Micros) {
        self.newest = (self.newest + 1) % HISTORY;
        self.history[self.newest] = (position, now);
        self.len = (self.len + 1).min(HISTORY);
    }

    fn current(&self) -> Point {
        self.history[self.newest].0
    }

    fn velocity(&self) -> (f32, f32) {
        let (newest, newest_time) = self.history[self.newest];
        let mut oldest = (newest, newest_time);
        for back in 1..self.len {
            let sample = self.history[(self.newest + HISTORY - back) % HISTORY];
            if newest_time - sample.1 > VELOCITY_WINDOW {
                break;
            }
            oldest = sample;
        }
        let elapsed = (newest_time - oldest.1) as f32 / 1e6;
        if elapsed <= 0.0 {
            return (0.0, 0.0);
        }
        let travel = newest - oldest.0;
        (travel.x as f32 / elapsed, travel.y as f32 / elapsed)
    }

    fn drag(&self) -> Drag {
        Drag {
            start: self.start,
            current: self.current(),
            velocity: self.velocity(),
        }
    }
}

/// Feed it every touch read, with or without contact, and nothing else: velocity comes from
/// the sample times, so repeating a stale sample would read as the finger holding still.
#[derive(Clone, Copy, Debug, Default)]
pub struct GestureTracker {
    contact: Option<Contact>,
}

impl GestureTracker {
    /// Whether a contact is in progress, including one waiting out [`LIFT_SAMPLES`]. The caller
    /// keeps reading touch until this is false, or the lift is never seen.
    #[must_use]
    pub fn in_contact(&self) -> bool {
        self.contact.is_some()
    }

    pub fn update(&mut self, position: Option<Point>, now: Micros) -> GestureEvent {
        match (position, &mut self.contact) {
            (Some(position), None) => {
                let mut contact = Contact {
                    start: position,
                    ..Contact::default()
                };
                contact.push(position, now);
                self.contact = Some(contact);
                GestureEvent::Down(position)
            }
            (Some(position), Some(contact)) => {
                contact.empty_samples = 0;
                contact.push(position, now);
                let offset = position - contact.start;
                if contact.dragging {
                    GestureEvent::DragMove(contact.drag())
                } else if offset.x.abs() > TAP_SLOP || offset.y.abs() > TAP_SLOP {
                    contact.dragging = true;
                    GestureEvent::DragStart(contact.drag())
                } else {
                    GestureEvent::None
                }
            }
            (None, Some(contact)) => {
                contact.empty_samples += 1;
                if contact.empty_samples < LIFT_SAMPLES {
                    return GestureEvent::None;
                }
                let contact = *contact;
                self.contact = None;
                if contact.dragging {
                    GestureEvent::DragEnd(contact.drag())
                } else {
                    GestureEvent::Tap(contact.start)
                }
            }
            (None, None) => GestureEvent::None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FRAME: Micros = 20_000;

    fn run(tracker: &mut GestureTracker, samples: &[Option<(i32, i32)>]) -> Vec<GestureEvent> {
        samples
            .iter()
            .enumerate()
            .map(|(index, sample)| {
                tracker.update(
                    sample.map(|(x, y)| Point::new(x, y)),
                    index as Micros * FRAME,
                )
            })
            .collect()
    }

    #[test]
    fn a_short_contact_is_a_tap_where_it_landed() {
        let mut tracker = GestureTracker::default();
        let events = run(
            &mut tracker,
            &[Some((100, 200)), Some((104, 196)), None, None, None],
        );
        assert_eq!(events[0], GestureEvent::Down(Point::new(100, 200)));
        assert_eq!(events[1..4], [GestureEvent::None; 3]);
        assert_eq!(events[4], GestureEvent::Tap(Point::new(100, 200)));
        assert!(!tracker.in_contact());
    }

    #[test]
    fn a_drag_reports_start_moves_and_end_with_velocity() {
        let mut tracker = GestureTracker::default();
        let events = run(
            &mut tracker,
            &[
                Some((300, 200)),
                Some((280, 200)),
                Some((260, 200)),
                Some((240, 200)),
                None,
                None,
                None,
            ],
        );
        assert!(matches!(events[1], GestureEvent::DragStart(drag) if drag.offset() == Point::new(-20, 0)));
        assert!(matches!(events[3], GestureEvent::DragMove(_)));
        let GestureEvent::DragEnd(end) = events[6] else {
            panic!("expected a drag end, got {:?}", events[6]);
        };
        assert_eq!(end.current, Point::new(240, 200));
        // 20 px per 20 ms sample; the lift samples carry no position and do not slow it.
        assert!((end.velocity.0 + 1000.0).abs() < 1.0);
        assert_eq!(end.velocity.1, 0.0);
    }

    #[test]
    fn a_finger_that_stops_before_lifting_has_no_velocity() {
        let mut tracker = GestureTracker::default();
        let mut samples = vec![Some((100, 100)), Some((140, 100)), Some((180, 100))];
        samples.extend([Some((180, 100)); 6]);
        samples.extend([None; 3]);
        let events = run(&mut tracker, &samples);
        let GestureEvent::DragEnd(end) = events.last().copied().unwrap() else {
            panic!("expected a drag end");
        };
        assert_eq!(end.velocity, (0.0, 0.0));
    }

    #[test]
    fn a_short_gap_does_not_end_the_contact() {
        let mut tracker = GestureTracker::default();
        let events = run(
            &mut tracker,
            &[Some((100, 100)), None, None, Some((101, 100))],
        );
        assert_eq!(events[3], GestureEvent::None);
        assert!(tracker.in_contact());
    }

    #[test]
    fn a_drag_that_returns_to_its_start_is_still_a_drag() {
        let mut tracker = GestureTracker::default();
        let events = run(
            &mut tracker,
            &[Some((100, 100)), Some((160, 100)), Some((100, 100)), None, None, None],
        );
        assert!(matches!(events[5], GestureEvent::DragEnd(_)));
    }
}
