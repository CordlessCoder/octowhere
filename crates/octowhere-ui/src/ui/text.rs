//! Placing text by its ink, the way the design documents give positions.

use embedded_graphics::prelude::Point;

use crate::chrome::{Color, FontdueRenderer};

#[must_use]
pub fn style(
    font: &FontdueRenderer<'static, Color>,
    color: Color,
    size: u32,
    index: usize,
) -> FontdueRenderer<'static, Color> {
    let mut style = font.clone();
    style.text_color = color;
    style.font_size = size;
    style.font_index = index;
    style
}

/// Cap height: how far above the baseline a capital's ink reaches.
#[must_use]
pub fn cap(style: &FontdueRenderer<'static, Color>) -> i32 {
    style.baseline_bounds("H", Point::zero()).size.height as i32
}

/// The pen that puts `text`'s ink left edge on column `x`.
#[must_use]
pub fn pen_x_for_ink_left(style: &FontdueRenderer<'static, Color>, text: &str, x: i32) -> i32 {
    x - style.baseline_bounds(text, Point::zero()).top_left.x
}

/// The pen that ends `text`'s ink on column `right`, inclusive.
#[must_use]
pub fn pen_x_for_ink_right(style: &FontdueRenderer<'static, Color>, text: &str, right: i32) -> i32 {
    let ink = style.baseline_bounds(text, Point::zero());
    right + 1 - (ink.top_left.x + ink.size.width as i32)
}

/// The pen that centres `text`'s ink on column `x`.
#[must_use]
pub fn pen_x_for_ink_centre(style: &FontdueRenderer<'static, Color>, text: &str, x: f32) -> i32 {
    let ink = style.baseline_bounds(text, Point::zero());
    libm::roundf(x - ink.size.width as f32 / 2.0) as i32 - ink.top_left.x
}

/// The baseline that puts `text`'s ink top on row `top`.
#[must_use]
pub fn baseline_for_ink_top(style: &FontdueRenderer<'static, Color>, text: &str, top: i32) -> i32 {
    top - style.baseline_bounds(text, Point::zero()).top_left.y
}

/// The baseline that ends `text`'s ink on row `bottom`, inclusive.
#[must_use]
pub fn baseline_for_ink_bottom(style: &FontdueRenderer<'static, Color>, text: &str, bottom: i32) -> i32 {
    let ink = style.baseline_bounds(text, Point::zero());
    bottom + 1 - (ink.top_left.y + ink.size.height as i32)
}

/// The baseline that centres `text`'s ink on row `y`.
#[must_use]
pub fn baseline_for_ink_middle(style: &FontdueRenderer<'static, Color>, text: &str, y: f32) -> i32 {
    let ink = style.baseline_bounds(text, Point::zero());
    libm::roundf(y - ink.size.height as f32 / 2.0) as i32 - ink.top_left.y
}
