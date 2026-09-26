//! The clock face: hours on the field, minutes and seconds knocked out of a band across the
//! circle in the state's colour, a status symbol, UTC and the battery in the band beside a
//! hatched battery column, the date and the zone as two lines of tokens, a 24-hour rail, and two
//! quiet scatter fields behind. It is K1 of `context/design/`, on the round 4 spec
//! (`context/design/specs/CLOCK-FACE-ROUND4-SPEC.md`) and the clock face spec before it.

use core::fmt::Write as _;

use embedded_graphics::{
    draw_target::DrawTarget as _,
    prelude::{Point, Size},
    primitives::Rectangle,
};
use heapless::String;

use super::{
    clock::{ClockState, ClockView, DateTime, ZoneMode, ZoneState},
    icon::{self, Glyph, Tile},
    reveal::{Reveal, draw_revealed, revealed_bounds},
    scatter::{Field, Look, Scatter},
    screens::Battery,
    startup, text,
};
use crate::chrome::{
    self, Color, CoverageTarget, FontdueRenderer, OnBackground, RgbColorExt as _, Window, FRAKTION,
    FRAKTION_BOLD, SHAPIRO,
};

const CENTER: Point = Point::new(233, 233);
const DIGITS_PX: u32 = 136;
const HOURS: Point = Point::new(62, 184);
const MINUTES: Point = Point::new(62, 305);
const SECONDS: Point = Point::new(260, 305);
const SECONDS_PX: u32 = 40;
const LABEL: Point = Point::new(261, 225);
const TILE: Tile = Tile {
    corner: Point::new(262, 90),
    module: 16,
    padding: 8,
};
/// The band ends where the ring's outer edge does, on the page's own circle, so a swipe carries
/// its ends round with the page.
const BAND_ROWS: core::ops::Range<i32> = 198..318;
const BAND_RADIUS: f32 = 232.0;
const BAND: Rectangle = Rectangle::new(Point::new(0, 198), Size::new(466, 120));
/// Where each line's ink and reveal blocks can land, whatever it shows, with a pixel spare.
/// A part whose region the damage misses is skipped without laying its text out.
/// `each_line_stays_in_its_region` checks every character each line can hold.
const HOURS_INK: Rectangle = Rectangle::new(Point::new(62, 88), Size::new(166, 99));
const MINUTES_INK: Rectangle = Rectangle::new(Point::new(62, 209), Size::new(166, 99));
const SECONDS_INK: Rectangle = Rectangle::new(Point::new(260, 276), Size::new(50, 31));
const LABEL_INK: Rectangle = Rectangle::new(Point::new(260, 211), Size::new(70, 19));
const NO_DATA_INK: Rectangle = Rectangle::new(Point::new(70, 245), Size::new(163, 24));
const MARK_INK: Rectangle = Rectangle::new(Point::new(363, 0), Size::new(25, 466));
/// `NO DATA`'s ink starts on the digits' ink column, centred on this row.
const NO_DATA_LEFT: i32 = 71;
const NO_DATA_MIDDLE_TWICE: i32 = 515;
/// The band's two lines, UTC and the battery, by their caps' tops, and the region either's ink
/// and reveal blocks can reach.
const BAND_LINES: [i32; 2] = [238, 256];
const BAND_LINE_LEFT: i32 = 262;
const BAND_LINES_INK: Rectangle = Rectangle::new(Point::new(260, 234), Size::new(104, 36));
/// The battery column at the band's right end: a black window, its edge inset a pixel, and the
/// hatch inset three, filled from the bottom.
const WINDOW: Rectangle = Rectangle::new(Point::new(394, 206), Size::new(46, 105));
const EDGE: Rectangle = Rectangle::new(Point::new(395, 207), Size::new(44, 103));
const HATCH: Rectangle = Rectangle::new(Point::new(397, 209), Size::new(40, 99));
const LOW: u8 = 15;
/// The token lines by their caps' tops, their sizes, and the widest the zone's line may be
/// before its name moves to a line of its own.
const LINE_TOPS: [i32; 3] = [340, 368, 386];
const LINE_SIZES: [u32; 3] = [19, 14, 14];
const ZONE_LINE_MAX: f32 = 320.0;
/// The 24-hour rail: a cell an hour, and a hollow marker on the local hour.
const RAIL_LEFT: i32 = 92;
const RAIL_PITCH: i32 = 12;
const RAIL_CELL: Rectangle = Rectangle::new(Point::new(0, 412), Size::new(5, 3));
const RAIL_MARKER: Rectangle = Rectangle::new(Point::new(-1, 408), Size::new(8, 11));
const RAIL_HOLE: Rectangle = Rectangle::new(Point::new(1, 410), Size::new(4, 7));
const RAIL: Rectangle = Rectangle::new(Point::new(91, 408), Size::new(284, 11));
const RAIL_LEVEL: u8 = 74;
/// Scatter keeps this far from every element on the field, which the design gives as a halo.
const HALO: u32 = 2;
/// The scatter: denser upper right, quieter lower left, clear of the band.
const UPPER: Field = Field { center: Point::new(304, 139), radius: 137.0, seed: 0x6f63_6b31 };
const LOWER: Field = Field { center: Point::new(164, 363), radius: 111.0, seed: 0x6f63_6b32 };
const UPPER_LOOK: Look = Look { facing: -0.60, density: 0.48 };
const LOWER_LOOK: Look = Look { facing: 2.55, density: 0.28 };
const SCATTER_LEVEL: u8 = 125;
const MARK: &str = "OCTOWHERE";
const MARK_PX: u32 = 26;
/// The caps' baseline column. A round letter's overshoot reaches just left of it.
const MARK_BASELINE: i32 = 366;
/// The mark's first letters sit on the field and the rest on the band, and the gap between the
/// two halves is centred on the band's top row. No cell crosses that row.
const MARK_ON_FIELD: usize = 4;

const GNSS: Glyph = [0b00100, 0b01010, 0b10101, 0b01010, 0b00100];
const RTC: Glyph = [0b11111, 0b10001, 0b10101, 0b10001, 0b11111];
const STOPPED: Glyph = [0b01010, 0b01010, 0b01010, 0b01010, 0b01010];
const NO_ZONE: Glyph = [0b01110, 0b10001, 0b00110, 0b00000, 0b00100];

