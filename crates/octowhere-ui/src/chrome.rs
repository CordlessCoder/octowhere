use core::cell::RefCell;

use alloc::rc::Rc;
use embedded_graphics::{
    Pixel,
    pixelcolor::{Gray8, Rgb565, Rgb888, raw::RawU16},
    prelude::{Dimensions, DrawTarget, GrayColor, PixelColor, Point, PointsIter, RawData, RgbColor, Size, Transform},
    primitives::Rectangle,
};
use embedded_layout::align::{HorizontalAlignment, VerticalAlignment};
use fontdue::{FontRepr, layout::Layout};

use crate::{board, ui::dirty::RowSpans};

// Rgb888 is higher quality, Rgb565 cuts the size of the framebuffer by a third.
// Gray8 is 3x smaller than Rgb888... but I'm not sure we love monochrome.
pub type Color = embedded_graphics::pixelcolor::Rgb565;

pub const DISPLAY_SIZE: Size = Size::new(board::LCD_WIDTH as u32, board::LCD_HEIGHT as u32);
pub const DISPLAY_BBOX: Rectangle = Rectangle::new(Point::new_equal(0), DISPLAY_SIZE);

pub type FB = crate::framebuffer::Framebuffer<
    {
        crate::framebuffer::buffer_size::<Color>(
            board::LCD_WIDTH as usize,
            board::LCD_HEIGHT as usize,
        )
    },
    { board::LCD_WIDTH as usize },
    { board::LCD_HEIGHT as usize },
    Color,
>;
/// Spans kept per band of damage. The turning dial puts marks on both sides of most rows, and a
/// letter or the slab can sit between them.
pub const DAMAGE_SPANS: usize = 4;
/// What starting another flush region costs, in pixels sent. A flush rectangle grows over a
/// neighbouring span while that wastes no more than this.
pub const FLUSH_OVERHEAD: u32 = 128;
pub type Dirty =
    RowSpans<{ board::LCD_WIDTH as usize }, { board::LCD_HEIGHT as usize / 2 }, DAMAGE_SPANS>;

/// A draw target that takes antialiased coverage a row at a time and blends it over what it
/// already holds, so edges come out right over any background.
pub trait CoverageTarget: DrawTarget {
    /// Blends `color` into row `y` from column `x` onward, one coverage byte per pixel: 0 leaves
    /// the pixel, 255 replaces it, and anything between mixes with it. Pixels outside the target
    /// are skipped.
    fn blend_row(&mut self, x: i32, y: i32, coverage: &[u8], color: Self::Color);

    /// Whether anything drawn in `area` can land. A drawing may skip work it knows is outside;
    /// it must still clip, since this is only a hint.
    fn visible(&self, area: &Rectangle) -> bool {
        !area.intersection(&self.bounding_box()).is_zero_sized()
    }

    fn blend_pixel(&mut self, point: Point, coverage: u8, color: Self::Color) {
        self.blend_row(point.x, point.y, &[coverage], color);
    }

    /// As [`blend_row`](Self::blend_row), for a caller that knows every pixel it touches is
    /// `background`. A target can then mix without reading its pixels back.
    fn blend_row_over(
        &mut self,
        x: i32,
        y: i32,
        coverage: &[u8],
        color: Self::Color,
        background: Self::Color,
    ) {
        let _ = background;
        self.blend_row(x, y, coverage, color);
    }

    /// Called by text drawings before each glyph's rows, with the box its rows fall in.
    fn begin_glyph(&mut self, bounds: Rectangle) {
        let _ = bounds;
    }

    /// Replaces every pixel of the row: `color` mixed over `background` by its coverage, so 0
    /// writes `background`. It writes each pixel once where filling and then blending writes twice.
    fn paint_row(&mut self, x: i32, y: i32, coverage: &[u8], color: Self::Color, background: Self::Color) {
        let _ = self.fill_solid(&Rectangle::new(Point::new(x, y), Size::new(coverage.len() as u32, 1)), background);
        self.blend_row_over(x, y, coverage, color, background);
    }
}

/// A coverage target whose pixels under anything drawn through it are all `background`, so
/// partly covered pixels are mixed with that colour instead of read back. Nothing checks the
/// promise: drawn over anything else, edges come out mixed with the wrong colour.
pub struct OnBackground<'a, T: CoverageTarget> {
    parent: &'a mut T,
    background: T::Color,
}

impl<'a, T: CoverageTarget> OnBackground<'a, T> {
    pub fn new(parent: &'a mut T, background: T::Color) -> Self {
        Self { parent, background }
    }
}

impl<T: CoverageTarget> Dimensions for OnBackground<'_, T> {
    fn bounding_box(&self) -> Rectangle {
        self.parent.bounding_box()
    }
}

impl<T: CoverageTarget> DrawTarget for OnBackground<'_, T> {
    type Color = T::Color;
    type Error = T::Error;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        self.parent.draw_iter(pixels)
    }

    fn fill_contiguous<I>(&mut self, area: &Rectangle, colors: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Self::Color>,
    {
        self.parent.fill_contiguous(area, colors)
    }

    fn fill_solid(&mut self, area: &Rectangle, color: Self::Color) -> Result<(), Self::Error> {
        self.parent.fill_solid(area, color)
    }
}

impl<T: CoverageTarget> CoverageTarget for OnBackground<'_, T> {
    fn visible(&self, area: &Rectangle) -> bool {
        self.parent.visible(area)
    }

    #[inline]
    fn blend_row(&mut self, x: i32, y: i32, coverage: &[u8], color: Self::Color) {
        self.parent.blend_row_over(x, y, coverage, color, self.background);
    }

    #[inline]
    fn blend_row_over(
        &mut self,
        x: i32,
        y: i32,
        coverage: &[u8],
        color: Self::Color,
        background: Self::Color,
    ) {
        self.parent.blend_row_over(x, y, coverage, color, background);
    }

    #[inline]
    fn paint_row(&mut self, x: i32, y: i32, coverage: &[u8], color: Self::Color, background: Self::Color) {
        self.parent.paint_row(x, y, coverage, color, background);
    }
}

impl CoverageTarget for FB {
    fn blend_row(&mut self, x: i32, y: i32, coverage: &[u8], color: Color) {
        self.blend_row_with(x, y, coverage, color, |pixel, covered| {
            let under = Rgb565::from(RawU16::new(u16::from_be_bytes([pixel[0], pixel[1]])));
            under.lerp(&color, covered)
        });
    }

    fn blend_row_over(&mut self, x: i32, y: i32, coverage: &[u8], color: Color, background: Color) {
        self.blend_row_with(x, y, coverage, color, |_, covered| background.lerp(&color, covered));
    }

    // PERF: storing uniform runs as words, two pixels a word, or eight bytes of coverage at a
    // time all measured slower in the fault screen's frame than this, though faster alone.
    fn paint_row(&mut self, x: i32, y: i32, coverage: &[u8], color: Color, background: Color) {
        const WIDTH: i32 = board::LCD_WIDTH as i32;
        if !(0..board::LCD_HEIGHT as i32).contains(&y) {
            return;
        }
        let start = x.max(0);
        let end = x.saturating_add(coverage.len() as i32).min(WIDTH);
        if start >= end {
            return;
        }
        let coverage = &coverage[(start - x) as usize..(end - x) as usize];
        let row = (y * WIDTH) as usize;
        let pixels = &mut self.buffer_mut()[(row + start as usize) * 2..(row + end as usize) * 2];
        let raw = |color: Color| RawU16::from(color).into_inner().to_be_bytes();
        let (full, empty) = (raw(color), raw(background));
        for (pixel, &covered) in pixels.as_chunks_mut::<2>().0.iter_mut().zip(coverage) {
            *pixel = match covered {
                0 => empty,
                u8::MAX => full,
                _ => raw(background.lerp(&color, covered)),
            };
        }
    }
}

