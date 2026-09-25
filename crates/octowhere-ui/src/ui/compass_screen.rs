//! The compass screen: a fixed slab carrying the heading or the calibration progress, a status
//! icon, and a dial that turns with the heading. `context/SCREEN-DESIGN-BRIEF.md` describes it
//! for design work.

use core::fmt::Write as _;

use embedded_graphics::{
    prelude::{Point, Size},
    primitives::Rectangle,
};
use embedded_layout::align::{horizontal, vertical};

use super::{
    compass::CompassView,
    icon::{self, Glyph, Tile},
    reveal::{Reveal, draw_revealed, revealed_bounds},
};
use crate::chrome::{
    self, Color, CoverageTarget, FontdueRenderer, OnBackground, RgbColorExt as _, Window,
    FRAKTION, FRAKTION_BOLD, SHAPIRO,
};

pub const CENTER: Point = Point::new(233, 233);
// The dial's damage is worked out for half of it and turned about the panel's centre for the rest.
const _: () = assert!(
    CENTER.x * 2 == crate::board::LCD_WIDTH as i32 && CENTER.y * 2 == crate::board::LCD_HEIGHT as i32
);
/// Every state shares the slab, so a state change does not move it. It ends 16 px above the tilt
/// line's ink. Every state fills it, which the clear before the screen relies on.
pub(crate) const SLAB: Rectangle = Rectangle::new(Point::new(135, 204), Size::new(196, 91));
/// Where a state's line of text sits between the caption and the slab, centred by its ink. It is
/// as tall as the tallest such line, `INTERFERENCE`.
const STATUS_BAND: Rectangle = Rectangle::new(Point::new(0, 179), Size::new(466, 18));
/// The caption's ink top is row 160, and its antialiased ink reaches its baseline row.
const CAPTION_BASELINE: i32 = 171;
/// The largest icon whose corners stay inside the letters' orbit at every heading.
const ICON: Tile = Tile {
    corner: Point::new(200, 87),
    module: 10,
    padding: 8,
};
/// The readout's first pen position and its suffix's, both on their baselines. Heading and
/// calibration share them, so completing a calibration does not move the digits.
const READOUT: Point = Point::new(143, SLAB.top_left.y + 79);
const SUFFIX: Point = Point::new(298, SLAB.top_left.y + 43);
const TILT_BASELINE: i32 = 327;
const LETTER_RADIUS: f32 = 172.0;

/// What the screen shows, in the order that decides between them.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Mode {
    NoData,
    /// Degrees, 0 to 359.
    Interference(u16),
    Heading(u16),
    Calibrating(u8),
    /// Calibrated, but the top edge points too close to vertical for a heading.
    TopEdgeUp,
}

impl Mode {
    #[must_use]
    pub fn of(view: &CompassView) -> Self {
        match view.heading_decidegrees {
            _ if !view.live => Self::NoData,
            Some(decidegrees) if view.disturbed => Self::Interference(degrees(decidegrees)),
            Some(decidegrees) => Self::Heading(degrees(decidegrees)),
            None if view.calibration_percent < 100 => Self::Calibrating(view.calibration_percent),
            None => Self::TopEdgeUp,
        }
    }

    /// Whether the two are the same state, whatever their values.
    #[must_use]
    pub fn same_state(self, other: Self) -> bool {
        core::mem::discriminant(&self) == core::mem::discriminant(&other)
    }

    /// The heading, in the two modes that turn the dial to it.
    #[must_use]
    pub fn heading(self) -> Option<u16> {
        match self {
            Self::Heading(degrees) | Self::Interference(degrees) => Some(degrees),
            _ => None,
        }
    }

    fn icon(self) -> &'static Glyph {
        match self {
            Self::NoData => &icon::NO_DATA,
            Self::Interference(_) => &INTERFERENCE,
            Self::Heading(_) => &ARROW,
            Self::Calibrating(_) => &OPEN_LOOP,
            Self::TopEdgeUp => &TOP_BAR,
        }
    }

    /// The slab's colour, which also colours the icon.
    fn color(self) -> Color {
        match self {
            Self::NoData => chrome::RED,
            Self::Interference(_) | Self::Calibrating(_) => chrome::ORANGE,
            Self::Heading(_) | Self::TopEdgeUp => chrome::WHITE,
        }
    }

    /// The status icon's fill: the slab's colour, except that a heading marks its icon blue.
    fn icon_color(self) -> Color {
        match self {
            Self::Heading(_) => chrome::BLUE,
            _ => self.color(),
        }
    }
}

