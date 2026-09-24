//! The screens the settings panel opens: the brightness editor, the device page and the clear
//! confirm, with the zone picker in `picker`. They share a top cap with a hint, a button and a
//! 96 px icon, and most of them an open field between two rules.
//! `context/settings-panel/SETTINGS-PANEL-SPEC.md` is their design.

use core::fmt::Write as _;

use embedded_graphics::{
    draw_target::DrawTarget as _,
    prelude::{Point, Size},
    primitives::Rectangle,
};
use heapless::String;

use super::{
    gesture::{GestureEvent, Micros},
    icon::{Glyph, Tile},
    panel,
    picker::Picker,
    reveal::{Reveal, draw_revealed},
    screens::PeripheralState,
    text::{self, style},
};
use crate::chrome::{
    self, Color, CoverageTarget, FontdueRenderer, OnBackground, RgbColorExt as _, Window, FRAKTION,
    FRAKTION_BOLD,
};

const CENTER: Point = Point::new(233, 233);
const HINT_TOP: i32 = 48;
/// The top cap: a tap anywhere in it presses the button.
const TOP_CAP: i32 = 150;
pub const BUTTON: Rectangle = Rectangle::new(Point::new(72, 104), Size::new(110, 39));
pub const ICON: Tile = Tile { corner: Point::new(262, 90), module: 16, padding: 8 };
/// The field's rules, and the rows between them where a tap or drag acts on it.
const FIELD_TOP: i32 = 198;
const FIELD_BOTTOM: i32 = 318;
const RULE: i32 = 4;
const FIELD_TAPS: core::ops::Range<i32> = 186..330;
/// The column the field's text starts on, as on the clock face.
pub const TEXT_LEFT: i32 = 62;
const ICON_ROW: Micros = 30_000;
const HINT_REVEAL: Micros = 160_000;

const CLEAR: Glyph = [0b10001, 0b01010, 0b00100, 0b01010, 0b10001];

/// A setting to store, and what it takes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Store {
    Brightness(u8),
    ManualZone(super::clock::ZoneId),
    AutomaticZone,
    /// Erase every stored setting and go back to the defaults.
    Clear,
}

/// Names the zone, since a `ZoneId` changes when the zone data is rebuilt.
#[cfg(feature = "defmt")]
impl defmt::Format for Store {
    fn format(&self, f: defmt::Formatter) {
        match self {
            Self::Brightness(level) => defmt::write!(f, "Brightness({})", level),
            Self::ManualZone(zone) => {
                defmt::write!(f, "ManualZone({=str})", crate::tz::DATABASE.zone(*zone).name);
            }
            Self::AutomaticZone => defmt::write!(f, "AutomaticZone"),
            Self::Clear => defmt::write!(f, "Clear"),
        }
    }
}

/// What a second-level screen asks of the stage in one step.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Effects {
    /// Show the display at this level now.
    pub brightness: Option<u8>,
    pub store: Option<Store>,
}

/// Where a second-level screen goes after a step.
#[derive(Debug)]
pub enum Next {
    Stay,
    Panel,
    Open(Page),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Page {
    Brightness(Brightness),
    Device(Device),
    Clear(Clear),
    Picker(Picker),
}

/// How far a second-level screen's icon and hint have come in since it opened.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Accents {
    pub icon_rows: u8,
    pub hint: u8,
}

impl Accents {
    pub const FULL: Self = Self { icon_rows: 5, hint: u8::MAX };

    #[must_use]
    pub fn at(opened: Micros, now: Micros) -> Self {
        let elapsed = now.saturating_sub(opened);
        Self {
            icon_rows: (elapsed / ICON_ROW + 1).min(5) as u8,
            hint: (elapsed.min(HINT_REVEAL) * 255 / HINT_REVEAL) as u8,
        }
    }
}

impl Page {
    /// Handles a gesture. A cover is the stage's, and never reaches a page.
    pub fn handle(&mut self, event: &GestureEvent, peripherals: &PeripheralState, effects: &mut Effects) -> Next {
        match self {
            Self::Brightness(editor) => editor.handle(event, effects),
            Self::Device(page) => page.handle(event, peripherals),
            Self::Clear(confirm) => confirm.handle(event, effects),
            Self::Picker(picker) => picker.handle(event, peripherals, effects),
        }
    }

