//! The UI between input and drawing: which screen shows, what touch does to it, and when it has
//! to be redrawn. The frame loop feeds it readings and draws it, and acts on the effects it
//! returns. It never touches a device, so the host drives it the same way.

use embedded_graphics::prelude::Point;

use super::{
    axis_check::{self, AxisCheck},
    clock::ZoneState,
    clock_screen,
    compass::CompassView,
    compass_screen::{self, Accents, DialFootprint, Mode},
    gesture::{GestureEvent, GestureTracker, Micros},
    input::TouchState,
    pager::Pager,
    prototypes::{self, ClockState, PeripheralState, Screen},
};
use crate::{
    board,
    chrome::{self, Color, CoverageTarget, Dirty, FontdueRenderer, FontdueRendererCtx},
};

/// What the motion task publishes each sample.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Motion {
    pub accel_micro_ms2: [i32; 3],
    pub gyro_micro_rad_s: [i32; 3],
    pub imu_valid: bool,
    pub magnetic_microtesla: [i32; 3],
    pub compass: CompassView,
}

/// The parts of the sensor task's snapshot the screens show.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Sensors {
    pub battery_mv: Option<u16>,
    pub vbus_mv: Option<u16>,
    pub vsys_mv: Option<u16>,
    pub gnss_bytes: u16,
    pub gnss_fix: bool,
    pub lora_irq: u8,
    pub clock: ClockState,
    pub zone: ZoneState,
}

/// One read of the touch controller.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Touch {
    /// Up to two contacts in panel coordinates; none when nothing touches.
    Contacts([Option<Point>; 2]),
    /// The controller recognised a hand covering the screen.
    Cover,
}

/// When each of the compass's accents starts after the page settles, and how long each takes.
/// The icon's modules land a row per `ICON_ROW`.
const COMPASS_ENTRY: CompassTimes<Micros> = CompassTimes {
    ring: 0,
    icon: 95_000,
    caption: 120_000,
    dial: 190_000,
    divider: 240_000,
    hint: 280_000,
};
const RING_FADE: Micros = 110_000;
const ICON_ROW: Micros = 30_000;
const CAPTION_REVEAL: Micros = 120_000;
const DIAL_SWEEP: Micros = 170_000;
const DIVIDER_DRAW: Micros = 100_000;
const HINT_REVEAL: Micros = 160_000;
/// A heading back from TOP EDGE UP within this shows the dial at once, so tilting through
/// vertical does not replay anything.
const TOP_EDGE_GRACE: Micros = 750_000;

/// When each of the compass's accents started, or starts. `None` shows it whole at once.
#[derive(Clone, Copy, Debug, Default)]
struct CompassTimes<T = Option<Micros>> {
    ring: T,
    icon: T,
    caption: T,
    dial: T,
    divider: T,
    hint: T,
}

/// Dragging the compass or the clock this fraction of the panel's width takes its accents out
/// entirely.
const SWIPE_FADE: f32 = 0.35;
/// When each of the clock face's accents starts after the page settles. The icon's modules land
/// a row per step, and the reveals run over their durations.
const CLOCK_ENTRY: ClockTimes<Micros> = ClockTimes {
    ring: 0,
    icon: 60_000,
    label: 100_000,
    plate: 160_000,
    zone: 240_000,
};
const CLOCK_LABEL_REVEAL: Micros = 120_000;
const CLOCK_PLATE_REVEAL: Micros = 120_000;
const CLOCK_ZONE_REVEAL: Micros = 160_000;

/// When each of the clock face's accents started, or starts. `None` shows it whole at once.
#[derive(Clone, Copy, Debug, Default)]
struct ClockTimes<T = Option<Micros>> {
    ring: T,
    icon: T,
    label: T,
    plate: T,
    zone: T,
}

/// How long without a cover report before another cover is a new hand. The controller does not
/// always report the hand lifting, and a held hand repeats its report up to about 180 ms apart.
/// Covering again after lifting took about 350 ms.
const COVER_REARM: Micros = 260_000;

