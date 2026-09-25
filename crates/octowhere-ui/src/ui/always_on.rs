//! The always-on face: the clock's layout drawn hollow, lit low while the screen rests.
//! Section 3 of the round 3 spec is its design.

use core::fmt::Write as _;

use embedded_graphics::prelude::Point;
use heapless::String;

use super::{
    clock::ClockView,
    clock_screen::{self, Mode, MONTHS, WEEKDAYS},
    screens, smooth,
};
use crate::chrome::{self, Color, CoverageTarget, FontdueRenderer, FRAKTION};

const CENTER: Point = Point::new(233, 233);
const DIGITS_PX: u32 = 136;
/// Each digit's pen, so the regular weight sits where the clock face's bold digits do.
const DIGIT_PENS: [i32; 2] = [62, 144];
const HOURS_BASELINE: i32 = 184;
const MINUTES_BASELINE: i32 = 305;
const RULES: [i32; 2] = [198, 317];
const RULE_RADIUS: f32 = 232.0;
const DATE_INK_TOP: i32 = 334;
const SMALL_PX: u32 = 16;
/// The state word's ink: its left column and its bottom row.
const STATE_INK_LEFT: i32 = 262;
const STATE_INK_BOTTOM: i32 = 305;

/// What the face shows. It redraws only when this changes, which is at most once a minute.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct View {
    state: State,
    /// The hours and minutes, dashes while withheld.
    time: [String<2>; 2],
    date: Option<String<10>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum State {
    Local,
    Stopped,
    NoZone,
    NoData,
}

impl View {
    #[must_use]
    pub fn of(view: &ClockView) -> Self {
        let local = view.local();
        let state = match (Mode::of(&view.clock(), &view.zone()), local) {
            (Mode::NoData, _) => State::NoData,
            (Mode::Stopped, _) => State::Stopped,
            (Mode::Local { .. }, Some(_)) => State::Local,
            _ => State::NoZone,
        };
        let dashes = || String::try_from("--").unwrap();
        let (time, date) = match local.filter(|_| state == State::Local) {
            Some(local) => {
                let two = |value: u8| {
                    let mut text = String::new();
                    _ = write!(text, "{value:02}");
                    text
                };
                let t = &local.time;
                let mut date = String::new();
                _ = write!(
                    date,
                    "{} {:02} {}",
                    WEEKDAYS[usize::from(t.weekday())],
                    t.day,
                    MONTHS[usize::from(t.month - 1)]
                );
                ([two(t.hour), two(t.minute)], Some(date))
            }
            None => ([dashes(), dashes()], None),
        };
        Self { state, time, date }
    }
}

/// Draws the face over the whole panel.
pub fn draw<D: CoverageTarget<Color = Color>>(
    view: &View,
    font: &FontdueRenderer<'static, Color>,
    target: &mut D,
) -> Result<(), D::Error> {
    screens::clear(target)?;
    let rule = if view.state == State::NoData { chrome::RED } else { chrome::WHITE };
    for y in RULES {
        smooth::disc_rows(target, y..y + 1, CENTER, RULE_RADIUS, rule)?;
    }
    if view.state == State::NoData {
        let mut style = clock_screen::no_data_style(font);
        style.text_color = chrome::RED;
        return style.draw_on_baseline("NO DATA", clock_screen::no_data_origin(font), target);
    }
    let digits = clock_screen::style(font, chrome::WHITE, DIGITS_PX, FRAKTION);
    for (text, baseline) in view.time.iter().zip([HOURS_BASELINE, MINUTES_BASELINE]) {
        for (i, pen) in DIGIT_PENS.into_iter().enumerate() {
            digits.draw_on_baseline(&text[i..=i], Point::new(pen, baseline), target)?;
        }
    }
    if let Some(date) = &view.date {
        let style = clock_screen::style(font, chrome::GRAY, SMALL_PX, FRAKTION);
        let ink = style.baseline_bounds(date, Point::zero());
        let left = CENTER.x - ink.size.width as i32 / 2 - ink.top_left.x;
        style.draw_on_baseline(date, Point::new(left, DATE_INK_TOP - ink.top_left.y), target)?;
    }
    let word = match view.state {
        State::Stopped => Some(("STOPPED", chrome::ORANGE)),
        State::NoZone => Some(("NO ZONE", chrome::GRAY)),
        _ => None,
    };
    if let Some((word, color)) = word {
        let style = clock_screen::style(font, color, SMALL_PX, FRAKTION);
        let ink = style.baseline_bounds(word, Point::zero());
        let bottom = ink.top_left.y + ink.size.height as i32 - 1;
        style.draw_on_baseline(word, Point::new(STATE_INK_LEFT - ink.top_left.x, STATE_INK_BOTTOM - bottom), target)?;
    }
    Ok(())
}
