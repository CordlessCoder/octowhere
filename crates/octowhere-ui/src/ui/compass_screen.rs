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

/// Draws `text` centred on `x` along the baseline at `baseline`.
fn centred<D: CoverageTarget<Color = Color>>(
    style: &FontdueRenderer<'static, Color>,
    text: &str,
    x: i32,
    baseline: i32,
    target: &mut D,
) -> Result<(), D::Error> {
    let left = x as f32 - style.advance(text) / 2.0;
    style.draw_on_baseline(text, Point::new(libm::roundf(left) as i32, baseline), target)
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
    let mode = Mode::of(view);
    let ring_color = if mode == Mode::NoData { chrome::RED } else { chrome::GRAY };
    {
        // The dial lands on the cleared field, and its parts do not overlap.
        let field = &mut OnBackground::new(&mut *target, chrome::BLACK);
        if accents.ring > 0 {
            RING.get_or_init(|| {
                super::smooth::Ring::new(
                    CENTER,
                    RING_RADIUS - RING_STROKE / 2.0,
                    RING_RADIUS + RING_STROKE / 2.0,
                )
            })
            .draw(field, faded(ring_color, accents.ring));
        }
        if let Some(heading) = mode.heading() {
            draw_dial(heading, accents, font, field)?;
        }
    }
    draw_icon(mode, accents.icon, target)?;

    let caption = match mode {
        Mode::NoData => "COMPASS",
        Mode::Calibrating(_) => "CALIBRATION",
        _ => "MAGNETIC",
    };
    let caption_color = match mode {
        Mode::Calibrating(_) => chrome::ORANGE,
        _ => chrome::GRAY,
    };
    let field = &mut OnBackground::new(&mut *target, chrome::BLACK);
    centred(
        &style(font, caption_color, 16, FRAKTION_BOLD),
        caption,
        CENTER.x,
        CAPTION_BASELINE,
        field,
    )?;

    target.fill_solid(&SLAB, mode.color())?;
    draw_readout(mode, font, &mut OnBackground::new(&mut *target, mode.color()))?;
    if mode == Mode::NoData {
        return Ok(());
    }

    let field = &mut OnBackground::new(&mut *target, chrome::BLACK);
    if let Some((status, style, baseline)) = status_line(mode, font) {
        centred(&style, status, CENTER.x, baseline, field)?;
    }
    let mut tilt = heapless::String::<24>::new();
    _ = write!(tilt, "P {:+03}  R {:+03}", view.pitch_deg, view.roll_deg);
    centred(&style(font, chrome::GRAY, 23, FRAKTION), &tilt, CENTER.x, TILT_BASELINE, field)?;
    field.fill_solid(&DIVIDER, chrome::GRAY)?;
    centred(
        &style(font, chrome::GRAY, 14, FRAKTION),
        "COVER SCREEN TO RECAL",
        CENTER.x,
        HINT_BASELINE,
        field,
    )
}

/// The line of text a state puts above the slab, if it has one, with the baseline that centres its
/// ink in the status band.
fn status_line(
    mode: Mode,
    font: &FontdueRenderer<'static, Color>,
) -> Option<(&'static str, FontdueRenderer<'static, Color>, i32)> {
    let (text, color, size, index) = match mode {
        Mode::Interference(_) => ("INTERFERENCE", chrome::ORANGE, 24, FRAKTION_BOLD),
        Mode::Calibrating(_) => ("TURN ALL WAYS", chrome::GRAY, 19, FRAKTION),
        Mode::TopEdgeUp => ("TOP EDGE UP", chrome::GRAY, 20, FRAKTION),
        Mode::Heading(_) | Mode::NoData => return None,
    };
    let style = style(font, color, size, index);
    let ink = style.baseline_bounds(text, Point::zero());
    let baseline = STATUS_BAND.top_left.y
        + (STATUS_BAND.size.height as i32 - ink.size.height as i32) / 2
        - ink.top_left.y;
    Some((text, style, baseline))
}

