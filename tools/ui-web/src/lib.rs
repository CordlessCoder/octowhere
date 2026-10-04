//! The firmware's UI stage as a WebAssembly module, for the page in `www/`. The page steps it on
//! the browser's clock with the pointer as the touchscreen and its controls as the readings,
//! and paints the RGBA frame [`pixels`] points at. `build.sh` has the commands.
//!
//! One device runs on a scripted mesh, whose other device pairs with whatever it asks. Several
//! run side by side, each its stage and the firmware's own mesh node, on `octowhere-sim`'s
//! simulated air, whose virtual time follows the page's clock at a speed the page sets. The
//! exports that read or set one device act on the one [`select`] chose.
//!
//! The exports take and return plain numbers, so the page needs no generated bindings. The
//! readings are synthetic, like the desktop simulator's.

use std::{cell::RefCell, fmt::Write};

use embedded_graphics::{
    pixelcolor::{Rgb888, RgbColor},
    prelude::Point,
};
use octowhere_sim::air::{Paced, Reach, SPEEDS, position};
use octowhere_ui::{
    board::{LCD_HEIGHT, LCD_WIDTH},
    chrome::{self, Clip, FB},
    ui::{
        clock::{ClockState, ZoneMode, ZoneState},
        compass::CompassView,
        drawer::{Child, Root},
        group::sim::Sim as Mesh,
        group::view::Request,
        rest::Rest,
        screens::{Battery, Gnss, GnssHealth, PeripheralState, Screen},
        second::Page,
        shift,
        stage::{Input, Key, Motion, Sensors, Stage, Store, Touch, TouchGesture},
        startup::{Outcome, Part, Report},
    },
};

const WIDTH: usize = LCD_WIDTH as usize;
const HEIGHT: usize = LCD_HEIGHT as usize;
/// The longest press the touch controller's gesture mode takes for a tap.
const TAP_US: u64 = 300_000;
/// The motion task's sample periods, fast while a compass screen shows, and the sensor task's.
const FAST_SAMPLE_US: u64 = 20_000;
const SLOW_SAMPLE_US: u64 = 250_000;
const SENSOR_PERIOD_US: u64 = 1_000_000;
/// How long a key is held before it counts as a long press, as the power controller is set to.
const KEY_LONG_US: u64 = 1_000_000;
/// How long PWR is held to power the board on, as the power controller is set to (ONLEVEL).
const POWER_ON_US: u64 = 512_000;
/// How far the heading turns each step while it spins, in degrees.
const SPIN_STEP: f32 = 0.5;
/// The most devices the page runs.
const MOST: usize = 4;
/// The most log lines the air keeps, and that the page is handed at once.
const LINES_KEPT: usize = 2_000;

/// What boot reports at start-up, in ms from power-on, as the desktop simulator's start-up
/// scene has it.
const BOOT_REPORTS: [(Report, u64); 8] = [
    (Report::Decided(Part::Power, Outcome::Answered), 150),
    (Report::Decided(Part::Clock, Outcome::Answered), 250),
    (Report::Decided(Part::Touch, Outcome::Answered), 500),
    (Report::Decided(Part::Motion, Outcome::Answered), 600),
    (Report::Decided(Part::Magnet, Outcome::Answered), 750),
    (Report::Decided(Part::Gnss, Outcome::Answered), 1_300),
    (Report::Started(Part::Radio), 1_300),
    (Report::Decided(Part::Radio, Outcome::Answered), 1_302),
];

/// The supplies the page offers: whether a battery is fitted, whether USB is in, and whether
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

/// The clock's states the page offers: whether GNSS has set it, whether it stopped, and whether
/// it can be read.
const CLOCKS: [(bool, bool, bool); 4] = [
    (true, false, true),
    (false, false, true),
    (false, true, true),
    (false, false, false),
];

/// The zones the page offers, and whether each was chosen by hand. `None` is automatic mode
/// before any fix.
const ZONES: [Option<(&str, ZoneMode)>; 4] = [
    None,
    Some(("Europe/Dublin", ZoneMode::Automatic)),
    Some(("America/New_York", ZoneMode::Manual)),
    Some(("Asia/Kolkata", ZoneMode::Manual)),
];

