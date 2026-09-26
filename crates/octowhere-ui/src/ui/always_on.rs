//! The always-on face, lit low while the screen rests: the time in regular-weight digits, a
//! 24-hour rail, dim blue blocks between the hours and the minutes, and the battery in every
//! state. H2b of `context/design/`; section 3 of the round 3 spec has its behaviour.

use core::fmt::Write as _;

use embedded_graphics::{
    prelude::{Point, Size},
    primitives::Rectangle,
};
use heapless::String;

use super::{
    clock::ClockView,
    clock_screen::{self, Mode, MONTHS, WEEKDAYS},
    screens::{self, Battery},
};
use crate::chrome::{self, Color, CoverageTarget, FontdueRenderer, FRAKTION, FRAKTION_BOLD, SHAPIRO};

const CENTER: Point = Point::new(233, 233);
const DIGITS_PX: u32 = 136;
/// Each digit's pen, so the regular weight sits where the clock face's bold digits do.
const DIGIT_PENS: [i32; 2] = [62, 144];
const HOURS_BASELINE: i32 = 184;
const MINUTES_BASELINE: i32 = 305;
const LINE_INK_TOP: i32 = 334;
const SMALL_PX: u32 = 16;
/// The battery: its line's ink right edge and top, and ten cells under it, a tenth of full each.
const BATTERY_RIGHT: i32 = 366;
const BATTERY_TOP: i32 = 94;
const CELLS_LEFT: i32 = 276;
const CELL_PITCH: i32 = 10;
const CELL: Size = Size::new(7, 4);
const CELLS_TOP: i32 = 112;
const LOW: u8 = 15;
/// The rail: a cell an hour, and a taller one on the hour the face knows.
const RAIL_LEFT: i32 = 64;
const RAIL_PITCH: i32 = 14;
const RAIL_CELL: Rectangle = Rectangle::new(Point::new(0, 408), Size::new(5, 2));
const RAIL_MARKER: Rectangle = Rectangle::new(Point::new(0, 404), Size::new(5, 7));
/// NO DATA's dashes, its word centred on this row, and its caption's ink top.
const NO_DATA_DASHES: [Rectangle; 2] =
    [Rectangle::new(Point::new(86, 204), Size::new(29, 4)), Rectangle::new(Point::new(350, 204), Size::new(29, 4))];
const NO_DATA_MIDDLE: f32 = 256.0;
const NO_DATA_CAPTION_TOP: i32 = 301;
/// The blocks between the hours and the minutes while the time is local: left, top, width,
/// height and shade. Every other one steps sideways with the minute, and every other one has
/// its top right corner notched.
const BRIDGES: [(i32, i32, u32, u32, usize); 7] = [
    (38, 194, 37, 13, 0),
    (87, 202, 18, 7, 1),
    (222, 192, 46, 13, 2),
    (277, 201, 24, 8, 1),
    (370, 195, 50, 12, 0),
    (36, 380, 32, 18, 1),
    (379, 373, 41, 21, 0),
];
/// The blocks while the time is unknown or UTC: left, top and width, each 6 px tall with a
/// scanline through it.
const BARS: [(i32, i32, u32); 5] = [(42, 196, 35), (227, 194, 43), (373, 198, 40), (36, 382, 26), (383, 380, 28)];
const TICKS: [i32; 8] = [30, 37, 44, 51, 374, 382, 390, 398];
const TICKS_TOP: i32 = 214;

// The dim colours, as their tokens' levels of 255.
const BRIDGE_LEVELS: [u8; 3] = [28, 39, 49];
const SCAN_LEVELS: [u8; 3] = [60, 68, 76];
const BAR_LEVEL: u8 = 31;
const BAR_SCAN_LEVEL: u8 = 57;
const TICK_LEVEL: u8 = 73;
const CELL_LEVEL: u8 = 64;
const RAIL_LEVEL: u8 = 51;
const LOCAL_MARKER_LEVEL: u8 = 79;
const UTC_MARKER_LEVEL: u8 = 92;

/// What the face shows. It redraws only when this changes, which is at most once a minute.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct View {
    state: State,
    /// The hours and minutes, dashes while unknown.
    time: [String<2>; 2],
    /// The hour and minute shown, while there is one.
    clock: Option<(u8, u8)>,
    date: Option<String<10>>,
    battery: Option<Battery>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum State {
    Local,
    Stopped,
    NoZone,
    NoData,
}

