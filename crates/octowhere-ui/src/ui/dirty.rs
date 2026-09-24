//! Damage as spans of columns per pair of rows.
//!
//! The panel takes partial writes in 2 × 2 blocks, so damage is kept at that grain: each band of
//! two rows holds up to `K` sorted, disjoint spans of whole column pairs. Past `K`, the two spans
//! with the smallest gap between them merge. Drawing clips to the spans exactly; the flush covers
//! them with rectangles, trading pixels sent for regions started.

use embedded_graphics::{
    prelude::{Point, Size},
    primitives::Rectangle,
};

/// Rows per band, and the column grain.
const GRAIN: i32 = 2;

/// Columns `start * GRAIN..end * GRAIN`.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct Span {
    start: u8,
    end: u8,
}

impl Span {
    fn len(self) -> u32 {
        u32::from(self.end - self.start) * GRAIN as u32
    }
}

#[derive(Clone, Debug)]
pub struct RowSpans<const WIDTH: usize, const BANDS: usize, const K: usize> {
    spans: [[Span; K]; BANDS],
    counts: [u8; BANDS],
    full: bool,
}

impl<const WIDTH: usize, const BANDS: usize, const K: usize> RowSpans<WIDTH, BANDS, K> {
    pub const HEIGHT: usize = BANDS * GRAIN as usize;
    const _CHECK: () = assert!(
        K >= 1 && K <= MAX_K && WIDTH.is_multiple_of(GRAIN as usize) && WIDTH / GRAIN as usize <= u8::MAX as usize,
        "K must be 1 to 7, and WIDTH even and at most 510"
    );

    #[must_use]
    pub const fn new() -> Self {
        let _: () = Self::_CHECK;
        Self {
            spans: [[Span { start: 0, end: 0 }; K]; BANDS],
            counts: [0; BANDS],
            full: false,
        }
    }

    #[must_use]
    pub const fn new_full() -> Self {
        let mut spans = Self::new();
        spans.full = true;
        spans
    }

    pub fn clear(&mut self) {
        self.counts = [0; BANDS];
        self.full = false;
    }

    #[inline(always)]
    #[must_use]
    pub fn is_full(&self) -> bool {
        self.full
    }

    pub fn make_full(&mut self) {
        self.full = true;
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        !self.full && self.counts.iter().all(|&count| count == 0)
    }

