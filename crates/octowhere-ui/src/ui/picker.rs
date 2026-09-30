//! The zone picker: an offset chosen by local time, then a zone at that offset.
//! Its D3 layout is in `context/design/handoffs/IMPLEMENTATION-HANDOFF-CURRENT.md`.

use alloc::vec::Vec;
use core::fmt::Write as _;

use embedded_graphics::{
    prelude::{Point, Size},
    primitives::Rectangle,
};
use heapless::String;

use super::{
    clock::{DateTime, ZoneId},
    clock_screen,
    gesture::{GestureEvent, Micros},
    panel,
    screens::PeripheralState,
    second::{self, Accents, Effects, Next, Store, TEXT_LEFT},
    text::{self, style},
};
use crate::{
    chrome::{
        self, Color, CoverageTarget, FRAKTION, FRAKTION_BOLD, FRAKTION_SANS_LIGHT, FontdueRenderer,
        INTERFERENCE_BOLD, OnBackground, Window,
    },
    tz::DATABASE,
};

const TOP_CAP: i32 = 150;
const BAND: core::ops::Range<i32> = 186..330;
const AUTO: Rectangle = Rectangle::new(Point::new(178, 326), Size::new(110, 34));
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
const NEIGHBOURS: [f32; 2] = [164.0, 301.0];
/// A zone's name and identifier lines, where each clips: the slab less its padding, from the
/// lines' left.
const LINES: [Rectangle; 2] = [
    Rectangle::new(Point::new(TEXT_LEFT, 189), Size::new(274, 60)),
    Rectangle::new(Point::new(TEXT_LEFT, 249), Size::new(274, 29)),
];
/// A line too long for its clip scrolls: it holds at its start, runs to its end, holds, and
/// runs back, over and over. The two lines share the cycle, run at the longer one's speed.
const SCROLL_HOLD: Micros = 1_200_000;
const SCROLL_SPEED: f32 = 40.0;

#[derive(Clone, Debug, PartialEq)]
enum Step {
    /// The distinct offsets in force, ascending, in seconds.
    Offset { offsets: Vec<i32> },
    /// The zones at `offset`, in the order they are listed, which is nearest first when the
    /// position is known.
    Zone {
        offset: i32,
        zones: Vec<ZoneId>,
        nearest: bool,
    },
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
    /// The zone whose lines scroll, since when, how far each line has run of how far it overhangs
    /// its clip, and when the next run starts while they hold.
    scroll: Option<Scroll>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Scroll {
    index: usize,
    since: Micros,
    overhang: [i32; 2],
    run: [i32; 2],
    next: Option<Micros>,
}

impl Scroll {
    /// How long one run takes, from start to end.
    fn travel(&self) -> Micros {
        let longest = self.overhang[0].max(self.overhang[1]);
        (longest as f32 / SCROLL_SPEED * 1e6) as Micros
    }

    /// How far through the cycle `now` is, and from when to when each run happens in it.
    fn phase(&self, now: Micros) -> (Micros, [core::ops::Range<Micros>; 2]) {
        let travel = self.travel();
        let period = 2 * (SCROLL_HOLD + travel);
        let at = now.saturating_sub(self.since) % period.max(1);
        let out = SCROLL_HOLD..SCROLL_HOLD + travel;
        let back = out.end + SCROLL_HOLD..period;
        (at, [out, back])
    }

    /// Each line's run at `now`, 0 at its start and its overhang at its end.
    fn at(&self, now: Micros) -> [i32; 2] {
        let (at, [out, back]) = self.phase(now);
        let travel = self.travel().max(1) as f32;
        let share = if out.contains(&at) {
            (at - out.start) as f32 / travel
        } else if back.contains(&at) {
            1.0 - (at - back.start) as f32 / travel
        } else if at < out.start {
            0.0
        } else {
            1.0
        };
        self.overhang
            .map(|overhang| libm::roundf(overhang as f32 * share) as i32)
    }