/// The readings [`set`] and [`get`] name by number, in the page's order.
#[derive(Clone, Copy)]
enum Reading {
    Heading,
    Pitch,
    Roll,
    Calibration,
    Disturbed,
    Live,
    Upright,
    Zone,
    Clock,
    Supply,
    Fix,
    Spinning,
    Level,
    Receiver,
}

impl Reading {
    const ALL: [Self; 14] = [
        Self::Heading,
        Self::Pitch,
        Self::Roll,
        Self::Calibration,
        Self::Disturbed,
        Self::Live,
        Self::Upright,
        Self::Zone,
        Self::Clock,
        Self::Supply,
        Self::Fix,
        Self::Spinning,
        Self::Level,
        Self::Receiver,
    ];
}

#[derive(Clone, Copy)]
struct Readings {
    heading: f32,
    pitch: i32,
    roll: i32,
    calibration: u8,
    disturbed: bool,
    live: bool,
    upright: bool,
    zone: usize,
    clock: usize,
    /// A zone chosen in the settings panel, which the sensor task would report from then on.
    chosen: Option<ZoneState>,
    supply: usize,
    /// The battery's level, 0 to 100.
    level: u8,
    fix: bool,
    spinning: bool,
    /// Where GNSS places the device, in degrees × 10⁷.
    position: (i32, i32),
    /// The GNSS receiver: answering (0), being reset after it stopped with 0 to 2 resets failed
    /// (1 to 3), or faulted after three (4).
    receiver: u8,
}

impl Readings {
    fn at(position: (i32, i32)) -> Self {
        Self {
            heading: 37.0,
            pitch: 0,
            roll: 0,
            calibration: 100,
            disturbed: false,
            live: true,
            upright: false,
            zone: 1,
            clock: 0,
            chosen: None,
            supply: 0,
            level: 87,
            fix: true,
            spinning: false,
            position,
            receiver: 0,
        }
    }

    /// Whether the RTC can be read, which the mesh takes UTC from without GPS.
    fn rtc_readable(&self) -> bool {
        let (_, stopped, readable) = CLOCKS[self.clock];
        readable && !stopped
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
            heading_decidegrees: (self.calibration >= 100 && !self.upright)
                .then(|| (self.heading.rem_euclid(360.0) * 10.0).round() as u16 % 3600),
            pitch_deg: self.pitch.clamp(-90, 90) as i8,
            roll_deg: self.roll.clamp(-128, 127) as i8,
            disturbed: self.disturbed,
        }
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

    /// The sensor task's snapshot at UTC second `utc`, the receiver having last answered at
    /// `answered` on the device's clock.
    fn sensors(&self, utc: i64, answered: Option<u64>) -> Sensors {
        let (gnss, stopped, readable) = CLOCKS[self.clock];
        Sensors {
            clock: ClockState {
                utc: readable.then_some(utc),
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
                    recovering: self.receiver > 0,
                    failed_resets: self.receiver.saturating_sub(1),
                    last_response: answered,
                    last_fix: answered.filter(|_| self.fix),
                },
            },
        }
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

    fn get(&self, reading: Reading) -> f64 {
        match reading {
            Reading::Heading => f64::from(self.heading.rem_euclid(360.0)),
            Reading::Pitch => f64::from(self.pitch),
            Reading::Roll => f64::from(self.roll),
            Reading::Calibration => f64::from(self.calibration),
            Reading::Disturbed => f64::from(u8::from(self.disturbed)),
            Reading::Live => f64::from(u8::from(self.live)),
            Reading::Upright => f64::from(u8::from(self.upright)),
            Reading::Zone => self.zone as f64,
            Reading::Clock => self.clock as f64,
            Reading::Supply => self.supply as f64,
            Reading::Fix => f64::from(u8::from(self.fix)),
            Reading::Spinning => f64::from(u8::from(self.spinning)),
            Reading::Level => f64::from(self.level),
            Reading::Receiver => f64::from(self.receiver),
        }
    }

