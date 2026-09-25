//! A halftone scatter: marks on a grid, each shown or not by a fixed random number against a
//! density that rises towards the edge of a circle and is highest on one side, which can turn.
//! The start-up's identity is its first use; section 2 of the round 3 spec defines it.

use embedded_graphics::{
    draw_target::DrawTarget,
    prelude::{Point, Size},
    primitives::Rectangle,
};

use crate::chrome::{self, Color};

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
    /// Each point draws two numbers in raster order whether or not it shows, so a point keeps its
    /// numbers as the density and the facing change.
    pub fn draw<D: DrawTarget<Color = Color>>(&self, facing: f32, density: f32, target: &mut D) -> Result<(), D::Error> {
        let (sin, cos) = (libm::sinf(facing), libm::cosf(facing));
        let reach = libm::ceilf(self.radius) as i32 + MARK;
        let mut n = 0;
        for y in (self.origin.y..self.center.y + reach).step_by(PITCH as usize) {
            for x in (self.origin.x..self.center.x + reach).step_by(PITCH as usize) {
                let (v, kind) = (self.number(n), self.number(n + 1));
                n += 2;
                let mut top = y;
                if let Some((rows, shift)) = &self.gap {
                    if y + MARK > *rows.start() && y <= *rows.end() {
                        continue;
                    }
                    if y > *rows.end() {
                        top += shift;
                    }
                }
                let half = MARK / 2;
                let (dx, dy) = ((x + half - self.center.x) as f32, (y + half - self.center.y) as f32);
                let r = libm::sqrtf(dx * dx + dy * dy);
                if r > self.radius {
                    continue;
                }
                let radial = 0.45 + 0.55 * ((r - 40.0) / 180.0).clamp(0.0, 1.0);
                let toward = if r > 0.0 { (dx * cos + dy * sin) / r } else { 0.0 };
                let turn = 0.45 + 0.55 * toward;
                if v >= radial * turn * density {
                    continue;
                }
                let corner = Point::new(x, top);
                if kind < HOLLOW {
                    for (offset, size) in [((0, 0), (6, 2)), ((0, 4), (6, 2)), ((0, 2), (2, 2)), ((4, 2), (2, 2))] {
                        let part = Rectangle::new(corner + Point::from(offset), Size::from(size));
                        target.fill_solid(&part, self.color)?;
                    }
                } else {
                    target.fill_solid(&Rectangle::new(corner + Point::new(1, 1), Size::new_equal(4)), self.color)?;
                }
            }
        }
        Ok(())
    }
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
}