fn band() -> &'static super::smooth::DiscRows {
    static BAND_DISC: embassy_sync::once_lock::OnceLock<super::smooth::DiscRows> =
        embassy_sync::once_lock::OnceLock::new();
    BAND_DISC.get_or_init(|| super::smooth::DiscRows::new(BAND_ROWS, CENTER, BAND_RADIUS))
}

/// The part of the settled face that the band always paints solid, so a clear can leave it.
#[must_use]
pub fn solid_band() -> Rectangle {
    band().solid()
}

/// How far each of the face's accents has come in, 0 to 255 along each one's own window: the
/// ring's fade, the band label and lines, the first token line, the zone's lines, the wordmark,
/// the scatter's bloom and the battery hatch's rise; how many rows of the icon's modules show, 0
/// to 5. Also the reveals of the time, across hours, minutes and seconds, and of the date line,
/// which run only when a fix or a zone change replaces the time. The face applies each part's
/// curve. `crawl` is how far the charging hatch has moved, a pixel a frame.
/// The time and date are centre content, so a page's entry and exit leave them whole.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Accents {
    pub ring: u8,
    pub icon_rows: u8,
    pub label: u8,
    pub plate: u8,
    pub zone: u8,
    pub mark: u8,
    pub scatter: u8,
    pub battery: u8,
    pub time: u8,
    pub date: u8,
    pub crawl: u8,
}

impl Accents {
    pub const FULL: Self = Self {
        ring: u8::MAX,
        icon_rows: 5,
        label: u8::MAX,
        plate: u8::MAX,
        zone: u8::MAX,
        mark: u8::MAX,
        scatter: u8::MAX,
        battery: u8::MAX,
        time: u8::MAX,
        date: u8::MAX,
        crawl: 0,
    };
    pub const HIDDEN: Self = Self {
        ring: 0,
        icon_rows: 0,
        label: 0,
        plate: 0,
        zone: 0,
        mark: 0,
        scatter: 0,
        battery: 0,
        time: u8::MAX,
        date: u8::MAX,
        crawl: 0,
    };

    /// Each accent at the lesser of the two, and `self`'s crawl.
    #[must_use]
    pub fn min(self, other: Self) -> Self {
        Self {
            ring: self.ring.min(other.ring),
            icon_rows: self.icon_rows.min(other.icon_rows),
            label: self.label.min(other.label),
            plate: self.plate.min(other.plate),
            zone: self.zone.min(other.zone),
            mark: self.mark.min(other.mark),
            scatter: self.scatter.min(other.scatter),
            battery: self.battery.min(other.battery),
            time: self.time.min(other.time),
            date: self.date.min(other.date),
            crawl: self.crawl,
        }
    }
}

impl Default for Accents {
    fn default() -> Self {
        Self::FULL
    }
}

// The entry's curves, from an accent's progress.

fn unit(progress: u8) -> f32 {
    f32::from(progress) / 255.0
}

fn out_cubic(progress: u8) -> f32 {
    let u = 1.0 - unit(progress);
    1.0 - u * u * u
}

/// Overshoots by up to about a tenth before it settles.
fn out_back(progress: u8) -> f32 {
    let u = unit(progress) - 1.0;
    1.0 + 2.701_58 * u * u * u + 1.701_58 * u * u
}

fn in_quad(progress: u8) -> f32 {
    unit(progress) * unit(progress)
}

fn in_expo(progress: u8) -> f32 {
    match progress {
        0 => 0.0,
        u8::MAX => 1.0,
        _ => libm::powf(2.0, 10.0 * unit(progress) - 10.0),
    }
}

/// A curve's value back as progress for a reveal, 0 to 255.
fn level(value: f32) -> u8 {
    libm::roundf(value.clamp(0.0, 1.0) * 255.0) as u8
}

/// What the face shows, in the order that decides between them.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Mode {
    /// The clock could not be read.
    NoData,
    /// The clock stopped since it was last set, so its time is unreliable.
    Stopped,
    /// No zone has ever been found.
    NoZone,
    /// Local time, and whether GNSS has set the clock since the firmware started.
    Local { gnss: bool },
}

impl Mode {
    #[must_use]
    pub fn of(clock: &ClockState, zone: &ZoneState) -> Self {
        match () {
            () if clock.utc.is_none() => Self::NoData,
            () if clock.stopped => Self::Stopped,
            () if zone.zone.is_none() => Self::NoZone,
            () => Self::Local {
                gnss: clock.set_from_gnss,
            },
        }
    }

    fn icon(self) -> (&'static Glyph, Color) {
        match self {
            Self::NoData => (&icon::NO_DATA, chrome::RED),
            Self::Stopped => (&STOPPED, chrome::ORANGE),
            Self::NoZone => (&NO_ZONE, chrome::WHITE),
            Self::Local { gnss: true } => (&GNSS, chrome::BLUE),
            Self::Local { gnss: false } => (&RTC, chrome::GRAY),
        }
    }

    fn band(self) -> Color {
        match self {
            Self::NoData => chrome::RED,
            Self::Stopped => chrome::ORANGE,
            Self::NoZone => chrome::WHITE,
            Self::Local { .. } => chrome::LIME,
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::NoData => "CLOCK",
            Self::Stopped => "STOPPED",
            Self::NoZone => "NO ZONE",
            Self::Local { .. } => "LOCAL",
        }
    }
}

/// The plate's cells: the mode tag, then values in ruled cells.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Plate {
    tag: &'static str,
    values: heapless::Vec<String<10>, 2>,
}

/// The parts of the face that re-reveal when they change while it shows.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Keys {
    pub mode: Mode,
    pub plate: Plate,
    pub zone: Option<&'static str>,
}

