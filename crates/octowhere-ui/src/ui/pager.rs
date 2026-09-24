//! Horizontal switching between a ring of pages, driven by drags and animated to rest.

use super::ease::Ease;
use super::gesture::{Drag, GestureEvent, Micros};

/// A release past this fraction of the width moves to the next page.
const COMMIT_FRACTION: i32 = 4;
/// A release faster than this, in pixels per second, moves to the next page in its direction
/// however short the drag.
const FLICK_VELOCITY: f32 = 600.0;

#[derive(Clone, Copy, Debug, PartialEq)]
enum Motion {
    Rest,
    /// Following a horizontal drag.
    Dragging { offset: i32 },
    /// A vertical drag, which the pager leaves alone until it lifts.
    Ignoring,
    /// Moving toward `target`: 0 to stay, or one width either way to change page.
    Settling { ease: Ease, offset: f32 },
}

/// What to draw: the current page shifted right by `offset` pixels, and while it is shifted, the
/// neighbour that the shift uncovers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PagerView {
    pub page: usize,
    pub offset: i32,
    /// The uncovered page and its own offset.
    pub neighbour: Option<(usize, i32)>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pager {
    page: usize,
    count: usize,
    width: i32,
    motion: Motion,
}

impl Pager {
    #[must_use]
    pub const fn new(page: usize, count: usize, width: i32) -> Self {
        Self {
            page,
            count,
            width,
            motion: Motion::Rest,
        }
    }

    #[must_use]
    pub const fn page(&self) -> usize {
        self.page
    }

    /// Whether the pages are off rest, so the view changes without further input.
    #[must_use]
    pub const fn is_moving(&self) -> bool {
        !matches!(self.motion, Motion::Rest | Motion::Ignoring)
    }

    fn wrap(&self, page: isize) -> usize {
        page.rem_euclid(self.count as isize) as usize
    }

    /// Settles any motion in progress at once, as though it had finished.
    fn finish(&mut self) {
        if let Motion::Settling { ease, .. } = self.motion {
            self.land(ease.to());
        }
        self.motion = Motion::Rest;
    }

    fn land(&mut self, target: i32) {
        if target < 0 {
            self.page = self.wrap(self.page as isize + 1);
        } else if target > 0 {
            self.page = self.wrap(self.page as isize - 1);
        }
        self.motion = Motion::Rest;
    }

    fn settle(&mut self, offset: f32, target: i32, now: Micros) {
        self.motion = Motion::Settling { ease: Ease::new(offset, target, now), offset };
    }

    /// Animates one page along, as a drag released past the commit point would.
    pub fn advance(&mut self, forward: bool, now: Micros) {
        self.finish();
        self.settle(0.0, if forward { -self.width } else { self.width }, now);
    }

    pub fn handle(&mut self, event: &GestureEvent, now: Micros) {
        match *event {
            // Catching a settling page completes its move rather than grabbing it mid-way.
            GestureEvent::Down(_) => self.finish(),
            GestureEvent::DragStart(drag) => {
                let offset = drag.offset();
                self.motion = if offset.x.abs() >= offset.y.abs() {
                    Motion::Dragging {
                        offset: self.clamp(offset.x),
                    }
                } else {
                    Motion::Ignoring
                };
            }
            GestureEvent::DragMove(drag) => {
                if let Motion::Dragging { offset } = &mut self.motion {
                    *offset = drag.offset().x.clamp(-self.width, self.width);
                }
            }
            GestureEvent::DragEnd(drag) => match self.motion {
                Motion::Dragging { .. } => {
                    let offset = drag.offset().x.clamp(-self.width, self.width);
                    let target = self.release_target(offset, &drag);
                    self.settle(offset as f32, target, now);
                }
                _ => self.motion = Motion::Rest,
            },
            GestureEvent::Tap(_) | GestureEvent::None => {}
        }
    }

    fn clamp(&self, offset: i32) -> i32 {
        offset.clamp(-self.width, self.width)
    }

    fn release_target(&self, offset: i32, drag: &Drag) -> i32 {
        let velocity = drag.velocity.0;
        let threshold = self.width / COMMIT_FRACTION;
        if offset < 0 && (offset <= -threshold || velocity <= -FLICK_VELOCITY) {
            -self.width
        } else if offset > 0 && (offset >= threshold || velocity >= FLICK_VELOCITY) {
            self.width
        } else {
            0
        }
    }

    /// Advances a settle to `now`. Call it before taking each frame's view.
    pub fn step(&mut self, now: Micros) {
        let Motion::Settling { ease, .. } = self.motion else {
            return;
        };
        match ease.at(now) {
            (_, true) => self.land(ease.to()),
            (offset, false) => self.motion = Motion::Settling { ease, offset },
        }
    }

