//! What the 2026-10-04 screens share, from the hand-off's consistency rules: the title and its
//! caption, the controls at the top, the footer's buttons, prose, the rows of figures, the
//! status squares and the right rim's scroll arc. Each adds its items to a [`List`].

use embedded_graphics::{prelude::Point, primitives::Rectangle};

use super::super::{
    group::layout::{Align, Arc, Face, Line, List, Text, Vertical, format, rect},
    text::style,
};
use crate::chrome::{self, Color, FontdueRenderer};

pub const CENTRE: i32 = 233;
/// The title's ink top, its nominal size, the smallest it shrinks to, and the widest its ink
/// may be.
const TITLE_TOP: i32 = 48;
const TITLE_SIZE: u8 = 32;
const TITLE_SMALLEST: u8 = 26;
const TITLE_WIDTH: u32 = 274;
const META_TOP: i32 = 94;
/// A root's down chevron, which closes the drawer, and a child's back arrow, in quarter pixels.
const CHEVRON: [(i16, i16); 3] = [(898, 98), (934, 126), (970, 98)];
const BACK: [(i16, i16); 3] = [(950, 82), (922, 110), (950, 138)];
/// The region a tap on either takes, at least 40 px high.
pub const TOP_HIT: Rectangle = rect(193, 4, 273, 46);
/// The two roots' page marker under the title.
const MARKERS: [Rectangle; 2] = [rect(224, 82, 230, 85), rect(236, 82, 242, 85)];
/// The footer: one button, one higher with a line under it, or two side by side.
pub const FOOTER: Rectangle = rect(145, 410, 321, 442);
pub const FOOTER_HIGH: Rectangle = rect(145, 375, 321, 407);
pub const PAIR: [Rectangle; 2] = [rect(93, 375, 227, 407), rect(239, 375, 373, 407)];
/// The least height a tap presses a control over.
const TOUCH_HEIGHT: u32 = 40;
/// Where a button's reason for being unavailable sits, under [`FOOTER_HIGH`] or [`PAIR`].
const REASON_TOP: i32 = 419;
/// Prose's reading column, size and line pitch.
pub const PROSE_X: i32 = 88;
pub const PROSE_SIZE: u8 = 18;
pub const PROSE_PITCH: i32 = 24;
/// Rows of figures: their label and value columns, first row and pitch.
const FIGURE_LABEL: i32 = 88;
const FIGURE_VALUE: i32 = 291;
const FIGURE_TOP: i32 = 285;
const FIGURE_PITCH: i32 = 27;
/// The scroll arc on the right rim, its radius and its span in tenths of a degree.
const SCROLL_RADIUS: u16 = 225 * 4;
const SCROLL_SPAN: i16 = 410;

/// Whether a tap at `point` presses the control drawn at `area`, which takes taps over at
/// least [`TOUCH_HEIGHT`] about its middle.
#[must_use]
pub fn pressed(area: Rectangle, point: Point) -> bool {
    let grow = TOUCH_HEIGHT.saturating_sub(area.size.height) as i32;
    let top = area.top_left.y - grow / 2;
    let bottom = area.top_left.y + area.size.height as i32 + grow - grow / 2;
    (area.top_left.x..area.top_left.x + area.size.width as i32).contains(&point.x)
        && (top..bottom).contains(&point.y)
}

/// Text whose ink starts at column `x`, sitting on the baseline a capital's top at `top` gives.
#[must_use]
pub fn text(content: &str, x: i32, top: i32, face: Face, size: u8, color: Color) -> Text {
    Text::new(content, face, size, color)
        .at(x, top)
        .align(Align::Left)
        .vertical(Vertical::Cap)
}

/// As [`text`], centred on column `x`.
#[must_use]
pub fn centred(content: &str, x: i32, top: i32, face: Face, size: u8, color: Color) -> Text {
    text(content, x, top, face, size, color).align(Align::Centre)
}