/// Everything that arrived since the last step.
#[derive(Clone, Copy, Debug, Default)]
pub struct Input {
    pub now: Micros,
    /// A fresh touch reading. `None` when the touch controller was not read this step; the
    /// last reading then stands.
    pub touch: Option<Touch>,
    pub motion: Option<Motion>,
    pub sensors: Option<Sensors>,
}

/// What the frame loop owes after a step. [`Stage::changed`] holds the pixels it changed.
#[derive(Debug)]
pub struct Update {
    /// A cover over the settled compass page asked for the calibration to restart.
    pub recalibrate: bool,
    /// The axis check finished capturing a pose.
    pub pose: Option<axis_check::Record>,
    /// A screen that needs fast motion samples shows, or is sliding in.
    pub samples_fast: bool,
}

pub struct Stage {
    screen: Screen,
    selected_node: Option<u8>,
    raw_touch: [Option<Point>; 2],
    gesture: GestureTracker,
    pager: Pager,
    axis_check: AxisCheck,
    touch_state: TouchState,
    peripherals: PeripheralState,
    renderer: FontdueRenderer<'static, Color>,
    /// When the last cover report arrived, while the hand that sent it may still be there.
    covered_at: Option<Micros>,
    /// When the compass page last settled into view, and the state it showed last step.
    compass_settled: Option<(CompassTimes, Mode)>,
    /// When the heading gave way to TOP EDGE UP, while nothing else has shown since.
    top_edge_since: Option<Micros>,
    accents: Accents,
    fading: bool,
    /// What the compass page showed after the last step, while it filled the panel.
    drawn_compass: Option<(CompassView, Accents)>,
    /// The pixels the last step changed. Boxed so the frame loop's stack never holds it.
    changed: alloc::boxed::Box<Dirty>,
    dial_footprint: DialFootprint,
    /// When the clock page last settled into view, and what its accents re-reveal on.
    clock_settled: Option<(ClockTimes, clock_screen::Keys)>,
    clock_accents: clock_screen::Accents,
    /// What the clock page showed after the last step, while it filled the panel.
    drawn_clock: Option<(ClockState, ZoneState, clock_screen::Accents)>,
}

impl Stage {
    #[must_use]
    pub fn new(peripherals: PeripheralState) -> Self {
        Self {
            screen: Screen::Map,
            selected_node: None,
            raw_touch: [None; 2],
            gesture: GestureTracker::default(),
            pager: Pager::new(0, Screen::ALL.len(), board::LCD_WIDTH as i32),
            axis_check: AxisCheck::default(),
            touch_state: TouchState::default(),
            peripherals,
            renderer: FontdueRenderer::new(
                FontdueRendererCtx::new_rc(),
                20,
                chrome::WHITE,
                chrome::BLACK,
                chrome::FONTS,
            ),
            covered_at: None,
            compass_settled: None,
            top_edge_since: None,
            accents: Accents::FULL,
            fading: false,
            drawn_compass: None,
            changed: alloc::boxed::Box::new(Dirty::new()),
            dial_footprint: DialFootprint::default(),
            clock_settled: None,
            clock_accents: clock_screen::Accents::FULL,
            drawn_clock: None,
        }
    }

    /// Jumps to `screen` as though the pager had come to rest on it.
    pub fn show(&mut self, screen: Screen) {
        let page = Screen::ALL
            .iter()
            .position(|&each| each == screen)
            .expect("every screen is in the ring");
        self.pager = Pager::new(page, Screen::ALL.len(), board::LCD_WIDTH as i32);
        self.screen = screen;
        self.selected_node = None;
        self.compass_settled = None;
        self.top_edge_since = None;
        self.drawn_compass = None;
        self.clock_settled = None;
        self.drawn_clock = None;
    }

    /// The pixels the last [`step`](Self::step) changed.
    #[must_use]
    pub fn changed(&self) -> &Dirty {
        &self.changed
    }

    #[must_use]
    pub fn screen(&self) -> Screen {
        self.screen
    }

    /// How far the compass page's dial accents have faded in.
    #[must_use]
    pub fn accents(&self) -> Accents {
        self.accents
    }

