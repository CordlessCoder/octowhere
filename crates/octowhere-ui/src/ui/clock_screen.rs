//! The clock face: hours on the field, minutes and seconds knocked out of a band across the
//! circle, a status symbol, the date, and a plate naming the zone.
//! `context/clock_face_design/CLOCK-FACE-SPEC.md` is its design.

use core::fmt::Write as _;

use embedded_graphics::{
    prelude::{Point, Size},
    primitives::Rectangle,
};
use heapless::String;

use super::{
    clock::{ClockState, ClockView, DateTime, ZoneMode, ZoneState},
    icon::{self, Glyph, Tile},
    reveal::{Reveal, draw_revealed, revealed_bounds},
};
use crate::chrome::{
    self, Color, CoverageTarget, FontdueRenderer, OnBackground, RgbColorExt as _, FRAKTION,
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
/// `NO DATA`'s ink starts on the digits' ink column, centred on this row.
const NO_DATA_LEFT: i32 = 71;
const NO_DATA_MIDDLE_TWICE: i32 = 515;
/// Every date row line is centred by ink in these rows.
const DATE_ROW: Rectangle = Rectangle::new(Point::new(0, 334), Size::new(466, 17));
const PLATE_TOP: i32 = 360;
const PLATE_HEIGHT: u32 = 20;
const PLATE_BASELINE: i32 = 375;
const PLATE_PADDING: i32 = 6;
const PLATE_GAP: i32 = 4;
const ZONE_BASELINE: i32 = 398;

const GNSS: Glyph = [0b00100, 0b01010, 0b10101, 0b01010, 0b00100];
const RTC: Glyph = [0b11111, 0b10001, 0b10101, 0b10001, 0b11111];
const STOPPED: Glyph = [0b01010, 0b01010, 0b01010, 0b01010, 0b01010];
const NO_ZONE: Glyph = [0b01110, 0b10001, 0b00110, 0b00000, 0b00100];

/// How far each of the face's accents has come in: the ring's fade, 0 to 255; how many rows of
/// the icon's modules show, 0 to 5; and the band label's, the plate's and the zone name's
/// reveals, 0 to 255. The centre content always shows whole.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Accents {
    pub ring: u8,
    pub icon_rows: u8,
    pub label: u8,
    pub plate: u8,
    pub zone: u8,
}

impl Accents {
    pub const FULL: Self = Self {
        ring: u8::MAX,
        icon_rows: 5,
        label: u8::MAX,
        plate: u8::MAX,
        zone: u8::MAX,
    };
    pub const HIDDEN: Self = Self {
        ring: 0,
        icon_rows: 0,
        label: 0,
        plate: 0,
        zone: 0,
    };

    /// Each accent at the lesser of the two.
    #[must_use]
    pub fn min(self, other: Self) -> Self {
        Self {
            ring: self.ring.min(other.ring),
            icon_rows: self.icon_rows.min(other.icon_rows),
            label: self.label.min(other.label),
            plate: self.plate.min(other.plate),
            zone: self.zone.min(other.zone),
        }
    }
}