    fn set(&mut self, reading: Reading, value: f64) {
        let index = |len: usize| (value.max(0.0) as usize).min(len - 1);
        match reading {
            Reading::Heading => self.heading = value as f32,
            Reading::Pitch => self.pitch = value as i32,
            Reading::Roll => self.roll = value as i32,
            Reading::Calibration => self.calibration = value.clamp(0.0, 100.0) as u8,
            Reading::Disturbed => self.disturbed = value != 0.0,
            Reading::Live => self.live = value != 0.0,
            Reading::Upright => self.upright = value != 0.0,
            Reading::Zone => {
                self.zone = index(ZONES.len());
                self.chosen = None;
            }
            Reading::Clock => self.clock = index(CLOCKS.len()),
            Reading::Supply => self.supply = index(SUPPLIES.len()),
            Reading::Fix => self.fix = value != 0.0,
            Reading::Spinning => self.spinning = value != 0.0,
            Reading::Level => self.level = value.clamp(0.0, 100.0) as u8,
            Reading::Receiver => self.receiver = value.clamp(0.0, 4.0) as u8,
        }
    }
}

/// A key held down: when it went down, and whether it has already reported a long press.
#[derive(Default)]
struct Held(Option<(u64, bool)>);

impl Held {
    /// The press `down` makes at `now`: long once held a second, short if let go before.
    fn press(&mut self, down: bool, now: u64) -> Option<Key> {
        match (down, self.0) {
            (true, None) => self.0 = Some((now, false)),
            (true, Some((since, false))) if now - since >= KEY_LONG_US => {
                self.0 = Some((since, true));
                return Some(Key::Long);
            }
            (false, Some((_, long))) => {
                self.0 = None;
                return (!long).then_some(Key::Short);
            }
            _ => {}
        }
        None
    }
}

/// What a step asks of the device's surroundings, and whether its frame changed.
#[derive(Default)]
struct Stepped {
    changed: bool,
    mesh: Option<Request>,
    /// The device powered off (false) or on again (true).
    powered: Option<bool>,
}

struct Device {
    stage: Stage,
    fb: Box<FB>,
    /// What the page paints: the framebuffer moved by the pixel shift, as RGBA.
    rgba: Vec<[u8; 4]>,
    shift: Point,
    readings: Readings,
    /// The page's clock when the device started, in µs, so that the stage's clock starts at 1
    /// on a device with no node to take its clock from.
    origin: Option<u64>,
    next_motion: u64,
    next_sensors: u64,
    samples_fast: bool,
    readings_changed: bool,
    sensors_changed: bool,
    redraw: bool,
    power_key: Held,
    boot_key: Held,
    /// When the pointer went down while the controller watched for gestures.
    pressed_at: Option<u64>,
    /// The start-up's reports not yet stepped in.
    reports: Vec<(Report, u64)>,
    powered_off: bool,
    /// The stage's time at the last step.
    now: u64,
    /// What the last step sent the panel, as the display core's flush would: x, y, width and
    /// height, on the panel.
    flushed: Vec<[i32; 4]>,
    /// When PWR went down while powered off, on the page's clock.
    power_on_since: Option<u64>,
    /// A mesh with no radio behind it, whose other device pairs with whatever this one asks,
    /// for a device with no air around it.
    scripted: Option<Mesh>,
    /// How many views the device's node had published when its stage last took one.
    seen: u32,
    /// When the GNSS receiver last answered, on the device's clock.
    answered: Option<u64>,
}

impl Device {
    fn new(start_up: bool, readings: Readings, scripted: bool) -> Self {
        let peripherals = PeripheralState {
            firmware: "0.1.0",
            ..PeripheralState::default()
        };
        let stage = if start_up {
            Stage::starting(peripherals)
        } else {
            Stage::new(peripherals)
        };
        Self {
            stage,
            fb: FB::boxed(),
            rgba: vec![[0, 0, 0, 0xff]; WIDTH * HEIGHT],
            shift: Point::zero(),
            readings,
            origin: None,
            next_motion: 0,
            next_sensors: 0,
            samples_fast: false,
            readings_changed: true,
            sensors_changed: true,
            redraw: true,
            power_key: Held::default(),
            boot_key: Held::default(),
            pressed_at: None,
            reports: if start_up {
                BOOT_REPORTS.iter().rev().copied().collect()
            } else {
                Vec::new()
            },
            powered_off: false,
            now: 0,
            flushed: Vec::new(),
            power_on_since: None,
            scripted: scripted.then(|| Mesh::new(None)),
            seen: 0,
            answered: None,
        }
    }