    /// How far the clock face's accents have come in.
    #[must_use]
    pub fn clock_accents(&self) -> clock_screen::Accents {
        self.clock_accents
    }

    #[must_use]
    pub fn peripherals(&self) -> &PeripheralState {
        &self.peripherals
    }

    /// A finger is down, or the gesture tracker has not yet seen it lift. Touch should be read
    /// again soon even without an interrupt.
    #[must_use]
    pub fn in_contact(&self) -> bool {
        self.peripherals.touch_position.is_some() || self.gesture.in_contact()
    }

    /// A page slide or a fade is under way, so the next step should come without waiting for
    /// input.
    #[must_use]
    pub fn is_animating(&self) -> bool {
        self.pager.is_moving() || self.fading
    }

    pub fn step(&mut self, input: Input) -> Update {
        let Input {
            now,
            touch,
            motion,
            sensors,
        } = input;
        self.changed.clear();
        let mut update = Update {
            recalibrate: false,
            pose: None,
            samples_fast: false,
        };
        // Set where a change needs the whole panel redrawn. The settled compass works out its own
        // damage instead.
        let mut full = false;
        let previous_touch = (
            self.peripherals.touch_points,
            self.peripherals.touch_position,
            self.peripherals.touch_positions,
        );

        if let Some(motion) = motion {
            let peripherals = &mut self.peripherals;
            peripherals.accel_micro_ms2 = motion.accel_micro_ms2;
            peripherals.gyro_micro_rad_s = motion.gyro_micro_rad_s;
            peripherals.imu_valid = motion.imu_valid;
            peripherals.magnetic_microtesla = motion.magnetic_microtesla;
            peripherals.compass = motion.compass;
            if matches!(
                self.screen,
                Screen::Motion | Screen::Compass | Screen::AxisCheck
            ) {
                full = true;
            }
        }
        if let Some(sensors) = sensors {
            let peripherals = &mut self.peripherals;
            peripherals.battery_mv = sensors.battery_mv;
            peripherals.vbus_mv = sensors.vbus_mv;
            peripherals.vsys_mv = sensors.vsys_mv;
            peripherals.gnss_bytes = sensors.gnss_bytes;
            peripherals.gnss_valid = sensors.gnss_fix;
            peripherals.lora_irq = sensors.lora_irq;
            peripherals.clock = sensors.clock;
            peripherals.zone = sensors.zone;
            full = true;
        }

        if let Some(touch) = touch {
            self.raw_touch = match touch {
                Touch::Contacts(contacts) => contacts,
                Touch::Cover => [None; 2],
            };
        }
        let (touch_points, touch_positions) = self.touch_state.update_positions(self.raw_touch);
        self.peripherals.touch_points = touch_points;
        self.peripherals.touch_position = touch_positions[0];
        self.peripherals.touch_positions = touch_positions;

        let previous_view = self.pager.view();
        let previous_selected_node = self.selected_node;
        let event = if touch.is_some() {
            self.gesture.update(self.raw_touch[0], now)
        } else {
            GestureEvent::None
        };
        self.pager.handle(&event, now);
        if self.screen == Screen::AxisCheck {
            if let GestureEvent::DragEnd(drag) = event {
                let offset = drag.offset();
                if offset.y.abs() > 60 && offset.y.abs() > offset.x.abs() {
                    self.axis_check.step(offset.y < 0);
                }
            }
            if let Some(motion) = motion {
                update.pose = self.axis_check.sample(
                    motion.magnetic_microtesla.map(|value| value as f32 / 1e3),
                    motion.accel_micro_ms2.map(|value| value as f32 / 1e6),
                    now,
                );
            }
        }
        if let GestureEvent::Tap(point) = event {
            if self.screen == Screen::AxisCheck {
                self.axis_check.start(now);
            } else if matches!(self.screen, Screen::Compass | Screen::Clock) {
                // Only a cover restarts calibration on the compass, and the clock takes no taps.
            } else if prototypes::HEADER.contains(point) {
                self.pager.advance(true, now);
            } else if self.screen == Screen::Map
                && let Some(node) = selected_node(point)
            {
                self.selected_node = Some(node);
            }
        }
        self.pager.step(now);
        let view = self.pager.view();
        let screen = Screen::ALL[view.page];
        if screen != self.screen {
            self.screen = screen;
            self.selected_node = None;
        }
        let samples_fast = |screen: Screen| matches!(screen, Screen::Compass | Screen::AxisCheck);
        update.samples_fast = samples_fast(screen)
            || view
                .neighbour
                .is_some_and(|(page, _)| samples_fast(Screen::ALL[page]));

        if let Some(touch) = touch {
            let cover = touch == Touch::Cover;
            let fresh = self
                .covered_at
                .is_none_or(|at| now.saturating_sub(at) >= COVER_REARM);
            if cover && fresh && self.accepts_cover() {
                update.recalibrate = true;
                // The motion task resets the calibration on its next sample; the dial goes now.
                let compass = &mut self.peripherals.compass;
                compass.calibration_percent = 0;
                compass.heading_decidegrees = None;
                full = true;
            }
            match touch {
                Touch::Cover => self.covered_at = Some(now),
                // A finger means the hand has gone. A report without one does not: the controller
                // sends unreadable reports while a hand is held, and those arrive as no contacts.
                Touch::Contacts(contacts) if contacts.iter().any(Option::is_some) => {
                    self.covered_at = None;
                }
                Touch::Contacts(_) => {}
            }
        }
        let accents = self.compass_accents(now);
        if accents != self.accents {
            self.accents = accents;
            full = true;
        }
        let clock_accents = self.advance_clock_accents(now);
        if clock_accents != self.clock_accents {
            self.clock_accents = clock_accents;
            full = true;
        }

        let axis_check = self.axis_check.view();
        if axis_check != self.peripherals.axis_check {
            self.peripherals.axis_check = axis_check;
            full = true;
        }
        if view != previous_view || self.selected_node != previous_selected_node {
            full = true;
        }
        if self.screen == Screen::Touch
            && (touch_points, touch_positions[0], touch_positions) != previous_touch
        {
            full = true;
        }
        // A settled compass or clock works out its own damage; anything else redraws in full.
        let settled = view.offset == 0 && view.neighbour.is_none();
        let compass = (self.screen == Screen::Compass && settled)
            .then_some((self.peripherals.compass, self.accents));
        let clock = (self.screen == Screen::Clock && settled).then_some((
            self.peripherals.clock,
            self.peripherals.zone,
            self.clock_accents,
        ));
        if let (Some(before), Some(after)) = (self.drawn_compass, compass) {
            compass_screen::damage(
                (&before.0, before.1),
                (&after.0, after.1),
                &self.renderer,
                &mut self.dial_footprint,
                &mut self.changed,
            );
        } else if let (Some(before), Some(after)) = (self.drawn_clock, clock) {
            clock_screen::damage(
                (&before.0, &before.1, before.2),
                (&after.0, &after.1, after.2),
                &self.renderer,
                &mut self.changed,
            );
        } else if full {
            self.changed.make_full();
        }
        self.drawn_compass = compass;
        self.drawn_clock = clock;
        update
    }

