//! A halftone scatter: marks on a grid, each shown or not by a fixed random number against a
//! density that rises towards the edge of a circle and is highest on one side, which can turn.
//! The start-up's identity is its first use; section 2 of the round 3 spec defines it.

use alloc::{vec, vec::Vec};

use embedded_graphics::{
    prelude::{Point, Size},
    primitives::Rectangle,
};

use crate::chrome::{self, Color, CoverageTarget, Dirty};

/// Where a scatter lies and how it looks. The density law and the two marks are fixed.
#[derive(Clone, Debug, PartialEq)]
pub struct Scatter {
    /// The circle's centre, a pixel corner, and its radius. A mark shows only with its centre
    /// inside.
    pub center: Point,
    pub radius: f32,
    /// The top-left of the first grid point's mark. The grid runs every [`PITCH`] from it, right
    /// and down, as far as the circle reaches.
    pub origin: Point,
    /// Rows no mark may touch, and how far the marks below them move down, so a band across the
    /// scatter has the same gap on both sides.
    pub gap: Option<(core::ops::RangeInclusive<i32>, i32)>,
    pub color: Color,
    /// Picks the pattern. The same seed draws the same pattern every time.
    pub seed: u32,
}

/// The grid's pitch, and the side of the hollow mark: 6 × 6 with a 2 × 2 hole. The solid mark
/// is 4 × 4, inset 1 px.
pub const PITCH: i32 = 8;
const MARK: i32 = 6;
/// Below this share of its numbers, a shown point draws the hollow mark.
const HOLLOW: f32 = 0.6;

impl Scatter {
    /// The identity's: the whole panel to radius 228, stopping short of the band on rows
    /// 198–317.
    pub const IDENTITY: Self = Self {
        center: Point::new(233, 233),
        radius: 228.0,
        origin: Point::new(12, -2),
        gap: Some((197..=317, 2)),
        color: chrome::PURPLE,
        seed: 0x6f63_7477,
    };

    /// A number from 0 to 1 for draw `n` of the pattern's generator.
    fn number(&self, n: u32) -> f32 {
        let mut x = n ^ self.seed;
        x ^= x >> 16;
        x = x.wrapping_mul(0x7feb_352d);
        x ^= x >> 15;
        x = x.wrapping_mul(0x846c_a68b);
        x ^= x >> 16;
        (x >> 8) as f32 / (1 << 24) as f32
    }

    /// Draws the marks, with the dense side facing `facing` radians clockwise from the right and
    /// every point's chance scaled by `density`. At a density of 1 about 45 % of the points at
    /// the edge on the dense side show; the identity runs at 1.15.
    ///
    /// Points whose marks the target would not show are skipped before any arithmetic.
    pub fn draw<D: CoverageTarget<Color = Color>>(&self, facing: f32, density: f32, target: &mut D) -> Result<(), D::Error> {
        let mut result = Ok(());
        let visible = |target: &mut D, area: Rectangle| target.visible(&area);
        self.each_shown(facing, density, target, visible, |target, _, corner, hollow| {
            if result.is_err() {
                return;
            }
            result = if hollow {
                [((0, 0), (6, 2)), ((0, 4), (6, 2)), ((0, 2), (2, 2)), ((4, 2), (2, 2))].into_iter().try_for_each(
                    |(offset, size)| {
                        target.fill_solid(&Rectangle::new(corner + Point::from(offset), Size::from(size)), self.color)
                    },
                )
            } else {
                target.fill_solid(&Rectangle::new(corner + Point::new(1, 1), Size::new_equal(4)), self.color)
            };
        });
        result
    }

    /// Which points show at `facing` and `density`, for [`Scatter::changed`].
    #[must_use]
    pub fn shown(&self, facing: f32, density: f32) -> Shown {
        let mut shown = Shown(vec![0; (self.columns() * self.rows()).cast_unsigned().div_ceil(32) as usize]);
        self.each_shown(facing, density, &mut shown, |_, _| true, |shown, point, _, _| {
            shown.0[point / 32] |= 1 << (point % 32);
        });
        shown
    }

