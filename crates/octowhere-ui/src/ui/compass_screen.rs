//! The compass screen: a fixed slab carrying the heading or the calibration progress, a status
//! icon, and a dial that turns with the heading. `context/compass-implementation-handoff.md` is
//! the design this implements.

use core::fmt::Write as _;

use embedded_graphics::{
    draw_target::DrawTarget,
    prelude::{Point, Size},
    primitives::Rectangle,
};
use embedded_layout::align::{horizontal, vertical};

use super::compass::CompassView;
use crate::chrome::{
    self, Color, CoverageTarget, FontdueRenderer, OnBackground, RgbColorExt as _, FRAKTION,
    FRAKTION_BOLD, SHAPIRO,
};

pub const CENTER: Point = Point::new(233, 233);
// The dial's damage is worked out for half of it and turned about the panel's centre for the rest.
const _: () = assert!(
    CENTER.x * 2 == crate::board::LCD_WIDTH as i32 && CENTER.y * 2 == crate::board::LCD_HEIGHT as i32
);
/// The perimeter ring's middle radius and stroke.
const RING_RADIUS: f32 = 231.0;
const RING_STROKE: f32 = 2.0;
/// Every state shares the slab, so a state change does not move it. It ends 16 px above the tilt
/// line's ink.
const SLAB: Rectangle = Rectangle::new(Point::new(135, 186), Size::new(196, 91));
/// The gap between the inks of the icon, caption, state line and slab, which stack as one group.
const GROUP_GAP: i32 = 7;
/// Where a state's line of text sits between the caption and the slab, centred by its ink. It is
/// as tall as the tallest such line, `INTERFERENCE`.
const STATUS_BAND: Rectangle =
    Rectangle::new(Point::new(0, SLAB.top_left.y - GROUP_GAP - 18), Size::new(466, 18));
/// The caption's antialiased ink reaches its baseline row.
const CAPTION_BASELINE: i32 = STATUS_BAND.top_left.y - GROUP_GAP - 1;
const CAPTION_HEIGHT: i32 = 12;
const ICON_MODULE: i32 = 5;
const ICON_PADDING: i32 = 4;
const ICON_SIDE: i32 = 2 * ICON_PADDING + 5 * ICON_MODULE;
const ICON: Point = Point::new(
    217,
    CAPTION_BASELINE + 1 - CAPTION_HEIGHT - GROUP_GAP - ICON_SIDE,
);
/// The readout's first pen position and its suffix's, both on their baselines. Heading and
/// calibration share them, so completing a calibration does not move the digits.
const READOUT: Point = Point::new(143, SLAB.top_left.y + 79);
const SUFFIX: Point = Point::new(298, SLAB.top_left.y + 43);
const TILT_BASELINE: i32 = 309;
const DIVIDER: Rectangle = Rectangle::new(Point::new(138, 321), Size::new(191, 1));
const HINT_BASELINE: i32 = 338;
const LETTER_RADIUS: f32 = 172.0;

/// The ring's coverage, computed on first use.
static RING: embassy_sync::once_lock::OnceLock<super::smooth::Ring> =
    embassy_sync::once_lock::OnceLock::new();

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

    /// The heading, in the two modes that turn the dial to it.
    #[must_use]
    pub fn heading(self) -> Option<u16> {
        match self {
            Self::Heading(degrees) | Self::Interference(degrees) => Some(degrees),
            _ => None,
        }
    }

    fn icon(self) -> &'static Icon {
        match self {
            Self::NoData => &NO_DATA,
            Self::Interference(_) => &INTERFERENCE,
            Self::Heading(_) => &ARROW,
            Self::Calibrating(_) => &OPEN_LOOP,
            Self::TopEdgeUp => &TOP_BAR,
        }
    }

    /// The slab's colour, which also colours the icon tile.
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

/// One value for each group of the dial's accents: by default how far it has faded in, 0 to
/// 255. The stage animates these; the centre content always draws at full strength.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Accents<T = u8> {
    pub ring: T,
    pub icon: T,
    pub ticks: T,
    pub letters: T,
}

impl Accents {
    pub const FULL: Self = Self {
        ring: u8::MAX,
        icon: u8::MAX,
        ticks: u8::MAX,
        letters: u8::MAX,
    };
    pub const HIDDEN: Self = Self {
        ring: 0,
        icon: 0,
        ticks: 0,
        letters: 0,
    };
}

impl Default for Accents {
    fn default() -> Self {
        Self::FULL
    }
}

/// A status mark: five rows of five modules, bit 4 the leftmost, where a set bit is a black
/// module on the tile's colour.
type Icon = [u8; 5];

