//! The screens in the pager's ring, and drawing one frame of them: the settled screen, or two
//! screens while a swipe moves between them.

use embedded_graphics::{
    draw_target::DrawTarget,
    prelude::{Point, Size},
    primitives::Rectangle,
};

use super::{
    clock::{ClockState, ZoneState},
    clock_screen,
    compass::CompassView,
    compass_screen,
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
        let index = Self::ALL.iter().position(|&screen| screen == self).unwrap_or(0);
        Self::ALL[(index + 1) % Self::ALL.len()]
    }
}

/// The readings the screens show.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PeripheralState {
    pub clock: ClockState,
    pub zone: ZoneState,
    pub compass: CompassView,
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
    // The settled compass paints its slab over whatever is there, so the clear leaves it.
    let painted = (state.screen == Screen::Compass && state.offset == 0 && state.neighbour.is_none())
        .then_some(compass_screen::SLAB);
    clear_visible(target, &bounds, painted)?;

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

/// Clears the part of `area` on the round panel, leaving `painted`, which the caller covers
/// itself. The corners outside the panel are never seen, and the clear is bound by memory
/// bandwidth, so skipping them saves in proportion.
fn clear_visible<D: DrawTarget<Color = Color>>(
    target: &mut D,
    area: &Rectangle,
    painted: Option<Rectangle>,
) -> Result<(), D::Error> {
    const RADIUS: f32 = board::LCD_WIDTH as f32 / 2.0;
    let Some(bottom_right) = area.bottom_right() else {
        return Ok(());
    };
    for y in area.top_left.y.max(0)..=bottom_right.y.min(board::LCD_HEIGHT as i32 - 1) {
        let dy = y as f32 + 0.5 - RADIUS;
        let half = libm::sqrtf((RADIUS * RADIUS - dy * dy).max(0.0));
        let row = Rectangle::with_corners(
            Point::new(libm::floorf(RADIUS - half) as i32, y),
            Point::new(libm::ceilf(RADIUS + half) as i32 - 1, y),
        );
        let row = row.intersection(area);
        let hole = painted
            .map(|painted| painted.intersection(&row))
            .filter(|hole| !hole.is_zero_sized());
        let Some(hole) = hole else {
            target.fill_solid(&row, chrome::BLACK)?;
            continue;
        };
        let right = hole.top_left.x + hole.size.width as i32;
        let row_right = row.top_left.x + row.size.width as i32;
        for (from, to) in [(row.top_left.x, hole.top_left.x), (right, row_right)] {
            if from < to {
                let part = Rectangle::new(Point::new(from, y), Size::new((to - from) as u32, 1));
                target.fill_solid(&part, chrome::BLACK)?;
            }
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
    let page = Rectangle::new(Point::new(offset, 0), chrome::DISPLAY_SIZE);
    let visible = page.intersection(&target.bounding_box());
    if visible.is_zero_sized() {
        return Ok(());
    }
    let target = &mut chrome::Window::new(target, Point::new(offset, 0), visible);
    let peripherals = &state.peripherals;
    match state.screen {
        Screen::Clock => clock_screen::draw(
            &peripherals.clock,
            &peripherals.zone,
            state.clock_accents,
            font,
            target,
        ),
        Screen::Compass => compass_screen::draw(&peripherals.compass, state.compass_accents, font, target),
    }
}
