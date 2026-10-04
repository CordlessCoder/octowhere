//! One device in the window: its stage, the readings the keyboard sets for it, its board keys
//! and its panel, stepped on whatever clock its board runs on.

use std::time::Duration;

use embedded_graphics::prelude::Point;
use minifb::Key;
use octowhere_ui::ui::{
    clock::{ClockState, ZoneMode, ZoneState},
    compass::CompassView,
    group::{sim::Sim as Scripted, view::Request},
    screens::{Battery, Gnss, GnssHealth, PeripheralState},
    stage::{Input, Key as PowerKey, Motion, Sensors, Stage, Store, Touch, TouchGesture, Update},
    startup::Report,
};

use crate::{Panel, scenes};

/// The longest press the touch controller's gesture mode takes for a tap.
const TAP_US: u64 = 300_000;
/// The motion task's sample periods, fast while a compass screen shows.
const FAST_SAMPLE_US: u64 = 20_000;
const SLOW_SAMPLE_US: u64 = 250_000;
const SENSOR_PERIOD_US: u64 = 1_000_000;
/// How long K is held before it counts as a long press, and how long to power the board on, as
/// the power controller is set to.
const KEY_LONG_US: u64 = 1_000_000;
const POWER_ON_US: u64 = 512_000;

/// A key held down: when it went down, and whether it has already reported a long press.
#[derive(Default)]
struct Held(Option<(u64, bool)>);

impl Held {
    /// The press `down` makes at `now`: long once held a second, short if let go before.
    fn press(&mut self, down: bool, now: u64) -> Option<PowerKey> {
        match (down, self.0) {
            (true, None) => self.0 = Some((now, false)),
            (true, Some((since, false))) if now - since >= KEY_LONG_US => {
                self.0 = Some((since, true));
                return Some(PowerKey::Long);
            }
            (false, Some((_, long))) => {
                self.0 = None;
                return (!long).then_some(PowerKey::Short);
            }
            _ => {}
        }
        None
    }
}

/// The readings the keyboard controls.
pub struct Readings {
    heading: f32,
    pitch: i32,
    roll: i32,
    pub calibration: u8,
    disturbed: bool,
    live: bool,
    spinning: bool,
    vertical: bool,
    /// Which of [`ZONES`] the clock shows.
    zone: usize,
    /// Which of [`CLOCKS`] the clock reads.
    clock: usize,
    /// A zone chosen in the settings panel, which the sensor task would report from then on.
    chosen: Option<ZoneState>,
    /// Which of [`SUPPLIES`] the power controller reports, and the battery's level.
    supply: usize,
    level: u8,
    /// Whether GNSS has a fix, and where, in degrees × 10⁷.
    pub fix: bool,
    pub position: (i32, i32),
    /// Which of [`RECEIVER`] the GNSS task reports of the receiver.
    receiver: usize,
}

/// The receiver's health N steps through: answering, being reset after it stopped (with the
/// failed resets so far), and faulted after three.
const RECEIVER: [Option<u8>; 5] = [None, Some(0), Some(1), Some(2), Some(3)];

/// The supplies B steps through: whether a battery is fitted, whether USB is in, and whether
/// the battery charges.
const SUPPLIES: [(bool, bool, bool); 3] = [
    (true, true, true),
    (true, false, false),
    (false, true, false),
];

/// A cell's voltage at each level, roughly, between which [`millivolts`] interpolates.
const DISCHARGE: [(u8, u16); 6] = [
    (0, 3300),
    (10, 3600),
    (20, 3700),
    (50, 3800),
    (80, 3950),
    (100, 4150),
];

/// The voltage the settings' battery screen shows at `percent`.
fn millivolts(percent: u8) -> u16 {
    let above = DISCHARGE
        .iter()
        .position(|&(at, _)| at >= percent)
        .unwrap_or(DISCHARGE.len() - 1)
        .max(1);
    let ((p0, v0), (p1, v1)) = (DISCHARGE[above - 1], DISCHARGE[above]);
    let along = u32::from(percent.clamp(p0, p1) - p0);
    (u32::from(v0) + u32::from(v1 - v0) * along / u32::from(p1 - p0)) as u16
}

/// The clock's states R steps through: whether GNSS has set it, whether it stopped, and whether
/// it can be read.
const CLOCKS: [(bool, bool, bool); 4] = [
    (true, false, true),
    (false, false, true),
    (false, true, true),
    (false, false, false),
];