    /// Discards anything unfinished, as leaving by cover does.
    pub fn discard(&self, effects: &mut Effects) {
        if let Self::Brightness(editor) = self {
            editor.restore(effects);
        }
    }

    /// Advances anything that moves on its own, and says whether it still does.
    pub fn step(&mut self, now: Micros) -> bool {
        match self {
            Self::Picker(picker) => picker.step(now),
            _ => false,
        }
    }

    pub fn draw<D: CoverageTarget<Color = Color>>(
        &self,
        peripherals: &PeripheralState,
        accents: Accents,
        font: &FontdueRenderer<'static, Color>,
        target: &mut D,
    ) -> Result<(), D::Error> {
        match self {
            Self::Brightness(editor) => editor.draw(accents, font, target),
            Self::Device(page) => page.draw(peripherals, accents, font, target),
            Self::Clear(confirm) => confirm.draw(accents, font, target),
            Self::Picker(picker) => picker.draw(peripherals, accents, font, target),
        }
    }
}

pub fn hint_style(font: &FontdueRenderer<'static, Color>) -> FontdueRenderer<'static, Color> {
    style(font, chrome::GRAY, 14, FRAKTION)
}

/// The ring, the hint, the button and the icon that every second-level screen has.
pub fn draw_cap<D: CoverageTarget<Color = Color>>(
    hint: &str,
    button: &str,
    glyph: &Glyph,
    icon: Color,
    accents: Accents,
    font: &FontdueRenderer<'static, Color>,
    target: &mut D,
) -> Result<(), D::Error> {
    super::smooth::perimeter().draw(&mut OnBackground::new(&mut *target, chrome::BLACK), chrome::GRAY);
    let style = hint_style(font);
    let pen = Point::new(
        text::pen_x_for_ink_centre(&style, hint, CENTER.x as f32),
        text::baseline_for_ink_top(&style, hint, HINT_TOP),
    );
    draw_revealed(&style, hint, pen, Reveal::of(accents.hint, hint.len()), &mut OnBackground::new(&mut *target, chrome::BLACK))?;
    draw_button(button, BUTTON, false, font, target)?;
    ICON.draw(glyph, icon, accents.icon_rows, target)
}

/// A box with a label: outlined for an action, filled `GRAY` for a mode in force.
pub fn draw_button<D: CoverageTarget<Color = Color>>(
    label: &str,
    bounds: Rectangle,
    filled: bool,
    font: &FontdueRenderer<'static, Color>,
    target: &mut D,
) -> Result<(), D::Error> {
    target.fill_solid(&bounds, chrome::GRAY)?;
    let (inside, color) = if filled {
        (chrome::GRAY, chrome::BLACK)
    } else {
        target.fill_solid(&bounds.offset(-2), chrome::BLACK)?;
        (chrome::BLACK, chrome::WHITE)
    };
    let style = style(font, color, 16, FRAKTION_BOLD);
    let middle = bounds.top_left + Point::new(bounds.size.width as i32, bounds.size.height as i32);
    let pen = Point::new(
        text::pen_x_for_ink_centre(&style, label, (bounds.top_left.x + middle.x) as f32 / 2.0),
        text::baseline_for_ink_middle(&style, label, (bounds.top_left.y + middle.y) as f32 / 2.0),
    );
    style.draw_on_baseline(label, pen, &mut OnBackground::new(&mut *target, inside))
}

/// The open field: white rules on the face's band rows, cut at the page's circle, and the
/// locators that mark the text column.
pub fn draw_field<D: CoverageTarget<Color = Color>>(target: &mut D) -> Result<(), D::Error> {
    let field = &mut OnBackground::new(&mut *target, chrome::BLACK);
    for rows in [FIELD_TOP..FIELD_TOP + RULE, FIELD_BOTTOM - RULE..FIELD_BOTTOM] {
        super::smooth::disc_rows(field, rows, CENTER, 232.0, chrome::WHITE)?;
    }
    for x in [TEXT_LEFT - 14, 400] {
        for top in [FIELD_TOP + RULE, FIELD_BOTTOM - 16] {
            target.fill_solid(&Rectangle::new(Point::new(x, top), Size::new(3, 12)), chrome::WHITE)?;
        }
    }
    Ok(())
}

fn in_top_cap(point: Point) -> bool {
    point.y < TOP_CAP
}

fn in_field(point: Point) -> bool {
    FIELD_TAPS.contains(&point.y)
}

/// The brightness editor: any whole percentage from 10 to 100, each shown on the display as the
/// finger reaches it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Brightness {
    opened: u8,
    level: u8,
}