/// The ticks and the cardinal letters, turned so the top edge reads `heading`.
fn draw_dial<D>(
    heading: u16,
    accents: Accents,
    font: &FontdueRenderer<'static, Color>,
    field: &mut D,
) -> Result<(), D::Error>
where
    D: CoverageTarget<Color = Color>,
{
    let turn = f32::from(heading);
    let center = (CENTER.x as f32, CENTER.y as f32);
    if accents.ticks > 0 {
        // One quarter of the ticks is filled, and each is drawn at all four quarter turns:
        // ticks nine apart are a quarter turn apart and all major or all minor alike.
        let mut raster = fontdue::raster::Raster::empty();
        let mut coverage = alloc::vec::Vec::new();
        for tick in 0..9 {
            let (sin, cos) = libm::sincosf((tick as f32 * 10.0 - turn).to_radians());
            let major = tick % 3 == 0;
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
            super::smooth::polygon_quarters(
                field,
                &mut raster,
                &mut coverage,
                &[
                    point(outer, -width / 2.0),
                    point(outer, width / 2.0),
                    point(inner, width / 2.0),
                    point(inner, -width / 2.0),
                ],
                CENTER,
                faded(color, accents.ticks),
            );
        }
    }
    if accents.letters > 0 {
        for (quarter, letter) in ["N", "E", "S", "W"].into_iter().enumerate() {
            let (sin, cos) = libm::sincosf((quarter as f32 * 90.0 - turn).to_radians());
            let at = Point::new(
                libm::roundf(center.0 + sin * LETTER_RADIUS) as i32,
                libm::roundf(center.1 - cos * LETTER_RADIUS) as i32,
            );
            let color = if quarter == 0 { chrome::ORANGE } else { chrome::WHITE };
            // Turned by the letter's own bearing, so its top faces outward all the way round.
            style(font, faded(color, accents.letters), 40, SHAPIRO)
                .draw_rotated(letter, at, cos, sin, field)?;
        }
    }
    Ok(())
}

fn draw_icon<D: DrawTarget<Color = Color>>(
    mode: Mode,
    amount: u8,
    target: &mut D,
) -> Result<(), D::Error> {
    if amount == 0 {
        return Ok(());
    }
    let side = ICON_SIDE;
    target.fill_solid(
        &Rectangle::new(ICON, Size::new_equal(side as u32)),
        faded(mode.color(), amount),
    )?;
    for (row, bits) in mode.icon().iter().enumerate() {
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
    mode: Mode,
    font: &FontdueRenderer<'static, Color>,
    slab: &mut D,
) -> Result<(), D::Error> {
    let numerals = style(font, chrome::BLACK, 86, FRAKTION_BOLD);
    let suffix = style(font, chrome::BLACK, 40, FRAKTION_BOLD);
    let mut digits = heapless::String::<4>::new();
    let unit = match mode {
        Mode::NoData => {
            return style(font, chrome::BLACK, 28, SHAPIRO)
                .draw_aligned("NO DATA", &SLAB, horizontal::Center, vertical::Center, slab)
                .map(drop);
        }
        Mode::Heading(degrees) | Mode::Interference(degrees) => {
            _ = write!(digits, "{degrees:03}");
            Some("\u{b0}")
        }
        Mode::Calibrating(percent) => {
            _ = write!(digits, "{percent:03}");
            Some("%")
        }
        Mode::TopEdgeUp => return numerals.draw_on_baseline(DASHES, dashes_origin(&numerals), slab),
    };
    numerals.draw_on_baseline(&digits, READOUT, slab)?;
    if let Some(unit) = unit {
        suffix.draw_on_baseline(unit, SUFFIX, slab)?;
    }
    Ok(())
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
        let numerals = style(&font, chrome::BLACK, 86, FRAKTION_BOLD);
        let suffix = style(&font, chrome::BLACK, 40, FRAKTION_BOLD);
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
        let numerals = style(&font, chrome::BLACK, 86, FRAKTION_BOLD);
        let ink = numerals.baseline_bounds(DASHES, dashes_origin(&numerals));
        let offset = ink.center() - SLAB.center();
        assert!(offset.x.abs() <= 1 && offset.y.abs() <= 1, "{ink:?} is off centre by {offset:?}");
    }

    #[test]
    fn the_status_lines_sit_in_their_band_and_clear_the_letters() {
        let font = renderer();
        for mode in [Mode::Interference(0), Mode::Calibrating(0), Mode::TopEdgeUp] {
            let (text, style, baseline) = status_line(mode, &font).unwrap();
            let left = CENTER.x - libm::roundf(style.advance(text) / 2.0) as i32;
            let ink = style.baseline_bounds(text, Point::new(left, baseline));
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