/// Whole degrees, truncated so that 359.9° reads 359.
#[must_use]
pub fn degrees(decidegrees: u16) -> u16 {
    decidegrees / 10 % 360
}

/// How far each of the screen's accents has come in: the ring's fade, how many rows of the
/// icon's modules show (0 to 5), and the caption's reveal and the dial's sweep, each 0 to 255.
/// The stage animates these, and the change of state running.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Accents {
    pub ring: u8,
    pub icon_rows: u8,
    pub caption: u8,
    pub dial: u8,
    pub change: Change,
}

impl Accents {
    pub const FULL: Self = Self {
        ring: u8::MAX,
        icon_rows: 5,
        caption: u8::MAX,
        dial: u8::MAX,
        change: Change::NONE,
    };
    pub const HIDDEN: Self = Self {
        ring: 0,
        icon_rows: 0,
        caption: 0,
        dial: 0,
        change: Change::NONE,
    };

    /// Each accent at the lesser of the two, with `self`'s change of state.
    #[must_use]
    pub fn min(self, other: Self) -> Self {
        Self {
            ring: self.ring.min(other.ring),
            icon_rows: self.icon_rows.min(other.icon_rows),
            caption: self.caption.min(other.caption),
            dial: self.dial.min(other.dial),
            change: self.change,
        }
    }
}

/// The slab wipe's steps: how many rows of the slab, from its top, each step gives the new
/// colour.
const WIPE_ROWS: [u32; 4] = [23, 46, 68, 91];
const WIPE_STEP: u64 = 20_000;
/// A state line that goes untypes over this, in µs, and one that replaces it types over the
/// next [`LINE_TYPE`].
const LINE_UNTYPE: u64 = 60_000;
const LINE_TYPE: u64 = 120_000;

/// The part of a change between states that the slab and the state line show: the slab wiping
/// to its new colour, and the old line untyping while the new one types in.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Change {
    /// The state left, while the change runs.
    from: Option<Mode>,
    /// How many of the wipe's steps have run, 1 to 4.
    wipe: u8,
    /// What is left of the old state line, and how far the new one has typed, 0 to 255.
    old_line: u8,
    new_line: u8,
}

impl Change {
    pub const NONE: Self = Self {
        from: None,
        wipe: WIPE_ROWS.len() as u8,
        old_line: 0,
        new_line: u8::MAX,
    };

    /// How far a change from `from` to `to` has come `elapsed` µs after it started. A change
    /// into or out of NO DATA shows at once, as a fault does, and so does one that leaves the
    /// slab's colour and the state line as they were.
    #[must_use]
    pub fn at(from: Mode, to: Mode, elapsed: u64) -> Self {
        if from == Mode::NoData || to == Mode::NoData {
            return Self::NONE;
        }
        let wipe = if from.color() == to.color() {
            WIPE_ROWS.len() as u8
        } else {
            (elapsed / WIPE_STEP + 1).min(WIPE_ROWS.len() as u64) as u8
        };
        let over = |start: u64, duration: u64| {
            (elapsed.saturating_sub(start).min(duration) * 255 / duration) as u8
        };
        let (old_line, new_line) = match (status(from), status(to)) {
            (old, new) if old == new => (0, u8::MAX),
            (Some(_), Some(_)) => (u8::MAX - over(0, LINE_UNTYPE), over(LINE_UNTYPE, LINE_TYPE)),
            (Some(_), None) => (u8::MAX - over(0, LINE_UNTYPE), u8::MAX),
            (None, _) => (0, over(0, LINE_TYPE)),
        };
        let change = Self {
            from: Some(from),
            wipe,
            old_line,
            new_line,
        };
        let settled = Self { from: None, ..change } == Self::NONE;
        if settled { Self::NONE } else { change }
    }

    /// Whether it has finished, so the screen shows the new state whole.
    #[must_use]
    pub fn done(self) -> bool {
        self == Self::NONE
    }
}