    /// The spans of band `band`, as column ranges.
    fn band(&self, band: usize) -> impl Iterator<Item = (i32, i32)> + '_ {
        let whole = Span {
            start: 0,
            end: (WIDTH / GRAIN as usize) as u8,
        };
        let count = if self.full { 0 } else { usize::from(self.counts[band]) };
        self.full
            .then_some(whole)
            .into_iter()
            .chain(self.spans[band][..count].iter().copied())
            .map(|span| (i32::from(span.start) * GRAIN, i32::from(span.end) * GRAIN))
    }

    /// The damaged column ranges of pixel row `y`, left to right.
    pub fn spans(&self, y: i32) -> impl Iterator<Item = (i32, i32)> + '_ {
        let band = usize::try_from(y / GRAIN)
            .ok()
            .filter(|&band| y >= 0 && band < BANDS);
        band.into_iter().flat_map(|band| self.band(band))
    }

    /// Marks columns `start..end` of bands `first..=last`, widened to the grain and clipped.
    fn add_bands(&mut self, first: i32, last: i32, start: i32, end: i32) {
        if self.full {
            return;
        }
        let start = start.max(0).div_euclid(GRAIN);
        let end = (end + GRAIN - 1).div_euclid(GRAIN).min((WIDTH / GRAIN as usize) as i32);
        if start >= end {
            return;
        }
        let span = Span {
            start: start as u8,
            end: end as u8,
        };
        for band in first.max(0)..=last.min(BANDS as i32 - 1) {
            self.insert(band as usize, span);
        }
    }

    fn add_row(&mut self, y: i32, start: i32, end: i32) {
        self.add_bands(y.div_euclid(GRAIN), y.div_euclid(GRAIN), start, end);
    }

    fn insert(&mut self, band: usize, mut new: Span) {
        let mut merged = [Span::default(); MAX_K + 1];
        let mut count = 0;
        let mut placed = false;
        for &span in &self.spans[band][..usize::from(self.counts[band])] {
            if span.end < new.start {
                merged[count] = span;
            } else if span.start > new.end {
                if !placed {
                    merged[count] = new;
                    count += 1;
                    placed = true;
                }
                merged[count] = span;
            } else {
                new.start = new.start.min(span.start);
                new.end = new.end.max(span.end);
                continue;
            }
            count += 1;
        }
        if !placed {
            merged[count] = new;
            count += 1;
        }
        if count > K {
            let closest = (0..count - 1)
                .min_by_key(|&index| merged[index + 1].start - merged[index].end)
                .expect("more than one span");
            merged[closest].end = merged[closest + 1].end;
            merged.copy_within(closest + 2..count, closest + 1);
            count -= 1;
        }
        self.spans[band][..count].copy_from_slice(&merged[..count]);
        self.counts[band] = count as u8;
    }

    pub fn add(&mut self, rect: Rectangle) {
        let Some(bottom_right) = rect.bottom_right() else {
            return;
        };
        self.add_bands(
            rect.top_left.y.div_euclid(GRAIN),
            bottom_right.y.div_euclid(GRAIN),
            rect.top_left.x,
            bottom_right.x + 1,
        );
    }

    /// Marks every pixel a convex polygon through `corners` touches, and `margin` pixels more
    /// either side along each row.
    pub fn add_polygon(&mut self, corners: &[(f32, f32)], margin: i32) {
        if self.full || corners.is_empty() {
            return;
        }
        let (top, bottom) = corners
            .iter()
            .fold((f32::MAX, f32::MIN), |(top, bottom), &(_, y)| (top.min(y), bottom.max(y)));
        let first = libm::floorf(top) as i32;
        let last = libm::ceilf(bottom) as i32 - 1;
        for y in first..=last.max(first) {
            let (low, high) = (y as f32, y as f32 + 1.0);
            let (mut left, mut right) = (f32::MAX, f32::MIN);
            for (index, &(x0, y0)) in corners.iter().enumerate() {
                let (x1, y1) = corners[(index + 1) % corners.len()];
                // The edge clipped to this row's height.
                let (from, to) = if y0 <= y1 { ((x0, y0), (x1, y1)) } else { ((x1, y1), (x0, y0)) };
                if to.1 < low || from.1 > high {
                    continue;
                }
                let at = |y: f32| {
                    if to.1 == from.1 {
                        from.0
                    } else {
                        from.0 + (to.0 - from.0) * ((y - from.1) / (to.1 - from.1)).clamp(0.0, 1.0)
                    }
                };
                for x in [at(low.max(from.1)), at(high.min(to.1))] {
                    left = left.min(x);
                    right = right.max(x);
                }
            }
            if left <= right {
                self.add_row(
                    y,
                    libm::floorf(left) as i32 - margin,
                    libm::ceilf(right) as i32 + margin,
                );
            }
        }
    }

    /// Marks every pixel a disc about `center` touches.
    pub fn add_disc(&mut self, center: (f32, f32), radius: f32) {
        if self.full {
            return;
        }
        let first = libm::floorf(center.1 - radius) as i32;
        let last = libm::ceilf(center.1 + radius) as i32 - 1;
        for y in first..=last {
            // The row's widest point is its nearest to the centre.
            let dy = (center.1 - (y as f32 + 0.5)).abs() - 0.5;
            let half = libm::sqrtf((radius * radius - dy.max(0.0) * dy.max(0.0)).max(0.0));
            self.add_row(
                y,
                libm::floorf(center.0 - half) as i32,
                libm::ceilf(center.0 + half) as i32,
            );
        }
    }

    pub fn extend(&mut self, other: &Self) {
        self.full |= other.full;
        if self.full {
            return;
        }
        for band in 0..BANDS {
            for index in 0..usize::from(other.counts[band]) {
                self.insert(band, other.spans[band][index]);
            }
        }
    }

    /// Whether any damaged pixel lies in `area`.
    #[must_use]
    pub fn intersects(&self, area: &Rectangle) -> bool {
        let Some(bottom_right) = area.bottom_right() else {
            return false;
        };
        let (left, right) = (area.top_left.x, bottom_right.x + 1);
        let first = area.top_left.y.div_euclid(GRAIN).max(0);
        let last = bottom_right.y.div_euclid(GRAIN).min(BANDS as i32 - 1);
        (first..=last).any(|band| {
            self.band(band as usize)
                .any(|(start, end)| start < right && left < end)
        })
    }

    #[must_use]
    pub fn contains(&self, point: Point) -> bool {
        self.spans(point.y)
            .any(|(start, end)| (start..end).contains(&point.x))
    }

    #[must_use]
    pub fn bounding_box(&self) -> Rectangle {
        let mut bands = (0..BANDS).filter(|&band| self.band(band).next().is_some());
        let Some(first) = bands.next() else {
            return Rectangle::zero();
        };
        let last = bands.next_back().unwrap_or(first);
        let (left, right) = (first..=last)
            .flat_map(|band| self.band(band))
            .fold((i32::MAX, i32::MIN), |(left, right), (start, end)| {
                (left.min(start), right.max(end))
            });
        Rectangle::with_corners(
            Point::new(left, first as i32 * GRAIN),
            Point::new(right - 1, (last as i32 + 1) * GRAIN - 1),
        )
    }

    /// How many pixels are damaged.
    #[must_use]
    pub fn pixels(&self) -> u32 {
        (0..BANDS)
            .flat_map(|band| self.band(band))
            .map(|(start, end)| (end - start) as u32 * GRAIN as u32)
            .sum()
    }

    /// Rectangles covering the damage. A rectangle grows over the next band's span when that
    /// wastes at most `overhead` pixels, the cost of starting another region; otherwise a new one
    /// starts. Every corner lands on the grain.
    #[must_use]
    pub fn rectangles(&self, overhead: u32) -> Rectangles<'_, WIDTH, BANDS, K> {
        Rectangles {
            spans: self,
            overhead,
            band: 0,
            open: [None; K],
            ready: [None; K],
        }
    }
}