    /// Adds the cell of every point that shows in one of `before` and `after` and not the other.
    pub fn changed(&self, before: &Shown, after: &Shown, changed: &mut Dirty) {
        for (word, (a, b)) in before.0.iter().zip(&after.0).enumerate() {
            let mut differ = a ^ b;
            while differ != 0 {
                let point = word as i32 * 32 + differ.trailing_zeros() as i32;
                differ &= differ - 1;
                let (row, column) = (point / self.columns(), point % self.columns());
                let corner = self.corner(self.origin.y + row * PITCH, column);
                changed.add(Rectangle::new(corner, Size::new_equal(MARK as u32)));
            }
        }
    }

    fn reach(&self) -> i32 {
        libm::ceilf(self.radius) as i32 + MARK
    }

    fn columns(&self) -> i32 {
        (self.center.x + self.reach() - self.origin.x + PITCH - 1) / PITCH
    }

    fn rows(&self) -> i32 {
        (self.center.y + self.reach() - self.origin.y + PITCH - 1) / PITCH
    }

    /// The top-left of the mark in `column` of the grid row at `y`, moved below the gap if the
    /// row is.
    fn corner(&self, y: i32, column: i32) -> Point {
        let below = self.gap.as_ref().is_some_and(|(rows, _)| y > *rows.end());
        let shift = self.gap.as_ref().map_or(0, |(_, shift)| *shift);
        Point::new(self.origin.x + column * PITCH, if below { y + shift } else { y })
    }

    /// Calls `mark` with the index, the mark's top-left corner and whether it is hollow for
    /// each shown point whose mark lies in an area `wanted` accepts. `wanted` sees a grid
    /// row's whole strip before its points. Both get `state`.
    ///
    /// Each point owns two numbers of the generator, counted in raster order over the whole
    /// grid, so a point keeps its numbers as the density and the facing change.
    fn each_shown<T: ?Sized>(
        &self,
        facing: f32,
        density: f32,
        state: &mut T,
        wanted: impl Fn(&mut T, Rectangle) -> bool,
        mut mark: impl FnMut(&mut T, usize, Point, bool),
    ) {
        let (sin, cos) = (libm::sinf(facing), libm::cosf(facing));
        let half = MARK / 2;
        let columns = self.columns();
        let limit = self.radius * self.radius;
        for (row, y) in (self.origin.y..self.center.y + self.reach()).step_by(PITCH as usize).enumerate() {
            if let Some((rows, _)) = &self.gap
                && y + MARK > *rows.start()
                && y <= *rows.end()
            {
                continue;
            }
            let top = self.corner(y, 0).y;
            let dy = (y + half - self.center.y) as f32;
            let room = limit - dy * dy;
            if room < 0.0 {
                continue;
            }
            // The columns whose centres can lie inside; the test on each point settles the ends.
            let chord = libm::sqrtf(room);
            let offset = (self.center.x - self.origin.x - half) as f32;
            let from = (libm::floorf((offset - chord) / PITCH as f32) as i32).max(0);
            let to = (libm::ceilf((offset + chord) / PITCH as f32) as i32).min(columns - 1);
            let strip = Rectangle::new(
                Point::new(self.origin.x + from * PITCH, top),
                Size::new(((to - from) * PITCH + MARK) as u32, MARK as u32),
            );
            if !wanted(state, strip) {
                continue;
            }
            for column in from..=to {
                let x = self.origin.x + column * PITCH;
                let dx = (x + half - self.center.x) as f32;
                let squared = dx * dx + dy * dy;
                if squared > limit {
                    continue;
                }
                let corner = Point::new(x, top);
                if !wanted(state, Rectangle::new(corner, Size::new_equal(MARK as u32))) {
                    continue;
                }
                let point = row * columns as usize + column as usize;
                let n = 2 * point as u32;
                let (r, toward) = if squared > 0.0 {
                    let inverse = inverse_sqrt(squared);
                    (squared * inverse, (dx * cos + dy * sin) * inverse)
                } else {
                    (0.0, 0.0)
                };
                let radial = 0.45 + 0.55 * ((r - 40.0) * (1.0 / 180.0)).clamp(0.0, 1.0);
                let turn = 0.45 + 0.55 * toward;
                if self.number(n) < radial * turn * density {
                    mark(state, point, corner, self.number(n + 1) < HOLLOW);
                }
            }
        }
    }
}