/// `content` in `style`, cut short with an ellipsis where its ink runs past `width`.
#[must_use]
pub fn fitted(style: &FontdueRenderer<'static, Color>, content: &str, width: f32) -> Line {
    let mut line = format(format_args!("{content}"));
    if line.len() == content.len() && style.advance(&line) <= width {
        return line;
    }
    while !line.is_empty() {
        line.pop();
        let trimmed = line.trim_end();
        if trimmed.len() + 3 <= line.capacity()
            && style.advance(trimmed) + style.advance("...") <= width
        {
            let mut cut = format(format_args!("{trimmed}"));
            _ = cut.push_str("...");
            return cut;
        }
    }
    line
}

/// The screen's title: Shapiro at 32 px, smaller as far as 26 px where its ink is too wide.
pub fn title(list: &mut List, content: &str, font: &FontdueRenderer<'static, Color>) {
    let mut size = TITLE_SIZE;
    while size > TITLE_SMALLEST {
        let ink = style(font, chrome::WHITE, u32::from(size), Face::Title.index())
            .baseline_bounds(content, Point::zero());
        if ink.size.width <= TITLE_WIDTH {
            break;
        }
        size -= 1;
    }
    list.text(centred(
        content,
        CENTRE,
        TITLE_TOP,
        Face::Title,
        size,
        chrome::WHITE,
    ));
}

/// The caption under the title.
pub fn meta(list: &mut List, content: &str) {
    list.text(centred(
        content,
        CENTRE,
        META_TOP,
        Face::Mono,
        12,
        chrome::GRAY,
    ));
}

/// A root's down chevron, and its page marker with page `active` lit.
pub fn root_top(list: &mut List, active: usize) {
    list.path(&CHEVRON, 4, chrome::GRAY);
    for (page, marker) in MARKERS.into_iter().enumerate() {
        list.fill(
            marker,
            if page == active {
                chrome::WHITE
            } else {
                chrome::TRACK
            },
        );
    }
}

/// A child's back arrow.
pub fn back(list: &mut List) {
    list.path(&BACK, 4, chrome::GRAY);
}

/// A button: a grey outline with its label in white, or both dimmed while it cannot be used.
pub fn button(list: &mut List, area: Rectangle, label: &str, enabled: bool) {
    let (outline, ink) = if enabled {
        (chrome::GRAY, chrome::WHITE)
    } else {
        (chrome::DISABLED, chrome::DISABLED)
    };
    list.outline(area, outline);
    let middle = area.center();
    list.text(
        Text::new(label, Face::Mono, 12, ink)
            .at(middle.x, middle.y)
            .vertical(Vertical::Middle),
    );
}

/// A button that writes or sends, outlined and labelled in `LIME`, or dimmed while it cannot be
/// used.
pub fn action(list: &mut List, area: Rectangle, label: &str, enabled: bool) {
    coloured(
        list,
        area,
        label,
        if enabled {
            chrome::LIME
        } else {
            chrome::DISABLED
        },
    );
}

/// A button that changes the group, outlined and labelled in `ORANGE`.
pub fn consequential(list: &mut List, area: Rectangle, label: &str) {
    coloured(list, area, label, chrome::ORANGE);
}

fn coloured(list: &mut List, area: Rectangle, label: &str, color: Color) {
    list.outline(area, color);
    let middle = area.center();
    list.text(
        Text::new(label, Face::Mono, 12, color)
            .at(middle.x, middle.y)
            .vertical(Vertical::Middle),
    );
}

/// Why the control above cannot be used yet.
pub fn reason(list: &mut List, content: &str) {
    list.text(centred(
        content,
        CENTRE,
        REASON_TOP,
        Face::Sans,
        15,
        chrome::GRAY,
    ));
}

/// Lines of prose down the reading column from `top`.
pub fn prose(list: &mut List, lines: &[&str], top: i32) {
    for (i, line) in lines.iter().enumerate() {
        list.text(text(
            line,
            PROSE_X,
            top + i as i32 * PROSE_PITCH,
            Face::Sans,
            PROSE_SIZE,
            chrome::WHITE,
        ));
    }
}