const TRACK_LEFT: i32 = 66;
const TRACK_RIGHT: i32 = 400;
const TRACK_TOP: i32 = 276;
const TRACK_HEIGHT: u32 = 24;
const STEPS: i32 = 10;
const STEP_GAP: i32 = 6;
/// The lowest level, which keeps the screen readable enough to undo.
const FLOOR_PERCENT: i32 = 10;

impl Brightness {
    #[must_use]
    pub fn new(level: u8) -> Self {
        Self { opened: level, level }
    }

    /// The display's level for `percent` of full.
    #[must_use]
    pub fn level_of(percent: i32) -> u8 {
        ((255 * percent + 50) / 100) as u8
    }

    /// The percentage under column `x`: the track runs from nothing at its left end to full at
    /// its right, and stops at the floor.
    fn percent_at(x: i32) -> i32 {
        let across = (x - TRACK_LEFT) as f32 / (TRACK_RIGHT - TRACK_LEFT) as f32;
        (libm::roundf(across * 100.0) as i32).clamp(FLOOR_PERCENT, 100)
    }

    fn restore(&self, effects: &mut Effects) {
        if self.level != self.opened {
            effects.brightness = Some(self.opened);
        }
    }

    fn handle(&mut self, event: &GestureEvent, effects: &mut Effects) -> Next {
        match *event {
            GestureEvent::DragStart(drag) | GestureEvent::DragMove(drag) | GestureEvent::DragEnd(drag)
                if in_field(drag.start) =>
            {
                let level = Self::level_of(Self::percent_at(drag.current.x));
                if level != self.level {
                    self.level = level;
                    effects.brightness = Some(level);
                }
                Next::Stay
            }
            GestureEvent::Tap(point) if in_top_cap(point) => {
                self.restore(effects);
                Next::Panel
            }
            GestureEvent::Tap(point) if in_field(point) => {
                effects.store = Some(Store::Brightness(self.level));
                Next::Panel
            }
            _ => Next::Stay,
        }
    }

    fn draw<D: CoverageTarget<Color = Color>>(
        &self,
        accents: Accents,
        font: &FontdueRenderer<'static, Color>,
        target: &mut D,
    ) -> Result<(), D::Error> {
        draw_cap("DRAG TO SET BRIGHTNESS", "CANCEL", &panel::BRIGHTNESS, chrome::WHITE, accents, font, target)?;
        draw_field(target)?;
        let percent = panel::percent(self.level);
        let mut level = String::<4>::new();
        _ = write!(level, "{percent}%");
        let field = &mut OnBackground::new(&mut *target, chrome::BLACK);
        let big = style(font, chrome::WHITE, 56, FRAKTION_BOLD);
        let pen = Point::new(
            text::pen_x_for_ink_left(&big, &level, TEXT_LEFT),
            text::baseline_for_ink_bottom(&big, &level, 262),
        );
        big.draw_on_baseline(&level, pen, field)?;
        // Each cell is a tenth of full; the one the level falls in fills as far as it reaches.
        let width = (TRACK_RIGHT - TRACK_LEFT - STEP_GAP * (STEPS - 1)) as f32 / STEPS as f32;
        for step in 0..STEPS {
            let left = libm::roundf(TRACK_LEFT as f32 + step as f32 * (width + STEP_GAP as f32)) as i32;
            let right = libm::roundf(left as f32 + width) as i32;
            let cell = Rectangle::new(Point::new(left, TRACK_TOP), Size::new((right - left) as u32, TRACK_HEIGHT));
            let filled = (i32::from(percent) - 10 * step).clamp(0, 10);
            if filled < 10 {
                target.fill_solid(&cell, chrome::GRAY)?;
                target.fill_solid(&cell.offset(-2), chrome::BLACK)?;
            }
            if filled > 0 {
                let reach = libm::roundf((right - left) as f32 * filled as f32 / 10.0) as u32;
                target.fill_solid(&Rectangle::new(cell.top_left, Size::new(reach, TRACK_HEIGHT)), chrome::WHITE)?;
            }
        }
        let small = hint_style(font);
        let field = &mut OnBackground::new(&mut *target, chrome::BLACK);
        let low = Point::new(text::pen_x_for_ink_left(&small, "10", TRACK_LEFT), text::baseline_for_ink_top(&small, "10", 326));
        small.draw_on_baseline("10", low, field)?;
        let high = Point::new(
            text::pen_x_for_ink_right(&small, "100", TRACK_RIGHT - 1),
            text::baseline_for_ink_top(&small, "100", 326),
        );
        small.draw_on_baseline("100", high, field)?;
        let keep = "TAP TO KEEP";
        let pen = Point::new(
            text::pen_x_for_ink_centre(&small, keep, CENTER.x as f32),
            text::baseline_for_ink_top(&small, keep, 372),
        );
        small.draw_on_baseline(keep, pen, field)
    }
}

