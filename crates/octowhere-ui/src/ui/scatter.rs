//! A halftone scatter: marks on a grid, each shown or not by a fixed random number against a
//! density that rises towards the edge of a circle and is highest on one side, which can turn.
//! Several such fields can share one grid. The start-up's identity is its first use; section 2
//! of the round 3 spec defines it.

use alloc::{vec, vec::Vec};

use embassy_sync::once_lock::OnceLock;

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
    /// Colours for the marks in place of `color`, if any.
    pub tones: Option<Tones>,
    /// Where on the grid marks may show, at most [`FIELDS`]. Where fields overlap, a point takes
    /// its mark from the first field that shows it.
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
    pub law: Law,
}

/// Marks' colours, darkest first. A mark takes one by its field's chance where it lies, before its
/// look scales it, so a mark keeps its colour as the field blooms and breathes: sparse marks the
/// darker, and marks at `dense` or above the brighter. Its own number spreads the choice a little.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tones {
    /// At most four.
    pub colors: &'static [Color],
    pub dense: f32,
}

/// How a field's chance of showing a point varies across it, before a look scales it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Law {
    /// Rising from the centre to the edge, and highest on the side the look faces.
    Radial,
    /// `peak` in `lobes`, easing to `quiet` at `reach` pixels from the nearest, by the mark's
    /// top-left corner. Looks' facings do not turn it.
    Lobes {
        lobes: &'static [Rectangle],
        quiet: f32,
        peak: f32,
        reach: f32,
    },
}

/// How a field looks on one frame: its dense side faces `facing` radians clockwise from the
/// right, and every point's chance is scaled by `density`. At a density of 1 about 45 % of the
/// points at the edge on the dense side show.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Look {
    pub facing: f32,
    pub density: f32,
}

/// The resting screens' scatter breathes: its density falls to `1 - BREATH_DEPTH` of full and
/// back over `BREATH_PERIOD` (owner).
const BREATH_PERIOD: u64 = 10_000_000;
const BREATH_DEPTH: f32 = 0.25;

/// How full the breathing scatter is at `now`, in µs, from 0 to 255.
#[must_use]
pub fn breath(now: u64) -> u8 {
    let turn = (now % BREATH_PERIOD) as f32 / BREATH_PERIOD as f32;
    let fall = BREATH_DEPTH * (1.0 - libm::cosf(core::f32::consts::TAU * turn)) / 2.0;
    libm::roundf((1.0 - fall) * 255.0) as u8
}

/// When [`breath`] next returns something other than it does at `now`, both in µs.
#[must_use]
pub fn next_breath(now: u64) -> u64 {
    let level = f32::from(breath(now));
    let into = now % BREATH_PERIOD;
    let turn = into as f32 / BREATH_PERIOD as f32;
    // The turn into the period's first half, where the breath falls, at which it is `value`
    // before rounding. It rises back through the same values over the second half.
    let falling_at = |value: f32| {
        let fall = 1.0 - value / 255.0;
        libm::acosf((1.0 - 2.0 * fall / BREATH_DEPTH).clamp(-1.0, 1.0)) / core::f32::consts::TAU
    };
    let lowest = (1.0 - BREATH_DEPTH) * 255.0;
    let next = if turn < 0.5 && level - 0.5 > lowest {
        falling_at(level - 0.5)
    } else if level + 0.5 < 255.0 {
        1.0 - falling_at(level + 0.5)
    } else {
        1.0 + falling_at(level - 0.5)
    };
    // In `f32` the turn comes out a few µs either side of the change.
    let after = libm::ceilf(next * BREATH_PERIOD as f32) as u64 + BREATH_MARGIN;
    (now - into + after).max(now + 1)
}

const BREATH_MARGIN: u64 = 16;

/// The grid's pitch, and the side of the hollow mark: 6 × 6 with a 2 × 2 hole. The solid mark
/// is 4 × 4, inset 1 px.
pub const PITCH: i32 = 8;
/// The most fields a scatter has.
pub const FIELDS: usize = 4;
const MARK: i32 = 6;
/// Below this share of its numbers, a shown point draws the hollow mark.
const HOLLOW: f32 = 0.6;
/// A mark's tone runs from `TONE_FLOOR` where its field is sparsest across `TONE_RANGE` to its
/// tones' `dense`, and its own number moves it by up to half of `TONE_SPREAD` either way, so
/// sparse marks mix the darker tones and dense marks the brighter. `TONE_SEED` seeds that number.
const TONE_FLOOR: f32 = 0.15;
const TONE_RANGE: f32 = 0.6;
const TONE_SPREAD: f32 = 0.6;
const TONE_SEED: u32 = 0x746f_6e65;
/// The glass's radius about the panel's centre. A mark shows only if it lies wholly inside.
const GLASS: i32 = DISPLAY_SIZE.width as i32 / 2;
/// The most grid points a scatter has across and down, and in all.
const SPAN: usize = DISPLAY_SIZE.width as usize / PITCH as usize + 2;
const MOST_POINTS: usize = SPAN * SPAN;

