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

    fn columns(&self) -> (i32, i32) {
        (i32::from(self.start) * GRAIN, i32::from(self.end) * GRAIN)
    }
}

#[derive(Debug)]
pub struct RowSpans<const WIDTH: usize, const BANDS: usize, const K: usize> {
    spans: [[Span; K]; BANDS],
    counts: [u8; BANDS],
    /// The first and last bands that hold a span; `first > last` when none does.
    first: u16,
    last: u16,
    full: bool,
    /// The span of a band while the damage is full.
    whole: Span,
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
            first: u16::MAX,
            last: 0,
            full: false,
            whole: Span {
                start: 0,
                end: (WIDTH / GRAIN as usize) as u8,
            },
        }
    }

    #[must_use]
    pub const fn new_full() -> Self {
        let mut spans = Self::new();
        spans.full = true;
        spans
    }

    pub fn clear(&mut self) {
        for band in self.used() {
            self.counts[band] = 0;
        }
        (self.first, self.last) = (u16::MAX, 0);
        self.full = false;
    }

    /// The bands that can hold a span.
    fn used(&self) -> core::ops::Range<usize> {
        if self.full {
            0..BANDS
        } else {
            usize::from(self.first)..usize::from(self.last) + 1
        }
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
        !self.full && self.first > self.last
    }

    /// The spans of band `band`, as column ranges.
    fn band(&self, band: usize) -> impl Iterator<Item = (i32, i32)> + '_ {
        self.band_spans(band).iter().map(Span::columns)
    }

    fn band_spans(&self, band: usize) -> &[Span] {
        if self.full {
            core::slice::from_ref(&self.whole)
        } else {
            &self.spans[band][..usize::from(self.counts[band])]
        }
    }

    /// The damaged column ranges of pixel row `y`, left to right.
    pub fn spans(&self, y: i32) -> impl Iterator<Item = (i32, i32)> + '_ {
        let spans = if (0..Self::HEIGHT as i32).contains(&y) {
            self.band_spans((y / GRAIN) as usize)
        } else {
            &[]
        };
        spans.iter().map(Span::columns)
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

    fn insert(&mut self, band: usize, mut new: Span) {
        self.first = self.first.min(band as u16);
        self.last = self.last.max(band as u16);
        if self.counts[band] == 0 {
            self.spans[band][0] = new;
            self.counts[band] = 1;
            return;
        }
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
    /// either side along each row. At most eight corners.
    pub fn add_polygon(&mut self, corners: &[(f32, f32)], margin: i32) {
        if self.full || corners.is_empty() {
            return;
        }
        // In 1/256 px, so the per-band work is integer. Rounding moves an edge by at most
        // 1/512 px, less than any margin.
        const ONE: i32 = 256;
        let fixed = |value: f32| (value * ONE as f32) as i32;
        // Each edge from its top end: that end's x and y, its bottom's y, and its run per unit
        // of y in 1/65536.
        let mut edges = [(0i32, 0i32, 0i32, 0i64); 8];
        let count = corners.len().min(edges.len());
        let (mut top, mut bottom) = (i32::MAX, i32::MIN);
        for index in 0..count {
            let (a, b) = (corners[index], corners[(index + 1) % count]);
            let (a, b) = ((fixed(a.0), fixed(a.1)), (fixed(b.0), fixed(b.1)));
            let (from, to) = if a.1 <= b.1 { (a, b) } else { (b, a) };
            let run = if to.1 > from.1 {
                (i64::from(to.0 - from.0) << 16) / i64::from(to.1 - from.1)
            } else {
                0
            };
            edges[index] = (from.0, from.1, to.1, run);
            (top, bottom) = (top.min(from.1), bottom.max(to.1));
        }
        let band_height = GRAIN * ONE;
        let first = top.div_euclid(band_height);
        let last = (bottom - 1).max(top).div_euclid(band_height);
        for band in first..=last {
            let (low, high) = (band * band_height, (band + 1) * band_height);
            let (mut left, mut right) = (i32::MAX, i32::MIN);
            for &(x, from, to, run) in &edges[..count] {
                if to < low || from > high {
                    continue;
                }
                for y in [low.max(from), high.min(to)] {
                    let at = x + ((run * i64::from(y - from)) >> 16) as i32;
                    (left, right) = (left.min(at), right.max(at));
                }
            }
            if left <= right {
                self.add_bands(
                    band,
                    band,
                    left.div_euclid(ONE) - margin,
                    (right + ONE - 1).div_euclid(ONE) + margin,
                );
            }
        }
    }

    /// Marks every pixel a disc about `center` touches.
    pub fn add_disc(&mut self, center: (f32, f32), radius: f32) {
        if self.full {
            return;
        }
        let first = (libm::floorf(center.1 - radius) as i32).div_euclid(GRAIN);
        let last = (libm::ceilf(center.1 + radius) as i32 - 1).div_euclid(GRAIN);
        for band in first..=last {
            // The band's widest point is its nearest to the centre.
            let (low, high) = ((band * GRAIN) as f32, ((band + 1) * GRAIN) as f32);
            let dy = (low - center.1).max(center.1 - high).max(0.0);
            let half = libm::sqrtf((radius * radius - dy * dy).max(0.0));
            self.add_bands(
                band,
                band,
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
        for band in other.used() {
            for index in 0..usize::from(other.counts[band]) {
                self.insert(band, other.spans[band][index]);
            }
        }
    }

    /// Adds `other` turned by half a turn about the panel's centre.
    pub fn extend_reflected(&mut self, other: &Self) {
        self.full |= other.full;
        if self.full {
            return;
        }
        let columns = (WIDTH / GRAIN as usize) as u8;
        for band in other.used() {
            for &span in other.band_spans(band) {
                let turned = Span {
                    start: columns - span.end,
                    end: columns - span.start,
                };
                self.insert(BANDS - 1 - band, turned);
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
        if self.is_empty() {
            return Rectangle::zero();
        }
        let (first, last) = (self.used().start, self.used().end - 1);
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
        self.used()
            .flat_map(|band| self.band(band))
            .map(|(start, end)| (end - start) as u32 * GRAIN as u32)
            .sum()
    }

    /// Rectangles covering the damage, for a flush where starting a region costs as much as
    /// sending `overhead` pixels. Each span joins the open rectangle it widens least, across
    /// any gap, when the pixels that adds beyond the span's own cost less than a region.
    /// Rectangles may overlap. Every corner lands on the grain.
    #[must_use]
    pub fn rectangles(&self, overhead: u32) -> Rectangles<'_, WIDTH, BANDS, K> {
        let used = self.used();
        let none = Open {
            span: Span::default(),
            first_band: 0,
            last_band: 0,
        };
        Rectangles {
            spans: self,
            overhead,
            band: used.start,
            end: used.end,
            open: [none; OPEN],
            opened: 0,
            ready: [Rectangle::zero(); OPEN + MAX_K],
            readied: 0,
        }
    }
}

const MAX_K: usize = 7;

impl<const WIDTH: usize, const BANDS: usize, const K: usize> Clone for RowSpans<WIDTH, BANDS, K> {
    fn clone(&self) -> Self {
        let mut copy = Self::new();
        copy.clone_from(self);
        copy
    }

    /// Copies in place, so that a large value never passes through the stack.
    fn clone_from(&mut self, source: &Self) {
        self.clear();
        for band in source.used() {
            let count = usize::from(source.counts[band]);
            self.spans[band][..count].copy_from_slice(&source.spans[band][..count]);
            self.counts[band] = source.counts[band];
        }
        (self.first, self.last, self.full) = (source.first, source.last, source.full);
    }
}

impl<const WIDTH: usize, const BANDS: usize, const K: usize> Default
    for RowSpans<WIDTH, BANDS, K>
{
    fn default() -> Self {
        Self::new()
    }
}

/// How many rectangles [`RowSpans::rectangles`] keeps open at once.
const OPEN: usize = 6;

#[derive(Clone, Copy, Debug)]
struct Open {
    span: Span,
    first_band: usize,
    last_band: usize,
}

impl Open {
    fn pixels(&self) -> u32 {
        self.span.len() * (self.last_band - self.first_band + 1) as u32 * GRAIN as u32
    }

    fn with(&self, span: Span, band: usize) -> Self {
        Self {
            span: Span {
                start: self.span.start.min(span.start),
                end: self.span.end.max(span.end),
            },
            first_band: self.first_band,
            last_band: self.last_band.max(band),
        }
    }

    fn rectangle(self) -> Rectangle {
        Rectangle::new(
            Point::new(i32::from(self.span.start) * GRAIN, self.first_band as i32 * GRAIN),
            Size::new(
                self.span.len(),
                (self.last_band - self.first_band + 1) as u32 * GRAIN as u32,
            ),
        )
    }
}

pub struct Rectangles<'a, const WIDTH: usize, const BANDS: usize, const K: usize> {
    spans: &'a RowSpans<WIDTH, BANDS, K>,
    overhead: u32,
    band: usize,
    end: usize,
    open: [Open; OPEN],
    opened: usize,
    ready: [Rectangle; OPEN + MAX_K],
    readied: usize,
}

impl<const WIDTH: usize, const BANDS: usize, const K: usize> Rectangles<'_, WIDTH, BANDS, K> {
    fn close(&mut self, index: usize) {
        self.ready[self.readied] = self.open[index].rectangle();
        self.readied += 1;
        self.opened -= 1;
        self.open[index] = self.open[self.opened];
    }

    /// Adds the next band's spans to the open rectangles.
    fn advance(&mut self) {
        let band = self.band;
        self.band += 1;
        // A rectangle whose gap alone would cost more than a region can never take a span.
        let mut index = 0;
        while index < self.opened {
            let open = self.open[index];
            let gap = (band - open.last_band - 1) as u32 * GRAIN as u32 * open.span.len();
            if gap > self.overhead {
                self.close(index);
            } else {
                index += 1;
            }
        }
        for &span in self.spans.band_spans(band) {
            let own = span.len() * GRAIN as u32;
            let mut best = (usize::MAX, u32::MAX);
            for (index, open) in self.open[..self.opened].iter().enumerate() {
                let waste = (open.with(span, band).pixels() - open.pixels()).saturating_sub(own);
                if waste < best.1 {
                    best = (index, waste);
                }
            }
            if best.1 <= self.overhead {
                self.open[best.0] = self.open[best.0].with(span, band);
                continue;
            }
            if self.opened == OPEN {
                let oldest = (0..OPEN)
                    .min_by_key(|&index| self.open[index].last_band)
                    .expect("OPEN is not zero");
                self.close(oldest);
            }
            self.open[self.opened] = Open {
                span,
                first_band: band,
                last_band: band,
            };
            self.opened += 1;
        }
    }
}

impl<const WIDTH: usize, const BANDS: usize, const K: usize> Iterator
    for Rectangles<'_, WIDTH, BANDS, K>
{
    type Item = Rectangle;

    fn next(&mut self) -> Option<Rectangle> {
        loop {
            if self.readied > 0 {
                self.readied -= 1;
                return Some(self.ready[self.readied]);
            }
            if self.band < self.end {
                self.advance();
                continue;
            }
            if self.opened == 0 {
                return None;
            }
            self.opened -= 1;
            return Some(self.open[self.opened].rectangle());
        }
    }
}
