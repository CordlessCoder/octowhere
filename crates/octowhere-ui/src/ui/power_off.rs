//! The confirmation the power key's long press opens, and what shows while the board powers
//! off. A placeholder until the screen has a design round.

use embedded_graphics::{
    prelude::{Point, Size},
    primitives::Rectangle,
};

use super::{
    gesture::{GestureEvent, Micros},
    second::{self, Slide},
    text::{self, style},
};
use crate::chrome::{self, Color, CoverageTarget, FontdueRenderer, OnBackground};

/// Left untouched this long, the confirmation cancels itself.
pub const TIMEOUT: Micros = 10_000_000;
/// How long the level takes to go dark once the slide confirms.
pub const FADE: Micros = 300_000;

const CENTER_X: f32 = 233.0;

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
    opened: Micros,
    confirmed: Option<Micros>,
}

impl PowerOff {
    #[must_use]
    pub fn new(now: Micros) -> Self {
        Self {
            slide: Slide::default(),
            opened: now,
            confirmed: None,
        }
    }

    /// Handles a gesture. Once confirmed, nothing more is taken.
    pub fn handle(&mut self, event: &GestureEvent, now: Micros) -> Answer {
        if self.confirmed.is_some() {
            return Answer::Stay;
        }
        if self.slide.handle(event) {
            self.slide.arrive();
            self.confirmed = Some(now);
            return Answer::Confirm;
        }
        match *event {
            GestureEvent::Tap(point) if second::in_top_cap(point) => Answer::Cancel,
            _ => Answer::Stay,
        }
    }

    /// When the confirmation cancels itself, while it has not been confirmed.
    #[must_use]
    pub fn deadline(&self) -> Option<Micros> {
        self.confirmed.is_none().then_some(self.opened + TIMEOUT)
    }

    /// When the slide confirmed it.
    #[must_use]
    pub fn confirmed(&self) -> Option<Micros> {
        self.confirmed
    }

    pub fn draw<D: CoverageTarget<Color = Color>>(
        &self,
        font: &FontdueRenderer<'static, Color>,
        target: &mut D,
    ) -> Result<(), D::Error> {
        let title = style(font, chrome::WHITE, 27, chrome::SHAPIRO);
        let pen = Point::new(
            text::pen_x_for_ink_centre(&title, "POWER OFF", CENTER_X),
            text::baseline_for_ink_top(&title, "POWER OFF", 29),
        );
        title.draw_on_baseline(
            "POWER OFF",
            pen,
            &mut OnBackground::new(&mut *target, chrome::BLACK),
        )?;
        for y in [83, 145] {
            target.fill_solid(
                &Rectangle::new(Point::new(83, y), Size::new(300, 1)),
                chrome::shade(chrome::GRAY, 145),
            )?;
        }
        let lines: &[&str] = if self.confirmed.is_some() {
            &["POWERING OFF"]
        } else {
            second::draw_button("CANCEL", second::BUTTON, false, font, target)?;
            &["SLIDE TO POWER OFF", "HOLD THE KEY TO WAKE"]
        };
        self.slide.draw(chrome::ORANGE, target)?;
        let small = second::hint_style(font);
        let field = &mut OnBackground::new(&mut *target, chrome::BLACK);
        for (line, top) in lines.iter().zip([338, 356]) {
            let pen = Point::new(
                text::pen_x_for_ink_centre(&small, line, CENTER_X),
                text::baseline_for_ink_top(&small, line, top),
            );
            small.draw_on_baseline(line, pen, field)?;
        }
        Ok(())
    }
}