impl View {
    /// The face for `view`, showing `battery` as last taken.
    #[must_use]
    pub fn of(view: &ClockView, battery: Option<Battery>) -> Self {
        let local = view.local();
        let state = match (Mode::of(&view.clock(), &view.zone()), local) {
            (Mode::NoData, _) => State::NoData,
            (Mode::Stopped, _) => State::Stopped,
            (Mode::Local { .. }, Some(_)) => State::Local,
            _ => State::NoZone,
        };
        let (clock, date) = match state {
            State::Local => local.map_or((None, None), |local| {
                let t = &local.time;
                let mut date = String::new();
                _ = write!(date, "{} {:02} {}", WEEKDAYS[usize::from(t.weekday())], t.day, MONTHS[usize::from(t.month - 1)]);
                (Some((t.hour, t.minute)), Some(date))
            }),
            // Without a zone the face shows the time it can vouch for, in UTC.
            State::NoZone => (view.clock().utc_time().map(|utc| (utc.hour, utc.minute)), None),
            State::Stopped | State::NoData => (None, None),
        };
        let two = |value: u8| {
            let mut text = String::new();
            _ = write!(text, "{value:02}");
            text
        };
        let time = clock.map_or_else(
            || [String::try_from("--").unwrap(), String::try_from("--").unwrap()],
            |(hour, minute)| [two(hour), two(minute)],
        );
        Self { state, time, clock, date, battery }
    }

    /// Whether the face's time stands still, so only the stage's own clock marks its minutes.
    #[must_use]
    pub fn is_still(&self) -> bool {
        self.clock.is_none()
    }
}

fn shaded(color: Color, level: u8) -> Color {
    chrome::shade(color, level)
}

fn at(area: Rectangle, x: i32) -> Rectangle {
    Rectangle::new(area.top_left + Point::new(x, 0), area.size)
}

/// Draws `text` with its ink centred on the panel's middle column, its top on `top`.
fn centred<D: CoverageTarget<Color = Color>>(
    style: &FontdueRenderer<'static, Color>,
    text: &str,
    top: i32,
    target: &mut D,
) -> Result<(), D::Error> {
    let ink = style.baseline_bounds(text, Point::zero());
    let left = CENTER.x - ink.size.width as i32 / 2 - ink.top_left.x;
    style.draw_on_baseline(text, Point::new(left, top - ink.top_left.y), target)
}

/// Draws the face over the whole panel.
pub fn draw<D: CoverageTarget<Color = Color>>(
    view: &View,
    font: &FontdueRenderer<'static, Color>,
    target: &mut D,
) -> Result<(), D::Error> {
    screens::clear(target)?;
    draw_battery(view.battery, font, target)?;
    if view.state == State::NoData {
        for dash in NO_DATA_DASHES {
            target.fill_solid(&dash, chrome::RED)?;
        }
        let word = clock_screen::style(font, chrome::RED, 28, SHAPIRO);
        let ink = word.baseline_bounds("NO DATA", Point::zero());
        let top = libm::roundf(NO_DATA_MIDDLE - ink.size.height as f32 / 2.0) as i32;
        centred(&word, "NO DATA", top, target)?;
        return centred(&clock_screen::style(font, chrome::GRAY, 13, FRAKTION), "CLOCK", NO_DATA_CAPTION_TOP, target);
    }

    match (view.state, view.clock) {
        (State::Local, Some((_, minute))) => draw_bridges(minute, target)?,
        _ => {
            for (x, y, width) in BARS {
                target.fill_solid(&Rectangle::new(Point::new(x, y), Size::new(width, 6)), shaded(chrome::BLUE, BAR_LEVEL))?;
                target.fill_solid(&Rectangle::new(Point::new(x, y + 2), Size::new(width, 1)), shaded(chrome::BLUE, BAR_SCAN_LEVEL))?;
            }
        }
    }
    for h in 0..24 {
        target.fill_solid(&at(RAIL_CELL, RAIL_LEFT + RAIL_PITCH * h), shaded(chrome::BLUE, RAIL_LEVEL))?;
    }
    // STOPPED knows no hour, and NO ZONE's is UTC's, so its marker is subdued.
    let marker = match (view.state, view.clock) {
        (State::Local, Some((hour, _))) => Some((hour, shaded(chrome::LIME, LOCAL_MARKER_LEVEL))),
        (State::NoZone, Some((hour, _))) => Some((hour, shaded(chrome::BLUE, UTC_MARKER_LEVEL))),
        _ => None,
    };
    if let Some((hour, color)) = marker {
        target.fill_solid(&at(RAIL_MARKER, RAIL_LEFT + RAIL_PITCH * i32::from(hour)), color)?;
    }

    let digits = clock_screen::style(font, chrome::WHITE, DIGITS_PX, FRAKTION);
    for (text, baseline) in view.time.iter().zip([HOURS_BASELINE, MINUTES_BASELINE]) {
        for (i, pen) in DIGIT_PENS.into_iter().enumerate() {
            digits.draw_on_baseline(&text[i..=i], Point::new(pen, baseline), target)?;
        }
    }
    let (line, color) = match view.state {
        State::Local => (view.date.as_deref().unwrap_or(""), chrome::GRAY),
        State::NoZone => ("UTC  /  NO ZONE", chrome::GRAY),
        State::Stopped | State::NoData => ("STOPPED", chrome::ORANGE),
    };
    centred(&clock_screen::style(font, color, SMALL_PX, FRAKTION), line, LINE_INK_TOP, target)
}