const ARROW: Icon = [0b00100, 0b01110, 0b10101, 0b00100, 0b00100];
const OPEN_LOOP: Icon = [0b01110, 0b10001, 0b10000, 0b10001, 0b01110];
const INTERFERENCE: Icon = [0b10101, 0b01110, 0b11011, 0b01110, 0b10101];
const TOP_BAR: Icon = [0b11111, 0b00100, 0b00100, 0b00100, 0b00100];
const NO_DATA: Icon = [0b11100, 0b11000, 0b00100, 0b00011, 0b00111];

/// `color` faded toward the black field by `amount`, 0 black to 255 full.
fn faded(color: Color, amount: u8) -> Color {
    chrome::BLACK.lerp(&color, amount)
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
    /// The heading the dial turns to, and how far its ticks and letters have faded in.
    dial: Option<(u16, u8, u8)>,
    icon: Option<(Icon, Color)>,
    caption: (&'static str, Color),
    slab: Color,
    readout: Readout,
    status: Option<Status>,
    tilt: Option<heapless::String<24>>,
    /// The divider and the hint under the tilt.
    footer: bool,
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
        let shown = mode != Mode::NoData;
        let tilt = shown.then(|| {
            let mut tilt = heapless::String::new();
            _ = write!(tilt, "P {:+03}  R {:+03}", view.pitch_deg, view.roll_deg);
            tilt
        });
        Self {
            ring: (accents.ring > 0).then(|| faded(ring_color, accents.ring)),
            dial: mode
                .heading()
                .filter(|_| accents.ticks > 0 || accents.letters > 0)
                .map(|heading| (heading, accents.ticks, accents.letters)),
            icon: (accents.icon > 0).then(|| (*mode.icon(), faded(mode.icon_color(), accents.icon))),
            caption: match mode {
                Mode::NoData => ("COMPASS", chrome::GRAY),
                Mode::Calibrating(_) => ("CALIBRATION", chrome::ORANGE),
                _ => ("MAGNETIC", chrome::GRAY),
            },
            slab: mode.color(),
            readout,
            status: status(mode),
            tilt,
            footer: shown,
        }
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

fn hint_style(font: &FontdueRenderer<'static, Color>) -> FontdueRenderer<'static, Color> {
    style(font, chrome::GRAY, 14, FRAKTION)
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

const HINT: &str = "COVER SCREEN TO RECAL";

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
            RING.get_or_init(|| {
                super::smooth::Ring::new(
                    CENTER,
                    RING_RADIUS - RING_STROKE / 2.0,
                    RING_RADIUS + RING_STROKE / 2.0,
                )
            })
            .draw(field, color);
        }
        if let Some((heading, ticks, letters)) = parts.dial {
            draw_dial(heading, ticks, letters, font, field)?;
        }
    }
    if let Some((icon, color)) = parts.icon {
        draw_icon(&icon, color, target)?;
    }

    let field = &mut OnBackground::new(&mut *target, chrome::BLACK);
    let (caption, caption_color) = parts.caption;
    centred(&caption_style(font, caption_color), caption, CENTER.x, CAPTION_BASELINE, field)?;

    target.fill_solid(&SLAB, parts.slab)?;
    draw_readout(&parts.readout, font, &mut OnBackground::new(&mut *target, parts.slab))?;

    let field = &mut OnBackground::new(&mut *target, chrome::BLACK);
    if let Some(status) = parts.status {
        let (style, baseline) = status.style(font);
        centred(&style, status.text, CENTER.x, baseline, field)?;
    }
    if let Some(tilt) = &parts.tilt {
        centred(&tilt_style(font), tilt, CENTER.x, TILT_BASELINE, field)?;
    }
    if parts.footer {
        field.fill_solid(&DIVIDER, chrome::GRAY)?;
        centred(&hint_style(font), HINT, CENTER.x, HINT_BASELINE, field)?;
    }
    Ok(())
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
        // The ring runs round the whole panel, and changes only while it fades or with NO DATA.
        damage.make_full();
        return;
    }
    if old.dial != new.dial {
        // The old place first: the last step worked it out as its new one.
        for dial in [old.dial, new.dial].into_iter().flatten() {
            footprint.mark(dial, font, damage);
        }
    }
    if old.icon != new.icon {
        damage.add(Rectangle::new(ICON, Size::new_equal(ICON_SIDE as u32)));
    }
    if old.caption != new.caption {
        for (text, color) in [old.caption, new.caption] {
            damage.add(centred_bounds(&caption_style(font, color), text, CENTER.x, CAPTION_BASELINE));
        }
    }
    if old.slab != new.slab {
        damage.add(SLAB);
    } else if old.readout != new.readout {
        readout_damage(&old.readout, &new.readout, font, damage);
    }
    if old.status != new.status {
        for status in [old.status, new.status].into_iter().flatten() {
            let (style, baseline) = status.style(font);
            damage.add(centred_bounds(&style, status.text, CENTER.x, baseline));
        }
    }
    if old.tilt != new.tilt {
        for tilt in [&old.tilt, &new.tilt].into_iter().flatten() {
            damage.add(centred_bounds(&tilt_style(font), tilt, CENTER.x, TILT_BASELINE));
        }
    }
    if old.footer != new.footer {
        damage.add(DIVIDER);
        damage.add(centred_bounds(&hint_style(font), HINT, CENTER.x, HINT_BASELINE));
    }
}

