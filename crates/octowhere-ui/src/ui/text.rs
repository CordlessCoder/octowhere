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
pub fn baseline_for_ink_bottom(
    style: &FontdueRenderer<'static, Color>,
    text: &str,
    bottom: i32,
) -> i32 {
    let ink = style.baseline_bounds(text, Point::zero());
    bottom + 1 - (ink.top_left.y + ink.size.height as i32)
}

/// The baseline that centres `text`'s ink on row `y`.
#[must_use]
pub fn baseline_for_ink_middle(style: &FontdueRenderer<'static, Color>, text: &str, y: f32) -> i32 {
    let ink = style.baseline_bounds(text, Point::zero());
    libm::roundf(y - ink.size.height as f32 / 2.0) as i32 - ink.top_left.y
}

/// The most lines [`wrap`] breaks a text into, and the longest text it takes whole.
pub const LINES: usize = 32;
pub const WRAPPED: usize = 256;
/// The most characters a line takes, as many as a list's text holds.
const MOST: usize = 40;

/// Breaks `text`, printable ASCII, into lines no wider than `width` in `style`, at spaces where
/// it can and inside a word too long for a line. Each line is a range of `text`, without the
/// spaces it broke at. Text past [`WRAPPED`] characters is left out.
#[must_use]
pub fn wrap(
    style: &FontdueRenderer<'static, Color>,
    text: &str,
    width: f32,
) -> heapless::Vec<core::ops::Range<usize>, LINES> {
    let text = &text[..text.len().min(WRAPPED)];
    // Where each character's pen starts, and the text's end.
    let mut pens: heapless::Vec<f32, { WRAPPED + 1 }> =
        style.pens(text).map(|(_, offset, _)| offset).collect();
    _ = pens.push(style.advance(text));
    let bytes = text.as_bytes();
    let mut lines = heapless::Vec::new();
    let mut start = 0;
    while start < bytes.len() && !lines.is_full() {
        while start < bytes.len() && bytes[start] == b' ' {
            start += 1;
        }
        if start == bytes.len() {
            break;
        }
        // The longest run from `start` that fits, at least a character, ending at a space if any
        // does.
        let mut end = start + 1;
        let mut space = None;
        for at in start + 1..=bytes.len() {
            if pens[at] - pens[start] > width || at - start > MOST {
                break;
            }
            end = at;
            if at < bytes.len() && bytes[at] == b' ' {
                space = Some(at);
            }
        }
        if end < bytes.len()
            && bytes[end] != b' '
            && let Some(space) = space
        {
            end = space;
        }
        let mut last = end;
        while last > start && bytes[last - 1] == b' ' {
            last -= 1;
        }
        _ = lines.push(start..last);
        start = end;
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chrome::{self, FontdueRendererCtx};

    fn sans() -> FontdueRenderer<'static, Color> {
        let font = FontdueRenderer::new(
            FontdueRendererCtx::new_rc(),
            20,
            chrome::WHITE,
            chrome::FONTS,
        );
        style(&font, chrome::WHITE, 18, chrome::FRAKTION_SANS_LIGHT)
    }

    fn lines(text: &str, width: f32) -> heapless::Vec<&str, LINES> {
        wrap(&sans(), text, width)
            .into_iter()
            .map(|range| &text[range])
            .collect()
    }

    #[test]
    fn a_text_breaks_at_spaces_within_the_width() {
        let text = "I am at the bridge. Please take the north path and wait at the turn.";
        let lines = lines(text, 280.0);
        assert!(lines.len() > 1);
        for line in &lines {
            assert!(sans().advance(line) <= 280.0, "{line}");
            assert!(!line.starts_with(' ') && !line.ends_with(' '));
        }
        assert_eq!(lines.join(" "), text);
    }

    #[test]
    fn a_word_too_long_for_a_line_breaks_inside() {
        let text = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        let lines = lines(text, 100.0);
        assert!(lines.len() > 1);
        assert_eq!(lines.concat(), text);
    }
}
