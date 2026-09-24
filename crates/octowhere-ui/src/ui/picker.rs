//! The zone picker: an offset chosen by local time, then a zone at that offset.
//! `context/clock_face_design/CLOCK-FACE-SPEC.md`, "Zone picker", is its design.

use alloc::vec::Vec;
use core::fmt::Write as _;

use embedded_graphics::{
    prelude::{Point, Size},
    primitives::Rectangle,
};
use heapless::String;

use super::{
    clock::{DateTime, ZoneId, ZoneMode},
    clock_screen,
    gesture::{GestureEvent, Micros},
    panel,
    screens::PeripheralState,
    second::{self, Accents, Effects, Next, Store, TEXT_LEFT},
    text::{self, style},
};
use crate::{
    chrome::{self, Color, CoverageTarget, FontdueRenderer, OnBackground, FRAKTION, FRAKTION_BOLD},
    tz::DATABASE,
};

const CENTER_X: f32 = 233.0;
const TOP_CAP: i32 = 150;
const BAND: core::ops::Range<i32> = 186..330;
const BOTTOM_CAP: i32 = 356;
const AUTO: Rectangle = Rectangle::new(Point::new(183, 366), Size::new(99, 39));
/// Travel per step of the list, and the release speed, in pixels per second, past which it
/// keeps stepping.
const STEP_TRAVEL: f32 = 40.0;
const FLING: f32 = 500.0;
/// How quickly a fling slows: its speed falls by a factor of e every this.
const FLING_TIME_CONSTANT: f32 = 200_000.0;
const FLING_STOP: f32 = 60.0;
/// Times a year apart from any zone's last rule change, in January and July, for a clock that
/// cannot be trusted. A zone is listed under the offset it keeps at each.
const RULES_ONLY: [i64; 2] = [4_102_444_800, 4_118_083_200];
const SELECTED_PENS: [i32; 5] = [57, 109, 148, 188, 240];
const NEIGHBOURS: [f32; 2] = [174.0, 341.0];

#[derive(Clone, Debug, PartialEq)]
enum Step {
    /// The distinct offsets in force, ascending, in seconds.
    Offset { offsets: Vec<i32> },
    /// The zones at `offset`, in the order they are listed, which is nearest first when the
    /// position is known.
    Zone { offset: i32, zones: Vec<ZoneId>, nearest: bool },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Picker {
    step: Step,
    index: usize,
    /// The index and travel when the current drag started.
    grabbed: Option<usize>,
    /// A released drag still stepping: its travel so far, speed, and when it was last advanced.
    fling: Option<(f32, f32, Micros)>,
    /// Where the fling started from.
    flung_from: usize,
}

impl Eq for Picker {}

fn time_of(peripherals: &PeripheralState) -> Option<i64> {
    peripherals.clock.clock.utc.filter(|_| !peripherals.clock.clock.stopped)
}

/// A zone's offset at `unix`, or without a time, each offset it keeps during a year.
fn offsets_of(zone: &crate::tz::Zone, unix: Option<i64>) -> heapless::Vec<crate::tz::Offset<'static>, 2> {
    let mut offsets = heapless::Vec::new();
    for time in unix.map_or(RULES_ONLY.to_vec(), |unix| alloc::vec![unix]) {
        let offset = zone.at(time);
        if !offsets.iter().any(|kept: &crate::tz::Offset<'_>| kept.utc_offset == offset.utc_offset) {
            _ = offsets.push(offset);
        }
    }
    offsets
}

/// The distinct offsets across the zone table, ascending: those in force at `unix`, or without a
/// time, every offset a zone keeps during a year.
fn offsets_at(unix: Option<i64>) -> Vec<i32> {
    let mut offsets: Vec<i32> = DATABASE
        .zones()
        .flat_map(|zone| offsets_of(&zone, unix).into_iter().map(|offset| offset.utc_offset))
        .collect();
    offsets.sort_unstable();
    offsets.dedup();
    offsets
}