    #[must_use]
    pub fn view(&self) -> PagerView {
        let offset = match self.motion {
            Motion::Dragging { offset } => offset,
            Motion::Settling { offset, .. } => libm::roundf(offset) as i32,
            Motion::Rest | Motion::Ignoring => 0,
        };
        let neighbour = match offset {
            0 => None,
            offset if offset < 0 => {
                Some((self.wrap(self.page as isize + 1), offset + self.width))
            }
            offset => Some((self.wrap(self.page as isize - 1), offset - self.width)),
        };
        PagerView {
            page: self.page,
            offset,
            neighbour,
        }
    }
}

#[cfg(test)]
mod tests {
    use embedded_graphics_core::geometry::Point;

    use super::*;
    use crate::ui::ease::SETTLE;

    const WIDTH: i32 = 466;

    fn drag(dx: i32, dy: i32, velocity: f32) -> Drag {
        Drag {
            start: Point::new(233, 233),
            current: Point::new(233 + dx, 233 + dy),
            velocity: (velocity, 0.0),
        }
    }

    fn settle(pager: &mut Pager, from: Micros) {
        for frame in 1..=100 {
            pager.step(from + frame * 20_000);
        }
    }

    #[test]
    fn a_drag_shows_the_neighbour_it_uncovers() {
        let mut pager = Pager::new(0, 7, WIDTH);
        pager.handle(&GestureEvent::DragStart(drag(-20, 0, 0.0)), 0);
        pager.handle(&GestureEvent::DragMove(drag(-100, 5, 0.0)), 0);
        assert_eq!(
            pager.view(),
            PagerView {
                page: 0,
                offset: -100,
                neighbour: Some((1, WIDTH - 100)),
            }
        );
        pager.handle(&GestureEvent::DragMove(drag(60, 0, 0.0)), 0);
        assert_eq!(pager.view().neighbour, Some((6, 60 - WIDTH)));
    }

    #[test]
    fn a_long_drag_moves_on_and_a_short_one_springs_back() {
        let mut pager = Pager::new(0, 7, WIDTH);
        pager.handle(&GestureEvent::DragStart(drag(-20, 0, 0.0)), 0);
        pager.handle(&GestureEvent::DragEnd(drag(-200, 0, 0.0)), 0);
        assert!(pager.is_moving());
        settle(&mut pager, 0);
        assert_eq!(pager.page(), 1);
        assert!(!pager.is_moving());

        pager.handle(&GestureEvent::DragStart(drag(20, 0, 0.0)), 0);
        pager.handle(&GestureEvent::DragEnd(drag(60, 0, 0.0)), 0);
        settle(&mut pager, 0);
        assert_eq!(pager.page(), 1);
        assert_eq!(pager.view().offset, 0);
    }

    #[test]
    fn a_flick_moves_on_however_short() {
        let mut pager = Pager::new(0, 7, WIDTH);
        pager.handle(&GestureEvent::DragStart(drag(20, 0, 0.0)), 0);
        pager.handle(&GestureEvent::DragEnd(drag(40, 0, 900.0)), 0);
        settle(&mut pager, 0);
        assert_eq!(pager.page(), 6);
    }

    #[test]
    fn the_settle_lands_after_a_fixed_time() {
        let mut pager = Pager::new(0, 7, WIDTH);
        pager.handle(&GestureEvent::DragStart(drag(-20, 0, 0.0)), 0);
        pager.handle(&GestureEvent::DragEnd(drag(-200, 0, 0.0)), 0);
        pager.step(SETTLE / 2);
        assert!(pager.is_moving());
        assert!(pager.view().offset > -WIDTH);
        pager.step(SETTLE);
        assert!(!pager.is_moving());
        assert_eq!(pager.page(), 1);
    }

    #[test]
    fn a_vertical_drag_is_left_alone() {
        let mut pager = Pager::new(3, 7, WIDTH);
        pager.handle(&GestureEvent::DragStart(drag(5, -30, 0.0)), 0);
        pager.handle(&GestureEvent::DragMove(drag(200, -40, 0.0)), 0);
        assert_eq!(pager.view().offset, 0);
        pager.handle(&GestureEvent::DragEnd(drag(200, -40, 0.0)), 0);
        assert_eq!(pager.page(), 3);
    }

    #[test]
    fn landing_during_a_settle_completes_it() {
        let mut pager = Pager::new(0, 7, WIDTH);
        pager.advance(true, 0);
        pager.step(10_000);
        pager.handle(&GestureEvent::Down(Point::new(10, 10)), 10_000);
        assert_eq!(pager.page(), 1);
        assert!(!pager.is_moving());
    }

    #[test]
    fn advance_animates_one_page_either_way() {
        let mut pager = Pager::new(0, 7, WIDTH);
        pager.advance(false, 0);
        assert_eq!(pager.view().offset, 0);
        pager.step(20_000);
        assert!(pager.view().offset > 0);
        settle(&mut pager, 20_000);
        assert_eq!(pager.page(), 6);
    }
}