impl Keys {
    #[must_use]
    pub fn of(view: &ClockView) -> Self {
        let (clock, zone) = (&view.clock(), &view.zone());
        let mode = Mode::of(clock, zone);
        let tag = match zone.mode {
            ZoneMode::Automatic => "AUTO",
            ZoneMode::Manual => "MANUAL",
        };
        let mut values = heapless::Vec::new();
        match (mode, offset(view)) {
            (Mode::NoZone, _) => _ = values.push(String::try_from("NO FIX YET").unwrap()),
            (_, Some(known)) => {
                let mut offset = String::new();
                let seconds = known.utc_offset;
                let sign = if seconds < 0 { '-' } else { '+' };
                let minutes = seconds.unsigned_abs() / 60;
                _ = write!(offset, "{sign}{:02}:{:02}", minutes / 60, minutes % 60);
                let mut abbreviation = String::new();
                _ = abbreviation.push_str(known.abbreviation);
                _ = values.push(abbreviation);
                _ = values.push(offset);
            }
            _ => {}
        }
        Self {
            mode,
            plate: Plate { tag, values },
            zone: zone
                .zone
                .map(|zone| octowhere_tz::DATABASE.zone(zone).name)
                .filter(|_| mode != Mode::NoZone),
        }
    }
}

/// The zone's offset now, or while the clock is stopped or unreadable, the one it last had.
#[must_use]
pub fn offset(view: &ClockView) -> Option<octowhere_tz::Offset<'static>> {
    let (clock, zone) = (view.clock(), view.zone());
    match (Mode::of(&clock, &zone), view.local()) {
        (Mode::Local { .. }, Some(local)) => Some(local.offset),
        (Mode::Stopped | Mode::NoData, _) => view
            .known()
            .filter(|known| Some(known.zone) == zone.zone)
            .map(|known| known.offset),
        _ => None,
    }
}

/// The zone's abbreviation, from [`offset`].
#[must_use]
pub fn abbreviation(view: &ClockView) -> Option<&'static str> {
    offset(view).map(|offset| offset.abbreviation)
}

/// What each part of the face shows. Two frames that agree on a part draw it identically.
#[derive(Clone, Debug, Eq, PartialEq)]
struct Parts {
    ring: Option<Color>,
    icon: (&'static Glyph, Color, u8),
    band: Color,
    label: (&'static str, Reveal),
    /// The hours and minutes, `--` while withheld, and `None` with no reading at all.
    hours: Option<String<2>>,
    minutes: Option<String<2>>,
    seconds: Option<String<2>>,
    /// Across the hours, minutes and seconds in turn.
    time: Reveal,
    /// UTC and the battery in the band, with no reading none.
    band_lines: Option<[(String<16>, Reveal); 2]>,
    column: Column,
    /// Each token line, its reveal, and everything it can cover.
    lines: heapless::Vec<(Tokens, Reveal, Rectangle), 3>,
    /// The rail, and the local hour its marker is on if the face knows one.
    rail: Option<Option<u8>>,
    /// How far the scatter has bloomed, with no reading none.
    scatter: Option<u8>,
    mark: Reveal,
}

/// The battery column's hatch: its colour, how many rows up it reaches, and how far the
/// charging crawl has moved it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Column {
    color: Color,
    rows: u32,
    shift: i32,
}

/// A line of runs in one colour and size, Regular or Bold, centred as a group on one baseline.
/// The runs share one string, each ending where its entry says: a frame of the clock holds two
/// sets of these on core 0's stack.
#[derive(Clone, Debug, Eq, PartialEq)]
struct Tokens {
    color: Color,
    row: usize,
    text: String<64>,
    runs: heapless::Vec<(u8, bool), 6>,
}

impl Tokens {
    fn new(color: Color, row: usize, runs: &[(&str, bool)]) -> Self {
        let mut tokens = Self { color, row, text: String::new(), runs: heapless::Vec::new() };
        for &(text, bold) in runs {
            _ = tokens.text.push_str(text);
            _ = tokens.runs.push((tokens.text.len() as u8, bold));
        }
        tokens
    }

    /// Each run's text and whether it is Bold.
    fn each(&self) -> impl Iterator<Item = (&str, bool)> + '_ {
        let mut start = 0;
        self.runs.iter().map(move |&(end, bold)| {
            let text = &self.text[start..usize::from(end)];
            start = usize::from(end);
            (text, bold)
        })
    }

    fn len(&self) -> usize {
        self.text.len()
    }

    fn style(&self, font: &FontdueRenderer<'static, Color>, bold: bool) -> FontdueRenderer<'static, Color> {
        style(font, self.color, LINE_SIZES[self.row], if bold { FRAKTION_BOLD } else { FRAKTION })
    }

    fn width(&self, font: &FontdueRenderer<'static, Color>) -> f32 {
        self.each().map(|(text, bold)| self.style(font, bold).advance(text)).sum()
    }

    /// Each run with its style and pen.
    fn placed<'t>(
        &'t self,
        font: &'t FontdueRenderer<'static, Color>,
    ) -> impl Iterator<Item = (&'t str, FontdueRenderer<'static, Color>, Point)> + 't {
        let baseline = LINE_TOPS[self.row] + text::cap(&self.style(font, false));
        let mut x = CENTER.x as f32 - self.width(font) / 2.0;
        self.each().map(move |(text, bold)| {
            let style = self.style(font, bold);
            let pen = Point::new(libm::roundf(x) as i32, baseline);
            x += style.advance(text);
            (text, style, pen)
        })
    }

    fn bounds(&self, font: &FontdueRenderer<'static, Color>) -> Rectangle {
        self.placed(font)
            .map(|(text, style, pen)| revealed_bounds(&style, text, pen))
            .filter(|bounds| !bounds.is_zero_sized())
            .reduce(|a, b| Rectangle::with_corners(a.top_left.component_min(b.top_left), a.bottom_right().unwrap_or(a.top_left).component_max(b.bottom_right().unwrap_or(b.top_left))))
            .unwrap_or(Rectangle::zero())
    }

    fn draw<D: CoverageTarget<Color = Color>>(
        &self,
        font: &FontdueRenderer<'static, Color>,
        reveal: Reveal,
        target: &mut D,
    ) -> Result<(), D::Error> {
        let mut from = 0;
        for (text, style, pen) in self.placed(font) {
            draw_revealed(&style, text, pen, reveal.part(from, text.len()), target)?;
            from += text.len();
        }
        Ok(())
    }
}