/// The zones at `offset` at `unix`, or at any time of year without it: nearest `position` first by their reference points, or A to
/// Z by city without a position. Zones without a reference point follow, A to Z, then the `Etc`
/// zones.
fn zones_at(offset: i32, unix: Option<i64>, position: Option<(i32, i32)>) -> Vec<ZoneId> {
    let mut zones: Vec<_> = DATABASE
        .zones()
        .filter(|zone| offsets_of(zone, unix).iter().any(|each| each.utc_offset == offset))
        .map(|zone| zone.id)
        .collect();
    zones.sort_by(|&a, &b| {
        let key = |id| {
            let zone = DATABASE.zone(id);
            let distance = position
                .and_then(|position| Some(distance(position, zone.reference()?)))
                .unwrap_or(f32::INFINITY);
            (zone.name.starts_with("Etc/"), distance, city(zone.name))
        };
        let (a, b) = (key(a), key(b));
        (a.0, a.1).partial_cmp(&(b.0, b.1)).unwrap_or(core::cmp::Ordering::Equal).then(a.2.cmp(&b.2))
    });
    zones
}

/// A measure of the distance between two points in 1e-7 degrees, good enough to rank by: the
/// flat-map distance in degrees with longitude scaled by the latitude.
fn distance(a: (i32, i32), b: (i32, i32)) -> f32 {
    let latitude = |point: (i32, i32)| point.0 as f32 / 1e7;
    let mut across = (b.1 - a.1) as f32 / 1e7;
    if across > 180.0 {
        across -= 360.0;
    } else if across < -180.0 {
        across += 360.0;
    }
    let middle = (latitude(a) + latitude(b)) / 2.0;
    let across = across * libm::cosf(middle.to_radians());
    libm::hypotf(latitude(b) - latitude(a), across)
}

/// A zone's city: the last part of its name, in capitals with spaces for underscores, or
/// `AT SEA` for an `Etc` zone.
fn city(name: &str) -> String<32> {
    let mut city = String::new();
    if name.starts_with("Etc/") {
        _ = city.push_str("AT SEA");
        return city;
    }
    for c in name.rsplit('/').next().unwrap_or(name).chars() {
        _ = city.push(if c == '_' { ' ' } else { c.to_ascii_uppercase() });
    }
    city
}

fn signed(offset: i32) -> String<8> {
    let mut text = String::new();
    let sign = if offset < 0 { '-' } else { '+' };
    let minutes = offset.unsigned_abs() / 60;
    _ = write!(text, "{sign}{:02}:{:02}", minutes / 60, minutes % 60);
    text
}

fn clock_at(unix: Option<i64>, offset: i32) -> String<5> {
    let mut text = String::new();
    match unix {
        Some(unix) => {
            let time = DateTime::from_unix(unix + i64::from(offset));
            _ = write!(text, "{:02}:{:02}", time.hour, time.minute);
        }
        None => _ = text.push_str("--:--"),
    }
    text
}

impl Picker {
    /// Opens on the offset in force, or on +00:00 without one.
    #[must_use]
    pub fn new(peripherals: &PeripheralState) -> Self {
        let offsets = offsets_at(time_of(peripherals));
        let current = clock_screen::offset(&peripherals.clock).map_or(0, |offset| offset.utc_offset);
        let index = offsets
            .iter()
            .position(|&offset| offset >= current)
            .unwrap_or(offsets.len().saturating_sub(1));
        Self { step: Step::Offset { offsets }, index, grabbed: None, fling: None, flung_from: 0 }
    }

    fn len(&self) -> usize {
        match &self.step {
            Step::Offset { offsets } => offsets.len(),
            Step::Zone { zones, .. } => zones.len(),
        }
    }

    fn stepped(&self, from: usize, travel: f32) -> usize {
        let steps = libm::truncf(-travel / STEP_TRAVEL) as isize;
        (from as isize + steps).clamp(0, self.len() as isize - 1) as usize
    }

