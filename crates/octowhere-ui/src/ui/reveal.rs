//! The cell reveal: Mono text appearing left to right, with a solid block standing in each
//! character's cell for a step before its glyph. A block stands only where a character will be,
//! so no step shows a wrong one.

use embedded_graphics::{prelude::Point, primitives::Rectangle};

use crate::chrome::{Color, CoverageTarget, FontdueRenderer};

/// How much of a revealed line shows: its first `glyphs` characters, then a block where the
/// next will be if `block`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Reveal {
    glyphs: usize,
    block: bool,
}

impl Reveal {
    /// At progress `k` of `n` characters, those below `k(n + 1) - 1` show and the next is a
    /// block.
    pub fn of(progress: u8, n: usize) -> Self {
        let shown = usize::from(progress) * (n + 1);
        let glyphs = (0..n).take_while(|&i| (i + 1) * 255 < shown).count();
        Self {
            glyphs,
            block: glyphs < n && glyphs * 255 < shown,
        }
    }

    /// How many characters show as glyphs.
    pub fn glyphs(self) -> usize {
        self.glyphs
    }

    /// How many cells show anything, glyph or block.
    pub fn cells(self) -> usize {
        self.glyphs + usize::from(self.block)
    }

    /// The share of the `len` characters from `from` on, for one reveal run across several
    /// strings in turn.
    #[must_use]
    pub fn part(self, from: usize, len: usize) -> Self {
        Self {
            glyphs: self.glyphs.saturating_sub(from).min(len),
            block: self.block && (from..from + len).contains(&self.glyphs),
        }
    }
}

/// The block that stands in character `index`'s cell, from the baseline up to cap height and
/// that character's advance wide less a pixel each side.
fn block(style: &FontdueRenderer<'static, Color>, text: &str, pen: Point, index: usize) -> Rectangle {
    let cap = style.baseline_bounds("H", Point::zero()).size.height as i32;
    let left = pen.x + libm::roundf(style.advance(&text[..index]) + 1.0) as i32;
    let right = pen.x + libm::roundf(style.advance(&text[..=index]) - 1.0) as i32;
    Rectangle::with_corners(Point::new(left, pen.y - cap), Point::new(right - 1, pen.y - 1))
}

pub fn draw_revealed<D: CoverageTarget<Color = Color>>(
    style: &FontdueRenderer<'static, Color>,
    text: &str,
    pen: Point,
    reveal: Reveal,
    target: &mut D,
) -> Result<(), D::Error> {
    style.draw_on_baseline(&text[..reveal.glyphs], pen, target)?;
    if reveal.block && text.as_bytes()[reveal.glyphs] != b' ' {
        target.fill_solid(&block(style, text, pen, reveal.glyphs), style.text_color)?;
    }
    Ok(())
}

/// Everything a revealed line can cover, at any stage of its reveal.
pub fn revealed_bounds(style: &FontdueRenderer<'static, Color>, text: &str, pen: Point) -> Rectangle {
    let ink = style.baseline_bounds(text, pen);
    if text.is_empty() {
        return ink;
    }
    let blocks = block(style, text, pen, 0);
    let last = block(style, text, pen, text.len() - 1);
    let top_left = ink.top_left.component_min(blocks.top_left);
    let bottom_right = ink
        .bottom_right()
        .unwrap_or(top_left)
        .component_max(last.bottom_right().unwrap_or(top_left));
    Rectangle::with_corners(top_left, bottom_right)
}
