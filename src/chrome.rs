use core::cell::RefCell;

use alloc::rc::Rc;
use embedded_graphics::{
    Pixel,
    pixelcolor::{Gray8, Rgb565, Rgb888, raw::RawU16},
    prelude::{Dimensions, DrawTarget, GrayColor, PixelColor, Point, PointsIter, RawData, RgbColor, Size, Transform},
    primitives::Rectangle,
    text::renderer::TextMetrics,
};
use embedded_layout::align::{HorizontalAlignment, VerticalAlignment};
use fontdue::{FontRepr, layout::Layout};

use crate::{board, ui::dirty::DirtyAreas};

// Rgb888 is higher quality, Rgb565 cuts the size of the framebuffer by a third.
// Gray8 is 3x smaller than Rgb888... but I'm not sure we love monochrome.
pub type Color = embedded_graphics::pixelcolor::Rgb565;

pub const DISPLAY_SIZE: Size = Size::new(board::LCD_WIDTH as u32, board::LCD_HEIGHT as u32);
pub const DISPLAY_BBOX: Rectangle = Rectangle::new(Point::new_equal(0), DISPLAY_SIZE);

pub type FB = crate::drivers::framebuffer::Framebuffer<
    {
        crate::drivers::framebuffer::buffer_size::<Color>(
            board::LCD_WIDTH as usize,
            board::LCD_HEIGHT as usize,
        )
    },
    { board::LCD_WIDTH as usize },
    { board::LCD_HEIGHT as usize },
    Color,
>;
pub type Dirty =
    DirtyAreas<{ board::LCD_WIDTH as usize }, { board::LCD_HEIGHT as usize }, 2, 2, { 2 * 2 }>;

/// A draw target that takes antialiased coverage a row at a time and blends it over what it
/// already holds, so edges come out right over any background.
pub trait CoverageTarget: DrawTarget {
    /// Blends `color` into row `y` from column `x` onward, one coverage byte per pixel: 0 leaves
    /// the pixel, 255 replaces it, and anything between mixes with it. Pixels outside the target
    /// are skipped.
    fn blend_row(&mut self, x: i32, y: i32, coverage: &[u8], color: Self::Color);

    fn blend_pixel(&mut self, point: Point, coverage: u8, color: Self::Color) {
        self.blend_row(point.x, point.y, &[coverage], color);
    }
}

