use core::cell::RefCell;

use alloc::rc::Rc;
use embedded_graphics::{
    Pixel,
    pixelcolor::{Gray8, Rgb565, Rgb888},
    prelude::{DrawTarget, GrayColor, PixelColor, Point, RgbColor, Size},
    primitives::Rectangle,
    text::renderer::TextMetrics,
};
use fontdue::{FontRepr, layout::Layout};

use crate::board;

// Rgb888 is higher quality, Rgb565 cuts the size of the framebuffer by a third.
// Gray8 is 3x smaller than Rgb888... but I'm not sure we love monochrome.
pub type Color = embedded_graphics::pixelcolor::Rgb565;

pub const DISPLAY_SIZE: Size = Size::new(board::LCD_WIDTH as u32, board::LCD_HEIGHT as u32);
pub const DISPLAY_BBOX: Rectangle = Rectangle::new(Point::new_equal(0), DISPLAY_SIZE);

pub const UPSCALE: usize = 1;
pub const HEADING_FONT_FAST: u8g2_fonts::FontRenderer = match UPSCALE {
    1 => u8g2_fonts::FontRenderer::new::<MarathonShapiro65_32>(),
    2 => u8g2_fonts::FontRenderer::new::<u8g2_fonts::fonts::u8g2_font_spleen8x16_mr>(),
    3 => u8g2_fonts::FontRenderer::new::<u8g2_fonts::fonts::u8g2_font_spleen6x12_mr>(),
    4 => u8g2_fonts::FontRenderer::new::<u8g2_fonts::fonts::u8g2_font_spleen5x8_mr>(),
    _ => todo!(),
};
pub const MEDIUM_FONT_FAST: u8g2_fonts::FontRenderer = match UPSCALE {
    1 => u8g2_fonts::FontRenderer::new::<FraktionMono24>(),
    2 => u8g2_fonts::FontRenderer::new::<u8g2_fonts::fonts::u8g2_font_spleen8x16_mr>(),
    3 => u8g2_fonts::FontRenderer::new::<u8g2_fonts::fonts::u8g2_font_spleen6x12_mr>(),
    4 => u8g2_fonts::FontRenderer::new::<u8g2_fonts::fonts::u8g2_font_spleen5x8_mr>(),
    _ => todo!(),
};

pub type FB = crate::drivers::framebuffer::Framebuffer<
    UPSCALE,
    {
        crate::drivers::framebuffer::buffer_size::<Color>(
            board::LCD_WIDTH as usize / UPSCALE,
            board::LCD_HEIGHT as usize / UPSCALE,
        )
    },
    { board::LCD_WIDTH as usize / UPSCALE },
    { board::LCD_HEIGHT as usize / UPSCALE },
    Color,
>;

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

pub const LIME: Color = color_from_hex("#c0fe04");
pub const RED: Color = color_from_hex("#f24723");
pub const PURPLE: Color = color_from_hex("#5500e4");
pub const ORANGE_RED: Color = color_from_hex("#f15227");
pub const GRAY: Color = color_from_hex("#999999");
pub const WHITE: Color = color_from_hex("#ffffff");
pub const BLACK: Color = color_from_hex("#000000");

pub const ACCENT: Color = LIME;
// color-background: var(--color-black);
// color-foreground: var(--color-white);

#[inline]
pub const fn lerp_u8(a: u8, b: u8, factor: u8) -> u8 {
    // PERF: The division can be approximated with a right-shift
    // ((a as u16 * (u8::MAX - factor) as u16 + b as u16 * factor as u16) / u8::MAX as u16) as u8
    ((a as u16 * (u8::MAX - factor) as u16 + b as u16 * factor as u16 + u8::MAX as u16) >> 8) as u8
}

pub struct MarathonShapiro65_32;
impl u8g2_fonts::Font for MarathonShapiro65_32 {
    const DATA: &'static [u8] = include_bytes!("../assets/marathon_shapiro_32.u8g2");
}
pub struct MarathonShapiro65_20;
impl u8g2_fonts::Font for MarathonShapiro65_20 {
    const DATA: &'static [u8] = include_bytes!("../assets/marathon_shapiro_20.u8g2");
}

pub struct FraktionMono28;
impl u8g2_fonts::Font for FraktionMono28 {
    const DATA: &'static [u8] = include_bytes!(
        "../assets/PPFraktion-Free for personal use v1.1/Mono/ppfraktionmono_regular_28.u8g2"
    );
}
pub struct FraktionMono24;
impl u8g2_fonts::Font for FraktionMono24 {
    const DATA: &'static [u8] = include_bytes!(
        "../assets/PPFraktion-Free for personal use v1.1/Mono/ppfraktionmono_regular_24.u8g2"
    );
}
pub struct FraktionMono20;
impl u8g2_fonts::Font for FraktionMono20 {
    const DATA: &'static [u8] = include_bytes!(
        "../assets/PPFraktion-Free for personal use v1.1/Mono/ppfraktionmono_regular_20.u8g2"
    );
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
    canvas: fontdue::raster::Raster,
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
    fn render_layout<D: DrawTarget<Color = C>>(
        &self,
        ctx: &mut FontdueRendererCtx,
        position: Point,
        target: &mut D,
    ) -> Result<(), D::Error> {
        let bbox = target.bounding_box();
        let usable_width = bbox.size.width.saturating_sub_signed(position.x);
        if usable_width == 0 {
            return Ok(());
        }
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

                let coverage_to_color =
                    |coverage: u8| self.background_color.lerp(&self.text_color, coverage);

                let width = metrics.width;
                let pixels = bitmap.enumerate().filter(|&(_, c)| c != 0).map(|(idx, c)| {
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
        Ok(())
    }
    #[inline]
    pub fn render<D: DrawTarget<Color = C>>(
        &self,
        layout_cb: impl FnOnce(&mut Layout<()>, &[&dyn FontRepr]),
        position: Point,
        target: &mut D,
    ) -> Result<(), D::Error> {
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
                    &fontdue::layout::TextStyle::new(text, self.font_size as f32, 0),
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
            &fontdue::layout::TextStyle::new(text, self.font_size as f32, 0),
        );
        let size = ctx
            .layout
            .glyphs()
            .last()
            .map(|g| Size::new(g.x as u32 + g.width as u32, g.y as u32 + g.height as u32))
            .unwrap_or(Size::zero());

        TextMetrics {
            bounding_box: Rectangle::new(position, size),
            next_position: position + size,
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