/// The device page: what the device reports, the data's attribution, and the way to clear
/// settings, in a list that scrolls under the top cap.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Device {
    scroll: i32,
    /// The scroll when the current drag started.
    grabbed: Option<i32>,
}

const LIST_TOP: i32 = 196;
const LIST_BOTTOM: i32 = 440;
const FIRST_ROW: i32 = 206;
const ROW_PITCH: i32 = 30;
const ROW_KEY_LEFT: i32 = TEXT_LEFT + 8;
const ROW_VALUE_RIGHT: i32 = 466 - TEXT_LEFT - 8;
const ROWS: usize = 5;
const ATTRIBUTION: usize = 5;
const LINE_PITCH: i32 = 20;
const CLEAR_LEFT: i32 = 143;
const CLEAR_WIDTH: u32 = 180;
const CLEAR_HEIGHT: u32 = 44;
/// Where the list stops scrolling: its last row sits this far down.
const LIST_END: i32 = 420;

impl Device {
    fn attribution_top() -> i32 {
        FIRST_ROW + ROW_PITCH * ROWS as i32 + 10
    }

    fn clear_box(&self) -> Rectangle {
        let top = Self::attribution_top() + LINE_PITCH * ATTRIBUTION as i32 + 18 - self.scroll;
        Rectangle::new(Point::new(CLEAR_LEFT, top), Size::new(CLEAR_WIDTH, CLEAR_HEIGHT))
    }

    fn max_scroll() -> i32 {
        let end = Self::attribution_top() + LINE_PITCH * ATTRIBUTION as i32 + 18 + CLEAR_HEIGHT as i32;
        (end - LIST_END).max(0)
    }

    fn handle(&mut self, event: &GestureEvent, _: &PeripheralState) -> Next {
        match *event {
            GestureEvent::DragStart(drag) => {
                self.grabbed = Some(self.scroll);
                self.scroll = (self.scroll - drag.offset().y).clamp(0, Self::max_scroll());
            }
            GestureEvent::DragMove(drag) | GestureEvent::DragEnd(drag) => {
                if let Some(from) = self.grabbed {
                    self.scroll = (from - drag.offset().y).clamp(0, Self::max_scroll());
                }
                if matches!(event, GestureEvent::DragEnd(_)) {
                    self.grabbed = None;
                }
            }
            GestureEvent::Tap(point) if in_top_cap(point) => return Next::Panel,
            GestureEvent::Tap(point)
                if (LIST_TOP..=LIST_BOTTOM).contains(&point.y) && self.clear_box().contains(point) =>
            {
                return Next::Open(Page::Clear(Clear::default()));
            }
            _ => {}
        }
        Next::Stay
    }

    fn rows(peripherals: &PeripheralState) -> [(&'static str, String<16>); ROWS] {
        let text = |value: core::fmt::Arguments<'_>| {
            let mut text = String::new();
            _ = text.write_fmt(value);
            text
        };
        let battery = match peripherals.battery {
            Some(battery) if battery.present => text(format_args!(
                "{}%  {}.{:02} V",
                battery.percent.min(100),
                battery.millivolts / 1000,
                battery.millivolts % 1000 / 10
            )),
            Some(_) => text(format_args!("NONE")),
            None => text(format_args!("--")),
        };
        let power = match peripherals.battery {
            Some(battery) => match (battery.usb, battery.charging) {
                (true, true) => text(format_args!("USB  CHARGING")),
                (true, false) => text(format_args!("USB")),
                (false, true) => text(format_args!("CHARGING")),
                (false, false) => text(format_args!("BATTERY")),
            },
            None => text(format_args!("--")),
        };
        let gnss = &peripherals.gnss;
        [
            ("VERSION", text(format_args!("{}", peripherals.firmware))),
            ("BATTERY", battery),
            ("POWER", power),
            ("GNSS", text(format_args!("{}", if gnss.fix { "FIX" } else { "NO FIX" }))),
            ("SATELLITES", text(format_args!("{} OF {}", gnss.in_use, gnss.in_view))),
        ]
    }