    pub fn handle(&mut self, event: &GestureEvent, peripherals: &PeripheralState, effects: &mut Effects) -> Next {
        match *event {
            GestureEvent::Down(_) => self.fling = None,
            GestureEvent::DragStart(drag) => {
                self.grabbed = Some(self.index);
                self.index = self.stepped(self.index, drag.offset().y as f32);
            }
            GestureEvent::DragMove(drag) => {
                if let Some(from) = self.grabbed {
                    self.index = self.stepped(from, drag.offset().y as f32);
                }
            }
            GestureEvent::DragEnd(drag) => {
                if let Some(from) = self.grabbed.take() {
                    self.index = self.stepped(from, drag.offset().y as f32);
                    let speed = drag.velocity.1;
                    if libm::fabsf(speed) > FLING {
                        // Carry on from the travel already made, so no step repeats.
                        let travel = drag.offset().y as f32;
                        self.flung_from = from;
                        self.fling = Some((travel, speed, 0));
                    }
                }
            }
            GestureEvent::Tap(point) => return self.tap(point, peripherals, effects),
            GestureEvent::None => {}
        }
        Next::Stay
    }

    fn tap(&mut self, point: Point, peripherals: &PeripheralState, effects: &mut Effects) -> Next {
        let unix = time_of(peripherals);
        match &self.step {
            Step::Offset { .. } if point.y < TOP_CAP => Next::Panel,
            Step::Offset { offsets } if BAND.contains(&point.y) => {
                let offset = offsets[self.index];
                let position = peripherals.gnss.position;
                let zones = zones_at(offset, unix, position);
                let current = peripherals.clock.zone.zone;
                self.index = zones.iter().position(|&zone| Some(zone) == current).unwrap_or(0);
                self.step = Step::Zone { offset, zones, nearest: position.is_some() };
                self.fling = None;
                Next::Stay
            }
            Step::Offset { .. } if point.y >= BOTTOM_CAP => {
                effects.store = Some(Store::AutomaticZone);
                Next::Panel
            }
            Step::Zone { offset, .. } if point.y < TOP_CAP => {
                let offset = *offset;
                let offsets = offsets_at(unix);
                self.index = offsets.iter().position(|&each| each == offset).unwrap_or(0);
                self.step = Step::Offset { offsets };
                self.fling = None;
                Next::Stay
            }
            Step::Zone { zones, .. } if BAND.contains(&point.y) => {
                effects.store = Some(Store::ManualZone(zones[self.index]));
                Next::Panel
            }
            _ => Next::Stay,
        }
    }

    /// Advances a fling, and says whether it is still going.
    pub fn step(&mut self, now: Micros) -> bool {
        let Some((travel, speed, last)) = self.fling else {
            return false;
        };
        if last == 0 {
            self.fling = Some((travel, speed, now));
            return true;
        }
        let elapsed = now.saturating_sub(last) as f32;
        let decay = libm::expf(-elapsed / FLING_TIME_CONSTANT);
        // The distance covered while the speed decays from `speed` to `speed * decay`.
        let travel = travel + speed * FLING_TIME_CONSTANT / 1e6 * (1.0 - decay);
        let speed = speed * decay;
        self.index = self.stepped(self.flung_from, travel);
        let at_end = self.index == 0 || self.index + 1 == self.len();
        self.fling = (libm::fabsf(speed) > FLING_STOP && !at_end).then_some((travel, speed, now));
        self.fling.is_some()
    }

    pub fn draw<D: CoverageTarget<Color = Color>>(
        &self,
        peripherals: &PeripheralState,
        accents: Accents,
        font: &FontdueRenderer<'static, Color>,
        target: &mut D,
    ) -> Result<(), D::Error> {
        match &self.step {
            Step::Offset { offsets } => self.draw_offsets(offsets, peripherals, accents, font, target),
            Step::Zone { .. } => self.draw_zones(peripherals, accents, font, target),
        }
    }