    /// A cover counts only on the compass page at rest, while its sensors report.
    fn accepts_cover(&self) -> bool {
        self.screen == Screen::Compass
            && !self.pager.is_moving()
            && self.peripherals.compass.live
    }

    /// Advances the compass page's builds and reveals to `now`, and returns how far each has
    /// come.
    fn compass_accents(&mut self, now: Micros) -> Accents {
        self.fading = false;
        if self.screen != Screen::Compass {
            self.compass_settled = None;
            self.top_edge_since = None;
            return Accents::FULL;
        }
        let view = self.pager.view();
        let mode = Mode::of(&self.peripherals.compass);
        let (times, shown) = match &mut self.compass_settled {
            Some(settled) => settled,
            None if view.offset == 0 => {
                let start = |delay: Micros| Some(now + delay);
                let mut times = CompassTimes {
                    ring: start(COMPASS_ENTRY.ring),
                    icon: start(COMPASS_ENTRY.icon),
                    caption: start(COMPASS_ENTRY.caption),
                    dial: start(COMPASS_ENTRY.dial),
                    divider: start(COMPASS_ENTRY.divider),
                    hint: start(COMPASS_ENTRY.hint),
                };
                if mode == Mode::NoData {
                    // A fault shows at once.
                    (times.ring, times.icon, times.caption) = (None, None, None);
                }
                self.compass_settled.insert((times, mode))
            }
            None => return Accents::HIDDEN,
        };
        if *shown != mode {
            if mode == Mode::NoData {
                (times.ring, times.icon, times.caption) = (None, None, None);
            } else if mode.heading().is_some() && shown.heading().is_none() {
                let quick = *shown == Mode::TopEdgeUp
                    && self
                        .top_edge_since
                        .is_some_and(|since| now.saturating_sub(since) < TOP_EDGE_GRACE);
                // The dial sweeps in, and the icon rebuilds with it. Only a sweep rebuilds the
                // icon, so interference coming and going swaps it in place.
                if !quick {
                    let from_now = |start: Option<Micros>| Some(start.map_or(now, |start| start.max(now)));
                    times.dial = from_now(times.dial);
                    times.icon = from_now(times.icon);
                    if compass_screen::caption(*shown).0 != compass_screen::caption(mode).0 {
                        times.caption = from_now(times.caption);
                    }
                }
            }
            if mode == Mode::TopEdgeUp && shown.heading().is_some() {
                self.top_edge_since = Some(now);
            } else if mode != Mode::TopEdgeUp {
                self.top_edge_since = None;
            }
            *shown = mode;
        }
        let entry = Accents {
            ring: progress(now, times.ring, RING_FADE),
            icon_rows: rows_built(now, times.icon),
            caption: progress(now, times.caption, CAPTION_REVEAL),
            dial: progress(now, times.dial, DIAL_SWEEP),
            divider: progress(now, times.divider, DIVIDER_DRAW),
            hint: progress(now, times.hint, HINT_REVEAL),
        };
        self.fading = entry != Accents::FULL;
        // Going out, the accents follow the page's offset, so reversing a drag restores them.
        let p = swipe_progress(view.offset);
        let exit = Accents {
            ring: leaving(p, 0.6, 0.4),
            icon_rows: rows_leaving(p, 0.3, 0.5),
            caption: leaving(p, 0.2, 0.3),
            dial: leaving(p, 0.2, 0.4),
            divider: leaving(p, 0.1, 0.2),
            hint: leaving(p, 0.0, 0.2),
        };
        entry.min(exit)
    }