impl FB {
    /// The colour stored at `point`, or `None` outside the panel.
    #[must_use]
    pub fn pixel(&self, point: Point) -> Option<Color> {
        let (width, height) = (board::LCD_WIDTH as i32, board::LCD_HEIGHT as i32);
        if !(0..width).contains(&point.x) || !(0..height).contains(&point.y) {
            return None;
        }
        let at = ((point.y * width + point.x) * 2) as usize;
        let bytes = [self.buffer()[at], self.buffer()[at + 1]];
        Some(Rgb565::from(RawU16::new(u16::from_be_bytes(bytes))))
    }
}

trait BlendRowWith {
    /// Clips the row to the framebuffer, skips uncovered pixels, stores `color` over fully covered
    /// ones, and stores what `mix` returns for the rest, given the pixel's bytes.
    fn blend_row_with(
        &mut self,
        x: i32,
        y: i32,
        coverage: &[u8],
        color: Color,
        mix: impl Fn(&[u8], u8) -> Color,
    );
}

impl BlendRowWith for FB {
    #[inline(always)]
    fn blend_row_with(
        &mut self,
        x: i32,
        y: i32,
        coverage: &[u8],
        color: Color,
        mix: impl Fn(&[u8], u8) -> Color,
    ) {
        const WIDTH: i32 = board::LCD_WIDTH as i32;
        if !(0..board::LCD_HEIGHT as i32).contains(&y) {
            return;
        }
        let start = x.max(0);
        let end = x.saturating_add(coverage.len() as i32).min(WIDTH);
        if start >= end {
            return;
        }
        let coverage = &coverage[(start - x) as usize..(end - x) as usize];
        let row = (y * WIDTH) as usize;
        let pixels = &mut self.buffer_mut()[(row + start as usize) * 2..(row + end as usize) * 2];
        let full = RawU16::from(color).into_inner().to_be_bytes();
        for (pixel, &covered) in pixels.as_chunks_mut::<2>().0.iter_mut().zip(coverage) {
            match covered {
                0 => {}
                u8::MAX => *pixel = full,
                _ => *pixel = RawU16::from(mix(pixel, covered)).into_inner().to_be_bytes(),
            }
        }
    }
}

/// A view of `parent` shifted by `offset` and clipped to `clip`, which is in the parent's
/// coordinates. It shifts and clips each row once rather than each pixel, which is why it stands
/// in for embedded-graphics' `translated` and `clipped`.
pub struct Window<'a, T> {
    parent: &'a mut T,
    offset: Point,
    clip: Rectangle,
}

impl<'a, T: DrawTarget> Window<'a, T> {
    pub fn new(parent: &'a mut T, offset: Point, clip: Rectangle) -> Self {
        let clip = clip.intersection(&parent.bounding_box());
        Self {
            parent,
            offset,
            clip,
        }
    }
}

impl<T: DrawTarget> Dimensions for Window<'_, T> {
    fn bounding_box(&self) -> Rectangle {
        self.clip.translate(-self.offset)
    }
}

impl<T: DrawTarget> DrawTarget for Window<'_, T> {
    type Color = T::Color;
    type Error = T::Error;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        let (offset, clip) = (self.offset, self.clip);
        self.parent.draw_iter(
            pixels
                .into_iter()
                .map(|Pixel(point, color)| Pixel(point + offset, color))
                .filter(|Pixel(point, _)| clip.contains(*point)),
        )
    }

    fn fill_contiguous<I>(&mut self, area: &Rectangle, colors: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Self::Color>,
    {
        let area = area.translate(self.offset);
        if self.clip.contains(area.top_left)
            && area.bottom_right().is_none_or(|corner| self.clip.contains(corner))
        {
            return self.parent.fill_contiguous(&area, colors);
        }
        let clip = self.clip;
        self.parent.draw_iter(
            area.points()
                .zip(colors)
                .filter(|(point, _)| clip.contains(*point))
                .map(|(point, color)| Pixel(point, color)),
        )
    }

    fn fill_solid(&mut self, area: &Rectangle, color: Self::Color) -> Result<(), Self::Error> {
        let area = area.translate(self.offset).intersection(&self.clip);
        if area.is_zero_sized() {
            return Ok(());
        }
        self.parent.fill_solid(&area, color)
    }
}

impl<T: CoverageTarget> Window<'_, T> {
    /// Shifts a row into the parent's coordinates and clips it: the parent's start column and the
    /// part of `coverage` that is visible.
    #[inline]
    fn clip_row<'c>(&self, x: i32, y: i32, coverage: &'c [u8]) -> Option<(i32, i32, &'c [u8])> {
        let (x, y) = (x + self.offset.x, y + self.offset.y);
        let top = self.clip.top_left;
        if y < top.y || y >= top.y + self.clip.size.height as i32 {
            return None;
        }
        let start = x.max(top.x);
        let end = x
            .saturating_add(coverage.len() as i32)
            .min(top.x + self.clip.size.width as i32);
        (start < end).then(|| (start, y, &coverage[(start - x) as usize..(end - x) as usize]))
    }
}

impl<T: CoverageTarget> CoverageTarget for Window<'_, T> {
    fn visible(&self, area: &Rectangle) -> bool {
        let area = area.translate(self.offset).intersection(&self.clip);
        !area.is_zero_sized() && self.parent.visible(&area)
    }

    #[inline]
    fn blend_row_over(
        &mut self,
        x: i32,
        y: i32,
        coverage: &[u8],
        color: Self::Color,
        background: Self::Color,
    ) {
        if let Some((x, y, coverage)) = self.clip_row(x, y, coverage) {
            self.parent.blend_row_over(x, y, coverage, color, background);
        }
    }

    #[inline]
    fn blend_row(&mut self, x: i32, y: i32, coverage: &[u8], color: Self::Color) {
        if let Some((x, y, coverage)) = self.clip_row(x, y, coverage) {
            self.parent.blend_row(x, y, coverage, color);
        }
    }

    #[inline]
    fn paint_row(&mut self, x: i32, y: i32, coverage: &[u8], color: Self::Color, background: Self::Color) {
        if let Some((x, y, coverage)) = self.clip_row(x, y, coverage) {
            self.parent.paint_row(x, y, coverage, color, background);
        }
    }
}

/// A view of `parent` that draws only on pixels whose centres lie within `radius` of the pixel
/// corner `center`. The edge is hard; a ring drawn over it afterwards gives the antialiasing.
pub struct Round<'a, T> {
    parent: &'a mut T,
    center: Point,
    radius: f32,
}

impl<'a, T: DrawTarget> Round<'a, T> {
    pub fn new(parent: &'a mut T, center: Point, radius: f32) -> Self {
        Self { parent, center, radius }
    }

    /// The columns of row `y` inside the circle, as a start and an end past it.
    fn chord(&self, y: i32) -> Option<(i32, i32)> {
        let dy = y as f32 + 0.5 - self.center.y as f32;
        let squared = self.radius * self.radius - dy * dy;
        if squared < 0.0 {
            return None;
        }
        let half = libm::sqrtf(squared);
        let start = libm::ceilf(self.center.x as f32 - half - 0.5) as i32;
        let end = libm::floorf(self.center.x as f32 + half - 0.5) as i32 + 1;
        (start < end).then_some((start, end))
    }
}