    /// Starts the device again, through the start-up when `start_up`, as a reset does, keeping
    /// its readings but the zone the settings chose.
    fn reset(&mut self, start_up: bool) {
        let readings = Readings {
            chosen: None,
            ..self.readings
        };
        let seen = self.seen;
        *self = Self::new(start_up, readings, self.scripted.is_some());
        self.seen = seen;
    }

    /// Lets go of every control without the press it would make.
    fn release_controls(&mut self) {
        (self.power_key, self.boot_key) = (Held::default(), Held::default());
        self.pressed_at = None;
    }

    /// Steps the stage at the page's time `page_us`, or at `clock` on the device's node when
    /// it has one, with UTC at `utc`, and redraws what changed.
    fn step(
        &mut self,
        page_us: u64,
        clock: Option<u64>,
        utc: i64,
        contact: Option<Point>,
        held: Controls,
    ) -> Stepped {
        let mut stepped = Stepped::default();
        if self.powered_off {
            if !held.power {
                self.power_on_since = None;
                return stepped;
            }
            let since = *self.power_on_since.get_or_insert(page_us);
            if page_us - since < POWER_ON_US {
                return stepped;
            }
            self.reset(true);
            // The power controller took that press, and the firmware never sees it.
            self.power_key = Held(Some((0, true)));
            stepped.powered = Some(true);
            // The node starts again with it, from zero on its clock.
            return stepped;
        }
        let now = match clock {
            Some(clock) => clock + 1,
            None => page_us - *self.origin.get_or_insert(page_us) + 1,
        };
        self.now = now;
        if self.readings.spinning {
            self.readings.heading += SPIN_STEP;
            self.readings_changed = true;
        }
        let motion_due = now >= self.next_motion || self.readings_changed;
        if motion_due {
            let period = if self.samples_fast {
                FAST_SAMPLE_US
            } else {
                SLOW_SAMPLE_US
            };
            self.next_motion = now + period;
            self.readings_changed = false;
        }
        let sensors_due = now >= self.next_sensors || self.sensors_changed;
        if sensors_due {
            self.next_sensors = now + SENSOR_PERIOD_US;
            self.sensors_changed = false;
        }
        let boot = match self.reports.last() {
            Some(&(report, at)) if now >= at * 1_000 => {
                self.reports.pop();
                Some(report)
            }
            _ => None,
        };
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
        if let Some(scripted) = &mut self.scripted
            && scripted.step(now)
        {
            self.stage.set_mesh(scripted.view().clone());
        }
        let update = self.stage.step(Input {
            now,
            touch,
            motion: motion_due.then(|| Motion {
                compass: self.readings.compass(),
            }),
            sensors: sensors_due.then(|| {
                if self.readings.receiver == 0 {
                    self.answered = Some(now);
                }
                self.readings.sensors(utc, self.answered)
            }),
            boot,
            key: self.power_key.press(held.power, now),
            boot_key: self.boot_key.press(held.boot, now),
        });
        self.samples_fast = update.samples_fast;
        if let Some(request) = update.mesh {
            match &mut self.scripted {
                Some(scripted) => scripted.request(request, now),
                None => stepped.mesh = Some(request),
            }
        }
        if update.recalibrate {
            self.readings.calibration = 0;
            self.readings_changed = true;
        }
        if let Some(Store::ManualZone(_) | Store::AutomaticZone | Store::Clear) = update.store {
            // As the sensor task takes the choice and reports it from then on.
            let zone = self.stage.peripherals().clock.zone();
            self.readings.chosen = Some(ZoneState {
                zone: zone.zone.or(self.readings.zone_state().zone),
                ..zone
            });
            self.sensors_changed = true;
        }
        if update.power_off {
            self.powered_off = true;
            stepped.powered = Some(false);
        }
        stepped.changed = self.draw();
        stepped
    }

