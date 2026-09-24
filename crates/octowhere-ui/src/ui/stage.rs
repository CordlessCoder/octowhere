//! The UI between input and drawing: which screen shows, what touch does to it, and when it has
//! to be redrawn. The frame loop feeds it readings and draws it, and acts on the effects it
//! returns. It never touches a device, so the host drives it the same way.

use embedded_graphics::prelude::Point;

use super::{
    axis_check::{self, AxisCheck},
    compass::CompassView,
    compass_screen::{self, Accents, Mode},
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
}

/// One read of the touch controller.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Touch {
    /// Up to two contacts in panel coordinates; none when nothing touches.
    Contacts([Option<Point>; 2]),
    /// The controller recognised a hand covering the screen.
    Cover,
}

/// When each group of the compass's accents starts fading in after the page settles, and how
/// long each fade takes.
const ENTRY_DELAYS: Accents<Micros> = Accents {
    ring: 0,
    icon: 95_000,
    ticks: 190_000,
    letters: 285_000,
};
const ENTRY_FADE: Micros = 110_000;
/// A heading that appears on a settled page reveals the ticks, then the letters this much later.
const REVEAL_FADE: Micros = 170_000;
const REVEAL_LETTERS: Micros = 70_000;
/// A heading back from TOP EDGE UP within this skips the reveal, so tilting through vertical does
/// not pulse the dial.
const TOP_EDGE_GRACE: Micros = 750_000;
/// Dragging the compass this fraction of the panel's width fades its accents out entirely.
const SWIPE_FADE: f32 = 0.35;
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

/// What the frame loop owes after a step.
#[derive(Debug)]
pub struct Update {
    /// The pixels that changed.
    pub changed: Dirty,
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
    /// When the compass page last settled into view, from another page.
    compass_settled: Option<Micros>,
    /// When the heading the dial turns to last appeared.
    heading_since: Option<Micros>,
    /// When the heading gave way to TOP EDGE UP, while nothing else has shown since.
    top_edge_since: Option<Micros>,
    accents: Accents,
    fading: bool,
    /// What the compass page showed after the last step, while it filled the panel.
    drawn_compass: Option<(CompassView, Accents)>,
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
            heading_since: None,
            top_edge_since: None,
            accents: Accents::FULL,
            fading: false,
            drawn_compass: None,
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
        self.heading_since = None;
        self.top_edge_since = None;
        self.drawn_compass = None;
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
        let mut update = Update {
            changed: Dirty::new(),
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
            } else if self.screen == Screen::Compass {
                // Only a cover restarts calibration here.
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
        let compass = (self.screen == Screen::Compass && view.offset == 0 && view.neighbour.is_none())
            .then_some((self.peripherals.compass, self.accents));
        match (self.drawn_compass, compass) {
            (Some(before), Some(after)) => compass_screen::damage(
                (&before.0, before.1),
                (&after.0, after.1),
                &self.renderer,
                &mut update.changed,
            ),
            _ if full => update.changed.make_full(),
            _ => {}
        }
        self.drawn_compass = compass;
        update
    }

    /// A cover counts only on the compass page at rest, while its sensors report.
    fn accepts_cover(&self) -> bool {
        self.screen == Screen::Compass
            && !self.pager.is_moving()
            && self.peripherals.compass.live
    }

    /// Advances the compass page's fades to `now`, and returns how far each group has come.
    fn compass_accents(&mut self, now: Micros) -> Accents {
        self.fading = false;
        if self.screen != Screen::Compass {
            self.compass_settled = None;
            self.heading_since = None;
            self.top_edge_since = None;
            return Accents::FULL;
        }
        let view = self.pager.view();
        if self.compass_settled.is_none() && view.offset == 0 {
            self.compass_settled = Some(now);
        }
        let mode = Mode::of(&self.peripherals.compass);
        let heading = mode.heading().is_some();
        if heading {
            if self.heading_since.is_none() {
                let quick = self
                    .top_edge_since
                    .is_some_and(|since| now.saturating_sub(since) < TOP_EDGE_GRACE);
                // A quick return from TOP EDGE UP shows the dial as though it had never gone.
                let revealed = REVEAL_LETTERS + REVEAL_FADE;
                self.heading_since = Some(if quick { now.saturating_sub(revealed) } else { now });
            }
            self.top_edge_since = None;
        } else if mode != Mode::TopEdgeUp {
            self.heading_since = None;
            self.top_edge_since = None;
        } else if self.heading_since.take().is_some() {
            self.top_edge_since = Some(now);
        }
        let Some(settled) = self.compass_settled else {
            return Accents::HIDDEN;
        };
        let since_settle = now.saturating_sub(settled);
        let since_heading = self.heading_since.map_or(0, |since| now.saturating_sub(since));
        let entry = |delay: Micros| fade(since_settle.saturating_sub(delay), ENTRY_FADE);
        let reveal = |delay: Micros| fade(since_heading.saturating_sub(delay), REVEAL_FADE);
        let swipe =
            (1.0 - view.offset.unsigned_abs() as f32 / board::LCD_WIDTH as f32 / SWIPE_FADE)
                .clamp(0.0, 1.0);
        let scale = |amount: u8| libm::roundf(f32::from(amount) * swipe) as u8;
        let entry_done = since_settle >= ENTRY_DELAYS.letters + ENTRY_FADE;
        let reveal_done = !heading || since_heading >= REVEAL_LETTERS + REVEAL_FADE;
        self.fading = !(entry_done && reveal_done);
        Accents {
            ring: scale(entry(ENTRY_DELAYS.ring)),
            icon: scale(entry(ENTRY_DELAYS.icon)),
            ticks: scale(entry(ENTRY_DELAYS.ticks).min(reveal(0))),
            letters: scale(entry(ENTRY_DELAYS.letters).min(reveal(REVEAL_LETTERS))),
        }
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

/// How far a fade of `duration` has come after `elapsed`, 0 to 255.
fn fade(elapsed: Micros, duration: Micros) -> u8 {
    (elapsed.min(duration) * 255 / duration) as u8
}