/// The zones Z steps through, and whether each was chosen by hand. `None` is automatic mode
/// before any fix.
const ZONES: [Option<(&str, ZoneMode)>; 4] = [
    None,
    Some(("Europe/Dublin", ZoneMode::Automatic)),
    Some(("America/New_York", ZoneMode::Manual)),
    Some(("Asia/Kolkata", ZoneMode::Manual)),
];

impl Readings {
    pub fn new(position: (i32, i32)) -> Self {
        Self {
            heading: 37.0,
            pitch: 0,
            roll: 0,
            calibration: 100,
            disturbed: false,
            live: true,
            spinning: false,
            vertical: false,
            zone: 1,
            clock: 0,
            chosen: None,
            supply: 0,
            level: 87,
            fix: true,
            position,
            receiver: 0,
        }
    }

    fn compass(&self) -> CompassView {
        if !self.live {
            return CompassView {
                calibration_percent: self.calibration,
                ..CompassView::default()
            };
        }
        CompassView {
            live: true,
            calibration_percent: self.calibration,
            heading_decidegrees: (self.calibration >= 100 && !self.vertical)
                .then(|| (self.heading.rem_euclid(360.0) * 10.0).round() as u16 % 3600),
            pitch_deg: self.pitch.clamp(-90, 90) as i8,
            roll_deg: self.roll.clamp(-128, 127) as i8,
            disturbed: self.disturbed,
        }
    }

    /// Applies a key, and says whether it changed a reading.
    fn press(&mut self, key: Key, shift: bool) -> bool {
        let step = if shift { 1.0 } else { 5.0 };
        let tilt = if shift { 1 } else { 5 };
        match key {
            Key::Left => self.heading -= step,
            Key::Right => self.heading += step,
            Key::Up => self.pitch += tilt,
            Key::Down => self.pitch -= tilt,
            Key::Q => self.roll -= tilt,
            Key::E => self.roll += tilt,
            Key::C => {
                self.calibration = match self.calibration {
                    0 => 54,
                    100 => 0,
                    _ => 100,
                }
            }
            Key::D => self.disturbed = !self.disturbed,
            Key::L => self.live = !self.live,
            Key::T => self.vertical = !self.vertical,
            Key::Space => self.spinning = !self.spinning,
            Key::Z => {
                self.zone = (self.zone + 1) % ZONES.len();
                self.chosen = None;
            }
            Key::B => self.supply = (self.supply + 1) % SUPPLIES.len(),
            Key::Minus => self.level = self.level.saturating_sub(tilt as u8),
            Key::Equal => self.level = (self.level + tilt as u8).min(100),
            Key::G => self.fix = !self.fix,
            Key::N => self.receiver = (self.receiver + 1) % RECEIVER.len(),
            Key::R => self.clock = (self.clock + 1) % CLOCKS.len(),
            _ => return false,
        }
        true
    }

    /// What the sensor task reports, which the stage takes once a second.
    fn reported(&self) -> (usize, usize, usize, u8, bool, usize) {
        (
            self.zone,
            self.clock,
            self.supply,
            self.level,
            self.fix,
            self.receiver,
        )
    }

    fn zone_state(&self) -> ZoneState {
        if let Some(chosen) = self.chosen {
            return chosen;
        }
        ZONES[self.zone].map_or(ZoneState::default(), |(name, mode)| ZoneState {
            mode,
            zone: octowhere_ui::tz::DATABASE.find(name).map(|zone| zone.id),
        })
    }

    fn battery(&self) -> Battery {
        let (present, usb, charging) = SUPPLIES[self.supply];
        let level = if present { self.level } else { 0 };
        Battery {
            present,
            percent: level,
            millivolts: if present { millivolts(level) } else { 0 },
            charging,
            usb,
        }
    }

    /// Whether the RTC can be read, which the mesh takes UTC from without GPS.
    pub fn rtc_readable(&self) -> bool {
        let (_, stopped, readable) = CLOCKS[self.clock];
        readable && !stopped
    }