    /// Draws the stage's damage, or all of it after a start, and refreshes `rgba`. Says whether
    /// anything changed.
    fn draw(&mut self) -> bool {
        let changed = self.stage.changed();
        let moved = self.stage.shift() != self.shift;
        self.shift = self.stage.shift();
        let whole = core::mem::take(&mut self.redraw);
        self.flushed.clear();
        if !whole && changed.is_empty() && !moved {
            return false;
        }
        if whole || changed.is_full() || moved {
            self.flushed.push([0, 0, WIDTH as i32, HEIGHT as i32]);
        } else {
            let shift = self.shift;
            self.flushed
                .extend(changed.rectangles(chrome::FLUSH_OVERHEAD).map(|region| {
                    let corner = region.top_left + shift;
                    [
                        corner.x,
                        corner.y,
                        region.size.width as i32,
                        region.size.height as i32,
                    ]
                }));
        }
        // One buffer, so each step repaints only its own damage, as the firmware's buffers
        // would with the step before added.
        if whole || changed.is_full() {
            self.stage.draw(&mut *self.fb);
        } else if !changed.is_empty() {
            self.stage.draw(&mut Clip::new(&mut *self.fb, changed));
        }
        for (index, pixel) in self.rgba.iter_mut().enumerate() {
            let point = Point::new(
                shift::source((index % WIDTH) as i32, self.shift.x, WIDTH as i32),
                shift::source((index / WIDTH) as i32, self.shift.y, HEIGHT as i32),
            );
            let color = Rgb888::from(self.fb.pixel(point).expect("inside the panel"));
            *pixel = [color.r(), color.g(), color.b(), 0xff];
        }
        true
    }

    fn status(&self) -> u32 {
        let rest = match self.stage.rest() {
            Rest::Awake => 0,
            Rest::Dimmed { .. } | Rest::Darkening { .. } => 1,
            Rest::AlwaysOn => 2,
            Rest::Off => 3,
        };
        let screen = match self.stage.screen() {
            Screen::Clock => 0,
            Screen::Compass => 1,
        };
        let view = if self.stage.power_off().is_some() {
            9
        } else if let Some(drawer) = self.stage.drawer() {
            match (drawer.child(), drawer.root()) {
                (Some(Child::Event(_)), _) => 13,
                (Some(Child::Manage), _) => 14,
                (None, Root::Events) => 11,
                (None, Root::Messages) => 12,
            }
        } else if let Some(page) = self.stage.page() {
            match page {
                Page::Brightness(_) => 2,
                Page::Device(_) => 3,
                Page::Clear(_) => 4,
                Page::Picker(_) => 5,
                Page::Replay(_) => 6,
                Page::Timeout(_) => 7,
                Page::AlwaysOn(_) => 8,
                Page::Group(_) => 10,
            }
        } else {
            u32::from(self.stage.panel_offset() > 0)
        };
        u32::from(self.stage.starting_up())
            | rest << 1
            | screen << 3
            | u32::from(self.powered_off) << 4
            | view << 5
    }
}

/// Which of the board's controls are held.
#[derive(Clone, Copy)]
struct Controls {
    power: bool,
    boot: bool,
    cover: bool,
}

/// The simulated air several devices share, and the log lines the page has not taken.
struct Air {
    paced: Paced,
    /// The number of the next log line the page has not taken.
    taken: usize,
}

/// The devices on the page, and the air between them when there are several.
struct World {
    devices: Vec<Device>,
    air: Option<Air>,
    /// The device the pointer, the controls and the exports that read one act on.
    selected: usize,
    /// Text handed to the page, which stays at its address until the next call that fills it.
    text: String,
}

thread_local! {
    static WORLD: RefCell<Option<World>> = const { RefCell::new(None) };
}

fn with_world<T>(f: impl FnOnce(&mut World) -> T) -> T {
    WORLD.with_borrow_mut(|world| f(world.as_mut().expect("`start` runs first")))
}

fn with<T>(f: impl FnOnce(&mut Device) -> T) -> T {
    with_world(|world| f(&mut world.devices[world.selected]))
}

#[link(wasm_import_module = "env")]
unsafe extern "C" {
    /// Logs `len` bytes of UTF-8 at `text` as an error in the browser's console.
    fn console_error(text: *const u8, len: usize);
}

fn set_panic_hook() {
    std::panic::set_hook(Box::new(|info| {
        let text = info.to_string();
        // SAFETY: the page's import reads `len` bytes at `text`, which outlive the call.
        unsafe { console_error(text.as_ptr(), text.len()) };
    }));
}

