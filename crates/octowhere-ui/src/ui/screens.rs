//! The screens in the pager's ring, and drawing one frame of them: the settled screen, or two
//! screens while a swipe moves between them.

use embedded_graphics::{
    draw_target::DrawTarget,
    prelude::{Point, Size},
    primitives::Rectangle,
};

use super::{
    clock::ClockView,
    clock_screen,
    compass::CompassView,
    compass_screen, panel,
    rest::{AlwaysOn, Timeout},
};
use crate::{
    board,
    chrome::{self, Color},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Screen {
    Clock,
    Compass,
}

impl Screen {
    /// Every screen, in the order the pager visits them.
    pub const ALL: [Self; 2] = [Self::Clock, Self::Compass];

    #[must_use]
    pub fn next(self) -> Self {
        let index = Self::ALL
            .iter()
            .position(|&screen| screen == self)
            .unwrap_or(0);
        Self::ALL[(index + 1) % Self::ALL.len()]
    }
}

/// The display's level at boot while none is stored, out of 255.
pub const DEFAULT_BRIGHTNESS: u8 = 120;

/// What the power controller reports about the battery and the supply.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct Battery {
    pub present: bool,
    /// Charge, 0 to 100, while a battery is present.
    pub percent: u8,
    pub millivolts: u16,
    pub charging: bool,
    /// USB power is present.
    pub usb: bool,
}

/// What the GNSS receiver reports.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct Gnss {
    pub fix: bool,
    /// Satellites used in the fix.
    pub in_use: u8,
    pub in_view: u8,
    /// The last fix's latitude and longitude, in 1e-7 degrees.
    pub position: Option<(i32, i32)>,
}

/// The readings the screens show.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PeripheralState {
    pub clock: ClockView,
    pub compass: CompassView,
    /// `None` until the power controller has been read.
    pub battery: Option<Battery>,
    pub gnss: Gnss,
    /// The display's level, out of 255.
    pub brightness: u8,
    pub timeout: Timeout,
    /// Whether the screen rests on the always-on face rather than going dark, and its level.
    pub always_on: AlwaysOn,
    /// The firmware's version.
    pub firmware: &'static str,
    /// The stored group's size, while there is one.
    pub members: Option<u8>,
    /// This device's stored name.
    pub name: super::group::view::Name,
}