pub(crate) const WEEKDAYS: [&str; 7] = ["SUN", "MON", "TUE", "WED", "THU", "FRI", "SAT"];
pub(crate) const MONTHS: [&str; 12] = [
    "JAN", "FEB", "MAR", "APR", "MAY", "JUN", "JUL", "AUG", "SEP", "OCT", "NOV", "DEC",
];

fn two(value: u8) -> String<2> {
    let mut text = String::new();
    _ = write!(text, "{value:02}");
    text
}

fn dashes() -> String<2> {
    String::try_from("--").unwrap()
}

/// The date after the weekday, as `24 SEP 2026`.
fn date(time: &DateTime) -> String<16> {
    let mut text = String::new();
    _ = write!(text, "{:02} {} {:04}", time.day, MONTHS[usize::from(time.month - 1)], time.year);
    text
}

/// The local time the face shows, in seconds, or `None` when it shows none.
#[must_use]
pub fn shown_time(view: &ClockView) -> Option<i64> {
    if !matches!(Mode::of(&view.clock(), &view.zone()), Mode::Local { .. }) {
        return None;
    }
    let local = view.local()?;
    Some(view.clock().utc? + i64::from(local.offset.utc_offset))
}

/// The time's characters: two each for the hours, minutes and seconds.
const TIME_CELLS: usize = 6;

/// The battery line and the hatch's colour and fill, 0 to 100, for `battery` under a band of
/// `band`. Unknown fills the hatch in `GRAY`.
fn battery(battery: Option<Battery>, band: Color) -> (String<16>, Color, u8, bool) {
    let mut line = String::new();
    match battery {
        Some(battery) if battery.present => {
            _ = write!(line, "BAT {}%", battery.percent);
            if battery.charging {
                _ = line.push_str(" CHG");
            }
            let color = match () {
                () if battery.percent <= LOW => chrome::ORANGE,
                // A clock fault is not a battery fault.
                () if band == chrome::RED => chrome::WHITE,
                () => band,
            };
            (line, color, battery.percent.min(100), battery.charging)
        }
        Some(battery) if battery.usb => {
            _ = line.push_str("USB");
            (line, chrome::GRAY, 100, false)
        }
        _ => {
            _ = line.push_str("BAT --");
            (line, chrome::GRAY, 100, false)
        }
    }
}

impl Parts {
    fn of(view: &ClockView, supply: Option<Battery>, accents: Accents, font: &FontdueRenderer<'static, Color>) -> Self {
        let clock = &view.clock();
        let keys = Keys::of(view);
        let mode = keys.mode;
        let local = view.local().filter(|_| matches!(mode, Mode::Local { .. }));
        let (hours, minutes, seconds) = match (mode, local) {
            (Mode::NoData, _) => (None, None, None),
            (_, Some(local)) => (
                Some(two(local.time.hour)),
                Some(two(local.time.minute)),
                Some(two(local.time.second)),
            ),
            _ => (Some(dashes()), Some(dashes()), Some(dashes())),
        };
        let band = mode.band();
        let (battery_line, hatch, fill, charging) = battery(supply, band);
        let label_reveal = level(out_cubic(accents.label));
        let band_lines = (mode != Mode::NoData).then(|| {
            let mut utc = String::<16>::new();
            match clock.utc_time().filter(|_| mode != Mode::Stopped) {
                Some(time) => _ = write!(utc, "UTC {:02}:{:02}", time.hour, time.minute),
                None => _ = utc.push_str("UTC --:--"),
            }
            let reveal = |text: &str| Reveal::of(label_reveal, text.len());
            [(utc.clone(), reveal(&utc)), (battery_line.clone(), reveal(&battery_line))]
        });
        let rise = out_back(accents.battery);
        let column = Column {
            color: hatch,
            rows: (libm::roundf(HATCH.size.height as f32 * f32::from(fill) / 100.0 * rise) as u32).min(HATCH.size.height),
            shift: if charging { i32::from(accents.crawl) } else { 0 },
        };

        let mut lines = heapless::Vec::new();
        let tag = keys.plate.tag;
        let first = level(out_cubic(accents.plate).min(out_cubic(accents.date)));
        let first = match (mode, local) {
            (Mode::Local { .. }, Some(local)) => Some(Tokens::new(chrome::LIME, 0, &[
                (WEEKDAYS[usize::from(local.time.weekday())], true),
                (" ", false),
                (&date(&local.time), false),
                ("  [", false),
                (tag, true),
                ("]", false),
            ])),
            (Mode::Stopped, _) => Some(Tokens::new(chrome::GRAY, 0, &[("WAITING FOR GNSS", false)])),
            (Mode::NoZone, _) => Some(Tokens::new(chrome::GRAY, 0, &[("NO FIX YET", false), ("  [", false), (tag, true), ("]", false)])),
            _ => None,
        }
        .map(|tokens| {
            let reveal = Reveal::of(first, tokens.len());
            let bounds = tokens.bounds(font);
            (tokens, reveal, bounds)
        });
        if let Some(first) = first {
            _ = lines.push(first);
        }
        if !matches!(mode, Mode::NoZone | Mode::NoData)
            && let ([abbreviation, offset], Some(name)) = (keys.plate.values.as_slice(), keys.zone)
        {
            let mut zone = String::<32>::new();
            _ = write!(zone, "{abbreviation} {offset}");
            let mut comment = String::<48>::new();
            _ = comment.push_str("// ");
            for c in name.chars() {
                _ = comment.push(c.to_ascii_uppercase());
            }
            let reveal = level(out_cubic(accents.zone));
            let mut whole = zone.clone();
            _ = whole.push(' ');
            let one = Tokens::new(chrome::GRAY, 1, &[(&whole, false), (&comment, false)]);
            if one.width(font) > ZONE_LINE_MAX {
                for tokens in [Tokens::new(chrome::GRAY, 1, &[(&zone, false)]), Tokens::new(chrome::GRAY, 2, &[(&comment, false)])] {
                    let (r, bounds) = (Reveal::of(reveal, tokens.len()), tokens.bounds(font));
                    _ = lines.push((tokens, r, bounds));
                }
            } else {
                let (r, bounds) = (Reveal::of(reveal, one.len()), one.bounds(font));
                _ = lines.push((one, r, bounds));
            }
        }

        let ring_color = if mode == Mode::NoData { chrome::RED } else { chrome::GRAY };
        let ring = (accents.ring > 0).then(|| {
            let fade = out_back(accents.ring);
            if fade <= 1.0 {
                chrome::BLACK.lerp(&ring_color, level(fade))
            } else {
                ring_color.lerp(&chrome::WHITE, level(fade - 1.0))
            }
        });
        let (glyph, icon_color) = mode.icon();
        let label = mode.label();
        Self {
            ring,
            icon: (glyph, icon_color, accents.icon_rows),
            band,
            label: (label, Reveal::of(label_reveal, label.len())),
            hours,
            minutes,
            seconds,
            time: Reveal::of(accents.time, TIME_CELLS),
            band_lines,
            column,
            lines,
            rail: (mode != Mode::NoData && accents.zone > 0).then(|| local.map(|local| local.time.hour)),
            scatter: (mode != Mode::NoData).then(|| level(in_quad(accents.scatter))),
            mark: Reveal::of(level(in_expo(accents.mark)), MARK.len()),
        }
    }