impl Scatter {
    /// The identity's: the whole panel to radius 228, stopping short of the band on rows
    /// 198–317.
    pub const IDENTITY: Self = Self {
        origin: Point::new(12, -2),
        gap: Some((197..=317, 2)),
        color: chrome::PURPLE,
        tones: None,
        fields: &[Field {
            center: Point::new(233, 233),
            radius: 228.0,
            seed: 0x6f63_7477,
            law: Law::Radial,
        }],
    };

    /// Draws the marks, with `looks` giving each field's look in order. The identity runs at a
    /// density of 1.15.
    ///
    /// Points whose marks the target would not show are skipped before any arithmetic.
    pub fn draw<D: CoverageTarget<Color = Color>>(
        &self,
        looks: &[Look],
        target: &mut D,
    ) -> Result<(), D::Error> {
        self.draw_clear_of(looks, &[], target)
    }

    /// As [`draw`](Self::draw), leaving out every mark that meets one of `clear`.
    pub fn draw_clear_of<D: CoverageTarget<Color = Color>>(
        &self,
        looks: &[Look],
        clear: &[Rectangle],
        target: &mut D,
    ) -> Result<(), D::Error> {
        let mut result = Ok(());
        let visible = |target: &mut D, area: Rectangle| target.visible(&area);
        self.each_shown(
            looks,
            clear,
            target,
            visible,
            |target, _, corner, hollow, tone| {
                if result.is_err() {
                    return;
                }
                let color = self
                    .tones
                    .map_or(self.color, |tones| tones.colors[usize::from(tone)]);
                result = if hollow {
                    [
                        ((0, 0), (6, 2)),
                        ((0, 4), (6, 2)),
                        ((0, 2), (2, 2)),
                        ((4, 2), (2, 2)),
                    ]
                    .into_iter()
                    .try_for_each(|(offset, size)| {
                        target.fill_solid(
                            &Rectangle::new(corner + Point::from(offset), Size::from(size)),
                            color,
                        )
                    })
                } else {
                    target.fill_solid(
                        &Rectangle::new(corner + Point::new(1, 1), Size::new_equal(4)),
                        color,
                    )
                };
            },
        );
        result
    }

    /// Which points show with `looks`, which of them are hollow, and their tones, for
    /// [`Scatter::changed`].
    #[must_use]
    pub fn shown(&self, looks: &[Look]) -> Shown {
        self.shown_clear_of(looks, &[])
    }

    /// As [`shown`](Self::shown), leaving out every mark that meets one of `clear`.
    #[must_use]
    pub fn shown_clear_of(&self, looks: &[Look], clear: &[Rectangle]) -> Shown {
        let words = (self.columns() * self.rows()).cast_unsigned().div_ceil(32) as usize;
        let mut shown = Shown {
            bits: vec![0; LAYERS * words],
        };
        self.each_shown(
            looks,
            clear,
            &mut shown,
            |_, _| true,
            |shown, point, _, hollow, tone| {
                shown.set(SHOWS, point);
                if hollow {
                    shown.set(HOLLOWS, point);
                }
                for place in 0..2 {
                    if tone >> place & 1 != 0 {
                        shown.set(TONES + place, point);
                    }
                }
            },
        );
        shown
    }

    /// Adds the cell of every point that shows in one of `before` and `after` and not the other,
    /// or shows in both with a different mark or tone.
    pub fn changed(&self, before: &Shown, after: &Shown, changed: &mut Dirty) {
        for word in 0..before.words().min(after.words()) {
            let mut differ = (0..LAYERS).fold(0, |differ, layer| {
                differ | (before.layer(layer)[word] ^ after.layer(layer)[word])
            });
            while differ != 0 {
                let point = word * 32 + differ.trailing_zeros() as usize;
                differ &= differ - 1;
                changed.add(self.cell(point));
            }
        }
    }

