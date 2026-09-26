//! A halftone scatter: marks on a grid, each shown or not by a fixed random number against a
//! density that rises towards the edge of a circle and is highest on one side, which can turn.
//! Several such fields can share one grid. The start-up's identity is its first use; section 2
//! of the round 3 spec defines it.

use alloc::{vec, vec::Vec};

use embedded_graphics::{
    prelude::{Point, Size},
    primitives::Rectangle,
};

use crate::chrome::{self, Color, CoverageTarget, DISPLAY_SIZE, Dirty};

/// Where a scatter's marks lie and how they look. The density law and the two marks are fixed.
#[derive(Clone, Debug, PartialEq)]
pub struct Scatter {
    /// The top-left of the first grid point's mark. The grid runs every [`PITCH`] from it, right
    /// and down, over the panel.
    pub origin: Point,
    /// Rows no mark may touch, and how far the marks below them move down, so a band across the
    /// scatter has the same gap on both sides.
    pub gap: Option<(core::ops::RangeInclusive<i32>, i32)>,
    pub color: Color,
    /// Where on the grid marks may show. Where fields overlap, a point takes its mark from the
    /// first field that shows it.
    pub fields: &'static [Field],
}

/// A circle of the grid in which marks show.
#[derive(Clone, Debug, PartialEq)]
pub struct Field {
    /// The circle's centre, a pixel corner, and its radius. A mark shows only with its centre
    /// inside.
    pub center: Point,
    pub radius: f32,
    /// Picks the pattern. The same seed draws the same pattern every time.
    pub seed: u32,
}

/// How a field looks on one frame: its dense side faces `facing` radians clockwise from the
/// right, and every point's chance is scaled by `density`. At a density of 1 about 45 % of the
/// points at the edge on the dense side show.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Look {
    pub facing: f32,
    pub density: f32,
}

/// The grid's pitch, and the side of the hollow mark: 6 × 6 with a 2 × 2 hole. The solid mark
/// is 4 × 4, inset 1 px.
pub const PITCH: i32 = 8;
const MARK: i32 = 6;
/// Below this share of its numbers, a shown point draws the hollow mark.
const HOLLOW: f32 = 0.6;
/// The glass's radius about the panel's centre. A mark shows only if it lies wholly inside.
const GLASS: i32 = DISPLAY_SIZE.width as i32 / 2;

impl Scatter {
    /// The identity's: the whole panel to radius 228, stopping short of the band on rows
    /// 198–317.
    pub const IDENTITY: Self = Self {
        origin: Point::new(12, -2),
        gap: Some((197..=317, 2)),
        color: chrome::PURPLE,
        fields: &[Field { center: Point::new(233, 233), radius: 228.0, seed: 0x6f63_7477 }],
    };