impl<T: DrawTarget> Dimensions for Round<'_, T> {
    fn bounding_box(&self) -> Rectangle {
        self.parent.bounding_box()
    }
}

impl<T: DrawTarget> DrawTarget for Round<'_, T> {
    type Color = T::Color;
    type Error = T::Error;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        let (center, radius) = (self.center, self.radius);
        self.parent.draw_iter(pixels.into_iter().filter(|Pixel(point, _)| {
            let dx = point.x as f32 + 0.5 - center.x as f32;
            let dy = point.y as f32 + 0.5 - center.y as f32;
            dx * dx + dy * dy <= radius * radius
        }))
    }

    fn fill_solid(&mut self, area: &Rectangle, color: Self::Color) -> Result<(), Self::Error> {
        let Some(bottom_right) = area.bottom_right() else {
            return Ok(());
        };
        for y in area.top_left.y..=bottom_right.y {
            let Some((start, end)) = self.chord(y) else {
                continue;
            };
            let (from, to) = (start.max(area.top_left.x), end.min(bottom_right.x + 1));
            if from < to {
                let row = Rectangle::new(Point::new(from, y), Size::new((to - from) as u32, 1));
                self.parent.fill_solid(&row, color)?;
            }
        }
        Ok(())
    }
}

impl<T: CoverageTarget> Round<'_, T> {
    fn clip_row<'c>(&self, x: i32, y: i32, coverage: &'c [u8]) -> Option<(i32, &'c [u8])> {
        let (start, end) = self.chord(y)?;
        let from = x.max(start);
        let to = x.saturating_add(coverage.len() as i32).min(end);
        (from < to).then(|| (from, &coverage[(from - x) as usize..(to - x) as usize]))
    }
}

impl<T: CoverageTarget> CoverageTarget for Round<'_, T> {
    fn visible(&self, area: &Rectangle) -> bool {
        self.parent.visible(area)
    }

    fn blend_row(&mut self, x: i32, y: i32, coverage: &[u8], color: Self::Color) {
        if let Some((x, coverage)) = self.clip_row(x, y, coverage) {
            self.parent.blend_row(x, y, coverage, color);
        }
    }

    fn blend_row_over(
        &mut self,
        x: i32,
        y: i32,
        coverage: &[u8],
        color: Self::Color,
        background: Self::Color,
    ) {
        if let Some((x, coverage)) = self.clip_row(x, y, coverage) {
            self.parent.blend_row_over(x, y, coverage, color, background);
        }
    }

    fn paint_row(&mut self, x: i32, y: i32, coverage: &[u8], color: Self::Color, background: Self::Color) {
        if let Some((x, coverage)) = self.clip_row(x, y, coverage) {
            self.parent.paint_row(x, y, coverage, color, background);
        }
    }
}

/// A view of `parent` that draws only where `damage` marks.
pub struct Clip<'a, T> {
    parent: &'a mut T,
    damage: &'a Dirty,
    bounds: Rectangle,
}

impl<'a, T: DrawTarget> Clip<'a, T> {
    pub fn new(parent: &'a mut T, damage: &'a Dirty) -> Self {
        let bounds = damage.bounding_box().intersection(&parent.bounding_box());
        Self {
            parent,
            damage,
            bounds,
        }
    }
}

impl<T: DrawTarget> Dimensions for Clip<'_, T> {
    fn bounding_box(&self) -> Rectangle {
        self.bounds
    }
}

impl<T: DrawTarget> DrawTarget for Clip<'_, T> {
    type Color = T::Color;
    type Error = T::Error;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        let damage = self.damage;
        self.parent.draw_iter(
            pixels
                .into_iter()
                .filter(|Pixel(point, _)| damage.contains(*point)),
        )
    }

    fn fill_contiguous<I>(&mut self, area: &Rectangle, colors: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Self::Color>,
    {
        let damage = self.damage;
        self.parent.draw_iter(
            area.points()
                .zip(colors)
                .filter(|(point, _)| damage.contains(*point))
                .map(|(point, color)| Pixel(point, color)),
        )
    }

    fn fill_solid(&mut self, area: &Rectangle, color: Self::Color) -> Result<(), Self::Error> {
        let Some(bottom_right) = area.bottom_right() else {
            return Ok(());
        };
        let (left, right) = (area.top_left.x, bottom_right.x + 1);
        let bounds = self.bounds;
        let Some(last) = bounds.bottom_right() else {
            return Ok(());
        };
        let mut result = Ok(());
        for y in area.top_left.y.max(bounds.top_left.y)..=bottom_right.y.min(last.y) {
            let parent = &mut *self.parent;
            self.damage.for_each_part(left, y, (right - left) as usize, |start, from, to| {
                let row = Rectangle::new(Point::new(start, y), Size::new((to - from) as u32, 1));
                if result.is_ok() {
                    result = parent.fill_solid(&row, color);
                }
            });
        }
        result
    }
}

impl<T: CoverageTarget> CoverageTarget for Clip<'_, T> {
    fn visible(&self, area: &Rectangle) -> bool {
        self.damage.intersects(area) && self.parent.visible(area)
    }

    #[inline]
    fn blend_row(&mut self, x: i32, y: i32, coverage: &[u8], color: Self::Color) {
        let parent = &mut *self.parent;
        self.damage.for_each_part(x, y, coverage.len(), |start, from, to| {
            parent.blend_row(start, y, &coverage[from..to], color);
        });
    }

    #[inline]
    fn blend_row_over(
        &mut self,
        x: i32,
        y: i32,
        coverage: &[u8],
        color: Self::Color,
        background: Self::Color,
    ) {
        let parent = &mut *self.parent;
        self.damage.for_each_part(x, y, coverage.len(), |start, from, to| {
            parent.blend_row_over(start, y, &coverage[from..to], color, background);
        });
    }

    #[inline]
    fn paint_row(&mut self, x: i32, y: i32, coverage: &[u8], color: Self::Color, background: Self::Color) {
        let parent = &mut *self.parent;
        self.damage.for_each_part(x, y, coverage.len(), |start, from, to| {
            parent.paint_row(start, y, &coverage[from..to], color, background);
        });
    }
}

/// A view of `parent` that paints text over a solid `background` across a run of rows, writing
/// each pixel once where filling and then blending writes twice. Each glyph's coverage is
/// gathered, then its whole box is painted when the next glyph begins, combined with the glyph
/// before it where their boxes overlap. [`finish`](Self::finish) paints the last glyph and fills
/// the rest of the rows. It keeps two glyphs' coverage, so it takes only text drawn left to right
/// through [`CoverageTarget::begin_glyph`], whose glyphs overlap only their neighbours.
pub struct Knockout<'a, T> {
    parent: &'a mut T,
    rows: core::ops::Range<i32>,
    /// Rows inside `rows` left untouched, for something drawn over them afterwards.
    skip: core::ops::Range<i32>,
    background: Color,
    color: Color,
    glyphs: [GlyphCoverage; 2],
    /// Which of `glyphs` gathers the glyph being drawn.
    current: usize,
    /// Per row, the column up to which it has been painted.
    painted: alloc::vec::Vec<i32>,
}

#[derive(Default)]
struct GlyphCoverage {
    bounds: Rectangle,
    coverage: alloc::vec::Vec<u8>,
}

impl GlyphCoverage {
    fn rows(&self, y: i32) -> Option<core::ops::Range<usize>> {
        let width = self.bounds.size.width as usize;
        let i = y - self.bounds.top_left.y;
        (0..self.bounds.size.height as i32)
            .contains(&i)
            .then(|| i as usize * width..(i as usize + 1) * width)
    }

