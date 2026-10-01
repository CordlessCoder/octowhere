//! Pixel shift against burn-in: the whole picture moves by a few pixels now and then, along a
//! fixed round of positions (round 3 §4, the 2026-10-01 always-on hand-off). The screens draw
//! as though unshifted; the display core moves the picture as it sends it, so nothing here
//! costs a draw. The stage picks when, and takes the offset off every touch.

use embedded_graphics::prelude::Point;

use super::gesture::Micros;

/// The positions, in order: the middle, then eight round a 3 px circle.
pub const POSITIONS: [(i8, i8); 9] = [
    (0, 0),
    (3, 0),
    (2, 2),
    (0, 3),
    (-2, 2),
    (-3, 0),
    (-2, -2),
    (0, -3),
    (2, -2),
];
/// The furthest any position moves the picture along either axis. Whatever is drawn to the
/// glass's edge reaches this far past it, so that it still meets the edge shifted.
pub const REACH: i32 = 3;
/// How long the picture may stay put while the screen is awake before a minute's change moves
/// it, failing a moment that hides the move sooner.
pub const HOLD: Micros = 10 * 60 * 1_000_000;

/// Where the picture is in its round, and when it last moved.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Shift {
    index: u8,
    moved_at: Micros,
}

impl Shift {
    /// How far the picture moves on the panel.
    #[must_use]
    pub fn offset(self) -> Point {
        let (x, y) = POSITIONS[usize::from(self.index)];
        Point::new(i32::from(x), i32::from(y))
    }

    /// Moves one step round.
    pub fn advance(&mut self, now: Micros) {
        self.index = (self.index + 1) % POSITIONS.len() as u8;
        self.moved_at = now;
    }

    /// Whether the picture has stayed put for `HOLD`.
    #[must_use]
    pub fn is_due(self, now: Micros) -> bool {
        now.saturating_sub(self.moved_at) >= HOLD
    }
}

/// The framebuffer column or row a panel one shows when the picture is moved by `offset`, with
/// the framebuffer's edge repeated past it.
#[must_use]
pub fn source(panel: i32, offset: i32, length: i32) -> i32 {
    (panel - offset).clamp(0, length - 1)
}

/// How panel columns `x0..x1` take a framebuffer row `width` wide with the picture moved `dx`
/// across: how many repeat the row's first pixel, the framebuffer column the rest start from,
/// and how many of those repeat its last pixel after the ones copied.
#[must_use]
pub fn columns(x0: i32, x1: i32, width: i32, dx: i32) -> (usize, usize, usize) {
    let span = x1 - x0;
    let left = (dx - x0).clamp(0, span);
    let right = (x1 - (width + dx)).clamp(0, span - left);
    (left as usize, (x0 + left - dx) as usize, right as usize)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_step_moves_at_most_the_reach_and_the_round_closes() {
        let mut shift = Shift::default();
        assert_eq!(shift.offset(), Point::zero());
        for step in 1..=POSITIONS.len() {
            let before = shift.offset();
            shift.advance(step as Micros);
            let moved = shift.offset() - before;
            assert!(moved.x.abs() <= REACH && moved.y.abs() <= REACH, "{step}");
            assert!(shift.offset().x.abs() <= REACH && shift.offset().y.abs() <= REACH);
        }
        assert_eq!(shift.offset(), Point::zero());
    }

    #[test]
    fn it_is_due_once_it_has_held_for_ten_minutes() {
        let mut shift = Shift::default();
        shift.advance(1_000);
        assert!(!shift.is_due(1_000 + HOLD - 1));
        assert!(shift.is_due(1_000 + HOLD));
    }

    #[test]
    fn a_rows_columns_split_as_each_pixel_takes_its_source() {
        for dx in -REACH..=REACH {
            for (x0, x1) in [
                (0, 466),
                (0, 2),
                (2, 10),
                (400, 466),
                (464, 466),
                (100, 300),
            ] {
                let (left, from, right) = columns(x0, x1, 466, dx);
                for (i, x) in (x0..x1).enumerate() {
                    let expected = source(x, dx, 466);
                    let got = if i < left {
                        0
                    } else if i >= (x1 - x0) as usize - right {
                        465
                    } else {
                        (from + i - left) as i32
                    };
                    assert_eq!(got, expected, "dx {dx} x {x} in {x0}..{x1}");
                }
            }
        }
    }

    #[test]
    fn a_source_past_the_framebuffer_repeats_its_edge() {
        assert_eq!(source(0, 3, 466), 0);
        assert_eq!(source(3, 3, 466), 0);
        assert_eq!(source(4, 3, 466), 1);
        assert_eq!(source(465, -3, 466), 465);
        assert_eq!(source(462, -3, 466), 465);
        assert_eq!(source(461, -3, 466), 464);
    }
}