/// Which of a scatter's grid points show, a bit each.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Shown(Vec<u32>);

/// 1/√x for positive x, to within a few units in the last place: a guess from the float's bits
/// and two Newton steps. A square root and a divide cost several times as much on the device.
fn inverse_sqrt(x: f32) -> f32 {
    let mut y = f32::from_bits(0x5f37_59df - (x.to_bits() >> 1));
    for _ in 0..2 {
        y *= 1.5 - 0.5 * x * y * y;
    }
    y
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_generator_spreads_over_the_unit_interval() {
        let numbers: alloc::vec::Vec<f32> = (0..4000).map(|n| Scatter::IDENTITY.number(n)).collect();
        assert!(numbers.iter().all(|n| (0.0..1.0).contains(n)));
        let below_half = numbers.iter().filter(|&&n| n < 0.5).count();
        assert!((1800..2200).contains(&below_half), "{below_half}");
    }

    /// The scatter as first written: every grid point, in raster order, tested on its distance.
    fn reference(scatter: &Scatter, facing: f32, density: f32) -> Vec<(Point, bool)> {
        let mut shown = Vec::new();
        let (sin, cos) = (libm::sinf(facing), libm::cosf(facing));
        let reach = libm::ceilf(scatter.radius) as i32 + MARK;
        let mut n = 0;
        for y in (scatter.origin.y..scatter.center.y + reach).step_by(PITCH as usize) {
            for x in (scatter.origin.x..scatter.center.x + reach).step_by(PITCH as usize) {
                let (v, kind) = (scatter.number(n), scatter.number(n + 1));
                n += 2;
                let mut top = y;
                if let Some((rows, shift)) = &scatter.gap {
                    if y + MARK > *rows.start() && y <= *rows.end() {
                        continue;
                    }
                    if y > *rows.end() {
                        top += shift;
                    }
                }
                let half = MARK / 2;
                let (dx, dy) = ((x + half - scatter.center.x) as f32, (y + half - scatter.center.y) as f32);
                let r = libm::sqrtf(dx * dx + dy * dy);
                if r > scatter.radius {
                    continue;
                }
                let radial = 0.45 + 0.55 * ((r - 40.0) / 180.0).clamp(0.0, 1.0);
                let toward = if r > 0.0 { (dx * cos + dy * sin) / r } else { 0.0 };
                let turn = 0.45 + 0.55 * toward;
                if v < radial * turn * density {
                    shown.push((Point::new(x, top), kind < HOLLOW));
                }
            }
        }
        shown
    }

    #[test]
    fn the_identity_shows_the_marks_it_was_designed_with() {
        // The identity's facings and densities, and a full turn beyond them.
        for step in 0..160 {
            let (facing, density) = (0.8 + 0.055 * step as f32, 1.15 * (0.5 + step.min(6) as f32 / 12.0));
            let mut shown = Vec::new();
            Scatter::IDENTITY.each_shown(facing, density, &mut shown, |_, _| true, |shown, _, corner, hollow| {
                shown.push((corner, hollow));
            });
            let expected = reference(&Scatter::IDENTITY, facing, density);
            assert_eq!(shown, expected, "step {step}");
        }
    }
}