impl CoverageTarget for FB {
    fn blend_row(&mut self, x: i32, y: i32, coverage: &[u8], color: Color) {
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
        for (pixel, &covered) in pixels.chunks_exact_mut(2).zip(coverage) {
            match covered {
                0 => {}
                u8::MAX => pixel.copy_from_slice(&full),
                _ => {
                    let under = Rgb565::from(RawU16::new(u16::from_be_bytes([pixel[0], pixel[1]])));
                    let mixed = under.lerp(&color, covered);
                    pixel.copy_from_slice(&RawU16::from(mixed).into_inner().to_be_bytes());
                }
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

impl<T: CoverageTarget> CoverageTarget for Window<'_, T> {
    fn blend_row(&mut self, x: i32, y: i32, coverage: &[u8], color: Self::Color) {
        let (x, y) = (x + self.offset.x, y + self.offset.y);
        let top = self.clip.top_left;
        if y < top.y || y >= top.y + self.clip.size.height as i32 {
            return;
        }
        let start = x.max(top.x);
        let end = x
            .saturating_add(coverage.len() as i32)
            .min(top.x + self.clip.size.width as i32);
        if start < end {
            self.parent.blend_row(
                start,
                y,
                &coverage[(start - x) as usize..(end - x) as usize],
                color,
            );
        }
    }
}

fontdue_macros::fontdue_font_from_file!(
    MarathonShapiroFont,
    "../assets/MarathonShapiro-Wide65_subset.ttf",
    // Picked with some trial and effort to offer some of the lowest flash usage while looking
    // great. Making it lower makes rendering faster, at the cost of quality.
    scale: 2.1
);

fontdue_macros::fontdue_font_from_file!(
    FraktionMonoRegularFont,
    "../assets/PPFraktion-Free for personal use v1.1/Mono/PPFraktionMono-Regular-subset.ttf",
    scale: 2.2
);

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
pub const GRAY: Color = color_from_hex("#888e98");
pub const WHITE: Color = color_from_hex("#d2d3d6");
pub const BLACK: Color = color_from_hex("#000000");

#[inline]
pub const fn lerp_u8(a: u8, b: u8, factor: u8) -> u8 {
    // `>> 8` with a +255 bias stands in for `/ 255`: it matches the floor division or exceeds it
    // by one, and factors 0 and 255 return `a` and `b` exactly.
    ((a as u16 * (u8::MAX - factor) as u16 + b as u16 * factor as u16 + u8::MAX as u16) >> 8) as u8
}

/// Blends a row-major coverage bitmap `width` pixels wide with its top-left pixel at `origin`.
fn blend_bitmap<D: CoverageTarget>(
    target: &mut D,
    origin: Point,
    width: usize,
    coverage: &[u8],
    color: D::Color,
) {
    if width == 0 {
        return;
    }
    for (row, line) in coverage.chunks_exact(width).enumerate() {
        target.blend_row(origin.x, origin.y + row as i32, line, color);
    }
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
    /// One glyph's coverage, row by row, for the row-blending draws.
    coverage: alloc::vec::Vec<u8>,
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

    /// Background color.
    pub background_color: C,

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
        background_color: C,
        fonts: &'f [&'f dyn FontRepr],
    ) -> Self {
        Self {
            text_color,
            background_color,
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

    fn draw_background<D>(
        &self,
        width: u32,
        position: Point,
        target: &mut D,
    ) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = C>,
    {
        if width == 0 {
            return Ok(());
        }

        target.fill_solid(
            &Rectangle::new(position, Size::new(width, self.font_size)),
            self.background_color,
        )?;

        Ok(())
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

    fn render_layout<D: DrawTarget<Color = C>>(
        &self,
        ctx: &mut FontdueRendererCtx,
        position: Point,
        target: &mut D,
    ) -> Result<Rectangle, D::Error> {
        let bbox = target.bounding_box();
        let usable_width = bbox.top_left.x + bbox.size.width as i32 - position.x;
        if usable_width <= 0 {
            return Ok(Rectangle::zero());
        }
        let mut rendered = Rectangle::zero();
        ctx.layout
            .glyphs()
            .iter()
            .filter(|g| g.x < usable_width as f32 && g.char_data.rasterize())
            .try_for_each(|g| {
                let (metrics, bitmap) = self.fonts[g.font_index].rasterize_indexed(
                    &mut ctx.canvas,
                    g.key.glyph_index,
                    g.key.px,
                );
                let x_off = g.x as i32;
                let y_off = g.y as i32;
                rendered = Self::union_rect(
                    rendered,
                    Rectangle::new(
                        position + Point::new(x_off, y_off),
                        Size::new(metrics.width as u32, metrics.height as u32),
                    ),
                );

                let coverage_to_color =
                    |coverage: u8| self.background_color.lerp(&self.text_color, coverage);

                let width = metrics.width;
                let pixels =
                    bitmap
                        .into_iter()
                        .enumerate()
                        .filter(|&(_, c)| c != 0)
                        .map(|(idx, c)| {
                            let y = idx / width;
                            let x = idx % width;
                            Pixel(
                                position + Point::new(x_off + x as i32, y_off + y as i32),
                                coverage_to_color(c),
                            )
                        });
                target.draw_iter(pixels)
            })?;
        // self.draw_background(width as u32, position, target)?;
        // target.draw_iter(pixels)?;
        // self.draw_strikethrough(width as u32, position, target)?;
        // self.draw_underline(width as u32, position, target)?;
        Ok(rendered)
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
            return Ok(());
        }
        // Pen-relative and y down: back half the advance, and down to the middle of the ink.
        let start = (-offsets.advance() / 2.0, (bottom + top) as f32 / 2.0);
        let ctx = &mut *self.ctx.borrow_mut();
        for (index, offset) in fontdue::PenOffsets::new(font, text, px) {
            let (dx, dy) = transform.apply(start.0 + offset, start.1);
            let pen = (center.x as f32 + dx, center.y as f32 + dy);
            let (metrics, bitmap) =
                font.rasterize_indexed_transformed(&mut ctx.canvas, index, px, transform, pen);
            ctx.coverage.clear();
            bitmap.for_each(|covered| ctx.coverage.push(covered));
            blend_bitmap(
                target,
                Point::new(metrics.x, metrics.y),
                metrics.width,
                &ctx.coverage,
                self.text_color,
            );
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
            let (metrics, bitmap) = self.fonts[glyph.font_index].rasterize_indexed(
                canvas,
                glyph.key.glyph_index,
                glyph.key.px,
            );
            coverage.clear();
            bitmap.for_each(|covered| coverage.push(covered));
            blend_bitmap(
                target,
                position + Point::new(glyph.x as i32, glyph.y as i32),
                metrics.width,
                coverage,
                self.text_color,
            );
        }
        Ok(bounds.translate(position))
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
    #[inline]
    pub fn render<D: DrawTarget<Color = C>>(
        &self,
        layout_cb: impl FnOnce(&mut Layout<()>, &[&dyn FontRepr]),
        position: Point,
        target: &mut D,
    ) -> Result<Rectangle, D::Error> {
        let ctx = &mut *self.borrow_ctx();
        ctx.reset_layout();
        layout_cb(&mut ctx.layout, self.fonts);
        self.render_layout(ctx, position, target)
    }
}

impl<C: PixelColor + RgbColorExt> embedded_graphics::text::renderer::TextRenderer
    for FontdueRenderer<'_, C>
{
    type Color = C;

    fn draw_string<D>(
        &self,
        text: &str,
        position: Point,
        _baseline: embedded_graphics::text::Baseline,
        target: &mut D,
    ) -> Result<Point, D::Error>
    where
        D: DrawTarget<Color = Self::Color>,
    {
        self.render(
            |layout, fonts| {
                layout.append(
                    fonts,
                    &fontdue::layout::TextStyle::new(text, self.font_size as f32, self.font_index),
                );
            },
            position,
            target,
        )?;
        let ctx = self.borrow_ctx();
        let pos = position
            + ctx
                .layout
                .glyphs()
                .last()
                .map(|g| Point::new(g.x as i32 + g.width as i32, g.y as i32 + g.height as i32))
                .unwrap_or(Point::zero());

        Ok(pos)
    }

    fn measure_string(
        &self,
        text: &str,
        position: Point,
        _baseline: embedded_graphics::text::Baseline,
    ) -> embedded_graphics::text::renderer::TextMetrics {
        let mut ctx = self.borrow_ctx();
        ctx.reset_layout();
        ctx.layout.append(
            self.fonts,
            &fontdue::layout::TextStyle::new(text, self.font_size as f32, self.font_index),
        );
        let bounding_box = Self::layout_bounds(&ctx, position);
        let next_position = ctx
            .layout
            .glyphs()
            .last()
            .map(|g| position + Point::new(g.x as i32 + g.width as i32, 0))
            .unwrap_or(position);

        TextMetrics {
            bounding_box,
            next_position,
        }
    }

    fn draw_whitespace<D>(
        &self,
        width: u32,
        position: Point,
        _baseline: embedded_graphics::text::Baseline,
        target: &mut D,
    ) -> Result<Point, D::Error>
    where
        D: DrawTarget<Color = Self::Color>,
    {
        self.draw_background(width, position, target)?;
        Ok(position + Size::new(width, 0))
    }

    fn line_height(&self) -> u32 {
        self.font_size
    }
}