/// Where the dial last drew, kept so that a turn marks the old place without working it out
/// again.
pub struct DialFootprint {
    /// The heading, and whether the ticks and the letters showed.
    key: Option<(u16, bool, bool)>,
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
    /// Marks the pixels a dial turned to `heading` covers.
    fn mark(
        &mut self,
        (heading, ticks, letters): (u16, u8, u8),
        font: &FontdueRenderer<'static, Color>,
        damage: &mut chrome::Dirty,
    ) {
        let key = (heading, ticks > 0, letters > 0);
        if self.key != Some(key) {
            self.half.clear();
            half_dial(key, font, &mut self.half);
            self.key = Some(key);
        }
        damage.extend(&self.half);
        damage.extend_reflected(&self.half);
    }
}

/// The pixels the first two quarters of a dial turned to `heading` cover, taking each letter as
/// the larger of it and the one opposite.
fn half_dial(
    (heading, ticks, letters): (u16, bool, bool),
    font: &FontdueRenderer<'static, Color>,
    half: &mut chrome::Dirty,
) {
    let turn = f32::from(heading);
    if ticks {
        let (cx, cy) = (CENTER.x as f32, CENTER.y as f32);
        for tick in 0..9 {
            let (mut corners, _, _) = tick_shape(tick, turn);
            for _ in 0..2 {
                half.add_polygon(&corners, 1);
                // A quarter turn clockwise about the centre, y down.
                corners = corners.map(|(x, y)| (cx - (y - cy), cy + (x - cx)));
            }
        }
    }
    if letters {
        let style = letter_style(font, chrome::WHITE);
        for quarter in 0..2 {
            let (at, _, _) = letter_place(quarter, turn);
            let reach = style
                .rotated_reach(LETTERS[quarter])
                .max(style.rotated_reach(LETTERS[quarter + 2]));
            half.add_disc((at.x as f32, at.y as f32), reach);
        }
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

/// The ticks and the cardinal letters, turned so the top edge reads `heading`, each faded in by
/// its amount.
fn draw_dial<D>(
    heading: u16,
    ticks: u8,
    letters: u8,
    font: &FontdueRenderer<'static, Color>,
    field: &mut D,
) -> Result<(), D::Error>
where
    D: CoverageTarget<Color = Color>,
{
    let turn = f32::from(heading);
    if ticks > 0 {
        // One quarter of the ticks is filled, and each is drawn at all four quarter turns.
        let mut raster = fontdue::raster::Raster::empty();
        let mut coverage = alloc::vec::Vec::new();
        for tick in 0..9 {
            let (corners, _, color) = tick_shape(tick, turn);
            super::smooth::polygon_quarters(
                field,
                &mut raster,
                &mut coverage,
                &corners,
                CENTER,
                faded(color, ticks),
            );
        }
    }
    if letters > 0 {
        for (quarter, letter) in LETTERS.into_iter().enumerate() {
            let (at, cos, sin) = letter_place(quarter, turn);
            let color = if quarter == 0 { chrome::ORANGE } else { chrome::WHITE };
            letter_style(font, faded(color, letters)).draw_rotated(letter, at, cos, sin, field)?;
        }
    }
    Ok(())
}

fn draw_icon<D: DrawTarget<Color = Color>>(
    icon: &Icon,
    color: Color,
    target: &mut D,
) -> Result<(), D::Error> {
    target.fill_solid(&Rectangle::new(ICON, Size::new_equal(ICON_SIDE as u32)), color)?;
    for (row, bits) in icon.iter().enumerate() {
        for column in (0..5).filter(|column| bits & (0b10000 >> column) != 0) {
            let corner = ICON
                + Point::new(
                    ICON_PADDING + column * ICON_MODULE,
                    ICON_PADDING + row as i32 * ICON_MODULE,
                );
            target.fill_solid(
                &Rectangle::new(corner, Size::new_equal(ICON_MODULE as u32)),
                chrome::BLACK,
            )?;
        }
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
            chrome::BLACK,
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