/// Rows of figures: a grey label and its value.
pub fn figures(list: &mut List, rows: &[(&str, &str)]) {
    for (i, (label, value)) in rows.iter().enumerate() {
        let top = FIGURE_TOP + i as i32 * FIGURE_PITCH;
        list.text(text(label, FIGURE_LABEL, top, Face::Mono, 12, chrome::GRAY));
        list.text(text(
            value,
            FIGURE_VALUE,
            top,
            Face::Mono,
            12,
            chrome::WHITE,
        ));
    }
}

/// The scroll arc: its track, and a thumb as long as the share of `content` the `viewport`
/// shows, where `offset` puts it. Nothing while everything fits.
pub fn scroll_arc(list: &mut List, content: i32, viewport: i32, offset: i32) {
    if content <= viewport {
        return;
    }
    let arc = |span, width, color| Arc {
        center: Point::new(CENTRE, CENTRE),
        radius: SCROLL_RADIUS,
        width,
        span,
        color,
    };
    list.arc(arc((-SCROLL_SPAN, SCROLL_SPAN), 4, chrome::TRACK));
    let whole = 2 * i32::from(SCROLL_SPAN);
    let thumb = (whole * viewport / content).max(80);
    let along = (offset.clamp(0, content - viewport) as f32 / (content - viewport) as f32
        * (whole - thumb) as f32) as i32;
    let start = -i32::from(SCROLL_SPAN) + along;
    list.arc(arc(
        (start as i16, (start + thumb) as i16),
        12,
        chrome::WHITE,
    ));
}

/// The symbol inside a status square.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Symbol {
    /// An exclamation: a fault.
    Alert,
    /// Two arrows, one each way: a refresh or a recovery.
    Exchange,
    /// A tick: settled well.
    Done,
    /// An envelope: a message.
    Envelope,
    /// A bar: a member removed.
    Remove,
}

/// A symbol in a 24 px square: filled boxes and paths, in quarter pixels from its corner, and
/// the paths' width.
pub struct Parts {
    pub boxes: &'static [[i16; 4]],
    pub paths: &'static [&'static [(i16, i16)]],
    pub width: u8,
}

impl Symbol {
    #[must_use]
    pub fn parts(self) -> Parts {
        match self {
            Symbol::Alert => Parts {
                boxes: &[[44, 24, 52, 64], [44, 72, 52, 80]],
                paths: &[],
                width: 4,
            },
            Symbol::Exchange => Parts {
                boxes: &[],
                paths: &[
                    &[(26, 38), (70, 38)],
                    &[(60, 28), (70, 38), (60, 48)],
                    &[(70, 58), (26, 58)],
                    &[(36, 48), (26, 58), (36, 68)],
                ],
                width: 4,
            },
            Symbol::Done => Parts {
                boxes: &[],
                paths: &[&[(26, 50), (40, 64), (70, 32)]],
                width: 8,
            },
            Symbol::Remove => Parts {
                boxes: &[[21, 45, 75, 53]],
                paths: &[],
                width: 4,
            },
            Symbol::Envelope => Parts {
                boxes: &[],
                paths: &[
                    &[(22, 30), (74, 30), (74, 66), (22, 66), (22, 30)],
                    &[(22, 30), (48, 50), (74, 30)],
                ],
                width: 4,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_control_takes_taps_over_forty_pixels() {
        // FOOTER is drawn from 410 to 442.
        assert!(pressed(FOOTER, Point::new(233, 406)));
        assert!(pressed(FOOTER, Point::new(233, 445)));
        assert!(!pressed(FOOTER, Point::new(233, 405)));
        assert!(!pressed(FOOTER, Point::new(233, 446)));
        assert!(!pressed(FOOTER, Point::new(144, 420)));
        // A control taller than that takes taps where it is drawn.
        let tall = rect(82, 135, 384, 229);
        assert!(!pressed(tall, Point::new(233, 134)));
        assert!(pressed(tall, Point::new(233, 228)));
    }
}