    /// The index of the colour for a mark where its field's chance is `chance`, spread by
    /// `spread`, from -0.5 to 0.5; 0 without tones.
    fn tone(&self, chance: f32, spread: f32) -> u8 {
        let Some(tones) = self.tones else {
            return 0;
        };
        let tone = TONE_FLOOR + TONE_RANGE * (chance / tones.dense).min(1.0) + TONE_SPREAD * spread;
        let last = tones.colors.len().saturating_sub(1);
        ((tone.max(0.0) * tones.colors.len() as f32) as usize).min(last) as u8
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
        Point::new(
            self.origin.x + column * PITCH,
            if below { y + shift } else { y },
        )
    }

    /// The cell of the mark at grid point `point`.
    fn cell(&self, point: usize) -> Rectangle {
        let (row, column) = (point as i32 / self.columns(), point as i32 % self.columns());
        Rectangle::new(
            self.corner(self.origin.y + row * PITCH, column),
            Size::new_equal(MARK as u32),
        )
    }

    /// Calls `mark` with the index, the mark's top-left corner, whether it is hollow and its tone
    /// for each shown point whose mark lies in an area `wanted` accepts. `wanted` sees a grid
    /// row's whole strip before its points. Both get `state`.
    fn each_shown<T: ?Sized>(
        &self,
        looks: &[Look],
        clear: &[Rectangle],
        state: &mut T,
        wanted: impl Fn(&mut T, Rectangle) -> bool,
        mut mark: impl FnMut(&mut T, usize, Point, bool, u8),
    ) {
        let turns = turns(looks);
        self.each_point(state, wanted, |state, at| {
            if let Some((hollow, tone)) = self.mark(at, looks, &turns, clear) {
                mark(state, at.point, at.cell.top_left, hollow, tone);
            }
        });
    }

    /// Adds the cell of every point whose mark differs between `before` and `after`, each the
    /// fields' looks and the areas marks keep clear of: shown in one and not the other, or shown
    /// in both as a different kind or tone.
    pub fn changed_between(
        &self,
        before: (&[Look], &[Rectangle]),
        after: (&[Look], &[Rectangle]),
        changed: &mut Dirty,
    ) {
        let turns = [turns(before.0), turns(after.0)];
        self.each_point(
            changed,
            |_, _| true,
            |changed, at| {
                if self.mark(at, before.0, &turns[0], before.1)
                    != self.mark(at, after.0, &turns[1], after.1)
                {
                    changed.add(at.cell);
                }
            },
        );
    }

    /// Calls `visit` for each grid point on the glass and within a field's circle's columns
    /// whose mark lies in an area `wanted` accepts. `wanted` sees a grid row's whole strip before
    /// its points. Both get `state`.
    fn each_point<T: ?Sized>(
        &self,
        state: &mut T,
        wanted: impl Fn(&mut T, Rectangle) -> bool,
        mut visit: impl FnMut(&mut T, &At<'_>),
    ) {
        assert!(
            self.fields.len() <= FIELDS,
            "a scatter has at most {FIELDS} fields"
        );
        let half = MARK / 2;
        let columns = self.columns();
        let mut spans = [None; FIELDS];
        for row in 0..self.rows() {
            let y = self.origin.y + row * PITCH;
            if let Some((rows, _)) = &self.gap
                && y + MARK > *rows.start()
                && y <= *rows.end()
            {
                continue;
            }
            let top = self.corner(y, 0).y;
            // A row the target would not show costs no arithmetic at all.
            if !wanted(
                state,
                Rectangle::new(
                    Point::new(0, top),
                    Size::new(DISPLAY_SIZE.width, MARK as u32),
                ),
            ) {
                continue;
            }
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
                    let last =
                        (libm::ceilf((offset + chord) / PITCH as f32) as i32).min(columns - 1);
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
                let cell = Rectangle::new(Point::new(x, top), Size::new_equal(MARK as u32));
                if !wanted(state, cell) {
                    continue;
                }
                let at = At {
                    point: row as usize * columns as usize + column as usize,
                    column,
                    center: Point::new(x + half, y + half),
                    cell,
                    spans: &spans,
                };
                visit(state, &at);
            }
        }
    }

    /// The mark at `at` with `looks`, whose facings' sines and cosines are `turns`, if it shows
    /// and meets none of `clear`: whether it is hollow, and its tone.
    ///
    /// Each point owns two numbers of each field's generator, counted in raster order over the
    /// whole grid, so a point keeps its numbers as the density and the facing change.
    fn mark(
        &self,
        at: &At<'_>,
        looks: &[Look],
        turns: &[(f32, f32); FIELDS],
        clear: &[Rectangle],
    ) -> Option<(bool, u8)> {
        debug_assert_eq!(looks.len(), self.fields.len());
        let n = 2 * at.point as u32;
        let fields = self.fields.iter().zip(looks).zip(turns).zip(at.spans);
        for (((field, look), &turn), span) in fields {
            let Some(chance) = chance(at, field, turn, *span) else {
                continue;
            };
            if number(field.seed, n) >= chance * look.density {
                continue;
            }
            // Checked only for a point that shows, as few do.
            if clear
                .iter()
                .any(|keep| !keep.intersection(&at.cell).is_zero_sized())
            {
                return None;
            }
            return Some(self.kind(field, n, chance));
        }
        None
    }

