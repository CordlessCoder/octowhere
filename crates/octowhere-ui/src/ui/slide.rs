//! The deliberate slide that confirms what cannot be taken back: a handle a drag carries to the
//! right. Only a drag that starts on the handle moves it, a release short of 90 % of the way
//! eases back over the settle, and nothing else confirms. Each screen places and draws its own.

use embedded_graphics::primitives::Rectangle;

use super::{
    ease::{Ease, SETTLE},
    gesture::{GestureEvent, Micros},
};

/// Where a drag must start to take the handle, and how far right it carries it.
pub struct Track {
    pub grab: Rectangle,
    pub travel: i32,
}

impl Track {
    /// 90 % of the travel, rounded up.
    const fn commit(&self) -> i32 {
        (self.travel * 9 + 9) / 10
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Slide {
    /// How far right the handle is, while a drag that started on it holds it.
    held: Option<i32>,
    /// Returning to the start from a release short of the end, since a time.
    back: Option<(i32, Micros)>,
    done: bool,
}

impl Slide {
    /// Follows a horizontal drag that starts on the handle, and returns whether it let go at
    /// the end. A tap, a flick that ends short and a vertical drag confirm nothing.
    pub fn handle(&mut self, event: &GestureEvent, now: Micros, track: &Track) -> bool {
        if self.done {
            return false;
        }
        match *event {
            GestureEvent::Down(_) => {
                self.back = None;
                self.held = None;
            }
            GestureEvent::DragStart(drag) => {
                self.held = (track.grab.contains(drag.start) && drag.is_horizontal())
                    .then(|| drag.offset().x.clamp(0, track.travel));
            }
            GestureEvent::DragMove(drag) => {
                if self.held.is_some() {
                    self.held = Some(drag.offset().x.clamp(0, track.travel));
                }
            }
            GestureEvent::DragEnd(drag) => {
                if self.held.take().is_some() {
                    let travel = drag.offset().x.clamp(0, track.travel);
                    if travel >= track.commit() {
                        self.done = true;
                        return true;
                    }
                    self.back = (travel > 0).then_some((travel, now));
                }
            }
            GestureEvent::Tap(_) | GestureEvent::None => {}
        }
        false
    }

    /// How far right the handle shows.
    #[must_use]
    pub fn travel(&self, now: Micros, track: &Track) -> i32 {
        if self.done {
            return track.travel;
        }
        if let Some(held) = self.held {
            return held;
        }
        match self.back {
            Some((from, start)) => libm::roundf(Ease::new(from as f32, 0, start).at(now).0) as i32,
            None => 0,
        }
    }

    /// Eases a released handle back, and says whether it still moves.
    pub fn step(&mut self, now: Micros) -> bool {
        if self
            .back
            .is_some_and(|(_, start)| now.saturating_sub(start) >= SETTLE)
        {
            self.back = None;
        }
        self.back.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::{gesture::Drag, group::layout::rect};
    use embedded_graphics::prelude::Point;

    const TRACK: Track = Track {
        grab: rect(82, 348, 142, 396),
        travel: 242,
    };

    fn drag(from: Point, to: Point) -> Drag {
        Drag {
            start: from,
            current: to,
            velocity: (0.0, 0.0),
        }
    }

    #[test]
    fn the_commit_is_ninety_percent_rounded_up() {
        assert_eq!(TRACK.commit(), 218);
        assert_eq!(
            Track {
                grab: TRACK.grab,
                travel: 168
            }
            .commit(),
            152
        );
    }

    #[test]
    fn only_a_drag_from_the_handle_to_near_the_end_confirms() {
        let start = Point::new(110, 372);
        let mut slide = Slide::default();
        let short = drag(start, start + Point::new(217, 0));
        assert!(!slide.handle(&GestureEvent::DragStart(short), 0, &TRACK));
        assert!(!slide.handle(&GestureEvent::DragEnd(short), 0, &TRACK));
        assert!(slide.step(1));
        let far = drag(start, start + Point::new(218, 0));
        slide.handle(&GestureEvent::DragStart(far), 0, &TRACK);
        assert!(slide.handle(&GestureEvent::DragEnd(far), 0, &TRACK));
        let elsewhere = drag(Point::new(200, 372), Point::new(420, 372));
        let mut other = Slide::default();
        other.handle(&GestureEvent::DragStart(elsewhere), 0, &TRACK);
        assert!(!other.handle(&GestureEvent::DragEnd(elsewhere), 0, &TRACK));
    }

    #[test]
    fn a_slide_cut_short_follows_no_later_drag() {
        let start = Point::new(110, 372);
        let mut slide = Slide::default();
        slide.handle(&GestureEvent::DragStart(drag(start, start)), 0, &TRACK);
        // Its end went elsewhere; a drag from off the handle follows.
        let later = drag(Point::new(110, 300), Point::new(400, 300));
        slide.handle(&GestureEvent::DragStart(later), 0, &TRACK);
        slide.handle(&GestureEvent::DragMove(later), 0, &TRACK);
        assert!(!slide.handle(&GestureEvent::DragEnd(later), 0, &TRACK));
    }
}