    fn attribution() -> [String<40>; ATTRIBUTION] {
        let upper = |parts: &[&str]| {
            let mut line = String::new();
            for part in parts {
                for c in part.chars() {
                    _ = line.push(c.to_ascii_uppercase());
                }
            }
            line
        };
        let database = &crate::tz::DATABASE;
        [
            upper(&["TIME ZONE BOUNDARIES:"]),
            upper(&["TIMEZONE-BOUNDARY-BUILDER ", database.boundary_release()]),
            upper(&["\u{a9} OPENSTREETMAP CONTRIBUTORS"]),
            upper(&["ODBL 1.0"]),
            upper(&["ZONE RULES: IANA TZDATA ", database.tzdata_release()]),
        ]
    }

    fn draw<D: CoverageTarget<Color = Color>>(
        &self,
        peripherals: &PeripheralState,
        accents: Accents,
        font: &FontdueRenderer<'static, Color>,
        target: &mut D,
    ) -> Result<(), D::Error> {
        {
            let clip = Rectangle::with_corners(Point::new(0, LIST_TOP), Point::new(465, LIST_BOTTOM));
            let list = &mut Window::new(&mut *target, Point::zero(), clip);
            let key_style = style(font, chrome::GRAY, 14, FRAKTION_BOLD);
            let value_style = style(font, chrome::WHITE, 16, FRAKTION);
            let rule = chrome::BLACK.lerp(&chrome::GRAY, 128);
            let mut top = FIRST_ROW - self.scroll;
            for (key, value) in Self::rows(peripherals) {
                let field = &mut OnBackground::new(&mut *list, chrome::BLACK);
                let pen = Point::new(ROW_KEY_LEFT, text::baseline_for_ink_top(&key_style, key, top));
                key_style.draw_on_baseline(key, pen, field)?;
                let pen = Point::new(
                    text::pen_x_for_ink_right(&value_style, &value, ROW_VALUE_RIGHT - 1),
                    text::baseline_for_ink_top(&value_style, &value, top - 1),
                );
                value_style.draw_on_baseline(&value, pen, field)?;
                let line = Rectangle::new(
                    Point::new(ROW_KEY_LEFT, top + 16),
                    Size::new((ROW_VALUE_RIGHT - ROW_KEY_LEFT) as u32, 1),
                );
                list.fill_solid(&line, rule)?;
                top += ROW_PITCH;
            }
            let small = hint_style(font);
            let mut top = Self::attribution_top() - self.scroll;
            for line in Self::attribution() {
                let pen = Point::new(
                    text::pen_x_for_ink_centre(&small, &line, CENTER.x as f32),
                    text::baseline_for_ink_top(&small, &line, top),
                );
                small.draw_on_baseline(&line, pen, &mut OnBackground::new(&mut *list, chrome::BLACK))?;
                top += LINE_PITCH;
            }
            draw_button("CLEAR SETTINGS", self.clear_box(), false, font, list)?;
        }
        super::smooth::disc_rows(
            &mut OnBackground::new(&mut *target, chrome::BLACK),
            LIST_TOP - 1..LIST_TOP,
            CENTER,
            232.0,
            chrome::GRAY,
        )?;
        draw_cap("DEVICE", "BACK", &panel::DEVICE, chrome::WHITE, accents, font, target)
    }
}

/// The clear confirm: a handle dragged across to a target erases the settings.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Clear {
    /// The handle's left edge past its start, while a drag that started on it holds it.
    dragged: Option<i32>,
}