const MAX_K: usize = 7;

impl<const WIDTH: usize, const BANDS: usize, const K: usize> Default
    for RowSpans<WIDTH, BANDS, K>
{
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug)]
struct Open {
    span: Span,
    first_band: usize,
    bands: u32,
}

impl Open {
    fn rectangle(self) -> Rectangle {
        Rectangle::new(
            Point::new(i32::from(self.span.start) * GRAIN, self.first_band as i32 * GRAIN),
            Size::new(self.span.len(), self.bands * GRAIN as u32),
        )
    }
}

pub struct Rectangles<'a, const WIDTH: usize, const BANDS: usize, const K: usize> {
    spans: &'a RowSpans<WIDTH, BANDS, K>,
    overhead: u32,
    band: usize,
    open: [Option<Open>; K],
    ready: [Option<Rectangle>; K],
}

impl<const WIDTH: usize, const BANDS: usize, const K: usize> Rectangles<'_, WIDTH, BANDS, K> {
    fn close(&mut self, open: Open) {
        let slot = self
            .ready
            .iter_mut()
            .find(|slot| slot.is_none())
            .expect("at most K rectangles close per band");
        *slot = Some(open.rectangle());
    }

    /// Extends or closes the open rectangles over the next band's spans.
    fn advance(&mut self) {
        let band = self.band;
        self.band += 1;
        let mut next = [None; K];
        let mut taken = [false; K];
        let spans = self.spans;
        for (slot, (start, end)) in spans.band(band).enumerate() {
            let span = Span {
                start: (start / GRAIN) as u8,
                end: (end / GRAIN) as u8,
            };
            // The first open rectangle over this span, if any. Spans and rectangles are both
            // sorted, so this pairs them left to right.
            let over = (0..K).find(|&index| {
                !taken[index]
                    && self.open[index]
                        .is_some_and(|open| open.span.start < span.end && span.start < open.span.end)
            });
            let grown = over.and_then(|index| {
                taken[index] = true;
                let open = self.open[index].expect("found above");
                let wide = Span {
                    start: open.span.start.min(span.start),
                    end: open.span.end.max(span.end),
                };
                let waste = (wide.len() - span.len()) + (wide.len() - open.span.len()) * open.bands;
                if waste * GRAIN as u32 <= self.overhead {
                    Some(Open {
                        span: wide,
                        bands: open.bands + 1,
                        ..open
                    })
                } else {
                    None
                }
            });
            next[slot] = Some(grown.unwrap_or(Open {
                span,
                first_band: band,
                bands: 1,
            }));
            if grown.is_some() {
                self.open[over.expect("grown")] = None;
            }
        }
        for index in 0..K {
            if let Some(open) = self.open[index].take() {
                self.close(open);
            }
        }
        self.open = next;
    }
}

impl<const WIDTH: usize, const BANDS: usize, const K: usize> Iterator
    for Rectangles<'_, WIDTH, BANDS, K>
{
    type Item = Rectangle;

    fn next(&mut self) -> Option<Rectangle> {
        loop {
            if let Some(ready) = self.ready.iter_mut().find_map(Option::take) {
                return Some(ready);
            }
            if self.band < BANDS {
                self.advance();
                continue;
            }
            return self.open.iter_mut().find_map(Option::take).map(Open::rectangle);
        }
    }
}