impl Default for PeripheralState {
    fn default() -> Self {
        Self {
            clock: ClockView::default(),
            compass: CompassView::default(),
            battery: None,
            gnss: Gnss::default(),
            brightness: DEFAULT_BRIGHTNESS,
            timeout: Timeout::default(),
            always_on: AlwaysOn::Off,
            firmware: "",
            members: None,
            name: super::group::view::MeshView::default().name,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct State {
    pub screen: Screen,
    pub peripherals: PeripheralState,
    /// How far right `screen` is shifted, while a page switch moves it.
    pub offset: i32,
    /// The screen a shift uncovers, and its own offset.
    pub neighbour: Option<(Screen, i32)>,
    /// How far the compass's accents have come in, when `screen` is the compass.
    pub compass_accents: compass_screen::Accents,
    /// How far the clock face's accents have come in, when `screen` is the clock.
    pub clock_accents: clock_screen::Accents,
    /// How far the settings panel has come down over the faces, which move down with it.
    pub sheet: i32,
    pub panel_scroll: i32,
    pub panel_accents: panel::Accents,
}

pub fn render<D>(
    state: State,
    font: &chrome::FontdueRenderer<'static, Color>,
    target: &mut D,
) -> Result<(), D::Error>
where
    D: chrome::CoverageTarget<Color = Color>,
{
    let bounds = target.bounding_box();
    let height = board::LCD_HEIGHT as i32;
    // The settled compass paints its slab over whatever is there, and the clock face its band
    // wherever the page is, so the clear leaves them.
    let settled = state.offset == 0 && state.neighbour.is_none() && state.sheet == 0;
    let clock_offset = match (state.screen, state.neighbour) {
        (Screen::Clock, _) => Some(state.offset),
        (_, Some((Screen::Clock, offset))) => Some(offset),
        _ => None,
    };
    let painted = if state.screen == Screen::Compass && settled {
        Some(compass_screen::SLAB)
    } else {
        clock_offset.filter(|_| state.sheet < height).map(|offset| {
            let band = clock_screen::solid_band();
            Rectangle::new(band.top_left + Point::new(offset, state.sheet), band.size)
        })
    };
    clear_visible(target, &bounds, painted, chrome::BLACK)?;

    if state.sheet > 0 {
        let visible = Rectangle::new(
            Point::zero(),
            Size::new(board::LCD_WIDTH.into(), state.sheet as u32),
        );
        let panel = &mut chrome::Window::new(target, Point::new(0, state.sheet - height), visible);
        panel::draw(
            &state.peripherals,
            state.panel_scroll,
            state.panel_accents,
            font,
            panel,
        )?;
    }
    if state.sheet >= height {
        return Ok(());
    }
    render_page(state, state.offset, font, target)?;
    if let Some((screen, offset)) = state.neighbour {
        render_page(
            // A page crossing into view has not settled, so its accents have not begun.
            State {
                screen,
                compass_accents: compass_screen::Accents::HIDDEN,
                clock_accents: clock_screen::Accents::HIDDEN,
                ..state
            },
            offset,
            font,
            target,
        )?;
    }
    Ok(())
}

/// Clears the round panel.
pub fn clear<D: DrawTarget<Color = Color>>(target: &mut D) -> Result<(), D::Error> {
    clear_to(target, chrome::BLACK)
}

/// Fills the round panel with `color`.
pub fn clear_to<D: DrawTarget<Color = Color>>(
    target: &mut D,
    color: Color,
) -> Result<(), D::Error> {
    let bounds = target.bounding_box();
    clear_visible(target, &bounds, None, color)
}

/// How many columns past the circle, on each side, a run of rows may clear so that the run is
/// one fill. It trades a few corner pixels written for most of the calls.
const CLEAR_SLACK: i32 = 8;

/// Clears the part of `area` on the round panel, and as far past its edge as the pixel shift
/// can bring onto it, leaving `painted`, which the caller covers itself. The corners beyond are
/// never seen, and the clear is bound by memory bandwidth, so skipping most of them saves in
/// proportion.
fn clear_visible<D: DrawTarget<Color = Color>>(
    target: &mut D,
    area: &Rectangle,
    painted: Option<Rectangle>,
    color: Color,
) -> Result<(), D::Error> {
    const RADIUS: f32 = board::LCD_WIDTH as f32 / 2.0 + super::shift::REACH as f32;
    const MIDDLE: f32 = board::LCD_WIDTH as f32 / 2.0;
    let chord = |y: i32| {
        let dy = y as f32 + 0.5 - MIDDLE;
        let half = libm::sqrtf((RADIUS * RADIUS - dy * dy).max(0.0));
        (
            (libm::floorf(MIDDLE - half) as i32).max(0),
            (libm::ceilf(MIDDLE + half) as i32).min(board::LCD_WIDTH as i32),
        )
    };
    let Some(bottom_right) = area.bottom_right() else {
        return Ok(());
    };
    let bottom = bottom_right.y.min(board::LCD_HEIGHT as i32 - 1);
    let mut y = area.top_left.y.max(0);
    while y <= bottom {
        // The run spans its widest row; `inner` is its narrowest.
        let (mut left, mut right) = chord(y);
        let mut inner = (left, right);
        let mut end = y + 1;
        while end <= bottom {
            let (l, r) = chord(end);
            let (wide_l, wide_r) = (left.min(l), right.max(r));
            let (narrow_l, narrow_r) = (inner.0.max(l), inner.1.min(r));
            if narrow_l - wide_l > CLEAR_SLACK || wide_r - narrow_r > CLEAR_SLACK {
                break;
            }
            (left, right, inner) = (wide_l, wide_r, (narrow_l, narrow_r));
            end += 1;
        }
        let run = Rectangle::with_corners(Point::new(left, y), Point::new(right - 1, end - 1))
            .intersection(area);
        fill_around(target, &run, painted, color)?;
        y = end;
    }
    Ok(())
}

/// Fills `area` except where it overlaps `hole`.
fn fill_around<D: DrawTarget<Color = Color>>(
    target: &mut D,
    area: &Rectangle,
    hole: Option<Rectangle>,
    color: Color,
) -> Result<(), D::Error> {
    let hole = hole
        .map(|hole| hole.intersection(area))
        .filter(|hole| !hole.is_zero_sized());
    let Some(hole) = hole else {
        return if area.is_zero_sized() {
            Ok(())
        } else {
            target.fill_solid(area, color)
        };
    };
    let (left, top) = (area.top_left.x, area.top_left.y);
    let (right, bottom) = (left + area.size.width as i32, top + area.size.height as i32);
    let (hole_left, hole_top) = (hole.top_left.x, hole.top_left.y);
    let (hole_right, hole_bottom) = (
        hole_left + hole.size.width as i32,
        hole_top + hole.size.height as i32,
    );
    for (x0, y0, x1, y1) in [
        (left, top, right, hole_top),
        (left, hole_top, hole_left, hole_bottom),
        (hole_right, hole_top, right, hole_bottom),
        (left, hole_bottom, right, bottom),
    ] {
        if x0 < x1 && y0 < y1 {
            let part = Rectangle::new(
                Point::new(x0, y0),
                Size::new((x1 - x0) as u32, (y1 - y0) as u32),
            );
            target.fill_solid(&part, color)?;
        }
    }
    Ok(())
}

/// Draws one screen shifted right by `offset`, clipped to the part of it that is on the panel.
fn render_page<D>(
    state: State,
    offset: i32,
    font: &chrome::FontdueRenderer<'static, Color>,
    target: &mut D,
) -> Result<(), D::Error>
where
    D: chrome::CoverageTarget<Color = Color>,
{
    let page = Rectangle::new(Point::new(offset, state.sheet), chrome::DISPLAY_SIZE);
    let visible = page.intersection(&target.bounding_box());
    if visible.is_zero_sized() {
        return Ok(());
    }
    let target = &mut chrome::Window::new(target, Point::new(offset, state.sheet), visible);
    let peripherals = &state.peripherals;
    match state.screen {
        Screen::Clock => clock_screen::draw(
            &peripherals.clock,
            peripherals.battery,
            state.clock_accents,
            font,
            target,
        ),
        Screen::Compass => {
            compass_screen::draw(&peripherals.compass, state.compass_accents, font, target)
        }
    }
}
