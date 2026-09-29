//! The confirmation the power key's long press opens, and what shows while the board powers
//! off.

use embedded_graphics::{
    prelude::{Point, Size},
    primitives::Rectangle,
};

use super::{
    gesture::{GestureEvent, Micros},
    icon::Glyph,
    rest::Rest,
    second::{self, Rail, Slide},
    smooth,
    text::{self, style},
};
use crate::chrome::{
    self, Color, CoverageTarget, FRAKTION, FRAKTION_BOLD, FontdueRenderer, OnBackground,
};

/// Left untouched this long, the confirmation cancels itself.
pub const TIMEOUT: Micros = 10_000_000;
/// How long the level takes to go dark once the slide confirms.
pub const FADE: Micros = 300_000;

const CENTER_X: f32 = 233.0;
const POWER: Glyph = [0b00100, 0b00100, 0b10001, 0b10001, 0b01110];
const RAIL: Rail = Rail {
    handle: Rectangle::new(Point::new(83, 225), Size::new(60, 64)),
    travel: 240,
    // From under the action line to over the help, and past the handle either side.
    grab: Rectangle::new(Point::new(40, 180), Size::new(140, 138)),
};
/// The slide's rows, all a drag redraws.
pub const SLIDER: Rectangle = Rectangle::new(Point::new(83, 225), Size::new(300, 64));
const GUIDE: Rectangle = Rectangle::new(Point::new(83, 256), Size::new(300, 3));
const HELP: [(&str, i32); 2] = [
    ("RELEASE IN TARGET TO CONFIRM", 323),
    ("HOLD KEY TO WAKE", 346),
];

/// How the screen showed when the key opened the confirmation, to go back to on a cancel.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Prior {
    pub rest: Rest,
    /// The display's level then.
    pub level: u8,
    /// When the key opened it, so that a dim or darkening resumes with the time it had left.
    pub at: Micros,
}

/// What a step of the confirmation decided.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Answer {
    Stay,
    Cancel,
    Confirm,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PowerOff {
    slide: Slide,
    /// The last touch, or when the confirmation came fully into view, whichever is later.
    since: Micros,
    confirmed: Option<Micros>,
    prior: Prior,
}

impl PowerOff {
    /// Opens over a screen that showed as `prior`, fully in view from `shown`.
    #[must_use]
    pub fn new(shown: Micros, prior: Prior) -> Self {
        Self {
            slide: Slide::default(),
            since: shown,
            confirmed: None,
            prior,
        }
    }

    /// Handles a gesture. Once confirmed, nothing more is taken.
    pub fn handle(&mut self, event: &GestureEvent, now: Micros) -> Answer {
        if self.confirmed.is_some() {
            return Answer::Stay;
        }
        if self.slide.handle(&RAIL, event) {
            self.slide.arrive(&RAIL);
            self.confirmed = Some(now);
            return Answer::Confirm;
        }
        match *event {
            GestureEvent::Tap(point) if second::in_top_cap(point) => Answer::Cancel,
            _ => Answer::Stay,
        }
    }

    /// Restarts the time left before the confirmation cancels itself.
    pub fn touched(&mut self, now: Micros) {
        self.since = self.since.max(now);
    }

    /// When the confirmation cancels itself, while it has not been confirmed.
    #[must_use]
    pub fn deadline(&self) -> Option<Micros> {
        self.confirmed.is_none().then_some(self.since + TIMEOUT)
    }

    /// When the slide confirmed it.
    #[must_use]
    pub fn confirmed(&self) -> Option<Micros> {
        self.confirmed
    }

    #[must_use]
    pub fn prior(&self) -> Prior {
        self.prior
    }

    pub fn draw<D: CoverageTarget<Color = Color>>(
        &self,
        font: &FontdueRenderer<'static, Color>,
        target: &mut D,
    ) -> Result<(), D::Error> {
        second::draw_heading("POWER OFF", "SYSTEM / POWER", font, target)?;
        second::ICON.draw(&POWER, chrome::ORANGE, 5, target)?;
        let confirmed = self.confirmed.is_some();
        if !confirmed {
            second::draw_button("CANCEL", second::BUTTON, false, font, target)?;
            let action = style(font, chrome::ORANGE, 14, FRAKTION);
            let line = "SLIDE TO POWER OFF";
            let pen = Point::new(
                text::pen_x_for_ink_left(&action, line, 91),
                text::baseline_for_ink_top(&action, line, 164),
            );
            action.draw_on_baseline(
                line,
                pen,
                &mut OnBackground::new(&mut *target, chrome::BLACK),
            )?;
        }
        self.draw_slider(target)?;
        if confirmed {
            let status = style(font, chrome::WHITE, 23, FRAKTION_BOLD);
            let line = "POWERING OFF";
            let pen = Point::new(
                text::pen_x_for_ink_centre(&status, line, CENTER_X),
                text::baseline_for_ink_top(&status, line, 327),
            );
            return status.draw_on_baseline(
                line,
                pen,
                &mut OnBackground::new(&mut *target, chrome::BLACK),
            );
        }
        let help = style(font, chrome::GRAY, 13, FRAKTION);
        for (line, top) in HELP {
            let pen = Point::new(
                text::pen_x_for_ink_centre(&help, line, CENTER_X),
                text::baseline_for_ink_top(&help, line, top),
            );
            help.draw_on_baseline(
                line,
                pen,
                &mut OnBackground::new(&mut *target, chrome::BLACK),
            )?;
        }
        second::draw_footer("AUTO CANCEL / 10 S", second::Accents::FULL, font, target)
    }

    /// The guide, the target, and the orange track to the handle with its arrow.
    fn draw_slider<D: CoverageTarget<Color = Color>>(
        &self,
        target: &mut D,
    ) -> Result<(), D::Error> {
        target.fill_solid(&SLIDER, chrome::BLACK)?;
        target.fill_solid(&GUIDE, chrome::shade(chrome::GRAY, 145))?;
        let goal = RAIL.target();
        target.fill_solid(&goal, chrome::ORANGE)?;
        target.fill_solid(&goal.offset(-1), chrome::BLACK)?;
        let handle = RAIL.handle_at(self.slide.travel());
        let track = Rectangle::with_corners(
            RAIL.handle.top_left,
            handle.bottom_right().unwrap_or(handle.top_left),
        );
        target.fill_solid(&track, chrome::ORANGE)?;
        let (x, y) = (handle.center().x as f32, 257.0);
        let arrow = &mut OnBackground::new(&mut *target, chrome::ORANGE);
        smooth::rect(
            arrow,
            (x - 19.0, y - 4.0),
            (x + 11.0, y + 4.0),
            chrome::BLACK,
        )?;
        // The head narrows by a pixel a row either side of its middle, to a point 14 px on.
        for row in -14..14 {
            let off_middle = if row < 0 {
                -row as f32 - 0.5
            } else {
                row as f32 + 0.5
            };
            let top = y + row as f32;
            smooth::rect(
                arrow,
                (x + 9.0, top),
                (x + 23.0 - off_middle, top + 1.0),
                chrome::BLACK,
            )?;
        }
        Ok(())
    }
}