    /// Advances the clock face's builds and reveals to `now`, and returns how far each has come.
    fn advance_clock_accents(&mut self, now: Micros) -> clock_screen::Accents {
        use clock_screen::Accents;
        if self.screen != Screen::Clock {
            self.clock_settled = None;
            return Accents::FULL;
        }
        let view = self.pager.view();
        let keys = clock_screen::Keys::of(&self.peripherals.clock, &self.peripherals.zone);
        let (times, shown) = match &mut self.clock_settled {
            Some(settled) => settled,
            None if view.offset == 0 => {
                let start = |delay: Micros| Some(now + delay);
                let mut times = ClockTimes {
                    ring: start(CLOCK_ENTRY.ring),
                    icon: start(CLOCK_ENTRY.icon),
                    label: start(CLOCK_ENTRY.label),
                    plate: start(CLOCK_ENTRY.plate),
                    zone: start(CLOCK_ENTRY.zone),
                };
                if keys.mode == clock_screen::Mode::NoData {
                    // A fault shows at once.
                    (times.ring, times.icon, times.label) = (None, None, None);
                }
                self.clock_settled.insert((times, keys.clone()))
            }
            None => return Accents::HIDDEN,
        };
        if *shown != keys {
            if keys.mode == clock_screen::Mode::NoData {
                // A fault shows at once.
                *times = ClockTimes::default();
            } else {
                // The icon rebuilds on any change of state, the label on a change of text, and
                // the plate and the zone name together on any change of zone. One the entry has
                // not reached yet shows the new text when it gets there.
                let restart = |start: &mut Option<Micros>, at: Micros| {
                    if start.is_none_or(|start| start <= now) {
                        *start = Some(at);
                    }
                };
                if shown.mode != keys.mode {
                    restart(&mut times.icon, now);
                }
                if shown.mode.label() != keys.mode.label() {
                    restart(&mut times.label, now);
                }
                if (&shown.plate, shown.zone) != (&keys.plate, keys.zone)
                    && times.plate.is_none_or(|start| start <= now)
                {
                    times.plate = Some(now);
                    restart(&mut times.zone, now + (CLOCK_ENTRY.zone - CLOCK_ENTRY.plate));
                }
            }
            *shown = keys;
        }
        let entry = Accents {
            ring: progress(now, times.ring, RING_FADE),
            icon_rows: rows_built(now, times.icon),
            label: progress(now, times.label, CLOCK_LABEL_REVEAL),
            plate: progress(now, times.plate, CLOCK_PLATE_REVEAL),
            zone: progress(now, times.zone, CLOCK_ZONE_REVEAL),
        };
        self.fading |= entry != Accents::FULL;
        let p = swipe_progress(view.offset);
        let exit = Accents {
            ring: leaving(p, 0.6, 0.4),
            icon_rows: rows_leaving(p, 0.3, 0.5),
            label: leaving(p, 0.2, 0.3),
            plate: leaving(p, 0.1, 0.3),
            zone: leaving(p, 0.0, 0.2),
        };
        entry.min(exit)
    }