    /// Whether `field`'s mark at the point whose first number is `n` is hollow, and its tone,
    /// where its chance is `chance`.
    fn kind(&self, field: &Field, n: u32, chance: f32) -> (bool, u8) {
        // A generator of its own, so the tones leave the pattern alone.
        let spread = number(field.seed ^ TONE_SEED, n) - 0.5;
        let hollow = number(field.seed, n + 1) < HOLLOW;
        (hollow, self.tone(chance, spread))
    }

    /// Where each mark changes as `looks` sets the fields' looks from a level, 0 to 255. The
    /// facings must not change with the level, nor the densities fall as it rises.
    #[must_use]
    pub fn changes<const N: usize>(&self, looks: impl Fn(u8) -> [Look; N]) -> Changes {
        let full = looks(u8::MAX);
        debug_assert!(
            looks(0)
                .iter()
                .zip(&full)
                .all(|(low, high)| low.facing == high.facing)
        );
        assert!((self.columns() * self.rows()) as usize <= MOST_POINTS);
        let turns = turns(&full);
        let mut found = (Vec::new(), Vec::new());
        self.each_point(
            &mut found,
            |_, _| true,
            |(found, shows), at| {
                let n = 2 * at.point as u32;
                // Each field's lowest level that shows the point, 256 for none, and its mark.
                let mut from = [(256, (false, 0)); FIELDS];
                let fields = self.fields.iter().zip(&turns).zip(at.spans);
                for (index, ((field, &turn), span)) in fields.enumerate() {
                    let Some(chance) = chance(at, field, turn, *span) else {
                        continue;
                    };
                    let drawn = number(field.seed, n);
                    let shows = |level: u16| drawn < chance * looks(level as u8)[index].density;
                    // Most points show at no level.
                    if !shows(255) {
                        continue;
                    }
                    let (mut low, mut high) = (0, 255);
                    while low < high {
                        let middle = (low + high) / 2;
                        if shows(middle) {
                            high = middle;
                        } else {
                            low = middle + 1;
                        }
                    }
                    from[index] = (low, self.kind(field, n, chance));
                }
                if let Some(lowest) = from
                    .iter()
                    .map(|(level, _)| *level)
                    .filter(|&level| level < 256)
                    .min()
                {
                    shows.push((at.point as u16, lowest as u8));
                }
                // The first field that shows the point at a level wins it.
                let mark = |level: u16| {
                    from.iter()
                        .find(|(lowest, _)| *lowest <= level)
                        .map(|(_, kind)| *kind)
                };
                for (index, &(level, _)) in from.iter().enumerate() {
                    if (1..256).contains(&level)
                        && !from[..index].iter().any(|(other, _)| *other == level)
                        && mark(level) != mark(level - 1)
                    {
                        found.push((level as u8, at.point as u16));
                    }
                }
            },
        );
        let (mut found, shows) = found;
        let mut starts = [0; 257];
        for &(level, _) in &found {
            starts[usize::from(level) + 1] += 1;
        }
        for level in 0..256 {
            starts[level + 1] += starts[level];
        }
        found.sort_unstable_by_key(|&(level, _)| level);
        Changes {
            scatter: self.clone(),
            starts,
            points: found.into_iter().map(|(_, point)| point).collect(),
            shows,
        }
    }

    /// Which points' cells meet one of `areas`.
    fn covered(&self, areas: &[Rectangle]) -> Covered {
        let mut covered = Covered([0; MOST_POINTS.div_ceil(32)]);
        let (columns, rows) = (self.columns(), self.rows());
        let shift = self.gap.as_ref().map_or(0, |(_, shift)| *shift);
        for area in areas.iter().filter(|area| !area.is_zero_sized()) {
            let (left, top) = (area.top_left.x, area.top_left.y);
            let (right, bottom) = (left + area.size.width as i32, top + area.size.height as i32);
            // The columns whose cells reach into the area, and the rows that might once those
            // below the gap move down.
            let first = -(self.origin.x + MARK - 1 - left).div_euclid(PITCH);
            let last = (right - 1 - self.origin.x)
                .div_euclid(PITCH)
                .min(columns - 1);
            let rows = (top - MARK - shift - self.origin.y)
                .div_euclid(PITCH)
                .max(0)
                ..=(bottom - self.origin.y).div_euclid(PITCH).min(rows - 1);
            for row in rows {
                let cell_top = self.corner(self.origin.y + row * PITCH, 0).y;
                if cell_top < bottom && top < cell_top + MARK {
                    for column in first.max(0)..=last {
                        covered.set((row * columns + column) as usize);
                    }
                }
            }
        }
        covered
    }
}