    /// Row `y`'s coverage in columns `columns`, which must lie inside the box.
    fn span(&self, y: i32, columns: core::ops::Range<i32>) -> Option<&[u8]> {
        let row = self.rows(y)?;
        let left = self.bounds.top_left.x;
        Some(&self.coverage[row][(columns.start - left) as usize..(columns.end - left) as usize])
    }
}

impl<'a, T: CoverageTarget<Color = Color>> Knockout<'a, T> {
    pub fn new(
        parent: &'a mut T,
        rows: core::ops::Range<i32>,
        skip: core::ops::Range<i32>,
        background: Color,
    ) -> Self {
        Self {
            parent,
            painted: alloc::vec![0; rows.len()],
            rows,
            skip,
            background,
            color: background,
            glyphs: Default::default(),
            current: 0,
        }
    }

    /// Paints the glyph gathered so far, and the background left of it on each of its rows. Where
    /// its box overlaps the glyph before, the earlier coverage is folded into it first.
    fn paint_current(&mut self) {
        const WIDTH: i32 = board::LCD_WIDTH as i32;
        let [first, second] = &mut self.glyphs;
        let (current, previous) = if self.current == 0 { (first, &*second) } else { (second, &*first) };
        let (left, right) = (current.bounds.top_left.x, current.bounds.top_left.x + current.bounds.size.width as i32);
        let shared = left.max(previous.bounds.top_left.x)
            ..right.min(previous.bounds.top_left.x + previous.bounds.size.width as i32);
        for y in current.bounds.rows() {
            if self.skip.contains(&y) {
                continue;
            }
            let Some(row) = current.rows(y) else {
                continue;
            };
            if let Some(before) = (!shared.is_empty()).then(|| previous.span(y, shared.clone())).flatten() {
                let over = &mut current.coverage[row.clone()][(shared.start - left) as usize..(shared.end - left) as usize];
                for (over, &under) in over.iter_mut().zip(before) {
                    *over = lerp_u8(under, u8::MAX, *over);
                }
            }
            let painted = &mut self.painted[(y - self.rows.start) as usize];
            if *painted < left {
                let gap = Rectangle::new(Point::new(*painted, y), Size::new((left.min(WIDTH) - *painted) as u32, 1));
                let _ = self.parent.fill_solid(&gap, self.background);
            }
            *painted = (*painted).max(right.clamp(0, WIDTH));
            self.parent.paint_row(left, y, &current.coverage[row], self.color, self.background);
        }
    }

    /// Paints the last glyph and fills what no glyph reached.
    pub fn finish(mut self) {
        const WIDTH: i32 = board::LCD_WIDTH as i32;
        self.paint_current();
        for (y, &painted) in self.rows.clone().zip(&self.painted) {
            if painted < WIDTH && !self.skip.contains(&y) {
                let rest = Rectangle::new(Point::new(painted, y), Size::new((WIDTH - painted) as u32, 1));
                let _ = self.parent.fill_solid(&rest, self.background);
            }
        }
    }
}

impl<T: DrawTarget> Dimensions for Knockout<'_, T> {
    fn bounding_box(&self) -> Rectangle {
        self.parent.bounding_box()
    }
}

impl<T: DrawTarget> DrawTarget for Knockout<'_, T> {
    type Color = T::Color;
    type Error = T::Error;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        self.parent.draw_iter(pixels)
    }
}

impl<T: CoverageTarget<Color = Color>> CoverageTarget for Knockout<'_, T> {
    fn visible(&self, area: &Rectangle) -> bool {
        self.parent.visible(area)
    }

    fn begin_glyph(&mut self, bounds: Rectangle) {
        self.paint_current();
        self.current = 1 - self.current;
        let (top, bottom) = (bounds.top_left.y.max(self.rows.start), (bounds.top_left.y + bounds.size.height as i32).min(self.rows.end));
        let glyph = &mut self.glyphs[self.current];
        glyph.bounds = Rectangle::new(
            Point::new(bounds.top_left.x, top),
            Size::new(bounds.size.width, (bottom - top).max(0) as u32),
        );
        glyph.coverage.clear();
        glyph.coverage.resize((glyph.bounds.size.width * glyph.bounds.size.height) as usize, 0);
    }

    /// A glyph sends each of its rows once, so its coverage is stored rather than blended.
    fn blend_row(&mut self, x: i32, y: i32, coverage: &[u8], color: Self::Color) {
        self.color = color;
        if self.skip.contains(&y) {
            return;
        }
        let glyph = &mut self.glyphs[self.current];
        let Some(row) = glyph.rows(y) else {
            return;
        };
        let left = glyph.bounds.top_left.x;
        let start = x.max(left);
        let end = x.saturating_add(coverage.len() as i32).min(left + glyph.bounds.size.width as i32);
        if start < end {
            glyph.coverage[row][(start - left) as usize..(end - left) as usize]
                .copy_from_slice(&coverage[(start - x) as usize..(end - x) as usize]);
        }
    }
}

// `scale` is the size in px per em the outlines are flattened for; larger text shows the facets.
// At 24 the compass readout matches a much finer flattening. Raising it costs rasterization time
// on every glyph drawn.
fontdue_macros::fontdue_font_from_file!(
    MarathonShapiroFont,
    "../../../assets/MarathonShapiro-Wide65_subset.ttf",
    scale: 24.0
);

fontdue_macros::fontdue_font_from_file!(
    FraktionMonoRegularFont,
    "../../../assets/PPFraktion-Free for personal use v1.1/Mono/PPFraktionMono-Regular.otf",
    scale: 24.0,
    chars: " !\"#$%&'()*+,-./0123456789:;<=>?@ABCDEFGHIJKLMNOPQRSTUVWXYZ[\\]^_`abcdefghijklmnopqrstuvwxyz{|}~\u{a9}\u{b0}"
);

fontdue_macros::fontdue_font_from_file!(
    FraktionMonoBoldFont,
    "../../../assets/PPFraktion-Free for personal use v1.1/Mono/PPFraktionMono-Bold.otf",
    scale: 24.0,
    chars: " !\"#$%&'()*+,-./0123456789:;<=>?@ABCDEFGHIJKLMNOPQRSTUVWXYZ[\\]^_`abcdefghijklmnopqrstuvwxyz{|}~\u{b0}"
);

// Only the start-up's BOOT is set in it.
fontdue_macros::fontdue_font_from_file!(
    InterferenceBoldFont,
    "../../../assets/KH Interference TRIAL/OTF/KHInterferenceTRIAL-Bold.otf",
    scale: 24.0,
    chars: "BOT"
);

/// The fonts a `FontdueRenderer` indexes with `font_index`.
pub const FONTS: &[&dyn FontRepr] =
    &[&MarathonShapiroFont, &FraktionMonoRegularFont, &FraktionMonoBoldFont, &InterferenceBoldFont];
/// Indices into [`FONTS`].
pub const SHAPIRO: usize = 0;
pub const FRAKTION: usize = 1;
pub const FRAKTION_BOLD: usize = 2;
pub const INTERFERENCE_BOLD: usize = 3;

const fn color_from_rgb(r: u8, g: u8, b: u8) -> Color {
    Color::new(
        (r as f64 / 255. * Color::MAX_R as f64) as u8,
        (g as f64 / 255. * Color::MAX_G as f64) as u8,
        (b as f64 / 255. * Color::MAX_B as f64) as u8,
    )
}