    fn neighbours(&self) -> [Option<usize>; 2] {
        [self.index.checked_sub(1), Some(self.index + 1).filter(|&next| next < self.len())]
    }

    fn draw_neighbour<D: CoverageTarget<Color = Color>>(
        line: &str,
        row: f32,
        font: &FontdueRenderer<'static, Color>,
        target: &mut D,
    ) -> Result<(), D::Error> {
        let style = style(font, chrome::GRAY, 23, FRAKTION);
        // Centred on the row by the digits' and capitals' ink, so every row sits alike.
        let pen = Point::new(
            text::pen_x_for_ink_left(&style, line, TEXT_LEFT),
            text::baseline_for_ink_middle(&style, "0", row),
        );
        style.draw_on_baseline(line, pen, &mut OnBackground::new(target, chrome::BLACK))
    }

    fn draw_offsets<D: CoverageTarget<Color = Color>>(
        &self,
        offsets: &[i32],
        peripherals: &PeripheralState,
        accents: Accents,
        font: &FontdueRenderer<'static, Color>,
        target: &mut D,
    ) -> Result<(), D::Error> {
        second::draw_cap("DRAG TO YOUR LOCAL TIME", "CANCEL", &panel::ZONE, chrome::WHITE, accents, font, target)?;
        second::draw_field(target)?;
        let unix = time_of(peripherals);
        let offset = offsets[self.index];
        let big = style(font, chrome::WHITE, 86, FRAKTION_BOLD);
        let field = &mut OnBackground::new(&mut *target, chrome::BLACK);
        let time = clock_at(unix, offset);
        // The pens put the colon in a narrower cell than the digits.
        for (index, pen) in SELECTED_PENS.into_iter().enumerate() {
            big.draw_on_baseline(&time[index..index + 1], Point::new(pen, 289), field)?;
        }
        let label = style(font, chrome::GRAY, 16, FRAKTION_BOLD);
        let pen = Point::new(
            text::pen_x_for_ink_left(&label, "UTC", 311),
            text::baseline_for_ink_top(&label, "UTC", 228),
        );
        label.draw_on_baseline("UTC", pen, field)?;
        let value = style(font, chrome::WHITE, 24, FRAKTION_BOLD);
        let text = signed(offset);
        let pen = Point::new(
            text::pen_x_for_ink_left(&value, &text, 311),
            text::baseline_for_ink_bottom(&value, &text, 288),
        );
        value.draw_on_baseline(&text, pen, field)?;
        for (neighbour, row) in self.neighbours().into_iter().zip(NEIGHBOURS) {
            if let Some(index) = neighbour {
                let mut line = String::<16>::new();
                _ = write!(line, "{}  {}", clock_at(unix, offsets[index]), signed(offsets[index]));
                Self::draw_neighbour(&line, row, font, target)?;
            }
        }
        let automatic = peripherals.clock.zone.mode == ZoneMode::Automatic;
        second::draw_button("AUTO", AUTO, automatic, font, target)
    }