/// Which of a scatter's grid points have cells that meet some areas, a bit each.
struct Covered([u32; MOST_POINTS.div_ceil(32)]);

impl Covered {
    fn set(&mut self, point: usize) {
        self.0[point / 32] |= 1 << (point % 32);
    }

    fn has(&self, point: usize) -> bool {
        self.0[point / 32] & 1 << (point % 32) != 0
    }
}

/// Where a scatter's marks change as one level sets its fields' looks, from
/// [`Scatter::changes`], so that the damage between two levels takes no pass over the grid.
#[derive(Debug)]
pub struct Changes {
    scatter: Scatter,
    /// `points[starts[level]..starts[level + 1]]` are the points whose marks differ between
    /// `level` and the level below it.
    starts: [u16; 257],
    points: Vec<u16>,
    /// Each point that shows at some level, in the grid's order, and the lowest level it shows
    /// at.
    shows: Vec<(u16, u8)>,
}

impl Changes {
    /// The changes in `kept`, built from `scatter` and `looks` the first time. They are built
    /// before the lock is taken: [`OnceLock::get_or_init`] masks interrupts for as long as its
    /// build takes.
    pub fn kept<const N: usize>(
        kept: &'static OnceLock<Self>,
        scatter: &Scatter,
        looks: impl Fn(u8) -> [Look; N],
    ) -> &'static Self {
        if kept.try_get().is_none() {
            _ = kept.init(scatter.changes(looks));
        }
        kept.try_get().expect("the changes were kept")
    }

    /// Adds the cell of every mark that differs between `before` and `after`, each a level and
    /// the areas its marks keep clear of, as [`Scatter::changed_between`] does. Between levels
    /// further apart than one, it may add a mark that changed and changed back.
    pub fn damage(
        &self,
        before: (u8, &[Rectangle]),
        after: (u8, &[Rectangle]),
        changed: &mut Dirty,
    ) {
        self.each_cell(before, after, |cell| changed.add(cell));
    }

    fn each_cell(
        &self,
        before: (u8, &[Rectangle]),
        after: (u8, &[Rectangle]),
        mut cell: impl FnMut(Rectangle),
    ) {
        let was = self.scatter.covered(before.1);
        let moved = before.1 != after.1;
        let is = if moved {
            &self.scatter.covered(after.1)
        } else {
            &was
        };
        // A mark clear of both changes where the level moves it.
        let start = |level: u8| usize::from(self.starts[usize::from(level) + 1]);
        let levels = start(before.0.min(after.0))..start(before.0.max(after.0));
        for point in self.points[levels].iter().map(|&point| usize::from(point)) {
            if !was.has(point) && !is.has(point) {
                cell(self.scatter.cell(point));
            }
        }
        if !moved {
            return;
        }
        // A mark kept clear on one side alone changes where the other side shows it.
        for &(point, lowest) in &self.shows {
            let point = usize::from(point);
            let shown = match (was.has(point), is.has(point)) {
                (true, false) => lowest <= after.0,
                (false, true) => lowest <= before.0,
                _ => false,
            };
            if shown {
                cell(self.scatter.cell(point));
            }
        }
    }
}