    /// Whether a run is under way, and if not, when the next one starts.
    fn running(&self, now: Micros) -> Result<(), Micros> {
        if self.overhang == [0, 0] {
            return Err(Micros::MAX);
        }
        let (at, [out, back]) = self.phase(now);
        let start = now - at;
        match () {
            () if out.contains(&at) || back.contains(&at) => Ok(()),
            () if at < out.start => Err(start + out.start),
            () => Err(start + back.start),
        }
    }
}

impl Eq for Picker {}

fn time_of(peripherals: &PeripheralState) -> Option<i64> {
    peripherals
        .clock
        .clock()
        .utc
        .filter(|_| !peripherals.clock.clock().stopped)
}

/// A zone's offset at `unix`, or without a time, each offset it keeps during a year.
fn offsets_of(
    zone: &crate::tz::Zone,
    unix: Option<i64>,
) -> heapless::Vec<crate::tz::Offset<'static>, 2> {
    let mut offsets = heapless::Vec::new();
    for time in unix.map_or(RULES_ONLY.to_vec(), |unix| alloc::vec![unix]) {
        let offset = zone.at(time);
        if !offsets
            .iter()
            .any(|kept: &crate::tz::Offset<'_>| kept.utc_offset == offset.utc_offset)
        {
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
        .flat_map(|zone| {
            offsets_of(&zone, unix)
                .into_iter()
                .map(|offset| offset.utc_offset)
        })
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
        .filter(|zone| {
            offsets_of(zone, unix)
                .iter()
                .any(|each| each.utc_offset == offset)
        })
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
        (a.0, a.1)
            .partial_cmp(&(b.0, b.1))
            .unwrap_or(core::cmp::Ordering::Equal)
            .then(a.2.cmp(&b.2))
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
        _ = city.push(if c == '_' {
            ' '
        } else {
            c.to_ascii_uppercase()
        });
    }
    city
}

/// The selected zone's name and identifier lines' styles.
fn line_styles(font: &FontdueRenderer<'static, Color>) -> [FontdueRenderer<'static, Color>; 2] {
    [
        style(font, chrome::BLACK, 37, INTERFERENCE_BOLD),
        style(font, chrome::BLACK, 14, FRAKTION_BOLD),
    ]
}

/// Where each line's pen starts before it scrolls: the name centred on row 220 by its ink, the
/// identifier's ink top on row 253, both from the lines' left.
fn line_pens(font: &FontdueRenderer<'static, Color>, lines: &[String<48>; 2]) -> [Point; 2] {
    let [big, small] = line_styles(font);
    [
        Point::new(
            text::pen_x_for_ink_left(&big, &lines[0], TEXT_LEFT),
            text::baseline_for_ink_middle(&big, &lines[0], 220.0),
        ),
        Point::new(
            text::pen_x_for_ink_left(&small, &lines[1], TEXT_LEFT),
            text::baseline_for_ink_top(&small, &lines[1], 253),
        ),
    ]
}

/// How far past its clip each line's ink reaches, or 0.
fn overhang(font: &FontdueRenderer<'static, Color>, lines: &[String<48>; 2]) -> [i32; 2] {
    let pens = line_pens(font, lines);
    let styles = line_styles(font);
    core::array::from_fn(|i| {
        let ink = styles[i].baseline_bounds(&lines[i], pens[i]);
        let end = LINES[i].top_left.x + LINES[i].size.width as i32;
        (ink.top_left.x + ink.size.width as i32 - end).max(0)
    })
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
        let current =
            clock_screen::offset(&peripherals.clock).map_or(0, |offset| offset.utc_offset);
        let index = offsets
            .iter()
            .position(|&offset| offset >= current)
            .unwrap_or(offsets.len().saturating_sub(1));
        Self {
            step: Step::Offset { offsets },
            index,
            grabbed: None,
            fling: None,
            flung_from: 0,
            scroll: None,
        }
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

    pub fn handle(
        &mut self,
        event: &GestureEvent,
        peripherals: &PeripheralState,
        effects: &mut Effects,
    ) -> Next {
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
            Step::Offset { .. } if AUTO.contains(point) => {
                effects.store = Some(Store::AutomaticZone);
                Next::Panel
            }
            Step::Offset { offsets } if BAND.contains(&point.y) => {
                let offset = offsets[self.index];
                let position = peripherals.gnss.position;
                let zones = zones_at(offset, unix, position);
                let current = peripherals.clock.zone().zone;
                self.index = zones
                    .iter()
                    .position(|&zone| Some(zone) == current)
                    .unwrap_or(0);
                self.step = Step::Zone {
                    offset,
                    zones,
                    nearest: position.is_some(),
                };
                self.fling = None;
                Next::Stay
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

    /// Advances a fling and the zone's lines' scroll, and says whether either is moving.
    pub fn step(
        &mut self,
        now: Micros,
        peripherals: &PeripheralState,
        font: &FontdueRenderer<'static, Color>,
    ) -> bool {
        let flinging = self.step_fling(now);
        self.scroll = self.zone_lines(peripherals).map(|lines| {
            let mut scroll = match self.scroll {
                Some(scroll) if scroll.index == self.index => scroll,
                _ => Scroll {
                    index: self.index,
                    since: now,
                    overhang: overhang(font, &lines),
                    run: [0, 0],
                    next: None,
                },
            };
            scroll.run = scroll.at(now);
            scroll.next = scroll.running(now).err().filter(|&at| at != Micros::MAX);
            scroll
        });
        let scrolling = self
            .scroll
            .is_some_and(|scroll| scroll.running(now).is_ok());
        flinging || scrolling
    }

    /// When the zone's lines next start to scroll, while they hold.
    #[must_use]
    pub fn next_change(&self) -> Option<Micros> {
        self.scroll?.next
    }

    /// The lines whose scroll alone tells `self` from `before`, if nothing else does.
    #[must_use]
    pub fn scroll_damage(&self, before: &Self) -> Option<&'static [Rectangle; 2]> {
        fn still(picker: &Picker) -> (&Step, usize, Option<usize>, usize, Option<Scroll>) {
            (
                &picker.step,
                picker.index,
                picker.grabbed,
                picker.flung_from,
                picker.scroll.map(|scroll| Scroll {
                    run: [0, 0],
                    next: None,
                    ..scroll
                }),
            )
        }
        (self.fling.is_none() && before.fling.is_none() && still(self) == still(before))
            .then_some(&LINES)
    }

    fn step_fling(&mut self, now: Micros) -> bool {
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
            Step::Offset { offsets } => {
                self.draw_offsets(offsets, peripherals, accents, font, target)
            }
            Step::Zone { .. } => self.draw_zones(peripherals, accents, font, target),
        }
    }

    fn neighbours(&self) -> [Option<usize>; 2] {
        [
            self.index.checked_sub(1),
            Some(self.index + 1).filter(|&next| next < self.len()),
        ]
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
        second::draw_cap(
            "OFFSET / 01",
            "CANCEL",
            &panel::ZONE,
            chrome::VIOLET,
            accents,
            font,
            target,
        )?;
        second::draw_slab(189, 278, chrome::VIOLET, target)?;
        let unix = time_of(peripherals);
        let offset = offsets[self.index];
        if let Some(previous) = self.neighbours()[0] {
            let mut line = String::<16>::new();
            _ = write!(
                line,
                "{}   {}",
                clock_at(unix, offsets[previous]),
                signed(offsets[previous])
            );
            let small = style(font, chrome::GRAY, 16, FRAKTION);
            let pen = Point::new(
                text::pen_x_for_ink_left(&small, &line, TEXT_LEFT),
                text::baseline_for_ink_middle(&small, &line, NEIGHBOURS[0]),
            );
            small.draw_on_baseline(
                &line,
                pen,
                &mut OnBackground::new(&mut *target, chrome::BLACK),
            )?;
        }
        let big = style(font, chrome::BLACK, 51, INTERFERENCE_BOLD);
        let time = clock_at(unix, offset);
        let pen = Point::new(
            text::pen_x_for_ink_left(&big, &time, TEXT_LEFT),
            text::baseline_for_ink_middle(&big, &time, 233.0),
        );
        big.draw_on_baseline(
            &time,
            pen,
            &mut OnBackground::new(&mut *target, chrome::VIOLET),
        )?;
        let label = style(font, chrome::BLACK, 11, FRAKTION_BOLD);
        let pen = Point::new(
            text::pen_x_for_ink_left(&label, "UTC", 309),
            text::baseline_for_ink_top(&label, "UTC", 251),
        );
        label.draw_on_baseline(
            "UTC",
            pen,
            &mut OnBackground::new(&mut *target, chrome::VIOLET),
        )?;
        let value = style(font, chrome::BLACK, 11, FRAKTION_BOLD);
        let text = signed(offset);
        let pen = Point::new(
            text::pen_x_for_ink_right(&value, &text, 378),
            text::baseline_for_ink_top(&value, &text, 251),
        );
        value.draw_on_baseline(
            &text,
            pen,
            &mut OnBackground::new(&mut *target, chrome::VIOLET),
        )?;
        if let Some(next) = self.neighbours()[1] {
            let mut line = String::<16>::new();
            _ = write!(
                line,
                "{}   {}",
                clock_at(unix, offsets[next]),
                signed(offsets[next])
            );
            let small = style(font, chrome::GRAY, 18, FRAKTION_SANS_LIGHT);
            let pen = Point::new(
                text::pen_x_for_ink_left(&small, &line, TEXT_LEFT),
                text::baseline_for_ink_middle(&small, &line, NEIGHBOURS[1]),
            );
            small.draw_on_baseline(
                &line,
                pen,
                &mut OnBackground::new(&mut *target, chrome::BLACK),
            )?;
        }
        second::draw_button("AUTO", AUTO, true, font, target)?;
        second::draw_footer("DRAG OFFSET / TAP FOR ZONES", accents, font, target)
    }

    /// The selected zone's name and its identifier with the abbreviation, on the zone step.
    fn zone_lines(&self, peripherals: &PeripheralState) -> Option<[String<48>; 2]> {
        let Step::Zone { offset, zones, .. } = &self.step else {
            return None;
        };
        let zone = DATABASE.zone(zones[self.index]);
        let mut name = String::new();
        _ = name.push_str(&city(zone.name));
        let mut line = String::<48>::new();
        for c in zone.name.chars() {
            _ = line.push(c.to_ascii_uppercase());
        }
        let abbreviation = offsets_of(&zone, time_of(peripherals))
            .into_iter()
            .find(|each| each.utc_offset == *offset)
            .map_or("", |each| each.abbreviation);
        _ = write!(line, "  {abbreviation}");
        Some([name, line])
    }

    fn draw_zones<D: CoverageTarget<Color = Color>>(
        &self,
        peripherals: &PeripheralState,
        accents: Accents,
        font: &FontdueRenderer<'static, Color>,
        target: &mut D,
    ) -> Result<(), D::Error> {
        let Step::Zone { zones, nearest, .. } = &self.step else {
            return Ok(());
        };
        let nearest = *nearest;
        second::draw_cap(
            "ZONE / 02",
            "BACK",
            &panel::ZONE,
            chrome::VIOLET,
            accents,
            font,
            target,
        )?;
        second::draw_slab(188, 278, chrome::VIOLET, target)?;
        let run = self.scroll.map_or([0, 0], |scroll| scroll.run);
        if let Some(lines) = self.zone_lines(peripherals) {
            for (((text, style), pen), (clip, run)) in lines
                .iter()
                .zip(line_styles(font))
                .zip(line_pens(font, &lines))
                .zip(LINES.iter().zip(run))
            {
                let clipped = &mut Window::new(&mut *target, Point::zero(), *clip);
                style.draw_on_baseline(
                    text,
                    pen - Point::new(run, 0),
                    &mut OnBackground::new(clipped, chrome::VIOLET),
                )?;
            }
        }
        for (neighbour, row) in self.neighbours().into_iter().zip(NEIGHBOURS) {
            if let Some(index) = neighbour {
                Self::draw_neighbour(&city(DATABASE.zone(zones[index]).name), row, font, target)?;
            }
        }
        // The order shares the count's line: above the slab it would meet the zone before.
        let mut position = String::<32>::new();
        let order = if nearest { "NEAREST FIRST" } else { "A-Z" };
        _ = write!(
            position,
            "{order}  {:02} / {:02}",
            self.index + 1,
            zones.len()
        );
        let style = second::hint_style(font);
        let pen = Point::new(
            text::pen_x_for_ink_right(&style, &position, 370),
            text::baseline_for_ink_top(&style, &position, 326),
        );
        style.draw_on_baseline(
            &position,
            pen,
            &mut OnBackground::new(&mut *target, chrome::BLACK),
        )?;
        second::draw_footer("DRAG TO CHOOSE / TAP TO SELECT", accents, font, target)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_long_line_holds_runs_out_holds_and_runs_back() {
        let scroll = Scroll {
            index: 0,
            since: 1_000,
            overhang: [40, 20],
            run: [0, 0],
            next: None,
        };
        let travel = scroll.travel();
        assert_eq!(travel, 1_000_000);
        let at = |t: Micros| scroll.at(1_000 + t);
        assert_eq!(at(0), [0, 0]);
        assert_eq!(at(SCROLL_HOLD - 1), [0, 0]);
        assert_eq!(at(SCROLL_HOLD + travel / 2), [20, 10]);
        assert_eq!(at(SCROLL_HOLD + travel + 1), [40, 20]);
        assert_eq!(at(2 * SCROLL_HOLD + travel + travel / 2), [20, 10]);
        assert_eq!(at(2 * (SCROLL_HOLD + travel)), [0, 0]);
        assert_eq!(scroll.running(1_000), Err(1_000 + SCROLL_HOLD));
        assert_eq!(scroll.running(1_000 + SCROLL_HOLD + 1), Ok(()));
        assert_eq!(
            scroll.running(1_000 + SCROLL_HOLD + travel),
            Err(1_000 + 2 * SCROLL_HOLD + travel)
        );
    }

    #[test]
    fn cities_are_the_last_part_in_capitals_and_etc_is_at_sea() {
        assert_eq!(city("America/Argentina/Buenos_Aires"), "BUENOS AIRES");
        assert_eq!(city("Etc/GMT-1"), "AT SEA");
        assert_eq!(signed(-9000), "-02:30");
        assert_eq!(signed(19_800), "+05:30");
    }

    #[test]
    fn the_offsets_are_distinct_and_ascending_and_every_one_has_zones() {
        let unix = DateTime {
            year: 2026,
            month: 9,
            day: 24,
            hour: 12,
            ..DateTime::default()
        }
        .to_unix();
        let offsets = offsets_at(Some(unix));
        assert!(offsets.len() > 30, "{} offsets", offsets.len());
        assert!(offsets.windows(2).all(|pair| pair[0] < pair[1]));
        for offset in offsets {
            assert!(!zones_at(offset, Some(unix), None).is_empty(), "{offset}");
        }
        let dublin = zones_at(3600, Some(unix), None);
        assert!(
            dublin
                .iter()
                .any(|&zone| DATABASE.zone(zone).name == "Europe/Dublin")
        );
    }

    #[test]
    fn with_a_position_the_nearest_zone_comes_first_and_etc_last() {
        let unix = DateTime {
            year: 2026,
            month: 9,
            day: 24,
            hour: 12,
            ..DateTime::default()
        }
        .to_unix();
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
        let dublin = |offset| {
            zones_at(offset, None, None)
                .into_iter()
                .any(|zone| DATABASE.zone(zone).name == "Europe/Dublin")
        };
        assert!(dublin(0) && dublin(3600));
        assert!(offsets_at(None).len() >= offsets_at(Some(RULES_ONLY[0])).len());
    }
}