    fn draw_zones<D: CoverageTarget<Color = Color>>(
        &self,
        peripherals: &PeripheralState,
        accents: Accents,
        font: &FontdueRenderer<'static, Color>,
        target: &mut D,
    ) -> Result<(), D::Error> {
        let Step::Zone { offset, zones, nearest } = &self.step else {
            return Ok(());
        };
        let (offset, nearest) = (*offset, *nearest);
        let mut hint = String::<24>::new();
        _ = write!(hint, "{}  {}", signed(offset), if nearest { "NEAREST FIRST" } else { "A-Z" });
        second::draw_cap(&hint, "BACK", &panel::ZONE, chrome::WHITE, accents, font, target)?;
        second::draw_field(target)?;
        let unix = time_of(peripherals);
        let zone = DATABASE.zone(zones[self.index]);
        let field = &mut OnBackground::new(&mut *target, chrome::BLACK);
        let big = style(font, chrome::WHITE, 40, FRAKTION_BOLD);
        let name = city(zone.name);
        let pen = Point::new(
            text::pen_x_for_ink_left(&big, &name, TEXT_LEFT),
            text::baseline_for_ink_bottom(&big, &name, 257),
        );
        big.draw_on_baseline(&name, pen, field)?;
        let mut line = String::<48>::new();
        for c in zone.name.chars() {
            _ = line.push(c.to_ascii_uppercase());
        }
        let abbreviation = offsets_of(&zone, unix)
            .into_iter()
            .find(|each| each.utc_offset == offset)
            .map_or("", |each| each.abbreviation);
        _ = write!(line, "  {abbreviation}");
        let small = style(font, chrome::GRAY, 16, FRAKTION_BOLD);
        let pen = Point::new(
            text::pen_x_for_ink_left(&small, &line, TEXT_LEFT),
            text::baseline_for_ink_top(&small, &line, 276),
        );
        small.draw_on_baseline(&line, pen, field)?;
        for (neighbour, row) in self.neighbours().into_iter().zip(NEIGHBOURS) {
            if let Some(index) = neighbour {
                Self::draw_neighbour(&city(DATABASE.zone(zones[index]).name), row, font, target)?;
            }
        }
        let mut position = String::<12>::new();
        _ = write!(position, "{}/{}", self.index + 1, zones.len());
        let style = second::hint_style(font);
        let pen = Point::new(
            text::pen_x_for_ink_centre(&style, &position, CENTER_X),
            text::baseline_for_ink_top(&style, &position, 381),
        );
        style.draw_on_baseline(&position, pen, &mut OnBackground::new(&mut *target, chrome::BLACK))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cities_are_the_last_part_in_capitals_and_etc_is_at_sea() {
        assert_eq!(city("America/Argentina/Buenos_Aires"), "BUENOS AIRES");
        assert_eq!(city("Etc/GMT-1"), "AT SEA");
        assert_eq!(signed(-9000), "-02:30");
        assert_eq!(signed(19_800), "+05:30");
    }

    #[test]
    fn the_offsets_are_distinct_and_ascending_and_every_one_has_zones() {
        let unix = DateTime { year: 2026, month: 9, day: 24, hour: 12, ..DateTime::default() }.to_unix();
        let offsets = offsets_at(Some(unix));
        assert!(offsets.len() > 30, "{} offsets", offsets.len());
        assert!(offsets.windows(2).all(|pair| pair[0] < pair[1]));
        for offset in offsets {
            assert!(!zones_at(offset, Some(unix), None).is_empty(), "{offset}");
        }
        let dublin = zones_at(3600, Some(unix), None);
        assert!(dublin.iter().any(|&zone| DATABASE.zone(zone).name == "Europe/Dublin"));
    }

    #[test]
    fn with_a_position_the_nearest_zone_comes_first_and_etc_last() {
        let unix = DateTime { year: 2026, month: 9, day: 24, hour: 12, ..DateTime::default() }.to_unix();
        // Just outside Dublin.
        let zones = zones_at(3600, Some(unix), Some((533_500_000, -63_000_000)));
        let names: Vec<_> = zones.iter().map(|&zone| DATABASE.zone(zone).name).collect();
        assert_eq!(names[0], "Europe/Dublin");
        assert_eq!(names[1], "Europe/Isle_of_Man");
        assert!(names.last().unwrap().starts_with("Etc/"), "{names:?}");
    }
}

#[cfg(test)]
mod untrusted {
    use super::*;

    #[test]
    fn without_a_time_a_zone_is_found_under_its_winter_and_summer_offsets() {
        let dublin = |offset| zones_at(offset, None, None).into_iter().any(|zone| DATABASE.zone(zone).name == "Europe/Dublin");
        assert!(dublin(0) && dublin(3600));
        assert!(offsets_at(None).len() >= offsets_at(Some(RULES_ONLY[0])).len());
    }
}