/// Asserts that `changes`, from `looks`, damages what [`Scatter::changed_between`] does between
/// two levels each clear of one of `clears`: exactly at the same level and at neighbouring
/// ones, every neighbouring pair while the clears stay the same, and at least that between a few
/// far apart.
#[cfg(test)]
pub(crate) fn assert_changes_match<const N: usize>(
    changes: &Changes,
    looks: impl Fn(u8) -> [Look; N],
    clears: &[&[Rectangle]],
) {
    let scatter = &changes.scatter;
    let corners = |cells: Vec<Rectangle>| {
        let mut corners: Vec<_> = cells
            .iter()
            .map(|cell| (cell.top_left.y, cell.top_left.x))
            .collect();
        corners.sort_unstable();
        corners.dedup();
        corners
    };
    for (index, &old) in clears.iter().enumerate() {
        for (other, &new) in clears.iter().enumerate() {
            let step = if index == other { 1 } else { 17 };
            let neighbours = (0..u8::MAX).step_by(step).map(|level| (level, level + 1));
            let same = [(0, 0), (200, 200), (u8::MAX, u8::MAX)];
            let far = [(0, u8::MAX), (u8::MAX, 191), (37, 200), (200, 37)];
            for (before, after) in neighbours.chain(same).chain(far) {
                let (was, is) = (looks(before), looks(after));
                let turns = [turns(&was), turns(&is)];
                let mut differ = Vec::new();
                scatter.each_point(
                    &mut differ,
                    |_, _| true,
                    |differ, at| {
                        if scatter.mark(at, &was, &turns[0], old)
                            != scatter.mark(at, &is, &turns[1], new)
                        {
                            differ.push(at.cell);
                        }
                    },
                );
                let mut damaged = Vec::new();
                changes.each_cell((before, old), (after, new), |cell| damaged.push(cell));
                let (damaged, differ) = (corners(damaged), corners(differ));
                let context = format!("from {before} to {after}, clears {index} to {other}");
                if before.abs_diff(after) <= 1 {
                    assert_eq!(damaged, differ, "{context}");
                } else {
                    // A mark that changes field twice between them can end as it began.
                    let missed: Vec<_> = differ
                        .iter()
                        .filter(|corner| !damaged.contains(corner))
                        .collect();
                    assert!(missed.is_empty(), "{context}: {missed:?}");
                }
            }
        }
    }
}

/// `field`'s chance of showing the point at `at` before a look scales it, if its circle holds
/// the point. `turn` is the sine and cosine of the field's facing, and `span` its columns on the
/// point's row.
fn chance(
    at: &At<'_>,
    field: &Field,
    (sin, cos): (f32, f32),
    span: Option<(i32, i32)>,
) -> Option<f32> {
    if !span.is_some_and(|(first, last)| (first..=last).contains(&at.column)) {
        return None;
    }
    let dx = (at.center.x - field.center.x) as f32;
    let dy = (at.center.y - field.center.y) as f32;
    let squared = dx * dx + dy * dy;
    if squared > field.radius * field.radius {
        return None;
    }
    Some(match field.law {
        Law::Radial => {
            let (r, toward) = if squared > 0.0 {
                let inverse = inverse_sqrt(squared);
                (squared * inverse, (dx * cos + dy * sin) * inverse)
            } else {
                (0.0, 0.0)
            };
            let radial = 0.45 + 0.55 * ((r - 40.0) * (1.0 / 180.0)).clamp(0.0, 1.0);
            radial * (0.45 + 0.55 * toward)
        }
        Law::Lobes {
            lobes,
            quiet,
            peak,
            reach,
        } => {
            let corner = at.cell.top_left;
            let squared = lobes
                .iter()
                .map(|lobe| {
                    let end = lobe.top_left + lobe.size - Point::new(1, 1);
                    let dx = (lobe.top_left.x - corner.x).max(corner.x - end.x).max(0);
                    let dy = (lobe.top_left.y - corner.y).max(corner.y - end.y).max(0);
                    (dx * dx + dy * dy) as f32
                })
                .fold(f32::MAX, f32::min);
            if squared >= reach * reach {
                quiet
            } else {
                let near = if squared > 0.0 {
                    1.0 - squared * inverse_sqrt(squared) / reach
                } else {
                    1.0
                };
                quiet + (peak - quiet) * near * near * (3.0 - 2.0 * near)
            }
        }
    })
}

/// A grid point [`Scatter::each_point`] visits: its index, column, the centre of its unshifted
/// place, its mark's cell, and each field's columns on its row.
struct At<'a> {
    point: usize,
    column: i32,
    center: Point,
    cell: Rectangle,
    spans: &'a [Option<(i32, i32)>; FIELDS],
}

/// The sines and cosines of `looks`' facings.
fn turns(looks: &[Look]) -> [(f32, f32); FIELDS] {
    let mut turns = [(0.0, 0.0); FIELDS];
    for (turn, look) in turns.iter_mut().zip(looks) {
        *turn = (libm::sinf(look.facing), libm::cosf(look.facing));
    }
    turns
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

/// Which of a scatter's grid points show, and which of those are hollow, a bit each, and the
/// index of each one's tone in two bits: [`LAYERS`] layers of a bit a point, one after another.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Shown {
    bits: Vec<u32>,
}

/// [`Shown`]'s layers: which points show, which of those are hollow, and the tone's low bit and
/// high bit.
const SHOWS: usize = 0;
const HOLLOWS: usize = 1;
const TONES: usize = 2;
const LAYERS: usize = 4;