    /// Draws the current state into `target`.
    pub fn draw<D>(&self, target: &mut D)
    where
        D: CoverageTarget<Color = Color>,
        D::Error: core::fmt::Debug,
    {
        let view = self.pager.view();
        prototypes::render(
            prototypes::ACTIVE_ARCHITECTURE,
            prototypes::State {
                screen: self.screen,
                selected_node: self.selected_node,
                peripherals: self.peripherals,
                offset: view.offset,
                neighbour: view
                    .neighbour
                    .map(|(page, offset)| (Screen::ALL[page], offset)),
                compass_accents: self.accents,
                clock_accents: self.clock_accents,
            },
            &self.renderer,
            target,
        )
        .expect("prototype renderer failed")
    }
}

fn selected_node(point: Point) -> Option<u8> {
    let (x, y) = (point.x, point.y);
    if (100..=164).contains(&x) && (146..=210).contains(&y) {
        Some(1)
    } else if (262..=326).contains(&x) && (206..=270).contains(&y) {
        Some(2)
    } else if (322..=386).contains(&x) && (284..=348).contains(&y) {
        Some(3)
    } else {
        None
    }
}

/// How far an accent that starts at `start` and takes `duration` has come, 0 to 255.
fn progress(now: Micros, start: Option<Micros>, duration: Micros) -> u8 {
    match start {
        None => u8::MAX,
        Some(start) if now < start => 0,
        Some(start) => ((now - start).min(duration) * 255 / duration) as u8,
    }
}

/// How many rows of an icon's modules have landed, a row every `ICON_ROW` from `start`.
fn rows_built(now: Micros, start: Option<Micros>) -> u8 {
    match start {
        None => 5,
        Some(start) if now < start => 0,
        Some(start) => ((now - start) / ICON_ROW + 1).min(5) as u8,
    }
}

/// How far a page at `offset` has gone towards hiding its accents: 0 at rest, 1 at
/// `SWIPE_FADE` of the panel's width.
fn swipe_progress(offset: i32) -> f32 {
    offset.unsigned_abs() as f32 / board::LCD_WIDTH as f32 / SWIPE_FADE
}

/// What is left of an accent that leaves over `from..from + over` of the swipe, 0 to 255.
fn leaving(p: f32, from: f32, over: f32) -> u8 {
    libm::roundf((1.0 - ((p - from) / over).clamp(0.0, 1.0)) * 255.0) as u8
}

fn rows_leaving(p: f32, from: f32, over: f32) -> u8 {
    libm::roundf(5.0 * f32::from(leaving(p, from, over)) / 255.0) as u8
}