    /// UTC at `utc_s`, in the clock state R selects, and the rest of the readings, with the
    /// receiver last answering at `answered` on the device's clock.
    fn sensors(&self, utc_s: i64, answered: Option<u64>) -> Sensors {
        let (gnss, stopped, readable) = CLOCKS[self.clock];
        Sensors {
            clock: ClockState {
                utc: readable.then_some(utc_s),
                set_from_gnss: gnss,
                stopped,
            },
            zone: self.zone_state(),
            battery: Some(self.battery()),
            gnss: Gnss {
                fix: self.fix,
                in_use: if self.fix { 9 } else { 0 },
                in_view: 14,
                position: self.fix.then_some(self.position),
                health: GnssHealth {
                    recovering: RECEIVER[self.receiver].is_some(),
                    failed_resets: RECEIVER[self.receiver].unwrap_or(0),
                    last_response: answered,
                    last_fix: answered.filter(|_| self.fix),
                },
            },
        }
    }
}

/// Which of the board's controls are held: PWR, BOOT, and a hand over the screen.
#[derive(Clone, Copy, Default)]
pub struct Controls {
    pub power: bool,
    pub boot: bool,
    pub cover: bool,
}

/// What a step asks of the device's surroundings.
#[derive(Default)]
pub struct Stepped {
    pub mesh: Option<Request>,
    /// The device powered off (false) or on again (true).
    pub powered: Option<bool>,
}

pub struct Device {
    pub stage: Stage,
    pub readings: Readings,
    pub panel: Panel,
    /// What prefixes the device's lines in the terminal.
    label: String,
    /// The mesh behind a device with no air around it: its other device pairs with whatever
    /// this one asks.
    scripted: Option<Scripted>,
    /// How many views the device's node had published when its stage last took one.
    pub seen: u32,
    next_motion: u64,
    next_sensors: u64,
    samples_fast: bool,
    readings_changed: bool,
    sensors_changed: bool,
    pub redraw: bool,
    power_key: Held,
    boot_key: Held,
    /// When the button went down while the controller watched for gestures.
    pressed_at: Option<u64>,
    pub powered_off: bool,
    /// While powered off, when PWR went down.
    power_on_since: Option<u64>,
    /// The start-up's reports not yet stepped in, in µs after power-on, latest first, and when
    /// the device powered on, which the first step after sets.
    reports: Vec<(Report, u64)>,
    booted: Option<u64>,
    /// The title's account of the last draw.
    pub drawn: Option<String>,
    /// When the receiver last answered, on the device's clock.
    answered: Option<u64>,
}

fn peripherals() -> PeripheralState {
    PeripheralState {
        firmware: "0.1.0",
        ..PeripheralState::default()
    }
}

impl Device {
    pub fn new(masked: bool, readings: Readings, label: String, scripted: bool) -> Self {
        Self {
            stage: Stage::new(peripherals()),
            readings,
            panel: Panel::new(masked),
            label,
            scripted: scripted.then(|| Scripted::new(None)),
            seen: 0,
            next_motion: 0,
            next_sensors: 0,
            samples_fast: false,
            readings_changed: true,
            sensors_changed: true,
            redraw: true,
            power_key: Held::default(),
            boot_key: Held::default(),
            pressed_at: None,
            powered_off: false,
            power_on_since: None,
            reports: Vec::new(),
            booted: None,
            drawn: None,
            answered: None,
        }
    }

    /// Applies a reading's key, and says whether it was one.
    pub fn press(&mut self, key: Key, shift: bool) -> bool {
        let before = self.readings.reported();
        let pressed = self.readings.press(key, shift);
        self.readings_changed |= pressed;
        self.sensors_changed |= self.readings.reported() != before;
        pressed
    }

    /// Starts the device again from power-on, as a reset does, keeping its readings but the
    /// zone the settings chose.
    pub fn reset(&mut self) {
        self.stage = Stage::starting(peripherals());
        if let Some(scripted) = &self.scripted {
            self.stage.set_mesh(scripted.view().clone());
        }
        self.readings.chosen = None;
        self.reports = scenes::ANSWERING
            .iter()
            .rev()
            .map(|&(report, at)| (report, at * 1_000))
            .collect();
        // The clock may start again from zero.
        self.panel.forget_touch();
        (self.booted, self.answered) = (None, None);
        (self.next_motion, self.next_sensors) = (0, 0);
        (self.power_key, self.boot_key) = (Held::default(), Held::default());
        (self.pressed_at, self.power_on_since) = (None, None);
        (self.powered_off, self.redraw) = (false, true);
    }

    /// Lets go of every control without the press it would make, as giving the keyboard to
    /// another device does.
    pub fn release_controls(&mut self) {
        (self.power_key, self.boot_key) = (Held::default(), Held::default());
        self.pressed_at = None;
    }