impl Shown {
    fn words(&self) -> usize {
        self.bits.len() / LAYERS
    }

    fn layer(&self, layer: usize) -> &[u32] {
        let words = self.words();
        &self.bits[layer * words..(layer + 1) * words]
    }

    fn layer_mut(&mut self, layer: usize) -> &mut [u32] {
        let words = self.words();
        &mut self.bits[layer * words..(layer + 1) * words]
    }

    fn set(&mut self, layer: usize, point: usize) {
        self.layer_mut(layer)[point / 32] |= 1 << (point % 32);
    }
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
    fn the_next_breath_comes_just_after_the_breath_changes() {
        for now in (0..2 * BREATH_PERIOD).step_by(997) {
            let due = next_breath(now);
            assert!(due > now);
            assert_ne!(breath(due), breath(now), "{now}");
            assert_eq!(
                breath(due.saturating_sub(32).max(now)),
                breath(now),
                "{now}"
            );
        }
    }

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
        scatter.each_shown(
            looks,
            &[],
            &mut shown,
            |_, _| true,
            |shown, _, corner, hollow, _| shown.push((corner, hollow)),
        );
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
                let (dx, dy) = (
                    (x + half - field.center.x) as f32,
                    (y + half - field.center.y) as f32,
                );
                let r = libm::sqrtf(dx * dx + dy * dy);
                if r > field.radius {
                    continue;
                }
                let radial = 0.45 + 0.55 * ((r - 40.0) / 180.0).clamp(0.0, 1.0);
                let toward = if r > 0.0 {
                    (dx * cos + dy * sin) / r
                } else {
                    0.0
                };
                let turn = 0.45 + 0.55 * toward;
                let inside = [
                    (x, top),
                    (x + MARK, top),
                    (x, top + MARK),
                    (x + MARK, top + MARK),
                ]
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
            let look = Look {
                facing: 0.8 + 0.055 * step as f32,
                density: 1.15 * (0.5 + step.min(6) as f32 / 12.0),
            };
            assert_eq!(
                each(&Scatter::IDENTITY, &[look]),
                reference(&Scatter::IDENTITY, look),
                "step {step}"
            );
        }
    }

    const UPPER: Field = Field {
        center: Point::new(270, 145),
        radius: 150.0,
        seed: 1,
        law: Law::Radial,
    };
    // Closer than the identity's two, so that many points fall in both.
    const LOWER: Field = Field {
        center: Point::new(200, 240),
        radius: 150.0,
        seed: 2,
        law: Law::Radial,
    };
    const TWO: Scatter = Scatter {
        origin: Point::new(12, -2),
        gap: None,
        color: chrome::PURPLE,
        tones: None,
        fields: &[UPPER, LOWER],
    };
    const LOOKS: [Look; 2] = [
        Look {
            facing: -0.65,
            density: 0.9,
        },
        Look {
            facing: -0.65,
            density: 0.9,
        },
    ];

    #[test]
    fn fields_show_their_own_marks_and_the_first_wins_where_they_overlap() {
        let upper = each(
            &Scatter {
                fields: &[UPPER],
                ..TWO
            },
            &LOOKS[..1],
        );
        let lower = each(
            &Scatter {
                fields: &[LOWER],
                ..TWO
            },
            &LOOKS[1..],
        );
        let overlap = |corner: &Point| upper.iter().any(|(other, _)| other == corner);
        assert!(
            lower.iter().any(|(corner, _)| overlap(corner)),
            "the fields overlap"
        );
        let mut expected = upper.clone();
        expected.extend(lower.iter().filter(|(corner, _)| !overlap(corner)));
        expected.sort_by_key(|(corner, _)| (corner.y, corner.x));
        assert_eq!(each(&TWO, &LOOKS), expected);
    }

    #[test]
    fn a_mark_that_changes_tone_is_damaged() {
        let toned = Scatter {
            tones: Some(Tones {
                colors: &chrome::HALFTONE,
                dense: 0.7,
            }),
            ..TWO
        };
        let before = toned.shown(&LOOKS);
        let point = before
            .layer(SHOWS)
            .iter()
            .enumerate()
            .find_map(|(word, bits)| {
                (*bits != 0).then(|| word * 32 + bits.trailing_zeros() as usize)
            });
        let point = point.expect("a mark shows");
        let mut after = before.clone();
        after.layer_mut(TONES + 1)[point / 32] ^= 1 << (point % 32);
        let mut changed = Dirty::default();
        toned.changed(&before, &after, &mut changed);
        assert!(!changed.is_empty());
        // The dense side faces the look, so turning it moves marks between tones.
        let turned = LOOKS.map(|look| Look {
            facing: look.facing + 1.0,
            ..look
        });
        let later = toned.shown(&turned);
        let tones = |shown: &Shown| [shown.layer(TONES).to_vec(), shown.layer(TONES + 1).to_vec()];
        assert_ne!(tones(&before), tones(&later));
    }

    #[test]
    fn comparing_two_looks_damages_what_their_shown_marks_differ_in() {
        let toned = Scatter {
            tones: Some(Tones {
                colors: &chrome::HALFTONE,
                dense: 0.7,
            }),
            ..TWO
        };
        let turned = LOOKS.map(|look| Look {
            facing: look.facing + 0.4,
            ..look
        });
        let thinner = LOOKS.map(|look| Look {
            density: 0.6,
            ..look
        });
        let clear = [Rectangle::new(Point::new(150, 120), Size::new(90, 60))];
        let rows = |damage: &Dirty| {
            (0..DISPLAY_SIZE.height as i32)
                .map(|y| damage.spans(y).collect::<Vec<_>>())
                .collect::<Vec<_>>()
        };
        for scatter in [&TWO, &toned] {
            for (before, after) in [
                ((&LOOKS, &[][..]), (&turned, &[][..])),
                ((&LOOKS, &[][..]), (&thinner, &clear[..])),
                ((&turned, &clear[..]), (&turned, &[][..])),
                ((&LOOKS, &[][..]), (&LOOKS, &[][..])),
            ] {
                let mut paired = Dirty::default();
                scatter.changed_between((before.0, before.1), (after.0, after.1), &mut paired);
                let mut shown = Dirty::default();
                scatter.changed(
                    &scatter.shown_clear_of(before.0, before.1),
                    &scatter.shown_clear_of(after.0, after.1),
                    &mut shown,
                );
                assert_eq!(rows(&paired), rows(&shown));
            }
        }
    }

    #[test]
    fn a_mark_that_changes_kind_is_damaged() {
        let before = TWO.shown(&LOOKS);
        let point = before
            .layer(SHOWS)
            .iter()
            .enumerate()
            .find_map(|(word, bits)| {
                (*bits != 0).then(|| word * 32 + bits.trailing_zeros() as usize)
            });
        let point = point.expect("a mark shows");
        let mut after = before.clone();
        after.layer_mut(HOLLOWS)[point / 32] ^= 1 << (point % 32);
        let mut changed = Dirty::default();
        TWO.changed(&before, &after, &mut changed);
        let mut expected = Dirty::default();
        let (row, column) = (point as i32 / TWO.columns(), point as i32 % TWO.columns());
        expected.add(Rectangle::new(
            TWO.corner(TWO.origin.y + row * PITCH, column),
            Size::new_equal(MARK as u32),
        ));
        assert_eq!(
            (changed.bounding_box(), changed.pixels()),
            (expected.bounding_box(), expected.pixels())
        );
    }

    #[test]
    fn a_level_s_changes_are_the_marks_that_differ_where_two_fields_overlap() {
        // A gap that moves the rows below it further than the identity's does, by more than a
        // row.
        let toned = Scatter {
            gap: Some((197..=237, 11)),
            tones: Some(Tones {
                colors: &chrome::HALFTONE,
                dense: 0.7,
            }),
            ..TWO
        };
        let looks = |level: u8| {
            LOOKS.map(|look| Look {
                density: look.density * f32::from(level) / 255.0,
                ..look
            })
        };
        let changes = toned.changes(looks);
        let clear = [Rectangle::new(Point::new(150, 120), Size::new(90, 60))];
        // Moved less than a cell and more, with an empty box and one partly off the panel.
        let nudged = [Rectangle::new(Point::new(153, 125), Size::new(90, 60))];
        let moved = [
            Rectangle::new(Point::new(190, 160), Size::new(90, 60)),
            Rectangle::new(Point::new(300, 200), Size::zero()),
            Rectangle::new(Point::new(-20, -10), Size::new(200, 30)),
            Rectangle::new(Point::new(120, 241), Size::new(100, 21)),
            // One whose first row is a moved one that starts above it.
            Rectangle::new(Point::new(150, 259), Size::new(120, 10)),
            // Empty boxes across a row's cells and down inside a column's.
            Rectangle::new(Point::new(150, 262), Size::new(120, 0)),
            Rectangle::new(Point::new(175, 250), Size::new(0, 40)),
        ];
        assert_changes_match(&changes, looks, &[&[], &clear, &nudged, &moved]);
    }
}
