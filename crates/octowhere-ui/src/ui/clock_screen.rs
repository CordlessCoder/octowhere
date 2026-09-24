//! The clock face: hours on the field, minutes and seconds knocked out of a band across the
//! circle, a status symbol, the date, and a plate naming the zone.
//! `context/clock_face_design/CLOCK-FACE-SPEC.md` is its design.

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
const DATE_INK: Rectangle = Rectangle::new(Point::new(0, 331), Size::new(466, 25));
const PLATE_INK: Rectangle = Rectangle::new(Point::new(0, 360), Size::new(466, 20));
const ZONE_INK: Rectangle = Rectangle::new(Point::new(0, 386), Size::new(466, 16));
const MARK_INK: Rectangle = Rectangle::new(Point::new(363, 0), Size::new(25, 466));
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

/// How far each of the face's accents has come in: the ring's fade, 0 to 255; how many rows of
/// the icon's modules show, 0 to 5; and the band label's, the plate's, the zone name's and the
/// wordmark's reveals, 0 to 255. Also the reveals of the time, across hours, minutes and
/// seconds, and of the date line, which run only when a fix or a zone change replaces the time.
/// The time and date are centre content, so a page's entry and exit leave them whole.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Accents {
    pub ring: u8,
    pub icon_rows: u8,
    pub label: u8,
    pub plate: u8,
    pub zone: u8,
    pub mark: u8,
    pub time: u8,
    pub date: u8,
}

impl Accents {
    pub const FULL: Self = Self {
        ring: u8::MAX,
        icon_rows: 5,
        label: u8::MAX,
        plate: u8::MAX,
        zone: u8::MAX,
        mark: u8::MAX,
        time: u8::MAX,
        date: u8::MAX,
    };
    pub const HIDDEN: Self = Self {
        ring: 0,
        icon_rows: 0,
        label: 0,
        plate: 0,
        zone: 0,
        mark: 0,
        time: u8::MAX,
        date: u8::MAX,
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
            mark: self.mark.min(other.mark),
            time: self.time.min(other.time),
            date: self.date.min(other.date),
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
    date: Option<(String<16>, Line, Reveal)>,
    /// The plate, and how many of its cells show.
    plate: (Plate, usize),
    zone: Option<(String<32>, Reveal)>,
    mark: Reveal,
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

impl Parts {
    fn of(view: &ClockView, accents: Accents) -> Self {
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
            time: Reveal::of(accents.time, TIME_CELLS),
            date: date.map(|(text, line)| {
                let reveal = Reveal::of(accents.date, text.len());
                (text, line, reveal)
            }),
            plate: (
                keys.plate,
                (0..cells).take_while(|&i| i * 255 < usize::from(accents.plate) * cells).count(),
            ),
            zone: zone_name,
            mark: Reveal::of(accents.mark, MARK.len()),
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

fn mark_style(font: &FontdueRenderer<'static, Color>, color: Color) -> FontdueRenderer<'static, Color> {
    style(font, color, MARK_PX, SHAPIRO)
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

/// The wordmark, inverting what is under it: white on the field and black on the band.
fn draw_mark<D: CoverageTarget<Color = Color>>(
    font: &FontdueRenderer<'static, Color>,
    reveal: Reveal,
    band: Color,
    target: &mut D,
) -> Result<(), D::Error> {
    let layout = MarkLayout::of(font);
    let whole = layout.span(0, reveal.cells());
    let text = &MARK[..reveal.glyphs()];
    let block = (reveal.cells() > reveal.glyphs()).then(|| layout.cell(reveal.glyphs()));
    for (rows, color, background) in [
        (0..BAND_ROWS.start, chrome::WHITE, chrome::BLACK),
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
    }
    if target.visible(&MARK_INK) {
        draw_mark(font, parts.mark, parts.band, target)?;
    }

    if let Some((text, line, reveal)) = parts.date.as_ref().filter(|_| target.visible(&DATE_INK)) {
        let field = &mut OnBackground::new(&mut *target, chrome::BLACK);
        let (style, origin) = date_origin(font, text, *line);
        draw_revealed(&style, text, origin, *reveal, field)?;
    }
    if target.visible(&PLATE_INK) {
        let (plate, shown) = &parts.plate;
        draw_plate(font, plate, *shown, target)?;
    }
    if let Some((name, reveal)) = parts.zone.as_ref().filter(|_| target.visible(&ZONE_INK)) {
        let field = &mut OnBackground::new(&mut *target, chrome::BLACK);
        draw_revealed(&zone_style(font), name, zone_pen(font, name), *reveal, field)?;
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

/// Marks in `damage` every pixel that differs between the face drawn for `before` and for
/// `after`.
pub fn damage(
    before: (&ClockView, Accents),
    after: (&ClockView, Accents),
    font: &FontdueRenderer<'static, Color>,
    damage: &mut chrome::Dirty,
) {
    if before == after {
        return;
    }
    let old = Parts::of(before.0, before.1);
    let new = Parts::of(after.0, after.1);
    if old == new {
        return;
    }
    if old.ring != new.ring {
        super::smooth::perimeter().damage(damage);
    }
    if old.band != new.band {
        // The band's colour also shows through the mark and the text on it.
        damage.add(BAND);
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
    if old.date != new.date {
        for (text, line, _) in [&old.date, &new.date].into_iter().flatten() {
            let (style, origin) = date_origin(font, text, *line);
            damage.add(revealed_bounds(&style, text, origin));
        }
    }
    if old.plate != new.plate {
        damage.add(plate_bounds(font, &old.plate.0));
        damage.add(plate_bounds(font, &new.plate.0));
    }
    if old.mark != new.mark {
        let from = old.mark.glyphs().min(new.mark.glyphs());
        let to = old.mark.cells().max(new.mark.cells());
        damage.add(MarkLayout::of(font).span(from, to));
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
        const NAMES: &str = "0123456789-+:ABCDEFGHIJKLMNOPQRSTUVWXYZ_/";
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
        for line in [Line::Date, Line::Waiting, Line::Utc] {
            check("date", DATE_INK, TEXT, 16, &|text| date_origin(&font, text, line));
        }
        check("zone", ZONE_INK, NAMES, 32, &|text| (zone_style(&font), zone_pen(&font, text)));
        let no_data = no_data_style(&font).baseline_bounds("NO DATA", no_data_origin(&font));
        assert!(inside(NO_DATA_INK, no_data), "NO DATA at {no_data:?}");
        let mark = MarkLayout::of(&font).span(0, MARK.len());
        assert!(inside(MARK_INK, mark), "the mark at {mark:?}");
        for c in TEXT.chars() {
            let value: String<10> = core::iter::repeat_n(c, 10).collect();
            let plate = Plate { tag: "MANUAL", values: [value.clone(), value].into_iter().collect() };
            for (tag, text, cell) in plate_cells(&font, &plate) {
                let size = if tag { FRAKTION_BOLD } else { FRAKTION };
                let pen = Point::new(cell.top_left.x + PLATE_PADDING, PLATE_BASELINE);
                let ink = style(&font, chrome::WHITE, 14, size).baseline_bounds(text, pen);
                assert!(inside(PLATE_INK, cell) && inside(PLATE_INK, ink), "plate {text:?}");
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