fn draw_bridges<D: CoverageTarget<Color = Color>>(minute: u8, target: &mut D) -> Result<(), D::Error> {
    let phase = i32::from(minute % 3);
    for (i, (x, y, width, height, shade)) in BRIDGES.into_iter().enumerate() {
        let x = if i % 2 == 0 { x } else { x + ((i32::from(minute) + i as i32) % 3 - 1) * 4 };
        let block = Rectangle::new(Point::new(x, y), Size::new(width, height));
        target.fill_solid(&block, shaded(chrome::BLUE, BRIDGE_LEVELS[shade]))?;
        for line in (y + phase..y + height as i32).step_by(3) {
            target.fill_solid(&Rectangle::new(Point::new(x, line), Size::new(width, 1)), shaded(chrome::BLUE, SCAN_LEVELS[shade]))?;
        }
        if i % 2 == 0 {
            target.fill_solid(&Rectangle::new(Point::new(x + width as i32 - 8, y), Size::new(8, 5)), chrome::BLACK)?;
        }
    }
    for x in TICKS {
        target.fill_solid(&Rectangle::new(Point::new(x, TICKS_TOP), Size::new_equal(2)), shaded(chrome::BLUE, TICK_LEVEL))?;
    }
    Ok(())
}

/// The battery's line and cells, which show in every state: a clock fault is not a battery one.
fn draw_battery<D: CoverageTarget<Color = Color>>(
    battery: Option<Battery>,
    font: &FontdueRenderer<'static, Color>,
    target: &mut D,
) -> Result<(), D::Error> {
    let mut line = String::<12>::new();
    let (percent, color) = match battery.filter(|battery| battery.present) {
        Some(battery) => {
            _ = write!(line, "BAT {}%", battery.percent);
            (Some(battery.percent.min(100)), if battery.percent <= LOW { chrome::ORANGE } else { chrome::GRAY })
        }
        None => {
            _ = line.push_str("BAT --");
            (None, chrome::GRAY)
        }
    };
    let style = clock_screen::style(font, color, 14, FRAKTION_BOLD);
    let ink = style.baseline_bounds(&line, Point::zero());
    let pen = Point::new(BATTERY_RIGHT + 1 - ink.top_left.x - ink.size.width as i32, BATTERY_TOP - ink.top_left.y);
    style.draw_on_baseline(&line, pen, target)?;
    for i in 0..10 {
        let cell = Rectangle::new(Point::new(CELLS_LEFT + CELL_PITCH * i, CELLS_TOP), CELL);
        target.fill_solid(&cell, shaded(chrome::BLUE, CELL_LEVEL))?;
        let fraction = percent.map_or(0.0, |percent| (f32::from(percent) / 10.0 - i as f32).clamp(0.0, 1.0));
        if fraction > 0.0 {
            let width = libm::roundf(6.0 * fraction) as u32 + 1;
            target.fill_solid(&Rectangle::new(cell.top_left, Size::new(width, CELL.height)), color)?;
        }
    }
    Ok(())
}