    /// Draws the marks, with `looks` giving each field's look in order. The identity runs at a
    /// density of 1.15.
    ///
    /// Points whose marks the target would not show are skipped before any arithmetic.
    pub fn draw<D: CoverageTarget<Color = Color>>(&self, looks: &[Look], target: &mut D) -> Result<(), D::Error> {
        let mut result = Ok(());
        let visible = |target: &mut D, area: Rectangle| target.visible(&area);
        self.each_shown(looks, target, visible, |target, _, corner, hollow| {
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

    /// Which points show with `looks`, and which of them are hollow, for [`Scatter::changed`].
    #[must_use]
    pub fn shown(&self, looks: &[Look]) -> Shown {
        let words = (self.columns() * self.rows()).cast_unsigned().div_ceil(32) as usize;
        let mut shown = Shown { shown: vec![0; words], hollow: vec![0; words] };
        self.each_shown(looks, &mut shown, |_, _| true, |shown, point, _, hollow| {
            shown.shown[point / 32] |= 1 << (point % 32);
            if hollow {
                shown.hollow[point / 32] |= 1 << (point % 32);
            }
        });
        shown
    }

    /// Adds the cell of every point that shows in one of `before` and `after` and not the other,
    /// or shows in both with a different mark.
    pub fn changed(&self, before: &Shown, after: &Shown, changed: &mut Dirty) {
        let words = before.shown.iter().zip(&after.shown).zip(before.hollow.iter().zip(&after.hollow));
        for (word, ((a, b), (hollow_a, hollow_b))) in words.enumerate() {
            let mut differ = (a ^ b) | (hollow_a ^ hollow_b);
            while differ != 0 {
                let point = word as i32 * 32 + differ.trailing_zeros() as i32;
                differ &= differ - 1;
                let (row, column) = (point / self.columns(), point % self.columns());
                let corner = self.corner(self.origin.y + row * PITCH, column);
                changed.add(Rectangle::new(corner, Size::new_equal(MARK as u32)));
            }
        }
    }

    fn columns(&self) -> i32 {
        (DISPLAY_SIZE.width as i32 - self.origin.x + PITCH - 1) / PITCH
    }

    fn rows(&self) -> i32 {
        (DISPLAY_SIZE.height as i32 - self.origin.y + PITCH - 1) / PITCH
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
    /// Each point owns two numbers of each field's generator, counted in raster order over the
    /// whole grid, so a point keeps its numbers as the density and the facing change.
    fn each_shown<T: ?Sized>(
        &self,
        looks: &[Look],
        state: &mut T,
        wanted: impl Fn(&mut T, Rectangle) -> bool,
        mut mark: impl FnMut(&mut T, usize, Point, bool),
    ) {
        debug_assert_eq!(looks.len(), self.fields.len());
        let turns: Vec<(f32, f32)> = looks.iter().map(|look| (libm::sinf(look.facing), libm::cosf(look.facing))).collect();
        let half = MARK / 2;
        let columns = self.columns();
        let mut spans: Vec<Option<(i32, i32)>> = vec![None; self.fields.len()];
        for row in 0..self.rows() {
            let y = self.origin.y + row * PITCH;
            if let Some((rows, _)) = &self.gap
                && y + MARK > *rows.start()
                && y <= *rows.end()
            {
                continue;
            }
            let top = self.corner(y, 0).y;
            // Each field's columns whose centres can lie inside; the test on each point settles
            // the ends.
            let (mut from, mut to) = (columns, -1);
            for (field, span) in self.fields.iter().zip(&mut spans) {
                let dy = (y + half - field.center.y) as f32;
                let room = field.radius * field.radius - dy * dy;
                *span = (room >= 0.0).then(|| {
                    let chord = libm::sqrtf(room);
                    let offset = (field.center.x - self.origin.x - half) as f32;
                    let first = (libm::floorf((offset - chord) / PITCH as f32) as i32).max(0);
                    let last = (libm::ceilf((offset + chord) / PITCH as f32) as i32).min(columns - 1);
                    (first, last)
                });
                if let Some((first, last)) = *span {
                    from = from.min(first);
                    to = to.max(last);
                }
            }
            if from > to {
                continue;
            }
            let strip = Rectangle::new(
                Point::new(self.origin.x + from * PITCH, top),
                Size::new(((to - from) * PITCH + MARK) as u32, MARK as u32),
            );
            if !wanted(state, strip) {
                continue;
            }
            // The glass's reach on the mark's rows, measured from its row farthest out.
            let far_y = (top - GLASS).abs().max((top + MARK - GLASS).abs());
            let glass = GLASS * GLASS - far_y * far_y;
            for column in from..=to {
                let x = self.origin.x + column * PITCH;
                let far_x = (x - GLASS).abs().max((x + MARK - GLASS).abs());
                if far_x * far_x > glass {
                    continue;
                }
                let corner = Point::new(x, top);
                if !wanted(state, Rectangle::new(corner, Size::new_equal(MARK as u32))) {
                    continue;
                }
                let point = row as usize * columns as usize + column as usize;
                let n = 2 * point as u32;
                let fields = self.fields.iter().zip(looks).zip(&turns).zip(&spans);
                for (((field, look), &(sin, cos)), span) in fields {
                    if !span.is_some_and(|(first, last)| (first..=last).contains(&column)) {
                        continue;
                    }
                    let dx = (x + half - field.center.x) as f32;
                    let dy = (y + half - field.center.y) as f32;
                    let squared = dx * dx + dy * dy;
                    if squared > field.radius * field.radius {
                        continue;
                    }
                    let (r, toward) = if squared > 0.0 {
                        let inverse = inverse_sqrt(squared);
                        (squared * inverse, (dx * cos + dy * sin) * inverse)
                    } else {
                        (0.0, 0.0)
                    };
                    let radial = 0.45 + 0.55 * ((r - 40.0) * (1.0 / 180.0)).clamp(0.0, 1.0);
                    let turn = 0.45 + 0.55 * toward;
                    if number(field.seed, n) < radial * turn * look.density {
                        mark(state, point, corner, number(field.seed, n + 1) < HOLLOW);
                        break;
                    }
                }
            }
        }
    }
}

/// A number from 0 to 1 for draw `n` of the generator seeded with `seed`.
fn number(seed: u32, n: u32) -> f32 {
    let mut x = n ^ seed;
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb_352d);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846c_a68b);
    x ^= x >> 16;
    (x >> 8) as f32 / (1 << 24) as f32
}

/// Which of a scatter's grid points show, and which of those are hollow, a bit each.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Shown {
    shown: Vec<u32>,
    hollow: Vec<u32>,
}

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
        let seed = Scatter::IDENTITY.fields[0].seed;
        let numbers: alloc::vec::Vec<f32> = (0..4000).map(|n| number(seed, n)).collect();
        assert!(numbers.iter().all(|n| (0.0..1.0).contains(n)));
        let below_half = numbers.iter().filter(|&&n| n < 0.5).count();
        assert!((1800..2200).contains(&below_half), "{below_half}");
    }

    fn each(scatter: &Scatter, looks: &[Look]) -> Vec<(Point, bool)> {
        let mut shown = Vec::new();
        scatter.each_shown(looks, &mut shown, |_, _| true, |shown, _, corner, hollow| shown.push((corner, hollow)));
        shown
    }

    /// The scatter as first written, for one field: every grid point, in raster order, tested
    /// on its distance, then dropped if its mark leaves the glass.
    fn reference(scatter: &Scatter, look: Look) -> Vec<(Point, bool)> {
        let field = &scatter.fields[0];
        let mut shown = Vec::new();
        let (sin, cos) = (libm::sinf(look.facing), libm::cosf(look.facing));
        let reach = libm::ceilf(field.radius) as i32 + MARK;
        let mut n = 0;
        for y in (scatter.origin.y..field.center.y + reach).step_by(PITCH as usize) {
            for x in (scatter.origin.x..field.center.x + reach).step_by(PITCH as usize) {
                let (v, kind) = (number(field.seed, n), number(field.seed, n + 1));
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
                let (dx, dy) = ((x + half - field.center.x) as f32, (y + half - field.center.y) as f32);
                let r = libm::sqrtf(dx * dx + dy * dy);
                if r > field.radius {
                    continue;
                }
                let radial = 0.45 + 0.55 * ((r - 40.0) / 180.0).clamp(0.0, 1.0);
                let toward = if r > 0.0 { (dx * cos + dy * sin) / r } else { 0.0 };
                let turn = 0.45 + 0.55 * toward;
                let inside = [(x, top), (x + MARK, top), (x, top + MARK), (x + MARK, top + MARK)]
                    .iter()
                    .all(|&(cx, cy)| (cx - GLASS).pow(2) + (cy - GLASS).pow(2) <= GLASS * GLASS);
                if v < radial * turn * look.density && inside {
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
            let look = Look { facing: 0.8 + 0.055 * step as f32, density: 1.15 * (0.5 + step.min(6) as f32 / 12.0) };
            assert_eq!(each(&Scatter::IDENTITY, &[look]), reference(&Scatter::IDENTITY, look), "step {step}");
        }
    }

    const UPPER: Field = Field { center: Point::new(270, 145), radius: 150.0, seed: 1 };
    // Closer than the identity's two, so that many points fall in both.
    const LOWER: Field = Field { center: Point::new(200, 240), radius: 150.0, seed: 2 };
    const TWO: Scatter = Scatter { origin: Point::new(12, -2), gap: None, color: chrome::PURPLE, fields: &[UPPER, LOWER] };
    const LOOKS: [Look; 2] = [Look { facing: -0.65, density: 0.9 }, Look { facing: -0.65, density: 0.9 }];

    #[test]
    fn fields_show_their_own_marks_and_the_first_wins_where_they_overlap() {
        let upper = each(&Scatter { fields: &[UPPER], ..TWO }, &LOOKS[..1]);
        let lower = each(&Scatter { fields: &[LOWER], ..TWO }, &LOOKS[1..]);
        let overlap = |corner: &Point| upper.iter().any(|(other, _)| other == corner);
        assert!(lower.iter().any(|(corner, _)| overlap(corner)), "the fields overlap");
        let mut expected = upper.clone();
        expected.extend(lower.iter().filter(|(corner, _)| !overlap(corner)));
        expected.sort_by_key(|(corner, _)| (corner.y, corner.x));
        assert_eq!(each(&TWO, &LOOKS), expected);
    }

    #[test]
    fn a_mark_that_changes_kind_is_damaged() {
        let before = TWO.shown(&LOOKS);
        let point = before.shown.iter().enumerate().find_map(|(word, bits)| {
            (*bits != 0).then(|| word * 32 + bits.trailing_zeros() as usize)
        });
        let point = point.expect("a mark shows");
        let mut after = before.clone();
        after.hollow[point / 32] ^= 1 << (point % 32);
        let mut changed = Dirty::default();
        TWO.changed(&before, &after, &mut changed);
        let mut expected = Dirty::default();
        let (row, column) = (point as i32 / TWO.columns(), point as i32 % TWO.columns());
        expected.add(Rectangle::new(TWO.corner(TWO.origin.y + row * PITCH, column), Size::new_equal(MARK as u32)));
        assert_eq!((changed.bounding_box(), changed.pixels()), (expected.bounding_box(), expected.pixels()));
    }
}