impl Default for Change {
    fn default() -> Self {
        Self::NONE
    }
}

impl Default for Accents {
    fn default() -> Self {
        Self::FULL
    }
}

const ARROW: Glyph = [0b00100, 0b01110, 0b10101, 0b00100, 0b00100];
const OPEN_LOOP: Glyph = [0b01110, 0b10001, 0b10000, 0b10001, 0b01110];
const INTERFERENCE: Glyph = [0b10101, 0b01110, 0b11011, 0b01110, 0b10101];
const TOP_BAR: Glyph = [0b11111, 0b00100, 0b00100, 0b00100, 0b00100];

/// The dial's marks sit every 10°, so the letters share bearings with ticks.
const MARKS: u8 = 36;

/// How many of the dial's bearings, from N clockwise, a sweep at `progress` has passed: every
/// bearing below `360° · progress / 255`.
fn swept(progress: u8) -> u8 {
    (0..MARKS)
        .take_while(|&mark| u32::from(mark) * 10 * 255 < u32::from(progress) * 360)
        .count() as u8
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

/// Draws `text` centred on `x` along the baseline at `baseline`. It skips the text outright when
/// no part of its line can land.
fn centred<D: CoverageTarget<Color = Color>>(
    style: &FontdueRenderer<'static, Color>,
    text: &str,
    x: i32,
    baseline: i32,
    target: &mut D,
) -> Result<(), D::Error> {
    let advance = style.advance(text);
    let left = libm::roundf(x as f32 - advance / 2.0) as i32;
    // Wide enough for any overhang, and from above a capital to below a descender.
    let size = style.font_size as i32;
    let line = Rectangle::new(
        Point::new(left - size / 2, baseline - size - 2),
        Size::new(advance as u32 + size as u32 + 2, (size * 2) as u32),
    );
    if !target.visible(&line) {
        return Ok(());
    }
    style.draw_on_baseline(text, Point::new(left, baseline), target)
}

/// What each part of the screen shows. Two frames that agree on a part draw it identically.
#[derive(Clone, Debug, Eq, PartialEq)]
struct Parts {
    ring: Option<Color>,
    /// The heading the dial turns to, and how many of its bearings the sweep has passed.
    dial: Option<(u16, u8)>,
    /// The glyph, its colour, and how many rows of its modules show.
    icon: (&'static Glyph, Color, u8),
    caption: (&'static str, Color, Reveal),
    slab: Slab,
    readout: Readout,
    /// The line leaving and the line arriving, each with how much of it shows.
    status: [Option<(Status, Reveal)>; 2],
    tilt: Option<heapless::String<24>>,
}

/// The slab's colour, or its two while a wipe runs: the new one down to `split` rows from the
/// slab's top, and the old one below.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Slab {
    new: Color,
    old: Color,
    split: u32,
}

impl Slab {
    fn whole(color: Color) -> Self {
        Self {
            new: color,
            old: color,
            split: SLAB.size.height,
        }
    }

    /// The two parts of the slab and their colours, the second empty when it is one colour.
    fn parts(self) -> [(Rectangle, Color); 2] {
        let top = Rectangle::new(SLAB.top_left, Size::new(SLAB.size.width, self.split));
        let bottom = Rectangle::new(
            SLAB.top_left + Point::new(0, self.split as i32),
            Size::new(SLAB.size.width, SLAB.size.height - self.split),
        );
        [(top, self.new), (bottom, self.old)]
    }

    /// The colour of the slab's row `y`.
    fn row(self, y: i32) -> Color {
        if y < SLAB.top_left.y + self.split as i32 { self.new } else { self.old }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Readout {
    NoData,
    Dashes,
    Digits(heapless::String<4>, &'static str),
}

impl Parts {
    fn of(view: &CompassView, accents: Accents) -> Self {
        let mode = Mode::of(view);
        let ring_color = if mode == Mode::NoData { chrome::RED } else { chrome::GRAY };
        let readout = match mode {
            Mode::NoData => Readout::NoData,
            Mode::TopEdgeUp => Readout::Dashes,
            Mode::Heading(value) | Mode::Interference(value) => {
                Readout::Digits(three_digits(value), "\u{b0}")
            }
            Mode::Calibrating(percent) => Readout::Digits(three_digits(percent.into()), "%"),
        };
        let tilt = (mode != Mode::NoData).then(|| {
            let mut tilt = heapless::String::new();
            _ = write!(tilt, "P {:+03}  R {:+03}", view.pitch_deg, view.roll_deg);
            tilt
        });
        let (caption, caption_color) = caption(mode);
        let marks = swept(accents.dial);
        let change = accents.change;
        let from = change.from.unwrap_or(mode);
        let slab = if change.wipe as usize >= WIPE_ROWS.len() {
            Slab::whole(mode.color())
        } else {
            Slab {
                new: mode.color(),
                old: from.color(),
                split: WIPE_ROWS[usize::from(change.wipe) - 1],
            }
        };
        let line = |status: Option<Status>, progress: u8| {
            status
                .map(|status| (status, Reveal::of(progress, status.text.len())))
                .filter(|(_, reveal)| reveal.cells() > 0)
        };
        Self {
            ring: (accents.ring > 0).then(|| chrome::BLACK.lerp(&ring_color, accents.ring)),
            dial: mode.heading().filter(|_| marks > 0).map(|heading| (heading, marks)),
            icon: (mode.icon(), mode.icon_color(), accents.icon_rows),
            caption: (caption, caption_color, Reveal::of(accents.caption, caption.len())),
            slab,
            readout,
            status: [
                line(status(from), change.old_line).filter(|_| change.from.is_some()),
                line(status(mode), change.new_line),
            ],
            tilt,
        }
    }
}

/// The caption over the state line.
#[must_use]
pub fn caption(mode: Mode) -> (&'static str, Color) {
    match mode {
        Mode::NoData => ("COMPASS", chrome::GRAY),
        Mode::Calibrating(_) => ("CALIBRATION", chrome::ORANGE),
        _ => ("MAGNETIC", chrome::GRAY),
    }
}

fn three_digits(value: u16) -> heapless::String<4> {
    let mut digits = heapless::String::new();
    _ = write!(digits, "{value:03}");
    digits
}

fn caption_style(font: &FontdueRenderer<'static, Color>, color: Color) -> FontdueRenderer<'static, Color> {
    style(font, color, 16, FRAKTION_BOLD)
}

fn tilt_style(font: &FontdueRenderer<'static, Color>) -> FontdueRenderer<'static, Color> {
    style(font, chrome::GRAY, 23, FRAKTION)
}

fn letter_style(font: &FontdueRenderer<'static, Color>, color: Color) -> FontdueRenderer<'static, Color> {
    style(font, color, 40, SHAPIRO)
}

fn numerals(font: &FontdueRenderer<'static, Color>) -> FontdueRenderer<'static, Color> {
    style(font, chrome::BLACK, 86, FRAKTION_BOLD)
}

fn suffix(font: &FontdueRenderer<'static, Color>) -> FontdueRenderer<'static, Color> {
    style(font, chrome::BLACK, 40, FRAKTION_BOLD)
}

pub fn draw<D>(
    view: &CompassView,
    accents: Accents,
    font: &FontdueRenderer<'static, Color>,
    target: &mut D,
) -> Result<(), D::Error>
where
    D: CoverageTarget<Color = Color>,
{
    let parts = Parts::of(view, accents);
    {
        // The dial lands on the cleared field, and its parts do not overlap.
        let field = &mut OnBackground::new(&mut *target, chrome::BLACK);
        if let Some(color) = parts.ring {
            super::smooth::perimeter().draw(field, color);
        }
        if let Some((heading, marks)) = parts.dial {
            draw_dial(heading, marks, font, field)?;
        }
    }
    let (glyph, color, rows) = parts.icon;
    if target.visible(&ICON.bounds()) {
        ICON.draw(glyph, color, rows, target)?;
    }

    let field = &mut OnBackground::new(&mut *target, chrome::BLACK);
    let (caption, caption_color, reveal) = parts.caption;
    let style = caption_style(font, caption_color);
    draw_revealed(&style, caption, caption_pen(&style, caption), reveal, field)?;

    for (part, color) in parts.slab.parts() {
        if part.is_zero_sized() || !target.visible(&part) {
            continue;
        }
        target.fill_solid(&part, color)?;
        let slab = &mut Window::new(&mut *target, Point::zero(), part);
        draw_readout(&parts.readout, font, &mut OnBackground::new(slab, color))?;
    }

    let field = &mut OnBackground::new(&mut *target, chrome::BLACK);
    for (status, reveal) in parts.status.into_iter().flatten() {
        let (style, baseline) = status.style(font);
        draw_revealed(&style, status.text, status.pen(&style, baseline), reveal, field)?;
    }
    if let Some(tilt) = &parts.tilt {
        centred(&tilt_style(font), tilt, CENTER.x, TILT_BASELINE, field)?;
    }
    Ok(())
}

fn caption_pen(style: &FontdueRenderer<'static, Color>, caption: &str) -> Point {
    Point::new(centred_left(style, caption, CENTER.x), CAPTION_BASELINE)
}

/// Marks in `damage` every pixel that differs between the screen drawn for `before` and for
/// `after`: the old and new places of each part that changed.
pub fn damage(
    before: (&CompassView, Accents),
    after: (&CompassView, Accents),
    font: &FontdueRenderer<'static, Color>,
    footprint: &mut DialFootprint,
    damage: &mut chrome::Dirty,
) {
    let (old, new) = (Parts::of(before.0, before.1), Parts::of(after.0, after.1));
    if old == new {
        return;
    }
    if old.ring != new.ring {
        super::smooth::perimeter().damage(damage);
    }
    if old.dial != new.dial {
        // The old place first: the last step worked it out as its new one.
        for dial in [old.dial, new.dial].into_iter().flatten() {
            footprint.mark(dial, font, damage);
        }
    }
    if old.icon != new.icon {
        let ((old_glyph, old_color, old_rows), (new_glyph, new_color, new_rows)) = (old.icon, new.icon);
        if (old_glyph, old_color) == (new_glyph, new_color) {
            damage.add(ICON.rows(old_rows.min(new_rows), old_rows.max(new_rows)));
        } else {
            damage.add(ICON.bounds());
        }
    }
    if old.caption != new.caption {
        for (text, color, _) in [old.caption, new.caption] {
            let style = caption_style(font, color);
            damage.add(revealed_bounds(&style, text, caption_pen(&style, text)));
        }
    }
    if old.slab != new.slab {
        // Whole rows of the slab whose colour changed, which the readout is redrawn across.
        let mut rows = SLAB.rows().filter(|&y| old.slab.row(y) != new.slab.row(y));
        if let (Some(first), Some(last)) = (rows.clone().next(), rows.next_back()) {
            damage.add(Rectangle::with_corners(
                Point::new(SLAB.top_left.x, first),
                Point::new(SLAB.top_left.x + SLAB.size.width as i32 - 1, last),
            ));
        }
    }
    if old.readout != new.readout {
        readout_damage(&old.readout, &new.readout, font, damage);
    }
    if old.status != new.status {
        for (status, _) in old.status.into_iter().chain(new.status).flatten() {
            let (style, baseline) = status.style(font);
            damage.add(revealed_bounds(&style, status.text, status.pen(&style, baseline)));
        }
    }
    if old.tilt != new.tilt {
        let style = tilt_style(font);
        match (&old.tilt, &new.tilt) {
            (Some(was), Some(now)) => style.glyph_damage(
                (was, Point::new(centred_left(&style, was, CENTER.x), TILT_BASELINE)),
                (now, Point::new(centred_left(&style, now, CENTER.x), TILT_BASELINE)),
                damage,
            ),
            (was, now) => {
                for tilt in [was, now].into_iter().flatten() {
                    damage.add(centred_bounds(&style, tilt, CENTER.x, TILT_BASELINE));
                }
            }
        }
    }
}

/// Where the dial last drew, kept so that a turn marks the old place without working it out
/// again.
pub struct DialFootprint {
    /// The heading.
    key: Option<u16>,
    /// Half the dial's pixels. The other half is this turned by half a turn.
    half: alloc::boxed::Box<chrome::Dirty>,
}

impl Default for DialFootprint {
    fn default() -> Self {
        Self {
            key: None,
            half: alloc::boxed::Box::new(chrome::Dirty::new()),
        }
    }
}

impl DialFootprint {
    /// Marks the pixels a whole dial turned to `heading` covers, which a partly swept one stays
    /// inside.
    fn mark(&mut self, (heading, _): (u16, u8), font: &FontdueRenderer<'static, Color>, damage: &mut chrome::Dirty) {
        if self.key != Some(heading) {
            self.half.clear();
            half_dial(heading, font, &mut self.half);
            self.key = Some(heading);
        }
        damage.extend(&self.half);
        damage.extend_reflected(&self.half);
    }
}

/// The pixels the first two quarters of a dial turned to `heading` cover, taking each letter as
/// the larger of it and the one opposite.
fn half_dial(heading: u16, font: &FontdueRenderer<'static, Color>, half: &mut chrome::Dirty) {
    let turn = f32::from(heading);
    let (cx, cy) = (CENTER.x as f32, CENTER.y as f32);
    for tick in 0..9 {
        let (mut corners, _, _) = tick_shape(tick, turn);
        for _ in 0..2 {
            half.add_polygon(&corners, 1);
            // A quarter turn clockwise about the centre, y down.
            corners = corners.map(|(x, y)| (cx - (y - cy), cy + (x - cx)));
        }
    }
    let style = letter_style(font, chrome::WHITE);
    for quarter in 0..2 {
        let (at, _, _) = letter_place(quarter, turn);
        let reach = style
            .rotated_reach(LETTERS[quarter])
            .max(style.rotated_reach(LETTERS[quarter + 2]));
        half.add_disc((at.x as f32, at.y as f32), reach);
    }
}

/// The pixels that differ between two readouts on the same slab.
fn readout_damage(
    old: &Readout,
    new: &Readout,
    font: &FontdueRenderer<'static, Color>,
    damage: &mut chrome::Dirty,
) {
    let (Readout::Digits(old_digits, old_unit), Readout::Digits(new_digits, new_unit)) = (old, new)
    else {
        damage.add(SLAB);
        return;
    };
    suffix(font).glyph_damage((old_unit, SUFFIX), (new_unit, SUFFIX), damage);
    numerals(font).glyph_damage((old_digits, READOUT), (new_digits, READOUT), damage);
}

fn centred_left(style: &FontdueRenderer<'static, Color>, text: &str, x: i32) -> i32 {
    libm::roundf(x as f32 - style.advance(text) / 2.0) as i32
}

/// The ink [`centred`] covers.
fn centred_bounds(style: &FontdueRenderer<'static, Color>, text: &str, x: i32, baseline: i32) -> Rectangle {
    style.baseline_bounds(text, Point::new(centred_left(style, text, x), baseline))
}

/// The line of text a state puts above the slab.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Status {
    text: &'static str,
    color: Color,
    size: u32,
    index: usize,
}

fn status(mode: Mode) -> Option<Status> {
    let (text, color, size, index) = match mode {
        Mode::Interference(_) => ("INTERFERENCE", chrome::ORANGE, 24, FRAKTION_BOLD),
        Mode::Calibrating(_) => ("TURN ALL WAYS", chrome::GRAY, 19, FRAKTION),
        Mode::TopEdgeUp => ("TOP EDGE UP", chrome::GRAY, 20, FRAKTION),
        Mode::Heading(_) | Mode::NoData => return None,
    };
    Some(Status {
        text,
        color,
        size,
        index,
    })
}

impl Status {
    /// Its style, and the baseline that centres its ink in the status band.
    fn style(self, font: &FontdueRenderer<'static, Color>) -> (FontdueRenderer<'static, Color>, i32) {
        let style = style(font, self.color, self.size, self.index);
        let ink = style.baseline_bounds(self.text, Point::zero());
        let baseline = STATUS_BAND.top_left.y
            + (STATUS_BAND.size.height as i32 - ink.size.height as i32) / 2
            - ink.top_left.y;
        (style, baseline)
    }

    /// Where its pen starts on `baseline`, centred on the panel.
    fn pen(self, style: &FontdueRenderer<'static, Color>, baseline: i32) -> Point {
        Point::new(centred_left(style, self.text, CENTER.x), baseline)
    }
}

const LETTERS: [&str; 4] = ["N", "E", "S", "W"];

/// The corners of tick `tick` of the first quarter turned by `turn` degrees, whether it is
/// major, and its colour. Ticks nine apart are a quarter turn apart and all major or all minor
/// alike.
fn tick_shape(tick: usize, turn: f32) -> ([(f32, f32); 4], bool, Color) {
    let center = (CENTER.x as f32, CENTER.y as f32);
    let (sin, cos) = libm::sincosf((tick as f32 * 10.0 - turn).to_radians());
    let major = tick.is_multiple_of(3);
    let (width, inner, color) = if major {
        (4.0, 202.0, chrome::WHITE)
    } else {
        (2.0, 216.0, chrome::GRAY)
    };
    let outer = 226.0;
    let point = |along: f32, across: f32| {
        (
            center.0 + sin * along + cos * across,
            center.1 - cos * along + sin * across,
        )
    };
    (
        [
            point(outer, -width / 2.0),
            point(outer, width / 2.0),
            point(inner, width / 2.0),
            point(inner, -width / 2.0),
        ],
        major,
        color,
    )
}

/// Where letter `quarter` centres with the dial turned by `turn` degrees, and the cosine and sine
/// it is turned by, so its top faces outward all the way round.
fn letter_place(quarter: usize, turn: f32) -> (Point, f32, f32) {
    let (sin, cos) = libm::sincosf((quarter as f32 * 90.0 - turn).to_radians());
    let at = Point::new(
        libm::roundf(CENTER.x as f32 + sin * LETTER_RADIUS) as i32,
        libm::roundf(CENTER.y as f32 - cos * LETTER_RADIUS) as i32,
    );
    (at, cos, sin)
}

/// The ticks and the cardinal letters at the first `marks` bearings from N clockwise, turned so
/// the top edge reads `heading`.
fn draw_dial<D>(
    heading: u16,
    marks: u8,
    font: &FontdueRenderer<'static, Color>,
    field: &mut D,
) -> Result<(), D::Error>
where
    D: CoverageTarget<Color = Color>,
{
    let turn = f32::from(heading);
    // One quarter of the ticks is filled, and each is drawn at the quarter turns swept past it.
    let mut raster = fontdue::raster::Raster::empty();
    let mut coverage = alloc::vec::Vec::new();
    let shown = |mark: usize| mark < usize::from(marks);
    for tick in 0..9 {
        let quarters = (0..4).filter(|quarter| shown(quarter * 9 + tick)).fold(0, |bits, quarter| bits | 1 << quarter);
        if quarters == 0 {
            continue;
        }
        let (corners, _, color) = tick_shape(tick, turn);
        super::smooth::polygon_quarters(field, &mut raster, &mut coverage, &corners, CENTER, quarters, color);
    }
    for (quarter, letter) in LETTERS.into_iter().enumerate().filter(|&(quarter, _)| shown(quarter * 9)) {
        let (at, cos, sin) = letter_place(quarter, turn);
        let color = if quarter == 0 { chrome::ORANGE } else { chrome::WHITE };
        letter_style(font, color).draw_rotated(letter, at, cos, sin, field)?;
    }
    Ok(())
}

/// The slab's black knockout content, over a target that knows the slab's colour.
/// The readout's stand-in while there is no heading.
const DASHES: &str = "---";

/// Where the dashes' pen starts so that their ink, taken as one group, is centred in the slab.
fn dashes_origin(numerals: &FontdueRenderer<'static, Color>) -> Point {
    SLAB.center() - numerals.baseline_bounds(DASHES, Point::zero()).center()
}

fn draw_readout<D: CoverageTarget<Color = Color>>(
    readout: &Readout,
    font: &FontdueRenderer<'static, Color>,
    slab: &mut D,
) -> Result<(), D::Error> {
    let numerals = numerals(font);
    match readout {
        Readout::NoData => style(font, chrome::BLACK, 28, SHAPIRO)
            .draw_aligned("NO DATA", &SLAB, horizontal::Center, vertical::Center, slab)
            .map(drop),
        Readout::Dashes => numerals.draw_on_baseline(DASHES, dashes_origin(&numerals), slab),
        Readout::Digits(digits, unit) => {
            numerals.draw_on_baseline(digits, READOUT, slab)?;
            suffix(font).draw_on_baseline(unit, SUFFIX, slab)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view(live: bool, percent: u8, heading: Option<u16>, disturbed: bool) -> CompassView {
        CompassView {
            live,
            calibration_percent: percent,
            heading_decidegrees: heading,
            disturbed,
            ..CompassView::default()
        }
    }

    #[test]
    fn modes_follow_the_handoffs_precedence() {
        assert_eq!(Mode::of(&view(false, 100, Some(900), true)), Mode::NoData);
        assert_eq!(Mode::of(&view(true, 100, Some(900), true)), Mode::Interference(90));
        assert_eq!(Mode::of(&view(true, 100, Some(900), false)), Mode::Heading(90));
        // A heading with calibration short of 100 still shows as a heading.
        assert_eq!(Mode::of(&view(true, 40, Some(900), false)), Mode::Heading(90));
        assert_eq!(Mode::of(&view(true, 54, None, true)), Mode::Calibrating(54));
        assert_eq!(Mode::of(&view(true, 100, None, false)), Mode::TopEdgeUp);
    }

    #[test]
    fn headings_truncate() {
        assert_eq!(degrees(3599), 359);
        assert_eq!(degrees(0), 0);
    }

    fn renderer() -> FontdueRenderer<'static, Color> {
        FontdueRenderer::new(
            chrome::FontdueRendererCtx::new_rc(),
            20,
            chrome::WHITE,
            chrome::FONTS,
        )
    }

    fn inside(inner: Rectangle, outer: Rectangle) -> bool {
        outer.intersection(&inner) == inner
    }

    #[test]
    fn every_readout_fits_the_slab() {
        let font = renderer();
        let (numerals, suffix) = (numerals(&font), suffix(&font));
        for (digits, unit) in [("000", "\u{b0}"), ("359", "\u{b0}"), ("000", "%"), ("100", "%")] {
            let ink = numerals.baseline_bounds(digits, READOUT);
            let mark = suffix.baseline_bounds(unit, SUFFIX);
            assert!(!mark.is_zero_sized(), "{unit} has no glyph");
            assert!(inside(ink, SLAB), "{digits} spills out of the slab: {ink:?}");
            assert!(inside(mark, SLAB), "{unit} spills out of the slab: {mark:?}");
            assert!(
                ink.intersection(&mark).is_zero_sized(),
                "{unit} overlaps {digits}: {ink:?} {mark:?}"
            );
        }
        let no_data = style(&font, chrome::BLACK, 28, SHAPIRO).aligned_bounds(
            "NO DATA",
            &SLAB,
            horizontal::Center,
            vertical::Center,
        );
        assert!(inside(no_data, SLAB), "NO DATA spills out of the slab: {no_data:?}");
    }

    #[test]
    fn the_dashes_are_centred_in_the_slab() {
        let font = renderer();
        let numerals = numerals(&font);
        let ink = numerals.baseline_bounds(DASHES, dashes_origin(&numerals));
        let offset = ink.center() - SLAB.center();
        assert!(offset.x.abs() <= 1 && offset.y.abs() <= 1, "{ink:?} is off centre by {offset:?}");
    }

    #[test]
    fn the_status_lines_sit_in_their_band_and_clear_the_letters() {
        let font = renderer();
        for mode in [Mode::Interference(0), Mode::Calibrating(0), Mode::TopEdgeUp] {
            let status = status(mode).unwrap();
            let (style, baseline) = status.style(&font);
            let text = status.text;
            let ink = centred_bounds(&style, text, CENTER.x, baseline);
            let band = STATUS_BAND;
            assert!(
                ink.top_left.y >= band.top_left.y
                    && ink.bottom_right().unwrap().y <= band.bottom_right().unwrap().y,
                "{text} leaves the status band: {ink:?}"
            );
            // The letters turn with the heading, so one can sit at any angle; a 40 px letter
            // reaches about 20 px inside its centre's radius.
            let corner = ink.top_left - CENTER;
            let reach = libm::hypotf(corner.x as f32, corner.y as f32);
            assert!(reach < LETTER_RADIUS - 20.0, "{text} reaches r {reach}: {ink:?}");
        }
    }
}