const fn unhex(hex: u8) -> u8 {
    match hex {
        b'0'..=b'9' => hex - b'0',
        b'a'..=b'f' => hex - (b'a' - 10),
        b'A'..=b'F' => hex - (b'A' - 10),
        _ => panic!("Not a valid hex digit"),
    }
}

const fn color_from_hex(hex: &str) -> Color {
    let hex = hex.as_bytes();
    assert!(matches!(hex[0], b'#'), "Hex color must start with a #");
    match hex.len() {
        7 => color_from_rgb(
            unhex(hex[1]) * 16 + unhex(hex[2]),
            unhex(hex[3]) * 16 + unhex(hex[4]),
            unhex(hex[5]) * 16 + unhex(hex[6]),
        ),
        _ => unreachable!(),
    }
}

// Values come from the reference board; see context/palette-reference.md.
// BLACK is the exception and stays pure. The doctrine asks for a dark neutral field, but an
// unlit pixel on this AMOLED is a contrast step no near-black reaches.
pub const LIME: Color = color_from_hex("#c0fe04");
pub const RED: Color = color_from_hex("#f24723");
pub const ORANGE: Color = color_from_hex("#f1710d");
pub const PURPLE: Color = color_from_hex("#5500e4");
pub const BLUE: Color = color_from_hex("#409de4");
pub const VIOLET: Color = color_from_hex("#b32be5");
pub const GRAY: Color = color_from_hex("#888e98");
pub const WHITE: Color = color_from_hex("#d2d3d6");
pub const BLACK: Color = color_from_hex("#000000");
/// The intro cinematic's opening blue, not a board swatch: the start-up's BOOT and the grid
/// behind it only (owner, 2026-09-26).
pub const DEEP_BLUE: Color = color_from_hex("#000df6");

/// `color` dimmed toward black, `level` of 255 of the way from it. The designs' dim marks and
/// fields are tokens seen this way, not colours of their own.
#[must_use]
pub fn shade(color: Color, level: u8) -> Color {
    BLACK.lerp(&color, level)
}

#[inline]
pub const fn lerp_u8(a: u8, b: u8, factor: u8) -> u8 {
    // `>> 8` with a +255 bias stands in for `/ 255`: it matches the floor division or exceeds it
    // by one, and factors 0 and 255 return `a` and `b` exactly.
    ((a as u16 * (u8::MAX - factor) as u16 + b as u16 * factor as u16 + u8::MAX as u16) >> 8) as u8
}

pub trait RgbColorExt {
    fn lerp(&self, other: &Self, factor: u8) -> Self;
}

impl RgbColorExt for Rgb888 {
    #[inline]
    fn lerp(&self, other: &Self, factor: u8) -> Self {
        let r = lerp_u8(self.r(), other.r(), factor);
        let g = lerp_u8(self.g(), other.g(), factor);
        let b = lerp_u8(self.b(), other.b(), factor);
        Self::new(r, g, b)
    }
}
impl RgbColorExt for Rgb565 {
    #[inline]
    fn lerp(&self, other: &Self, factor: u8) -> Self {
        let r = lerp_u8(self.r(), other.r(), factor);
        let g = lerp_u8(self.g(), other.g(), factor);
        let b = lerp_u8(self.b(), other.b(), factor);
        Self::new(r, g, b)
    }
}
impl RgbColorExt for Gray8 {
    #[inline]
    fn lerp(&self, other: &Self, factor: u8) -> Self {
        let luma = lerp_u8(self.luma(), other.luma(), factor);
        Self::new(luma)
    }
}

pub struct FontdueRendererCtx {
    layout: fontdue::layout::Layout,
    canvas: fontdue::raster::Raster<'static>,
    /// One row of a glyph's coverage, as `BitmapIter::rows` fills it.
    coverage: alloc::vec::Vec<u8>,
}

/// Rasterizes one upright glyph and blends it with its top-left pixel at `corner`.
#[expect(clippy::too_many_arguments)]
fn blend_glyph<D: CoverageTarget>(
    canvas: &mut fontdue::raster::Raster<'static>,
    row: &mut alloc::vec::Vec<u8>,
    font: &dyn FontRepr,
    index: u16,
    px: f32,
    corner: Point,
    color: D::Color,
    target: &mut D,
) {
    let (metrics, bitmap) = font.rasterize_indexed(canvas, index, px);
    row.resize(metrics.width, 0);
    bitmap.rows(row, |y, x, span| {
        target.blend_row(corner.x + x as i32, corner.y + y as i32, span, color);
    });
}

/// Grows `coverage`, `width` bytes a row, by a pixel into its neighbours: a plus shape, or with
/// `square` a 3 × 3 square, each byte taking the most of those around it. Alternating the two
/// keeps an outline's width close to even across angles, where the square alone thickens
/// diagonals and the plus alone thins them. Only the interior is worked out, so the outermost
/// pixels must stay zero: the caller pads the coverage by one more pixel than it grows it.
fn dilate(coverage: &mut [u8], width: usize, square: bool, scratch: &mut alloc::vec::Vec<u8>) {
    let max3 = |three: &[u8]| three[0].max(three[1]).max(three[2]);
    scratch.clear();
    scratch.extend_from_slice(coverage);
    let rows = scratch.chunks_exact(width);
    let out = coverage.chunks_exact_mut(width).skip(1);
    if square {
        // Three across into `scratch`, then three of those down.
        for (source, across) in coverage.chunks_exact(width).zip(scratch.chunks_exact_mut(width)) {
            for (out, three) in across[1..width - 1].iter_mut().zip(source.windows(3)) {
                *out = max3(three);
            }
        }
        let rows = scratch.chunks_exact(width);
        let out = coverage.chunks_exact_mut(width).skip(1);
        for ((out, up), (middle, down)) in out.zip(rows.clone()).zip(rows.clone().skip(1).zip(rows.skip(2))) {
            for (((out, &up), &middle), &down) in out[1..width - 1].iter_mut().zip(&up[1..]).zip(&middle[1..]).zip(&down[1..]) {
                *out = up.max(middle).max(down);
            }
        }
    } else {
        for ((out, up), (middle, down)) in out.zip(rows.clone()).zip(rows.clone().skip(1).zip(rows.skip(2))) {
            for (((out, &up), three), &down) in out[1..width - 1].iter_mut().zip(&up[1..]).zip(middle.windows(3)).zip(&down[1..]) {
                *out = max3(three).max(up).max(down);
            }
        }
    }
}

/// The farthest a glyph's ink box reaches from `origin`, given its pen at `pen` relative to it, y
/// down, plus a pixel for the edges' coverage.
fn glyph_reach(pen: (f32, f32), metrics: &fontdue::Metrics) -> f32 {
    if metrics.width == 0 || metrics.height == 0 {
        return 0.0;
    }
    let (left, right) = (pen.0 + metrics.xmin as f32, pen.0 + (metrics.xmin + metrics.width as i32) as f32);
    let (top, bottom) = (pen.1 - (metrics.ymin + metrics.height as i32) as f32, pen.1 - metrics.ymin as f32);
    let far = |a: f32, b: f32| a.abs().max(b.abs());
    libm::hypotf(far(left, right), far(top, bottom)) + 1.5
}

impl Default for FontdueRendererCtx {
    fn default() -> Self {
        Self::new()
    }
}