const HANDLE: i32 = 64;
const HANDLE_TOP: i32 = 226;
const RAIL_LEFT: i32 = TEXT_LEFT;
const RAIL_RIGHT: i32 = 466 - TEXT_LEFT;
const TARGET_LEFT: i32 = RAIL_RIGHT - HANDLE;
const TRAVEL: i32 = TARGET_LEFT - RAIL_LEFT;
const WARNING: [&str; 2] = ["ERASES ZONE, LAST FIX ZONE", "AND BRIGHTNESS"];

impl Clear {
    fn handle_at(travel: i32) -> Rectangle {
        Rectangle::new(Point::new(RAIL_LEFT + travel, HANDLE_TOP), Size::new_equal(HANDLE as u32))
    }

    fn handle(&mut self, event: &GestureEvent, effects: &mut Effects) -> Next {
        match *event {
            GestureEvent::DragStart(drag) if Self::handle_at(0).contains(drag.start) => {
                self.dragged = Some(drag.offset().x.clamp(0, TRAVEL));
            }
            GestureEvent::DragMove(drag) if self.dragged.is_some() => {
                self.dragged = Some(drag.offset().x.clamp(0, TRAVEL));
            }
            GestureEvent::DragEnd(drag) if self.dragged.is_some() => {
                self.dragged = None;
                // The handle's middle over the target counts as reaching it.
                if drag.offset().x.clamp(0, TRAVEL) + HANDLE / 2 >= TRAVEL {
                    effects.store = Some(Store::Clear);
                    return Next::Panel;
                }
            }
            GestureEvent::Tap(point) if in_top_cap(point) => {
                return Next::Open(Page::Device(Device::default()));
            }
            _ => {}
        }
        Next::Stay
    }

    fn draw<D: CoverageTarget<Color = Color>>(
        &self,
        accents: Accents,
        font: &FontdueRenderer<'static, Color>,
        target: &mut D,
    ) -> Result<(), D::Error> {
        draw_cap("DRAG ACROSS TO CLEAR", "CANCEL", &CLEAR, chrome::ORANGE, accents, font, target)?;
        draw_field(target)?;
        let middle = HANDLE_TOP + HANDLE / 2;
        target.fill_solid(
            &Rectangle::new(Point::new(RAIL_LEFT, middle - 1), Size::new((RAIL_RIGHT - RAIL_LEFT) as u32, 2)),
            chrome::GRAY,
        )?;
        let goal = Self::handle_at(TRAVEL);
        target.fill_solid(&goal, chrome::GRAY)?;
        target.fill_solid(&goal.offset(-2), chrome::BLACK)?;
        let travel = self.dragged.unwrap_or(0);
        let handle = Self::handle_at(travel);
        if travel > 0 {
            let swept = Rectangle::with_corners(
                Point::new(RAIL_LEFT, HANDLE_TOP),
                Point::new(handle.top_left.x + HANDLE - 1, HANDLE_TOP + HANDLE - 1),
            );
            target.fill_solid(&swept, chrome::ORANGE)?;
        }
        target.fill_solid(&handle, chrome::ORANGE)?;
        let arrow = handle.top_left + Point::new(16, HANDLE / 2);
        target.fill_solid(&Rectangle::new(arrow - Point::new(0, 3), Size::new(26, 6)), chrome::BLACK)?;
        for k in 0..4 {
            let chevron = Rectangle::new(
                arrow + Point::new(18 + 3 * k, -12 + 3 * k),
                Size::new(3, (24 - 6 * k) as u32),
            );
            target.fill_solid(&chevron, chrome::BLACK)?;
        }
        let small = hint_style(font);
        let field = &mut OnBackground::new(&mut *target, chrome::BLACK);
        for (line, top) in WARNING.into_iter().zip([338, 356]) {
            let pen = Point::new(
                text::pen_x_for_ink_centre(&small, line, CENTER.x as f32),
                text::baseline_for_ink_top(&small, line, top),
            );
            small.draw_on_baseline(line, pen, field)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_drag_sets_any_percentage_from_ten_to_full() {
        assert_eq!(Brightness::level_of(10), 26);
        assert_eq!(Brightness::level_of(47), 120);
        assert_eq!(Brightness::level_of(100), 255);
        assert_eq!([60, 66, 99, 223, 400, 460].map(Brightness::percent_at), [10, 10, 10, 47, 100, 100]);
    }

    #[test]
    fn the_device_list_scrolls_to_its_end() {
        assert_eq!(Device::max_scroll(), 108);
    }
}
