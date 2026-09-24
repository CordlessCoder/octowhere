//! The settings panel's vertical travel over the faces: it follows a drag, and settles open or
//! closed on release.

use super::ease::Ease;
use super::gesture::{Drag, Micros};

/// A release past this fraction of the height completes the open or the close.
const COMMIT_FRACTION: i32 = 4;
/// A release faster than this, in pixels per second, completes it however short the drag.
const FLICK_VELOCITY: f32 = 600.0;

#[derive(Clone, Copy, Debug, PartialEq)]
enum Motion {
    Rest,
    /// Following a drag, which started at `from`.
    Dragging { from: i32, offset: i32 },
    Settling { ease: Ease, offset: f32 },
}

/// How far the panel has come down over the faces: 0 closed, the height open.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sheet {
    height: i32,
    open: bool,
    motion: Motion,
}

impl Sheet {
    #[must_use]
    pub const fn new(height: i32) -> Self {
        Self { height, open: false, motion: Motion::Rest }
    }

    #[must_use]
    pub const fn height(&self) -> i32 {
        self.height
    }

    /// The panel's bottom edge, in rows from the top of the panel.
    #[must_use]
    pub fn offset(&self) -> i32 {
        match self.motion {
            Motion::Rest => if self.open { self.height } else { 0 },
            Motion::Dragging { offset, .. } => offset,
            Motion::Settling { offset, .. } => libm::roundf(offset) as i32,
        }
    }

    #[must_use]
    pub const fn is_moving(&self) -> bool {
        !matches!(self.motion, Motion::Rest)
    }

    #[must_use]
    pub const fn is_dragging(&self) -> bool {
        matches!(self.motion, Motion::Dragging { .. })
    }

    /// Open and at rest.
    #[must_use]
    pub const fn is_open(&self) -> bool {
        self.open && matches!(self.motion, Motion::Rest)
    }

    /// Closed and at rest.
    #[must_use]
    pub const fn is_closed(&self) -> bool {
        !self.open && matches!(self.motion, Motion::Rest)
    }

    /// Completes a settle in progress at once.
    pub fn finish(&mut self) {
        if let Motion::Settling { ease, .. } = self.motion {
            self.open = ease.to() == self.height;
            self.motion = Motion::Rest;
        }
    }

    /// Starts following a drag from where the panel is now.
    pub fn grab(&mut self, drag: &Drag) {
        self.finish();
        let from = self.offset();
        self.motion = Motion::Dragging { from, offset: self.follow(from, drag) };
    }

    pub fn drag(&mut self, drag: &Drag) {
        if let Motion::Dragging { from, .. } = self.motion {
            self.motion = Motion::Dragging { from, offset: self.follow(from, drag) };
        }
    }

    fn follow(&self, from: i32, drag: &Drag) -> i32 {
        (from + drag.offset().y).clamp(0, self.height)
    }

    /// Lets go: completes the open or close the drag was making if it went far or fast
    /// enough, and otherwise returns to where it started.
    pub fn release(&mut self, drag: &Drag, now: Micros) {
        let Motion::Dragging { from, offset } = self.motion else {
            return;
        };
        let threshold = self.height / COMMIT_FRACTION;
        let velocity = drag.velocity.1;
        let opening = from == 0;
        let target = if opening {
            offset >= threshold || velocity >= FLICK_VELOCITY
        } else {
            !(self.height - offset >= threshold || velocity <= -FLICK_VELOCITY)
        };
        self.settle_to(target, offset as f32, now);
    }

    /// Animates to open or closed from wherever the panel is.
    pub fn go(&mut self, open: bool, now: Micros) {
        self.finish();
        self.settle_to(open, self.offset() as f32, now);
    }

    /// Shows it open or closed at once.
    pub fn set(&mut self, open: bool) {
        self.open = open;
        self.motion = Motion::Rest;
    }

    fn settle_to(&mut self, open: bool, offset: f32, now: Micros) {
        let target = if open { self.height } else { 0 };
        self.open = open;
        self.motion = if target as f32 == offset {
            Motion::Rest
        } else {
            Motion::Settling { ease: Ease::new(offset, target, now), offset }
        };
    }

    /// Advances a settle to `now`. Call it before taking each frame's offset.
    pub fn step(&mut self, now: Micros) {
        let Motion::Settling { ease, .. } = self.motion else {
            return;
        };
        self.motion = match ease.at(now) {
            (_, true) => Motion::Rest,
            (offset, false) => Motion::Settling { ease, offset },
        };
    }
}

#[cfg(test)]
mod tests {
    use embedded_graphics_core::geometry::Point;

    use super::*;

    const HEIGHT: i32 = 466;

    fn drag(dy: i32, velocity: f32) -> Drag {
        Drag {
            start: Point::new(233, 100),
            current: Point::new(233, 100 + dy),
            velocity: (0.0, velocity),
        }
    }

    fn settle(sheet: &mut Sheet) {
        for frame in 1..=100 {
            sheet.step(frame * 20_000);
        }
    }

    #[test]
    fn a_drag_past_a_quarter_opens_and_a_short_one_springs_back() {
        let mut sheet = Sheet::new(HEIGHT);
        sheet.grab(&drag(20, 0.0));
        sheet.drag(&drag(200, 0.0));
        assert_eq!(sheet.offset(), 200);
        sheet.release(&drag(200, 0.0), 0);
        settle(&mut sheet);
        assert!(sheet.is_open());

        sheet.grab(&drag(-20, 0.0));
        sheet.release(&drag(-60, 0.0), 0);
        settle(&mut sheet);
        assert!(sheet.is_open());
    }

    #[test]
    fn a_flick_completes_however_short() {
        let mut sheet = Sheet::new(HEIGHT);
        sheet.grab(&drag(30, 0.0));
        sheet.release(&drag(40, 900.0), 0);
        settle(&mut sheet);
        assert!(sheet.is_open());
        sheet.grab(&drag(-30, 0.0));
        sheet.release(&drag(-40, -900.0), 0);
        settle(&mut sheet);
        assert!(sheet.is_closed());
    }

    #[test]
    fn the_panel_stops_at_both_ends() {
        let mut sheet = Sheet::new(HEIGHT);
        sheet.grab(&drag(20, 0.0));
        sheet.drag(&drag(900, 0.0));
        assert_eq!(sheet.offset(), HEIGHT);
        sheet.drag(&drag(-50, 0.0));
        assert_eq!(sheet.offset(), 0);
    }
}