impl FontdueRendererCtx {
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        Self {
            layout: Layout::new(fontdue::layout::CoordinateSystem::PositiveYDown),
            canvas: fontdue::raster::Raster::empty(),
            coverage: alloc::vec::Vec::new(),
        }
    }
    #[inline]
    #[must_use]
    pub fn new_rc() -> Rc<RefCell<Self>> {
        Rc::new(RefCell::new(Self::new()))
    }

    fn reset_layout(&mut self) {
        self.layout.reset(&fontdue::layout::LayoutSettings {
            x: 0.,
            y: 0.,
            max_width: None,
            max_height: None,
            horizontal_align: fontdue::layout::HorizontalAlign::Left,
            vertical_align: fontdue::layout::VerticalAlign::Bottom,
            line_height: 1.0,
            wrap_style: fontdue::layout::WrapStyle::Letter,
            wrap_hard_breaks: true,
        });
    }
}

#[derive(Clone)]
pub struct FontdueRenderer<'f, C> {
    /// Text color.
    pub text_color: C,

    // /// Underline color.
    // pub underline_color: DecorationColor<C>,
    //
    // /// Strikethrough color.
    // pub strikethrough_color: DecorationColor<C>,
    pub font_size: u32,
    pub font_index: usize,
    pub ctx: Rc<RefCell<FontdueRendererCtx>>,
    pub fonts: &'f [&'f dyn FontRepr],
}
impl<'f, C: PixelColor> FontdueRenderer<'f, C> {
    #[must_use]
    pub fn new(
        ctx: Rc<RefCell<FontdueRendererCtx>>,
        font_size: u32,
        text_color: C,
        fonts: &'f [&'f dyn FontRepr],
    ) -> Self {
        Self {
            text_color,
            ctx,
            font_size,
            font_index: 0,
            fonts,
        }
    }
    #[must_use]
    #[inline]
    fn borrow_ctx(&self) -> core::cell::RefMut<'_, FontdueRendererCtx> {
        let mut ctx = self.ctx.borrow_mut();
        ctx.layout.clear();
        ctx
    }
}