/// Device `n`'s readings from the last run, or new ones where it stands.
fn readings_from(last: Option<&World>, n: usize) -> Readings {
    last.and_then(|world| world.devices.get(n)).map_or_else(
        || Readings::at(position(n)),
        |device| Readings {
            chosen: None,
            ..device.readings
        },
    )
}

/// Starts the simulation again with one device on a scripted mesh, on the start-up sequence
/// when `start_up` is non-zero or on the clock face. The readings carry over from the last run.
#[unsafe(no_mangle)]
pub extern "C" fn start(start_up: u32) {
    set_panic_hook();
    WORLD.with_borrow_mut(|world| {
        let readings = readings_from(world.as_ref(), 0);
        *world = Some(World {
            devices: vec![Device::new(start_up != 0, readings, true)],
            air: None,
            selected: 0,
            text: String::new(),
        });
    });
}

/// Starts the simulation again with `count` devices, up to four, on the simulated air, each on
/// its clock face, all in one group when `in_group` is non-zero or else each alone, with UTC at
/// `utc` seconds. The first device is selected.
#[unsafe(no_mangle)]
pub extern "C" fn start_devices(count: u32, in_group: u32, utc: f64) {
    set_panic_hook();
    let count = (count as usize).clamp(2, MOST);
    WORLD.with_borrow_mut(|world| {
        let devices = (0..count)
            .map(|n| Device::new(false, readings_from(world.as_ref(), n), false))
            .collect();
        *world = Some(World {
            devices,
            air: Some(Air {
                paced: Paced::new(count, in_group != 0, utc as i64, LINES_KEPT),
                taken: 0,
            }),
            selected: 0,
            text: String::new(),
        });
    });
}

/// Chooses the device the pointer, the controls and the exports that read one act on.
#[unsafe(no_mangle)]
pub extern "C" fn select(n: u32) {
    with_world(|world| {
        if (n as usize) < world.devices.len() {
            world.selected = n as usize;
        }
    });
}

/// Gives device `n` the pointer and the controls. The device left behind lets go of them
/// without the press they would make.
#[unsafe(no_mangle)]
pub extern "C" fn focus(n: u32) {
    with_world(|world| {
        if (n as usize) < world.devices.len() && n as usize != world.selected {
            world.devices[world.selected].release_controls();
            world.selected = n as usize;
        }
    });
}

/// Steps every device at the page's clock, `page_ms`, with the host's UTC time in seconds, and
/// the selected one with the pointer at (`x`, `y`) in panel pixels when `down` and `controls`
/// holding which of PWR (1), BOOT (2) and a hand over the screen (4) are held. Several devices
/// take UTC from the air instead. Returns a bit for each device whose frame changed, the
/// first device's lowest. Held long enough on a device powered off, PWR powers it on again.
#[unsafe(no_mangle)]
pub extern "C" fn step(page_ms: f64, utc: f64, x: f64, y: f64, down: u32, controls: u32) -> u32 {
    with_world(|world| {
        let page_us = (page_ms * 1_000.0) as u64;
        let input = down != 0 || controls != 0;
        if let Some(air) = &mut world.air {
            air.paced.advance(page_us, input);
        }
        let utc = world
            .air
            .as_ref()
            .map_or(utc as i64, |air| air.paced.utc_s());
        let mut changed = 0;
        for (n, device) in world.devices.iter_mut().enumerate() {
            let selected = n == world.selected;
            let contact =
                (selected && down != 0).then(|| Point::new(x.round() as i32, y.round() as i32));
            let held = Controls {
                power: selected && controls & 1 != 0,
                boot: selected && controls & 2 != 0,
                cover: selected && controls & 4 != 0,
            };
            let clock = world.air.as_ref().map(|air| {
                let readings = &device.readings;
                air.paced.sense(
                    n,
                    readings.fix.then_some(readings.position),
                    readings.rtc_readable(),
                );
                if let Some((view, seen)) = air.paced.sim.view_since(n, device.seen) {
                    device.seen = seen;
                    device.stage.set_mesh(view);
                }
                air.paced.sim.clock(n) as u64
            });
            let stepped = device.step(page_us, clock, utc, contact, held);
            if let Some(air) = &mut world.air {
                if let Some(request) = stepped.mesh {
                    air.paced.sim.command(n, request.into());
                }
                match stepped.powered {
                    Some(false) => air.paced.sim.power_off(n),
                    Some(true) => air.paced.sim.restart(n),
                    None => {}
                }
            }
            changed |= u32::from(stepped.changed) << n;
        }
        changed
    })
}