impl Default for Accents {
    fn default() -> Self {
        Self::FULL
    }
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
            _ => chrome::WHITE,
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
        let ClockView { clock, zone, known } = view;
        let mode = Mode::of(clock, zone);
        let local = clock.local(*zone);
        let tag = match zone.mode {
            ZoneMode::Automatic => "AUTO",
            ZoneMode::Manual => "MANUAL",
        };
        let mut values = heapless::Vec::new();
        // A stopped or unreadable clock has no time to find the offset from, so the plate keeps
        // the one the zone last had.
        let offset = match (mode, local) {
            (Mode::Local { .. }, Some(local)) => Some(local.offset),
            (Mode::Stopped | Mode::NoData, _) => known
                .filter(|known| Some(known.zone) == zone.zone)
                .map(|known| known.offset),
            _ => None,
        };
        match (mode, offset) {
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
    date: Option<(String<16>, Line)>,
    /// The plate, and how many of its cells show.
    plate: (Plate, usize),
    zone: Option<(String<32>, Reveal)>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Line {
    Date,
    Waiting,
    Utc,
}

impl Line {
    fn style(self, font: &FontdueRenderer<'static, Color>) -> FontdueRenderer<'static, Color> {
        match self {
            Self::Date => style(font, chrome::WHITE, 23, FRAKTION),
            Self::Waiting => style(font, chrome::GRAY, 19, FRAKTION),
            Self::Utc => style(font, chrome::GRAY, 23, FRAKTION),
        }
    }
}

const WEEKDAYS: [&str; 7] = ["SUN", "MON", "TUE", "WED", "THU", "FRI", "SAT"];
const MONTHS: [&str; 12] = [
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

fn date(time: &DateTime) -> String<16> {
    let mut text = String::new();
    _ = write!(
        text,
        "{} {:02} {} {:04}",
        WEEKDAYS[usize::from(time.weekday())],
        time.day,
        MONTHS[usize::from(time.month - 1)],
        time.year
    );
    text
}

impl Parts {
    fn of(view: &ClockView, accents: Accents) -> Self {
        let keys = Keys::of(view);
        let ClockView { clock, zone, .. } = view;
        let mode = keys.mode;
        let local = clock.local(*zone).filter(|_| matches!(mode, Mode::Local { .. }));
        let (hours, minutes, seconds) = match (mode, local) {
            (Mode::NoData, _) => (None, None, None),
            (_, Some(local)) => (
                Some(two(local.time.hour)),
                Some(two(local.time.minute)),
                Some(two(local.time.second)),
            ),
            _ => (Some(dashes()), Some(dashes()), None),
        };
        let date = match (mode, local) {
            (_, Some(local)) => Some((date(&local.time), Line::Date)),
            (Mode::Stopped, _) => Some((String::try_from("WAITING FOR GNSS").unwrap(), Line::Waiting)),
            (Mode::NoZone, _) => clock.utc_time().map(|utc| {
                let mut text = String::new();
                _ = write!(text, "UTC {:02}:{:02}", utc.hour, utc.minute);
                (text, Line::Utc)
            }),
            _ => None,
        };
        let ring_color = if mode == Mode::NoData { chrome::RED } else { chrome::GRAY };
        let (glyph, icon_color) = mode.icon();
        let label = mode.label();
        let cells = keys.plate.values.len() + 1;
        let zone_name = keys.zone.map(|name| {
            let mut upper = String::<32>::new();
            for c in name.chars() {
                _ = upper.push(c.to_ascii_uppercase());
            }
            let reveal = Reveal::of(accents.zone, upper.len());
            (upper, reveal)
        });
        Self {
            ring: (accents.ring > 0).then(|| chrome::BLACK.lerp(&ring_color, accents.ring)),
            icon: (glyph, icon_color, accents.icon_rows),
            band: mode.band(),
            label: (label, Reveal::of(accents.label, label.len())),
            hours,
            minutes,
            seconds,
            date,
            plate: (
                keys.plate,
                (0..cells).take_while(|&i| i * 255 < usize::from(accents.plate) * cells).count(),
            ),
            zone: zone_name,
        }
    }
}

fn style(
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

fn zone_style(font: &FontdueRenderer<'static, Color>) -> FontdueRenderer<'static, Color> {
    style(font, chrome::GRAY, 14, FRAKTION)
}

fn no_data_style(font: &FontdueRenderer<'static, Color>) -> FontdueRenderer<'static, Color> {
    style(font, chrome::BLACK, 28, SHAPIRO)
}

/// Where `text`'s pen starts for its advance to centre on `x`.
fn centred_left(style: &FontdueRenderer<'static, Color>, text: &str, x: i32) -> i32 {
    libm::roundf(x as f32 - style.advance(text) / 2.0) as i32
}

/// The baseline that centres `text`'s ink in `rows`.
fn centred_baseline(style: &FontdueRenderer<'static, Color>, text: &str, rows: Rectangle) -> i32 {
    let ink = style.baseline_bounds(text, Point::zero());
    rows.top_left.y + (rows.size.height as i32 - ink.size.height as i32) / 2 - ink.top_left.y
}

fn date_origin(font: &FontdueRenderer<'static, Color>, text: &str, line: Line) -> (FontdueRenderer<'static, Color>, Point) {
    let style = line.style(font);
    let origin = Point::new(
        centred_left(&style, text, CENTER.x),
        centred_baseline(&style, text, DATE_ROW),
    );
    (style, origin)
}

fn no_data_origin(font: &FontdueRenderer<'static, Color>) -> Point {
    let ink = no_data_style(font).baseline_bounds("NO DATA", Point::zero());
    let top = (NO_DATA_MIDDLE_TWICE - ink.size.height as i32) / 2;
    Point::new(NO_DATA_LEFT - ink.top_left.x, top - ink.top_left.y)
}

fn zone_pen(font: &FontdueRenderer<'static, Color>, name: &str) -> Point {
    Point::new(centred_left(&zone_style(font), name, CENTER.x), ZONE_BASELINE)
}

/// Each cell of `plate` with its box and its text's pen.
fn plate_cells<'p>(
    font: &FontdueRenderer<'static, Color>,
    plate: &'p Plate,
) -> impl Iterator<Item = (bool, &'p str, Rectangle)> {
    let advance = style(font, chrome::WHITE, 14, FRAKTION).advance("0");
    let texts = core::iter::once((true, plate.tag)).chain(plate.values.iter().map(|value| (false, value.as_str())));
    let widths = texts.clone().map(move |(_, text)| text.len() as f32 * advance + 2.0 * PLATE_PADDING as f32);
    let total = widths.clone().sum::<f32>() + (PLATE_GAP * plate.values.len() as i32) as f32;
    let mut x = libm::roundf(CENTER.x as f32 - total / 2.0) as i32;
    texts.zip(widths).map(move |((tag, text), width)| {
        let width = libm::roundf(width) as u32;
        let cell = Rectangle::new(Point::new(x, PLATE_TOP), Size::new(width, PLATE_HEIGHT));
        x += width as i32 + PLATE_GAP;
        (tag, text, cell)
    })
}

fn plate_bounds(font: &FontdueRenderer<'static, Color>, plate: &Plate) -> Rectangle {
    plate_cells(font, plate)
        .map(|(_, _, cell)| cell)
        .reduce(|a, b| Rectangle::with_corners(a.top_left, b.bottom_right().unwrap()))
        .unwrap_or(Rectangle::zero())
}

fn draw_plate<D: CoverageTarget<Color = Color>>(
    font: &FontdueRenderer<'static, Color>,
    plate: &Plate,
    shown: usize,
    target: &mut D,
) -> Result<(), D::Error> {
    for (tag, text, cell) in plate_cells(font, plate).take(shown) {
        let pen = Point::new(cell.top_left.x + PLATE_PADDING, PLATE_BASELINE);
        target.fill_solid(&cell, chrome::GRAY)?;
        if tag {
            style(font, chrome::BLACK, 14, FRAKTION_BOLD).draw_on_baseline(
                text,
                pen,
                &mut OnBackground::new(&mut *target, chrome::GRAY),
            )?;
        } else {
            target.fill_solid(&cell.offset(-1), chrome::BLACK)?;
            style(font, chrome::WHITE, 14, FRAKTION).draw_on_baseline(
                text,
                pen,
                &mut OnBackground::new(&mut *target, chrome::BLACK),
            )?;
        }
    }
    Ok(())
}

pub fn draw<D>(
    view: &ClockView,
    accents: Accents,
    font: &FontdueRenderer<'static, Color>,
    target: &mut D,
) -> Result<(), D::Error>
where
    D: CoverageTarget<Color = Color>,
{
    let parts = Parts::of(view, accents);
    if let Some(color) = parts.ring {
        super::smooth::perimeter().draw(&mut OnBackground::new(&mut *target, chrome::BLACK), color);
    }
    {
        let field = &mut OnBackground::new(&mut *target, chrome::BLACK);
        if let Some(hours) = &parts.hours {
            digits(font, chrome::WHITE).draw_on_baseline(hours, HOURS, field)?;
        }
    }
    let (glyph, color, rows) = parts.icon;
    if target.visible(&TILE.bounds()) {
        TILE.draw(glyph, color, rows, target)?;
    }

    if target.visible(&BAND) {
        super::smooth::disc_rows(target, BAND_ROWS, CENTER, BAND_RADIUS, parts.band)?;
    }
    {
        let band = &mut OnBackground::new(&mut *target, parts.band);
        let (label, reveal) = parts.label;
        draw_revealed(&label_style(font), label, LABEL, reveal, band)?;
        if let Some(minutes) = &parts.minutes {
            digits(font, chrome::BLACK).draw_on_baseline(minutes, MINUTES, band)?;
        } else {
            no_data_style(font).draw_on_baseline("NO DATA", no_data_origin(font), band)?;
        }
        if let Some(seconds) = &parts.seconds {
            seconds_style(font).draw_on_baseline(seconds, SECONDS, band)?;
        }
    }

    let field = &mut OnBackground::new(&mut *target, chrome::BLACK);
    if let Some((text, line)) = &parts.date {
        let (style, origin) = date_origin(font, text, *line);
        style.draw_on_baseline(text, origin, field)?;
    }
    let (plate, shown) = &parts.plate;
    draw_plate(font, plate, *shown, target)?;
    if let Some((name, reveal)) = &parts.zone {
        let field = &mut OnBackground::new(&mut *target, chrome::BLACK);
        draw_revealed(&zone_style(font), name, zone_pen(font, name), *reveal, field)?;
    }
    Ok(())
}

fn text(digits: &Option<String<2>>) -> &str {
    digits.as_ref().map_or("", String::as_str)
}

/// Marks in `damage` every pixel that differs between the face drawn for `before` and for
/// `after`.
pub fn damage(
    before: (&ClockView, Accents),
    after: (&ClockView, Accents),
    font: &FontdueRenderer<'static, Color>,
    damage: &mut chrome::Dirty,
) {
    let old = Parts::of(before.0, before.1);
    let new = Parts::of(after.0, after.1);
    if old == new {
        return;
    }
    if old.ring != new.ring || old.band != new.band {
        damage.make_full();
        return;
    }
    if old.hours != new.hours {
        let (was, now) = (text(&old.hours), text(&new.hours));
        digits(font, chrome::WHITE).glyph_damage((was, HOURS), (now, HOURS), damage);
    }
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
    if old.minutes != new.minutes {
        if old.minutes.is_none() || new.minutes.is_none() {
            // `NO DATA` comes and goes only with the band's colour.
            damage.make_full();
            return;
        }
        let (was, now) = (text(&old.minutes), text(&new.minutes));
        digits(font, chrome::BLACK).glyph_damage((was, MINUTES), (now, MINUTES), damage);
    }
    if old.seconds != new.seconds {
        let (was, now) = (text(&old.seconds), text(&new.seconds));
        seconds_style(font).glyph_damage((was, SECONDS), (now, SECONDS), damage);
    }
    if old.date != new.date {
        for (text, line) in [&old.date, &new.date].into_iter().flatten() {
            let (style, origin) = date_origin(font, text, *line);
            damage.add(style.baseline_bounds(text, origin));
        }
    }
    if old.plate != new.plate {
        damage.add(plate_bounds(font, &old.plate.0));
        damage.add(plate_bounds(font, &new.plate.0));
    }
    if old.zone != new.zone {
        for (name, _) in [&old.zone, &new.zone].into_iter().flatten() {
            damage.add(revealed_bounds(&zone_style(font), name, zone_pen(font, name)));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::clock::{ClockState, ZoneState};

    fn view(stopped: bool, readable: bool, zone: &str) -> ClockView {
        ClockView {
            clock: ClockState {
                utc: readable.then_some(
                    DateTime { year: 2026, month: 9, day: 24, hour: 12, ..DateTime::default() }.to_unix(),
                ),
                set_from_gnss: false,
                stopped,
            },
            zone: ZoneState {
                mode: ZoneMode::Manual,
                zone: octowhere_tz::DATABASE.find(zone).map(|zone| zone.id),
            },
            known: None,
        }
    }

    fn values(view: &ClockView) -> heapless::Vec<String<10>, 2> {
        Keys::of(view).plate.values
    }

    #[test]
    fn a_stopped_or_unread_clock_keeps_the_offset_the_zone_last_had() {
        let trusted = view(false, true, "Europe/Dublin").remembering();
        assert_eq!(values(&trusted), ["IST", "+01:00"]);
        for (stopped, readable) in [(true, true), (false, false)] {
            let later = ClockView { known: trusted.known, ..view(stopped, readable, "Europe/Dublin") }.remembering();
            assert_eq!(values(&later), ["IST", "+01:00"], "stopped {stopped}, readable {readable}");
        }
    }

    #[test]
    fn without_a_trusted_reading_for_the_zone_the_plate_holds_only_the_mode() {
        assert!(values(&view(true, true, "Europe/Dublin").remembering()).is_empty());
        let trusted = view(false, true, "Europe/Dublin").remembering();
        let moved = ClockView { known: trusted.known, ..view(true, true, "Asia/Kolkata") }.remembering();
        assert!(values(&moved).is_empty());
    }
}