    /// The boxes the scatter keeps clear of: every element on the field, grown by the halo.
    fn clear(&self, font: &FontdueRenderer<'static, Color>) -> heapless::Vec<Rectangle, 8> {
        let mut clear = heapless::Vec::new();
        _ = clear.push(HOURS_INK);
        _ = clear.push(TILE.bounds());
        _ = clear.push(mark_layout(font).span(0, MARK_ON_FIELD));
        for (_, _, bounds) in &self.lines {
            _ = clear.push(*bounds);
        }
        if self.rail.is_some() {
            _ = clear.push(RAIL);
        }
        for area in &mut clear {
            *area = area.offset(HALO as i32);
        }
        clear
    }
}

fn scatter() -> Scatter {
    Scatter {
        origin: Point::new(12, -2),
        gap: Some((193..=322, 0)),
        color: chrome::shade(chrome::PURPLE, SCATTER_LEVEL),
        fields: &[UPPER, LOWER],
    }
}

fn looks(bloom: u8) -> [Look; 2] {
    let k = f32::from(bloom) / 255.0;
    [Look { density: UPPER_LOOK.density * k, ..UPPER_LOOK }, Look { density: LOWER_LOOK.density * k, ..LOWER_LOOK }]
}

pub(crate) fn style(
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

fn digits(font: &FontdueRenderer<'static, Color>, color: Color) -> FontdueRenderer<'static, Color> {
    style(font, color, DIGITS_PX, FRAKTION_BOLD)
}

fn seconds_style(font: &FontdueRenderer<'static, Color>) -> FontdueRenderer<'static, Color> {
    style(font, chrome::BLACK, SECONDS_PX, FRAKTION_BOLD)
}

fn label_style(font: &FontdueRenderer<'static, Color>) -> FontdueRenderer<'static, Color> {
    style(font, chrome::BLACK, 16, FRAKTION_BOLD)
}

pub(crate) fn no_data_style(font: &FontdueRenderer<'static, Color>) -> FontdueRenderer<'static, Color> {
    style(font, chrome::BLACK, 28, SHAPIRO)
}

pub(crate) fn no_data_origin(font: &FontdueRenderer<'static, Color>) -> Point {
    let ink = no_data_style(font).baseline_bounds("NO DATA", Point::zero());
    let top = (NO_DATA_MIDDLE_TWICE - ink.size.height as i32) / 2;
    Point::new(NO_DATA_LEFT - ink.top_left.x, top - ink.top_left.y)
}

fn mark_style(font: &FontdueRenderer<'static, Color>, color: Color) -> FontdueRenderer<'static, Color> {
    style(font, color, MARK_PX, SHAPIRO)
}

/// The wordmark's layout, worked out once: it depends only on the font, and laying it out reads
/// glyph metrics from flash.
fn mark_layout(font: &FontdueRenderer<'static, Color>) -> &'static MarkLayout {
    static LAYOUT: embassy_sync::once_lock::OnceLock<MarkLayout> = embassy_sync::once_lock::OnceLock::new();
    LAYOUT.get_or_init(|| MarkLayout::of(font))
}

/// Where the mark sits along the column: the rows each letter's cell spans, and how far either
/// side of the baseline its caps and its ink reach.
struct MarkLayout {
    pens: [f32; MARK.len() + 1],
    cap: f32,
    below: f32,
    above: f32,
}

impl MarkLayout {
    fn of(font: &FontdueRenderer<'static, Color>) -> Self {
        let style = mark_style(font, chrome::WHITE);
        let mut pens = [0.0; MARK.len() + 1];
        let (mut below, mut above, mut cap) = (0.0_f32, 0.0_f32, 0.0_f32);
        let (mut field_end, mut band_start) = (0.0, 0.0);
        for (i, (_, offset, metrics)) in style.pens(MARK).enumerate() {
            let bounds = metrics.bounds;
            pens[i] = offset;
            below = below.min(bounds.ymin);
            above = above.max(bounds.ymin + bounds.height);
            if bounds.ymin == 0.0 {
                cap = cap.max(bounds.height);
            }
            if i + 1 == MARK_ON_FIELD {
                field_end = offset + bounds.xmin + bounds.width;
            } else if i == MARK_ON_FIELD {
                band_start = offset + bounds.xmin;
            }
        }
        pens[MARK.len()] = style.advance(MARK);
        let top = BAND_ROWS.start as f32 - (field_end + band_start) / 2.0;
        Self {
            pens: pens.map(|pen| top + pen),
            cap,
            below,
            above,
        }
    }

    /// Letter `index`'s advance along the column less a pixel at each end, across the caps.
    fn cell(&self, index: usize) -> Rectangle {
        let (top, bottom) = (self.pens[index] + 1.0, self.pens[index + 1] - 1.0);
        let round = |value: f32| libm::roundf(value) as i32;
        Rectangle::with_corners(
            Point::new(MARK_BASELINE, round(top)),
            Point::new(MARK_BASELINE + round(self.cap) - 1, round(bottom) - 1),
        )
    }

    /// Everything letters `from..to` or their cells can cover.
    fn span(&self, from: usize, to: usize) -> Rectangle {
        let floor = |value: f32| libm::floorf(value) as i32;
        Rectangle::with_corners(
            Point::new(MARK_BASELINE + floor(self.below) - 1, floor(self.pens[from]) - 1),
            Point::new(MARK_BASELINE + floor(self.above) + 1, floor(self.pens[to]) + 1),
        )
    }
}

/// The wordmark: its letters on the field in the band's colour, and on the band black.
fn draw_mark<D: CoverageTarget<Color = Color>>(
    font: &FontdueRenderer<'static, Color>,
    reveal: Reveal,
    band: Color,
    target: &mut D,
) -> Result<(), D::Error> {
    let layout = mark_layout(font);
    let whole = layout.span(0, reveal.cells());
    let text = &MARK[..reveal.glyphs()];
    let block = (reveal.cells() > reveal.glyphs()).then(|| layout.cell(reveal.glyphs()));
    for (rows, color, background) in [
        (0..BAND_ROWS.start, band, chrome::BLACK),
        (BAND_ROWS, chrome::BLACK, band),
    ] {
        let clip = Rectangle::new(Point::new(0, rows.start), Size::new(466, rows.len() as u32));
        let window = &mut Window::new(&mut *target, Point::zero(), clip);
        if !window.visible(&whole) {
            continue;
        }
        let on = &mut OnBackground::new(window, background);
        mark_style(font, color).draw_turned(text, (MARK_BASELINE as f32, layout.pens[0]), on)?;
        if let Some(block) = block {
            on.fill_solid(&block, color)?;
        }
    }
    Ok(())
}

pub fn draw<D>(
    view: &ClockView,
    supply: Option<Battery>,
    accents: Accents,
    font: &FontdueRenderer<'static, Color>,
    target: &mut D,
) -> Result<(), D::Error>
where
    D: CoverageTarget<Color = Color>,
{
    let parts = parts(&(*view, supply, accents), font);
    if let Some(bloom) = parts.scatter {
        scatter().draw_clear_of(&looks(bloom), &parts.clear(font), target)?;
    }
    if let Some(hours) = parts.hours.as_ref().filter(|_| target.visible(&HOURS_INK)) {
        let field = &mut OnBackground::new(&mut *target, chrome::BLACK);
        draw_revealed(&digits(font, chrome::WHITE), hours, HOURS, parts.time.part(0, 2), field)?;
    }
    let (glyph, color, rows) = parts.icon;
    if target.visible(&TILE.bounds()) {
        TILE.draw(glyph, color, rows, target)?;
    }

    band().draw(target, parts.band)?;
    {
        let band = &mut OnBackground::new(&mut *target, parts.band);
        let (label, reveal) = parts.label;
        if band.visible(&LABEL_INK) {
            draw_revealed(&label_style(font), label, LABEL, reveal, band)?;
        }
        match &parts.minutes {
            Some(minutes) if band.visible(&MINUTES_INK) => {
                draw_revealed(&digits(font, chrome::BLACK), minutes, MINUTES, parts.time.part(2, 2), band)?;
            }
            Some(_) => {}
            None if band.visible(&NO_DATA_INK) => {
                no_data_style(font).draw_on_baseline("NO DATA", no_data_origin(font), band)?;
            }
            None => {}
        }
        if let Some(seconds) = parts.seconds.as_ref().filter(|_| band.visible(&SECONDS_INK)) {
            draw_revealed(&seconds_style(font), seconds, SECONDS, parts.time.part(4, 2), band)?;
        }
        if let Some(lines) = parts.band_lines.as_ref().filter(|_| band.visible(&BAND_LINES_INK)) {
            let style = band_line_style(font);
            for ((text, reveal), top) in lines.iter().zip(BAND_LINES) {
                draw_revealed(&style, text, band_line_pen(&style, top), *reveal, band)?;
            }
        }
    }
    if target.visible(&WINDOW) {
        draw_column(parts.column, target)?;
    }
    if target.visible(&MARK_INK) {
        draw_mark(font, parts.mark, parts.band, target)?;
    }
    for (tokens, reveal, bounds) in &parts.lines {
        if target.visible(bounds) {
            tokens.draw(font, *reveal, &mut OnBackground::new(&mut *target, chrome::BLACK))?;
        }
    }
    if let Some(hour) = parts.rail.filter(|_| target.visible(&RAIL)) {
        draw_rail(hour, target)?;
    }
    if let Some(color) = parts.ring {
        super::smooth::perimeter().draw(&mut OnBackground::new(&mut *target, chrome::BLACK), color);
    }
    Ok(())
}

fn band_line_style(font: &FontdueRenderer<'static, Color>) -> FontdueRenderer<'static, Color> {
    style(font, chrome::BLACK, 14, FRAKTION)
}

fn band_line_pen(style: &FontdueRenderer<'static, Color>, top: i32) -> Point {
    Point::new(BAND_LINE_LEFT, top + text::cap(style))
}

fn draw_column<D: CoverageTarget<Color = Color>>(column: Column, target: &mut D) -> Result<(), D::Error> {
    target.fill_solid(&WINDOW, chrome::BLACK)?;
    let (left, top) = (EDGE.top_left.x, EDGE.top_left.y);
    let (right, bottom) = (left + EDGE.size.width as i32 - 1, top + EDGE.size.height as i32 - 1);
    for edge in [
        Rectangle::with_corners(Point::new(left, top), Point::new(right, top)),
        Rectangle::with_corners(Point::new(left, bottom), Point::new(right, bottom)),
        Rectangle::with_corners(Point::new(left, top), Point::new(left, bottom)),
        Rectangle::with_corners(Point::new(right, top), Point::new(right, bottom)),
    ] {
        target.fill_solid(&edge, column.color)?;
    }
    let filled = Rectangle::new(
        HATCH.top_left + Point::new(0, (HATCH.size.height - column.rows) as i32),
        Size::new(HATCH.size.width, column.rows),
    );
    // Stripes move up as the shift grows: along a stripe x + y is constant.
    startup::hatch(filled, 8, 4, column.shift, column.color, target)
}

fn moved(area: Rectangle, x: i32) -> Rectangle {
    Rectangle::new(area.top_left + Point::new(x, 0), area.size)
}

fn draw_rail<D: CoverageTarget<Color = Color>>(hour: Option<u8>, target: &mut D) -> Result<(), D::Error> {
    let cell = chrome::shade(chrome::GRAY, RAIL_LEVEL);
    for h in 0..24 {
        target.fill_solid(&moved(RAIL_CELL, RAIL_LEFT + RAIL_PITCH * h), cell)?;
    }
    if let Some(hour) = hour {
        let x = RAIL_LEFT + RAIL_PITCH * i32::from(hour);
        target.fill_solid(&moved(RAIL_MARKER, x), chrome::LIME)?;
        target.fill_solid(&moved(RAIL_HOLE, x), chrome::BLACK)?;
    }
    Ok(())
}

fn text(digits: &Option<String<2>>) -> &str {
    digits.as_ref().map_or("", String::as_str)
}

/// Marks what changed in a pair of digits: each changed glyph on a tick, or every cell either
/// shows while its reveal moves.
fn digit_damage(
    style: &FontdueRenderer<'static, Color>,
    pen: Point,
    (old, old_reveal): (&Option<String<2>>, Reveal),
    (new, new_reveal): (&Option<String<2>>, Reveal),
    damage: &mut chrome::Dirty,
) {
    if old_reveal != new_reveal {
        for digits in [old, new].into_iter().flatten() {
            damage.add(revealed_bounds(style, digits, pen));
        }
    } else if old != new {
        style.glyph_damage((text(old), pen), (text(new), pen), damage);
    }
}

/// What the clock face draws from: the clock, the supply and the accents.
pub type Face = (ClockView, Option<Battery>, Accents);

/// The parts for `face`, from the last two worked out if it is one of them: a step works out the
/// face before and after for its damage, and the draw after it the same face again.
fn parts(face: &Face, font: &FontdueRenderer<'static, Color>) -> Parts {
    use embassy_sync::blocking_mutex::{Mutex, raw::CriticalSectionRawMutex};
    static RECENT: Mutex<CriticalSectionRawMutex, core::cell::RefCell<heapless::Deque<(Face, Parts), 2>>> =
        Mutex::new(core::cell::RefCell::new(heapless::Deque::new()));
    let found = RECENT.lock(|recent| recent.borrow().iter().find(|(seen, _)| seen == face).map(|(_, parts)| parts.clone()));
    if let Some(parts) = found {
        return parts;
    }
    let parts = Parts::of(&face.0, face.1, face.2, font);
    RECENT.lock(|recent| {
        let mut recent = recent.borrow_mut();
        if recent.is_full() {
            recent.pop_front();
        }
        _ = recent.push_back((*face, parts.clone()));
    });
    parts
}

/// Marks in `damage` every pixel that differs between the face drawn for `before` and for
/// `after`.
pub fn damage(before: &Face, after: &Face, font: &FontdueRenderer<'static, Color>, damage: &mut chrome::Dirty) {
    if before == after {
        return;
    }
    let old = parts(before, font);
    let new = parts(after, font);
    if old == new {
        return;
    }
    if old.ring != new.ring {
        super::smooth::perimeter().damage(damage);
    }
    if old.band != new.band {
        // The band's colour also shows through the mark and the text on it, and colours the
        // mark's letters off it.
        damage.add(BAND);
        damage.add(MARK_INK);
    }
    let white = digits(font, chrome::WHITE);
    digit_damage(&white, HOURS, (&old.hours, old.time.part(0, 2)), (&new.hours, new.time.part(0, 2)), damage);
    if old.icon != new.icon {
        let ((old_glyph, old_color, old_rows), (new_glyph, new_color, new_rows)) = (old.icon, new.icon);
        if (old_glyph, old_color) == (new_glyph, new_color) {
            damage.add(TILE.rows(old_rows.min(new_rows), old_rows.max(new_rows)));
        } else {
            damage.add(TILE.bounds());
        }
    }
    if old.label != new.label {
        for (label, _) in [old.label, new.label] {
            damage.add(revealed_bounds(&label_style(font), label, LABEL));
        }
    }
    if old.minutes.is_none() != new.minutes.is_none() {
        damage.add(MINUTES_INK);
        damage.add(NO_DATA_INK);
    }
    let black = digits(font, chrome::BLACK);
    digit_damage(&black, MINUTES, (&old.minutes, old.time.part(2, 2)), (&new.minutes, new.time.part(2, 2)), damage);
    let seconds = seconds_style(font);
    digit_damage(&seconds, SECONDS, (&old.seconds, old.time.part(4, 2)), (&new.seconds, new.time.part(4, 2)), damage);
    if old.band_lines != new.band_lines {
        let style = band_line_style(font);
        for lines in [&old.band_lines, &new.band_lines].into_iter().flatten() {
            for ((text, _), top) in lines.iter().zip(BAND_LINES) {
                damage.add(revealed_bounds(&style, text, band_line_pen(&style, top)));
            }
        }
    }
    if old.column != new.column {
        damage.add(if old.column.color == new.column.color { HATCH } else { WINDOW });
    }
    if old.mark != new.mark {
        let from = old.mark.glyphs().min(new.mark.glyphs());
        let to = old.mark.cells().max(new.mark.cells());
        damage.add(mark_layout(font).span(from, to));
    }
    if old.lines != new.lines {
        for (_, _, bounds) in old.lines.iter().chain(&new.lines) {
            damage.add(*bounds);
        }
    }
    if old.rail != new.rail {
        damage.add(RAIL);
    }
    let (old_clear, new_clear) = (old.clear(font), new.clear(font));
    if (old.scatter, &old_clear) != (new.scatter, &new_clear) {
        let scatter = scatter();
        let shown = |bloom: Option<u8>, clear: &[Rectangle]| scatter.shown_clear_of(&looks(bloom.unwrap_or(0)), clear);
        scatter.changed(&shown(old.scatter, &old_clear), &shown(new.scatter, &new_clear), damage);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::clock::{ClockState, ZoneState};

    /// The view after reading a clock in `zone`, from `before`.
    fn read(before: ClockView, stopped: bool, readable: bool, zone: &str) -> ClockView {
        let clock = ClockState {
            utc: readable
                .then_some(DateTime { year: 2026, month: 9, day: 24, hour: 12, ..DateTime::default() }.to_unix()),
            set_from_gnss: false,
            stopped,
        };
        let zone = ZoneState {
            mode: ZoneMode::Manual,
            zone: octowhere_tz::DATABASE.find(zone).map(|zone| zone.id),
        };
        before.read(clock, zone)
    }

    fn view(stopped: bool, readable: bool, zone: &str) -> ClockView {
        read(ClockView::default(), stopped, readable, zone)
    }

    fn values(view: &ClockView) -> heapless::Vec<String<10>, 2> {
        Keys::of(view).plate.values
    }

    #[test]
    fn a_stopped_or_unread_clock_keeps_the_offset_the_zone_last_had() {
        let trusted = view(false, true, "Europe/Dublin");
        assert_eq!(values(&trusted), ["IST", "+01:00"]);
        for (stopped, readable) in [(true, true), (false, false)] {
            let later = read(trusted, stopped, readable, "Europe/Dublin");
            assert_eq!(values(&later), ["IST", "+01:00"], "stopped {stopped}, readable {readable}");
        }
    }

    #[test]
    fn the_mark_splits_between_field_and_band_on_the_band_edge() {
        let font = FontdueRenderer::new(chrome::FontdueRendererCtx::new_rc(), 20, chrome::WHITE, chrome::FONTS);
        let layout = MarkLayout::of(&font);
        for index in 0..MARK.len() {
            let cell = layout.cell(index);
            let bottom = cell.bottom_right().unwrap().y;
            if index < MARK_ON_FIELD {
                assert!(bottom < BAND_ROWS.start, "cell {index} ends on row {bottom}");
            } else {
                assert!(cell.top_left.y >= BAND_ROWS.start, "cell {index} starts on row {}", cell.top_left.y);
                assert!(bottom < BAND_ROWS.end, "cell {index} ends on row {bottom}");
            }
        }
        let ink = mark_style(&font, chrome::WHITE).baseline_bounds("OCTOWHERE", Point::zero());
        let last = layout.pens[0] + ink.bottom_right().unwrap().x as f32;
        assert!(last < BAND_ROWS.end as f32, "the ink ends on row {last}");
    }

    fn inside(outer: Rectangle, inner: Rectangle) -> bool {
        inner.is_zero_sized() || outer.intersection(&inner) == inner
    }

    #[test]
    fn each_line_stays_in_its_region() {
        let font = FontdueRenderer::new(chrome::FontdueRendererCtx::new_rc(), 20, chrome::WHITE, chrome::FONTS);
        const DIGITS: &str = "0123456789-";
        const TEXT: &str = "0123456789-+:ABCDEFGHIJKLMNOPQRSTUVWXYZ";
        // Each character repeated to the line's longest, since every font here is monospaced.
        let check = |name: &str, region: Rectangle, chars: &str, len: usize, place: &dyn Fn(&str) -> (FontdueRenderer<'static, Color>, Point)| {
            for c in chars.chars() {
                let text: String<32> = core::iter::repeat_n(c, len).collect();
                let (style, pen) = place(&text);
                let bounds = revealed_bounds(&style, &text, pen);
                assert!(inside(region, bounds), "{name} {text:?} at {bounds:?} leaves {region:?}");
            }
        };
        check("hours", HOURS_INK, DIGITS, 2, &|_| (digits(&font, chrome::WHITE), HOURS));
        check("minutes", MINUTES_INK, DIGITS, 2, &|_| (digits(&font, chrome::BLACK), MINUTES));
        check("seconds", SECONDS_INK, DIGITS, 2, &|_| (seconds_style(&font), SECONDS));
        let longest = [Mode::NoData, Mode::Stopped, Mode::NoZone, Mode::Local { gnss: true }]
            .map(|mode| mode.label().len())
            .into_iter()
            .max()
            .unwrap();
        check("label", LABEL_INK, TEXT, longest, &|_| (label_style(&font), LABEL));
        let band_line = band_line_style(&font);
        for top in BAND_LINES {
            check("band line", BAND_LINES_INK, TEXT, 12, &|_| (band_line.clone(), band_line_pen(&band_line, top)));
        }
        let no_data = no_data_style(&font).baseline_bounds("NO DATA", no_data_origin(&font));
        assert!(inside(NO_DATA_INK, no_data), "NO DATA at {no_data:?}");
        let mark = MarkLayout::of(&font).span(0, MARK.len());
        assert!(inside(MARK_INK, mark), "the mark at {mark:?}");
        // The band lines stop short of the wordmark, and the battery window of the glass.
        assert!(BAND_LINES_INK.bottom_right().unwrap().x < mark.top_left.x);
        let corner = WINDOW.bottom_right().unwrap() - CENTER;
        assert!(corner.x * corner.x + corner.y * corner.y < 226 * 226);
    }

    #[test]
    fn the_token_lines_stay_inside_the_glass() {
        let font = FontdueRenderer::new(chrome::FontdueRendererCtx::new_rc(), 20, chrome::WHITE, chrome::FONTS);
        let widest = [
            Tokens::new(chrome::LIME, 0, &[("WED", true), (" ", false), ("30 SEP 2026", false), ("  [", false), ("MANUAL", true), ("]", false)]),
            Tokens::new(chrome::GRAY, 1, &[("ART -03:00", false)]),
            Tokens::new(chrome::GRAY, 2, &[("// AMERICA/ARGENTINA/BUENOS_AIRES", false)]),
        ];
        for tokens in widest {
            let bounds = tokens.bounds(&font);
            for corner in [bounds.top_left, bounds.bottom_right().unwrap(), Point::new(bounds.top_left.x, bounds.bottom_right().unwrap().y)] {
                let d = corner - CENTER;
                assert!(d.x * d.x + d.y * d.y < 228 * 228, "{:?} at {bounds:?}", tokens.text);
            }
        }
    }

    #[test]
    fn without_a_trusted_reading_for_the_zone_the_plate_holds_only_the_mode() {
        assert!(values(&view(true, true, "Europe/Dublin")).is_empty());
        let trusted = view(false, true, "Europe/Dublin");
        let moved = read(trusted, true, true, "Asia/Kolkata");
        assert!(values(&moved).is_empty());
    }
}