/// Resets the selected device, as its reset button does: it starts again, through the
/// start-up when `start_up` is non-zero or on the clock face, and its node starts from what it
/// stored.
#[unsafe(no_mangle)]
pub extern "C" fn reset(start_up: u32) {
    with_world(|world| {
        world.devices[world.selected].reset(start_up != 0);
        if let Some(air) = &mut world.air {
            air.paced.sim.restart(world.selected);
        }
    });
}

/// Which of the air's speeds runs, by its place in their list: 1, 2, 5, 10, 30, 60 and 120
/// virtual seconds a second.
#[unsafe(no_mangle)]
pub extern "C" fn speed() -> u32 {
    with_world(|world| world.air.as_ref().map_or(0, |air| air.paced.speed as u32))
}

#[unsafe(no_mangle)]
pub extern "C" fn set_speed(index: u32) {
    with_world(|world| {
        if let Some(air) = &mut world.air {
            air.paced.speed = (index as usize).min(SPEEDS.len() - 1);
        }
    });
}

/// The air's time since it started, in seconds.
#[unsafe(no_mangle)]
pub extern "C" fn air_seconds() -> f64 {
    with_world(|world| world.air.as_ref().map_or(0.0, |air| air.paced.sim.now_s()))
}

/// How a packet from device `from` reaches device `to`: 0 not at all, 1 in reach, 2 lossy.
#[unsafe(no_mangle)]
pub extern "C" fn link(from: u32, to: u32) -> u32 {
    with_world(|world| match world.air.as_ref() {
        Some(air) => match air.paced.reach(from as usize, to as usize) {
            Reach::None => 0,
            Reach::InReach => 1,
            Reach::Lossy => 2,
        },
        None => 0,
    })
}

/// Steps how a packet from device `from` reaches device `to` through in reach, lossy and out
/// of reach.
#[unsafe(no_mangle)]
pub extern "C" fn cycle_link(from: u32, to: u32) {
    with_world(|world| {
        let (from, to) = (from as usize, to as usize);
        if let Some(air) = &world.air
            && from != to
            && from.max(to) < world.devices.len()
        {
            air.paced
                .set_reach(from, to, air.paced.reach(from, to).next());
        }
    });
}

/// What the selected device's node holds of its group, as text: its name, then its id, the
/// generation of the key it stored, its members and those it has heard, or that it has none.
/// It stays at [`text`] until the next call that fills it, [`text_len`] bytes long.
#[unsafe(no_mangle)]
pub extern "C" fn describe() {
    with_world(|world| {
        let n = world.selected;
        let device = &world.devices[n];
        let view = device.stage.mesh();
        let mut text = std::mem::take(&mut world.text);
        text.clear();
        let _ = writeln!(text, "{}", view.name);
        if device.powered_off {
            text.push_str("powered off");
        } else if let Some(group) = &view.group {
            let generation = world
                .air
                .as_ref()
                .and_then(|air| air.paced.sim.key(n))
                .map_or(0, |(_, generation)| generation);
            let members = group.members().count();
            let heard = group
                .members()
                .filter(|(_, member)| member.heard.is_some())
                .count();
            let _ = write!(
                text,
                "id {}, generation {generation}, {members} members, {heard} heard",
                group.own
            );
        } else {
            text.push_str("no group");
        }
        world.text = text;
    });
}