    /// Steps the stage at `now` on the device's clock, with UTC at `utc_s`, a finger at
    /// `contact` and `held` of its controls, and draws what changed.
    pub fn step(
        &mut self,
        now: u64,
        utc_s: i64,
        contact: Option<Point>,
        held: Controls,
    ) -> Stepped {
        let mut stepped = Stepped::default();
        if self.readings.spinning {
            self.readings.heading += 0.5;
            self.readings_changed = true;
        }
        if self.powered_off {
            // Only the power controller watches the key now.
            if !held.power {
                self.power_on_since = None;
            } else if now - *self.power_on_since.get_or_insert(now) >= POWER_ON_US {
                println!("{}powered on", self.label);
                self.reset();
                // The power controller took that press, and the firmware never sees it.
                self.power_key = Held(Some((0, true)));
                stepped.powered = Some(true);
            }
            return stepped;
        }
        let booted = *self.booted.get_or_insert(now);
        let motion_due = now >= self.next_motion || self.readings_changed;
        if motion_due {
            self.next_motion = now
                + if self.samples_fast {
                    FAST_SAMPLE_US
                } else {
                    SLOW_SAMPLE_US
                };
            self.readings_changed = false;
        }
        let sensors_due = now >= self.next_sensors || self.sensors_changed;
        if sensors_due {
            self.next_sensors = now + SENSOR_PERIOD_US;
            self.sensors_changed = false;
        }
        let touch = if held.cover {
            Some(Touch::Cover)
        } else if self.stage.watches_for_wake() {
            // The controller's gesture mode reports a tap as the finger lifts, and no contacts.
            match (contact, self.pressed_at) {
                (Some(_), None) => {
                    self.pressed_at = Some(now);
                    None
                }
                (None, Some(at)) => {
                    self.pressed_at = None;
                    (now - at < TAP_US).then_some(Touch::Gesture(TouchGesture::Tap))
                }
                _ => None,
            }
        } else {
            self.pressed_at = None;
            Some(Touch::Contacts([contact, None]))
        };
        let boot = match self.reports.last() {
            Some(&(report, at)) if now - booted >= at => {
                self.reports.pop();
                Some(report)
            }
            _ => None,
        };
        if let Some(scripted) = &mut self.scripted
            && scripted.step(now)
        {
            self.stage.set_mesh(scripted.view().clone());
        }
        let update: Update = self.stage.step(Input {
            now,
            touch,
            motion: motion_due.then(|| Motion {
                compass: self.readings.compass(),
            }),
            sensors: sensors_due.then(|| {
                if self.readings.receiver == 0 {
                    self.answered = Some(now);
                }
                self.readings.sensors(utc_s, self.answered)
            }),
            boot,
            key: self.power_key.press(held.power, now),
            boot_key: self.boot_key.press(held.boot, now),
        });
        if update.power_off {
            println!("{}powered off; hold K 512 ms to power on", self.label);
            self.powered_off = true;
            self.panel.forget_touch();
            stepped.powered = Some(false);
        }
        self.samples_fast = update.samples_fast;
        if let Some(request) = update.mesh {
            println!("{}mesh {request:?}", self.label);
            match &mut self.scripted {
                Some(scripted) => scripted.request(request, now),
                None => stepped.mesh = Some(request),
            }
        }
        if update.recalibrate {
            self.readings.calibration = 0;
            self.readings_changed = true;
        }
        if let Some(level) = update.brightness {
            println!("{}brightness {level}", self.label);
        }
        if let Some(store) = update.store {
            println!("{}store {store:?}", self.label);
            // As the sensor task takes the choice and reports it from then on.
            if !matches!(store, Store::Brightness(_)) {
                let zone = self.stage.peripherals().clock.zone();
                self.readings.chosen = Some(ZoneState {
                    zone: zone.zone.or(self.readings.zone_state().zone),
                    ..zone
                });
                self.sensors_changed = true;
            }
        }
        if let Some((pixels_drawn, took)) = self.panel.draw(&self.stage, self.redraw) {
            self.drawn = Some(describe(&self.stage, pixels_drawn, took));
            self.redraw = false;
        }
        stepped
    }
}

fn describe(stage: &Stage, pixels_drawn: u32, took: Duration) -> String {
    format!(
        "{:?}, {} px drawn in {:.2} ms on the host",
        stage.screen(),
        pixels_drawn,
        took.as_secs_f64() * 1e3
    )
}