impl<C: PixelColor + RgbColorExt> FontdueRenderer<'_, C> {
    fn union_rect(a: Rectangle, b: Rectangle) -> Rectangle {
        if a.is_zero_sized() {
            return b;
        }
        if b.is_zero_sized() {
            return a;
        }
        Rectangle::with_corners(
            a.top_left.component_min(b.top_left),
            a.bottom_right()
                .unwrap()
                .component_max(b.bottom_right().unwrap()),
        )
    }

    /// Where [`draw_rotated`](Self::draw_rotated) starts each glyph's pen, relative to `center`
    /// and before turning, with the glyph's metrics.
    fn rotated_pens<'s>(
        &'s self,
        text: &'s str,
    ) -> Option<impl Iterator<Item = (u16, (f32, f32), fontdue::Metrics)> + 's> {
        let font = self.fonts[self.font_index];
        let px = self.font_size as f32;
        let mut offsets = fontdue::PenOffsets::new(font, text, px);
        let (mut bottom, mut top) = (i32::MAX, i32::MIN);
        for (index, _) in offsets.by_ref() {
            let metrics = font.metrics_indexed(index, px);
            if metrics.height > 0 {
                bottom = bottom.min(metrics.ymin);
                top = top.max(metrics.ymin + metrics.height as i32);
            }
        }
        if bottom > top {
            return None;
        }
        // Pen-relative and y down: back half the advance, and down to the middle of the ink.
        let start = (-offsets.advance() / 2.0, (bottom + top) as f32 / 2.0);
        Some(
            fontdue::PenOffsets::new(font, text, px).map(move |(index, offset)| {
                (index, (start.0 + offset, start.1), font.metrics_indexed(index, px))
            }),
        )
    }

    /// How far from `center` the ink of [`draw_rotated`](Self::draw_rotated) can reach at any
    /// angle, counting pixels its edges touch.
    #[must_use]
    pub fn rotated_reach(&self, text: &str) -> f32 {
        self.rotated_pens(text).map_or(0.0, |pens| {
            pens.map(|(_, pen, metrics)| glyph_reach(pen, &metrics))
                .fold(0.0, f32::max)
        })
    }

    /// Draws `text` centred on `center`, turned clockwise by the angle with this cosine and sine.
    pub fn draw_rotated<D: CoverageTarget<Color = C>>(
        &self,
        text: &str,
        center: Point,
        cos: f32,
        sin: f32,
        target: &mut D,
    ) -> Result<(), D::Error> {
        let font = self.fonts[self.font_index];
        let px = self.font_size as f32;
        let transform = fontdue::Transform::rotation(cos, sin);
        let Some(pens) = self.rotated_pens(text) else {
            return Ok(());
        };
        let ctx = &mut *self.ctx.borrow_mut();
        for (index, start, metrics) in pens {
            let (dx, dy) = transform.apply(start.0, start.1);
            let pen = (center.x as f32 + dx, center.y as f32 + dy);
            // The glyph turns about its pen, so it stays within its unturned reach of it.
            let reach = libm::ceilf(glyph_reach((0.0, 0.0), &metrics)) as i32;
            let around = Point::new(libm::floorf(pen.0) as i32, libm::floorf(pen.1) as i32);
            if !target.visible(&Rectangle::with_center(around, Size::new_equal(2 * reach as u32 + 2)))
            {
                continue;
            }
            let (metrics, bitmap) =
                font.rasterize_indexed_transformed(&mut ctx.canvas, index, px, transform, pen);
            ctx.coverage.resize(metrics.width, 0);
            let color = self.text_color;
            bitmap.rows(&mut ctx.coverage, |y, x, row| {
                target.blend_row(metrics.x + x as i32, metrics.y + y as i32, row, color);
            });
        }
        Ok(())
    }

    /// Each glyph of `text` with its pen's distance from the first along the baseline, and its
    /// metrics.
    pub fn pens<'s>(&'s self, text: &'s str) -> impl Iterator<Item = (u16, f32, fontdue::Metrics)> + 's {
        let font = self.fonts[self.font_index];
        let px = self.font_size as f32;
        fontdue::PenOffsets::new(font, text, px)
            .map(move |(index, offset)| (index, offset, font.metrics_indexed(index, px)))
    }

    /// Draws `text` a quarter turn clockwise, reading down, with the first glyph's pen at `pen`.
    /// Letter tops face right, so the baseline is the column's left edge.
    pub fn draw_turned<D: CoverageTarget<Color = C>>(
        &self,
        text: &str,
        pen: (f32, f32),
        target: &mut D,
    ) -> Result<(), D::Error> {
        let font = self.fonts[self.font_index];
        let px = self.font_size as f32;
        let transform = fontdue::Transform::rotation(0.0, 1.0);
        let ctx = &mut *self.ctx.borrow_mut();
        for (index, offset, metrics) in self.pens(text) {
            if metrics.width == 0 || metrics.height == 0 {
                continue;
            }
            let pen = (pen.0, pen.1 + offset);
            // The upright bitmap box turned, a pixel wider each way for the pen's fraction.
            let turned = Rectangle::new(
                Point::new(libm::floorf(pen.0) as i32 + metrics.ymin - 1, libm::floorf(pen.1) as i32 + metrics.xmin - 1),
                Size::new(metrics.height as u32 + 2, metrics.width as u32 + 2),
            );
            if !target.visible(&turned) {
                continue;
            }
            let (metrics, bitmap) =
                font.rasterize_indexed_transformed(&mut ctx.canvas, index, px, transform, pen);
            ctx.coverage.resize(metrics.width, 0);
            let color = self.text_color;
            bitmap.rows(&mut ctx.coverage, |y, x, row| {
                target.blend_row(metrics.x + x as i32, metrics.y + y as i32, row, color);
            });
        }
        Ok(())
    }

    /// The ink box of one glyph drawn by [`draw_stretched`](Self::draw_stretched), a pixel wider
    /// each way for its edges.
    fn stretched_glyph(origin: Point, offset: f32, metrics: &fontdue::Metrics, scale: f32) -> Rectangle {
        let left = origin.x + libm::roundf(offset) as i32 + metrics.xmin - 1;
        let top = libm::floorf(origin.y as f32 - (metrics.ymin + metrics.height as i32) as f32 * scale) as i32 - 1;
        let bottom = libm::ceilf(origin.y as f32 - metrics.ymin as f32 * scale) as i32 + 1;
        Rectangle::with_corners(Point::new(left, top), Point::new(left + metrics.width as i32 + 1, bottom))
    }

    /// The ink bounds [`draw_stretched`](Self::draw_stretched) can cover, edges included.
    #[must_use]
    pub fn stretched_bounds(&self, text: &str, origin: Point, scale: f32) -> Rectangle {
        self.pens(text)
            .filter(|(_, _, metrics)| metrics.width > 0 && metrics.height > 0)
            .fold(Rectangle::zero(), |bounds, (_, offset, metrics)| {
                Self::union_rect(bounds, Self::stretched_glyph(origin, offset, &metrics, scale))
            })
    }

    /// Draws `text` with its pen starting at `origin` on the baseline, `scale` times as tall as
    /// the font draws it and no wider.
    pub fn draw_stretched<D: CoverageTarget<Color = C>>(
        &self,
        text: &str,
        origin: Point,
        scale: f32,
        target: &mut D,
    ) -> Result<(), D::Error> {
        let font = self.fonts[self.font_index];
        let px = self.font_size as f32;
        let transform = fontdue::Transform::new(1.0, 0.0, 0.0, scale);
        let ctx = &mut *self.ctx.borrow_mut();
        for (index, offset, metrics) in self.pens(text) {
            if metrics.width == 0
                || metrics.height == 0
                || !target.visible(&Self::stretched_glyph(origin, offset, &metrics, scale))
            {
                continue;
            }
            let pen = (origin.x as f32 + libm::roundf(offset), origin.y as f32);
            let (metrics, bitmap) =
                font.rasterize_indexed_transformed(&mut ctx.canvas, index, px, transform, pen);
            target.begin_glyph(Rectangle::new(
                Point::new(metrics.x, metrics.y),
                Size::new(metrics.width as u32, metrics.height as u32),
            ));
            ctx.coverage.resize(metrics.width, 0);
            let color = self.text_color;
            bitmap.rows(&mut ctx.coverage, |y, x, row| {
                target.blend_row(metrics.x + x as i32, metrics.y + y as i32, row, color);
            });
        }
        Ok(())
    }

    /// Draws `text` at twice this renderer's size with its pen at `origin` on the baseline. Each
    /// glyph is rasterized at this size into a raster of its own, freed on return, and each pixel
    /// drawn as a 2 × 2 block: at full size the largest glyph's raster would not fit the heap.
    pub fn draw_doubled_on_baseline<D: CoverageTarget<Color = C>>(
        &self,
        text: &str,
        origin: Point,
        target: &mut D,
    ) -> Result<(), D::Error> {
        let px = self.font_size as f32;
        let font = self.fonts[self.font_index];
        let color = self.text_color;
        let mut canvas = fontdue::raster::Raster::empty();
        let (mut row, mut doubled) = (alloc::vec::Vec::new(), alloc::vec::Vec::new());
        for (index, corner, metrics) in self.glyphs_on_baseline(text, Point::zero()) {
            let corner = origin + corner * 2;
            let size = Size::new(2 * metrics.width as u32, 2 * metrics.height as u32);
            if size.width == 0 || size.height == 0 || !target.visible(&Rectangle::new(corner, size)) {
                continue;
            }
            let (metrics, bitmap) = font.rasterize_indexed(&mut canvas, index, px);
            target.begin_glyph(Rectangle::new(corner, Size::new(2 * metrics.width as u32, 2 * metrics.height as u32)));
            row.resize(metrics.width, 0);
            bitmap.rows(&mut row, |y, x, span| {
                doubled.clear();
                doubled.extend(span.iter().flat_map(|&coverage| [coverage, coverage]));
                let (x, y) = (corner.x + 2 * x as i32, corner.y + 2 * y as i32);
                target.blend_row(x, y, &doubled, color);
                target.blend_row(x, y + 1, &doubled, color);
            });
        }
        Ok(())
    }

    fn lay_out(&self, ctx: &mut FontdueRendererCtx, text: &str) {
        ctx.reset_layout();
        ctx.layout.append(
            self.fonts,
            &fontdue::layout::TextStyle::new(text, self.font_size as f32, self.font_index),
        );
    }

    /// The ink bounds of `text` once aligned in `region`, as [`draw_aligned`](Self::draw_aligned)
    /// would place it.
    pub fn aligned_bounds<H, V>(&self, text: &str, region: &Rectangle, horizontal: H, vertical: V) -> Rectangle
    where
        H: HorizontalAlignment,
        V: VerticalAlignment,
    {
        let mut ctx = self.borrow_ctx();
        self.lay_out(&mut ctx, text);
        let bounds = Self::layout_bounds(&ctx, Point::zero());
        bounds.translate(Point::new(
            horizontal.align(bounds, *region),
            vertical.align(bounds, *region),
        ))
    }

    /// Draws `text` where embedded-layout's `align_to` would put it in `region`, laying it out
    /// once, and blends its edges over whatever is already drawn. Returns its ink bounds.
    pub fn draw_aligned<D, H, V>(
        &self,
        text: &str,
        region: &Rectangle,
        horizontal: H,
        vertical: V,
        target: &mut D,
    ) -> Result<Rectangle, D::Error>
    where
        D: CoverageTarget<Color = C>,
        H: HorizontalAlignment,
        V: VerticalAlignment,
    {
        let mut ctx = self.borrow_ctx();
        self.lay_out(&mut ctx, text);
        let bounds = Self::layout_bounds(&ctx, Point::zero());
        let position = Point::new(
            horizontal.align(bounds, *region),
            vertical.align(bounds, *region),
        );
        let FontdueRendererCtx {
            layout,
            canvas,
            coverage,
        } = &mut *ctx;
        for glyph in layout.glyphs().iter().filter(|g| g.char_data.rasterize()) {
            let corner = position + Point::new(glyph.x as i32, glyph.y as i32);
            let size = Size::new(glyph.width as u32, glyph.height as u32);
            if !target.visible(&Rectangle::new(corner, size)) {
                continue;
            }
            blend_glyph(
                canvas,
                coverage,
                self.fonts[glyph.font_index],
                glyph.key.glyph_index,
                glyph.key.px,
                corner,
                self.text_color,
                target,
            );
        }
        Ok(bounds.translate(position))
    }

    /// How far the pen moves across `text`.
    #[must_use]
    pub fn advance(&self, text: &str) -> f32 {
        let mut offsets =
            fontdue::PenOffsets::new(self.fonts[self.font_index], text, self.font_size as f32);
        offsets.by_ref().for_each(drop);
        offsets.advance()
    }

    /// Each glyph's top-left corner and metrics, for `text` whose pen starts at `origin` on the
    /// baseline.
    fn glyphs_on_baseline<'s>(
        &'s self,
        text: &'s str,
        origin: Point,
    ) -> impl Iterator<Item = (u16, Point, fontdue::Metrics)> + 's {
        let font = self.fonts[self.font_index];
        let px = self.font_size as f32;
        fontdue::PenOffsets::new(font, text, px).map(move |(index, offset)| {
            let metrics = font.metrics_indexed(index, px);
            let corner = Point::new(
                origin.x + libm::roundf(offset) as i32 + metrics.xmin,
                origin.y - metrics.ymin - metrics.height as i32,
            );
            (index, corner, metrics)
        })
    }

    /// Marks in `damage` the ink of each glyph that differs between `old` drawn from `old_origin`
    /// and `new` from `new_origin`, pens on the baseline. A glyph differs when its character or
    /// its pen does, so a proportional font's shifted tail counts. Metrics are worked out only
    /// for those glyphs.
    pub fn glyph_damage(
        &self,
        (old, old_origin): (&str, Point),
        (new, new_origin): (&str, Point),
        damage: &mut Dirty,
    ) {
        let font = self.fonts[self.font_index];
        let px = self.font_size as f32;
        let pens = |text, origin: Point| {
            fontdue::PenOffsets::new(font, text, px)
                .map(move |(index, offset)| (index, origin.x + libm::roundf(offset) as i32))
        };
        let mut mark = |glyph: Option<(u16, i32)>, baseline: i32| {
            if let Some((index, pen)) = glyph {
                let metrics = font.metrics_indexed(index, px);
                let corner = Point::new(pen + metrics.xmin, baseline - metrics.ymin - metrics.height as i32);
                damage.add(Rectangle::new(corner, Size::new(metrics.width as u32, metrics.height as u32)));
            }
        };
        let (mut olds, mut news) = (pens(old, old_origin), pens(new, new_origin));
        loop {
            let (was, now) = (olds.next(), news.next());
            if was.is_none() && now.is_none() {
                return;
            }
            if was != now || old_origin.y != new_origin.y {
                mark(was, old_origin.y);
                mark(now, new_origin.y);
            }
        }
    }

    /// The ink bounds [`draw_on_baseline`](Self::draw_on_baseline) would cover.
    #[must_use]
    pub fn baseline_bounds(&self, text: &str, origin: Point) -> Rectangle {
        self.glyphs_on_baseline(text, origin)
            .filter(|(_, _, metrics)| metrics.width > 0 && metrics.height > 0)
            .fold(Rectangle::zero(), |bounds, (_, corner, metrics)| {
                Self::union_rect(
                    bounds,
                    Rectangle::new(corner, Size::new(metrics.width as u32, metrics.height as u32)),
                )
            })
    }

    /// Draws `text` with its pen starting at `origin` on the baseline, and blends its edges over
    /// whatever is already drawn.
    pub fn draw_on_baseline<D: CoverageTarget<Color = C>>(
        &self,
        text: &str,
        origin: Point,
        target: &mut D,
    ) -> Result<(), D::Error> {
        let px = self.font_size as f32;
        let font = self.fonts[self.font_index];
        let mut ctx = self.ctx.borrow_mut();
        let FontdueRendererCtx {
            canvas, coverage, ..
        } = &mut *ctx;
        for (index, corner, metrics) in self.glyphs_on_baseline(text, origin) {
            let size = Size::new(metrics.width as u32, metrics.height as u32);
            if size.width == 0 || size.height == 0 || !target.visible(&Rectangle::new(corner, size)) {
                continue;
            }
            let color = self.text_color;
            blend_glyph(canvas, coverage, font, index, px, corner, color, target);
        }
        Ok(())
    }

    /// Draws the outline of `text`, its pen at `origin` on the baseline: a ring `radius` px wide
    /// just outside each glyph's edge, in the text colour, leaving the glyph's inside alone. Text
    /// drawn over it in another colour gives a halo. The ring is the glyph's coverage grown by
    /// `radius` less the coverage itself, so it keeps the glyph's antialiasing, but its width is
    /// whole pixels, and a counter or gap narrower than twice `radius` fills in. Each glyph's ring
    /// is its own, so where glyphs sit closer than `radius`, a ring crosses its neighbour.
    pub fn draw_outline_on_baseline<D: CoverageTarget<Color = C>>(
        &self,
        text: &str,
        origin: Point,
        radius: u8,
        target: &mut D,
    ) -> Result<(), D::Error> {
        self.draw_ring_on_baseline(text, origin, radius, false, target)
    }

    /// Draws `text` hollow, its pen at `origin` on the baseline: a ring `width` px wide just
    /// inside each glyph's edge, so the letters keep the ink box they have filled. The ring is
    /// the coverage less the coverage shrunk by `width`, and a stroke narrower than twice `width`
    /// stays solid.
    pub fn draw_hollow_on_baseline<D: CoverageTarget<Color = C>>(
        &self,
        text: &str,
        origin: Point,
        width: u8,
        target: &mut D,
    ) -> Result<(), D::Error> {
        self.draw_ring_on_baseline(text, origin, width, true, target)
    }

    fn draw_ring_on_baseline<D: CoverageTarget<Color = C>>(
        &self,
        text: &str,
        origin: Point,
        radius: u8,
        inside: bool,
        target: &mut D,
    ) -> Result<(), D::Error> {
        let px = self.font_size as f32;
        let font = self.fonts[self.font_index];
        // One pixel more than the ring, which the dilation leaves as it is.
        let r = usize::from(radius) + 1;
        let mut ctx = self.ctx.borrow_mut();
        let FontdueRendererCtx { canvas, coverage, .. } = &mut *ctx;
        let (mut glyph, mut grown, mut scratch) = (alloc::vec::Vec::new(), alloc::vec::Vec::new(), alloc::vec::Vec::new());
        for (index, corner, metrics) in self.glyphs_on_baseline(text, origin) {
            let corner = corner - Point::new_equal(r as i32);
            let (width, height) = (metrics.width + 2 * r, metrics.height + 2 * r);
            let size = Size::new(width as u32, height as u32);
            if metrics.width == 0 || metrics.height == 0 || !target.visible(&Rectangle::new(corner, size)) {
                continue;
            }
            let (metrics, bitmap) = font.rasterize_indexed(canvas, index, px);
            glyph.clear();
            glyph.resize(width * height, 0);
            coverage.resize(metrics.width, 0);
            bitmap.rows(coverage, |y, x, span| {
                let at = (y + r) * width + x + r;
                glyph[at..at + span.len()].copy_from_slice(span);
            });
            // Inside, the ring is what growing the space around the glyph takes from it.
            grown.clear();
            grown.extend(glyph.iter().map(|&v| if inside { u8::MAX - v } else { v }));
            for pass in 0..usize::from(radius) {
                dilate(&mut grown, width, pass % 2 == 1, &mut scratch);
            }
            for (y, (ring, glyph)) in grown.chunks_exact_mut(width).zip(glyph.chunks_exact(width)).enumerate() {
                for (ring, &glyph) in ring.iter_mut().zip(glyph) {
                    *ring = if inside { glyph.saturating_sub(u8::MAX - *ring) } else { ring.saturating_sub(glyph) };
                }
                target.blend_row(corner.x, corner.y + y as i32, ring, self.text_color);
            }
        }
        Ok(())
    }

    fn layout_bounds(ctx: &FontdueRendererCtx, position: Point) -> Rectangle {
        ctx.layout
            .glyphs()
            .iter()
            .fold(Rectangle::zero(), |bounds, glyph| {
                Self::union_rect(
                    bounds,
                    Rectangle::new(
                        position + Point::new(glyph.x as i32, glyph.y as i32),
                        Size::new(glyph.width as u32, glyph.height as u32),
                    ),
                )
            })
    }
}