/// The log lines every node wrote since the last call, one a line: the device's number from
/// 1, the level, and the text, apart by tabs. Only the latest are kept. They stay at [`text`]
/// until the next call that fills it, [`text_len`] bytes long.
#[unsafe(no_mangle)]
pub extern "C" fn take_log() {
    with_world(|world| {
        let mut text = std::mem::take(&mut world.text);
        text.clear();
        if let Some(air) = &mut world.air {
            let (lines, next) = air.paced.sim.lines_since(air.taken);
            air.taken = next;
            for line in lines {
                let _ = writeln!(text, "{}\t{}\t{}", line.node + 1, line.level, line.text);
            }
        }
        world.text = text;
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn text() -> *const u8 {
    with_world(|world| world.text.as_ptr())
}

#[unsafe(no_mangle)]
pub extern "C" fn text_len() -> u32 {
    with_world(|world| world.text.len() as u32)
}

/// The frame as RGBA, `WIDTH` × `HEIGHT`, row by row. It stays at this address until the next
/// [`start`].
#[unsafe(no_mangle)]
pub extern "C" fn pixels() -> *const u8 {
    with(|device| device.rgba.as_ptr().cast())
}

#[unsafe(no_mangle)]
pub extern "C" fn size() -> u32 {
    WIDTH as u32
}

/// The display's level against the stored one, which the page shows by dimming the frame.
#[unsafe(no_mangle)]
pub extern "C" fn light() -> f32 {
    with(|device| {
        let stored = device.stage.peripherals().brightness.max(1);
        (f32::from(device.stage.shown_level()) / f32::from(stored)).min(1.0)
    })
}

/// Where the device is: bit 0 while it starts up, bits 1–2 its rest (awake, dimming, on the
/// always-on face, dark), bit 3 on the compass, bit 4 once powered off, and bits 5–8 what
/// shows over the faces: nothing (0), the settings panel (1), a screen it opened (2
/// brightness, 3 device, 4 clear, 5 zone picker, 6 replay, 7 timeout, 8 always on), or the
/// power-off confirmation (9).
#[unsafe(no_mangle)]
pub extern "C" fn status() -> u32 {
    with(|device| device.status())
}

/// What the last step flushed, as [`flushed_count`] runs of x, y, width and height on the
/// panel: the whole panel after a full redraw or a move, and otherwise the regions the display
/// core sends. It stays at this address until the next step.
#[unsafe(no_mangle)]
pub extern "C" fn flushed() -> *const i32 {
    with(|device| device.flushed.as_ptr().cast())
}

#[unsafe(no_mangle)]
pub extern "C" fn flushed_count() -> u32 {
    with(|device| device.flushed.len() as u32)
}

/// Pixel shift: the picture's position in the round in bits 0–3, plus 16 while it is pinned.
#[unsafe(no_mangle)]
pub extern "C" fn shift_state() -> u32 {
    with(|device| {
        let state = device.stage.shift_state();
        state.position() as u32 | u32::from(state.is_pinned()) << 4
    })
}

/// How far the picture shows moved on the panel, across and down. The start-up shows unmoved.
#[unsafe(no_mangle)]
pub extern "C" fn shift_x() -> i32 {
    with(|device| device.stage.shift().x)
}

#[unsafe(no_mangle)]
pub extern "C" fn shift_y() -> i32 {
    with(|device| device.stage.shift().y)
}

/// How long ago, in ms, the picture last moved.
#[unsafe(no_mangle)]
pub extern "C" fn shift_age() -> f64 {
    with(|device| {
        device
            .now
            .saturating_sub(device.stage.shift_state().moved_at()) as f64
            / 1_000.0
    })
}

/// Holds the picture at position `index` of the round, or with a negative `index` lets the
/// stage move it again.
#[unsafe(no_mangle)]
pub extern "C" fn pin_shift(index: i32) {
    with(|device| {
        let now = device.now;
        device.stage.pin_shift(usize::try_from(index).ok(), now);
    });
}

/// Sets a reading, by its place in [`Reading::ALL`].
#[unsafe(no_mangle)]
pub extern "C" fn set(reading: u32, value: f64) {
    with(|device| {
        let Some(&reading) = Reading::ALL.get(reading as usize) else {
            return;
        };
        device.readings.set(reading, value);
        match reading {
            Reading::Zone
            | Reading::Clock
            | Reading::Supply
            | Reading::Level
            | Reading::Fix
            | Reading::Receiver => {
                device.sensors_changed = true;
            }
            _ => device.readings_changed = true,
        }
    });
}

/// A reading, by its place in [`Reading::ALL`]. The stage changes some of them itself, as a
/// recalibration resets the calibration.
#[unsafe(no_mangle)]
pub extern "C" fn get(reading: u32) -> f64 {
    with(|device| {
        Reading::ALL
            .get(reading as usize)
            .map_or(f64::NAN, |&reading| device.readings.get(reading))
    })
}
