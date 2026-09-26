//! Scenes for `--play` and `--record`: the screens driven on a fixed timeline with synthetic
//! readings, so a recording comes out the same every run.

use embedded_graphics::prelude::Point;

use crate::caption::say;
use octowhere_ui::ui::{
    panel::Cell,
    rest::Timeout,
    screens::PeripheralState,
    stage::Stage,
    clock::{ClockState, DateTime, ZoneMode, ZoneState},
    compass::CompassView,
    gesture::Micros,
    screens::{Battery, Gnss, Screen},
    second::Store,
    script::Driver,
    stage::{Motion, Sensors},
    startup::{Outcome, Part},
};

pub struct Scene {
    pub name: &'static str,
    pub about: &'static str,
    pub run: fn(&mut Driver),
    /// Whether a recording adds a column for what the scene says with [`say`].
    pub captioned: bool,
}

pub const SCENES: &[Scene] = &[
    Scene {
        name: "startup",
        about: "the self-test as each part answers, the identity, the logo card and the clock",
        run: startup,
        captioned: false,
    },
    Scene {
        name: "startup-failed",
        about: "the self-test with the magnetometer failing, the fault screen and the clock",
        run: startup_failed,
        captioned: false,
    },
    Scene {
        name: "clock-entry",
        about: "the clock face building in, then ticking",
        run: clock_entry,
        captioned: false,
    },
    Scene {
        name: "swipe-to-compass",
        about: "from the clock to the compass, a turn, and back",
        run: swipe_to_compass,
        captioned: false,
    },
    Scene {
        name: "compass-calibration",
        about: "the compass calibrating, finding its heading, turning, and meeting interference",
        run: compass_calibration,
        captioned: false,
    },
    Scene {
        name: "compass-states",
        about: "each change between the compass's states",
        run: compass_states,
        captioned: false,
    },
    Scene {
        name: "settings",
        about: "the settings panel: opening, scrolling, brightness, timeout, always on, the zone picker, and closing",
        run: settings,
        captioned: false,
    },
    Scene {
        name: "rest-always-on",
        about: "a 15 s timeout: the dim, the always-on face, and a touch back to the clock",
        run: rest_always_on,
        captioned: false,
    },
    Scene {
        name: "rest-off",
        about: "a 15 s timeout on the compass: the dim, the panel off, and a touch back",
        run: rest_off,
        captioned: false,
    },
    Scene {
        name: "tour",
        about: "every screen and state, slowly: the start-up, the clock, the battery, the compass, settings, the always-on face and a demonstrated failure",
        run: tour,
        captioned: true,
    },
];

pub fn find(name: &str) -> &'static Scene {
    SCENES.iter().find(|scene| scene.name == name).unwrap_or_else(|| {
        let names: Vec<_> = SCENES.iter().map(|scene| scene.name).collect();
        panic!("no scene {name}; there are {}", names.join(", "))
    })
}

const fn ms(milliseconds: u64) -> Micros {
    milliseconds * 1_000
}

/// 13:07:42 on Thu 24 Sep 2026 in Europe/Dublin, found automatically, with GNSS having set the
/// clock.
fn dublin() -> Sensors {
    Sensors {
        clock: ClockState {
            utc: Some(
                DateTime { year: 2026, month: 9, day: 24, hour: 12, minute: 7, second: 42 }
                    .to_unix(),
            ),
            set_from_gnss: true,
            stopped: false,
        },
        zone: ZoneState {
            mode: ZoneMode::Automatic,
            zone: octowhere_ui::tz::DATABASE.find("Europe/Dublin").map(|zone| zone.id),
        },
        battery: Some(Battery { present: true, percent: 87, millivolts: 4020, charging: true, usb: true }),
        gnss: Gnss { fix: true, in_use: 9, in_view: 14, position: Some((533_498_000, -62_603_000)) },
    }
}

/// Calibrated and level, facing `degrees`.
fn facing(degrees: f32) -> Motion {
    Motion {
        compass: CompassView {
            live: true,
            calibration_percent: 100,
            heading_decidegrees: Some((degrees.rem_euclid(360.0) * 10.0).round() as u16 % 3600),
            ..CompassView::default()
        },
    }
}

fn ease(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

/// Starts on `screen` with the clock running in Dublin, facing 37°.
fn start(driver: &mut Driver, screen: Screen) {
    driver.stage.show(screen);
    driver.run_clock();
    driver.sensors(dublin());
    driver.motion(facing(37.0));
}

fn page_left(driver: &mut Driver) {
    driver.swipe(Point::new(420, 233), Point::new(60, 233), ms(250));
    driver.settle();
}

fn page_right(driver: &mut Driver) {
    driver.swipe(Point::new(60, 233), Point::new(420, 233), ms(250));
    driver.settle();
}

fn clock_entry(driver: &mut Driver) {
    start(driver, Screen::Clock);
    driver.wait(ms(3_000));
}

fn swipe_to_compass(driver: &mut Driver) {
    start(driver, Screen::Clock);
    driver.wait(ms(1_200));
    page_left(driver);
    driver.wait(ms(800));
    driver.motion_over(ms(700), |t| facing(37.0 + 50.0 * ease(t)));
    driver.wait(ms(800));
    page_right(driver);
    driver.wait(ms(1_000));
}

/// Live, calibrating, with no heading yet.
fn calibrating(percent: u8) -> Motion {
    Motion {
        compass: CompassView {
            live: true,
            calibration_percent: percent,
            heading_decidegrees: None,
            ..CompassView::default()
        },
    }
}

/// From calibration starting, through finding a heading and turning, to interference and back.
fn compass_walk(driver: &mut Driver) {
    driver.wait(ms(600));
    driver.motion_over(ms(1_500), |t| calibrating((t * 99.0) as u8));
    driver.wait(ms(300));
    driver.motion(facing(212.0));
    driver.wait(ms(1_000));
    driver.motion_over(ms(900), |t| facing(212.0 + 70.0 * ease(t)));
    driver.wait(ms(700));
    let mut disturbed = facing(282.0);
    disturbed.compass.disturbed = true;
    driver.motion(disturbed);
    driver.wait(ms(1_200));
    driver.motion(facing(282.0));
    driver.wait(ms(1_000));
}

fn compass_calibration(driver: &mut Driver) {
    driver.stage.show(Screen::Compass);
    driver.motion(calibrating(0));
    compass_walk(driver);
}

/// Calibrated, with the top edge raised too near vertical for a heading.
fn top_edge_up() -> Motion {
    Motion {
        compass: CompassView {
            live: true,
            calibration_percent: 100,
            heading_decidegrees: None,
            pitch_deg: 84,
            ..CompassView::default()
        },
    }
}

fn disturbed(degrees: f32) -> Motion {
    let mut motion = facing(degrees);
    motion.compass.disturbed = true;
    motion
}

/// A row of each kind in `SCREEN-DESIGN-BRIEF.md`'s table of the compass's changes of state.
fn compass_states(driver: &mut Driver) {
    driver.stage.show(Screen::Compass);
    driver.motion(calibrating(80));
    driver.wait(ms(600));
    driver.motion_over(ms(600), |t| calibrating(80 + (t * 19.0) as u8));
    // Calibration completes with the top edge raised, then the device is laid level.
    driver.motion(top_edge_up());
    driver.wait(ms(1_000));
    driver.motion(facing(120.0));
    driver.wait(ms(1_000));
    // Raised briefly, inside the grace, and then for longer than it.
    driver.motion(top_edge_up());
    driver.wait(ms(400));
    driver.motion(facing(120.0));
    driver.wait(ms(800));
    driver.motion(top_edge_up());
    driver.wait(ms(1_200));
    driver.motion(facing(120.0));
    driver.wait(ms(1_000));
    // Interference as the motion task's holds let it through: shown at least a second.
    driver.motion(disturbed(120.0));
    driver.wait(ms(1_200));
    driver.motion(facing(120.0));
    driver.wait(ms(800));
    driver.motion(Motion { compass: CompassView::default() });
    driver.wait(ms(1_000));
    // Calibration never falls on the device, but going through NO DATA shows both of its rows.
    driver.motion(calibrating(96));
    driver.wait(ms(1_000));
    driver.motion(facing(120.0));
    driver.wait(ms(1_200));
    driver.motion(Motion { compass: CompassView::default() });
    driver.wait(ms(1_000));
    driver.motion(facing(120.0));
    driver.wait(ms(1_200));
}

/// How long the tour holds a state for a viewer to read it.
const HOLD: Micros = ms(3_500);
/// How long the tour's swipes and drags take.
const SLOW: Micros = ms(700);

/// Dublin's readings at the driver's time, as the clock started by `boot` has run to.
fn dublin_now(driver: &Driver) -> Sensors {
    let mut sensors = dublin();
    sensors.clock.utc = sensors.clock.utc.map(|utc| utc + (driver.now() / 1_000_000) as i64);
    sensors
}

/// Says `caption`, steps in Dublin's readings at the driver's time changed by `change`, and
/// holds them.
fn show(driver: &mut Driver, caption: &'static str, change: impl FnOnce(&mut Sensors)) {
    say(caption);
    let mut sensors = dublin_now(driver);
    change(&mut sensors);
    driver.sensors(sensors);
    driver.wait(HOLD);
}

fn slow_page_left(driver: &mut Driver) {
    driver.swipe(Point::new(420, 233), Point::new(60, 233), SLOW);
    driver.settle();
}

fn slow_page_right(driver: &mut Driver) {
    driver.swipe(Point::new(60, 233), Point::new(420, 233), SLOW);
    driver.settle();
}

/// A tap, and a pause to see what it did.
fn slow_tap(driver: &mut Driver, x: i32, y: i32) {
    tap(driver, x, y);
    driver.wait(ms(1_500));
}

/// Taps a row of the settings panel, turning to its page first.
fn tap_row(driver: &mut Driver, cell: Cell) {
    if !octowhere_ui::ui::panel::in_view(cell.page(), driver.stage.panel_scroll()) {
        let (from, to) = if cell.page() == 1 { (380, 80) } else { (80, 380) };
        driver.swipe(Point::new(from, 250), Point::new(to, 250), SLOW);
        driver.settle();
        driver.wait(ms(1_200));
    }
    let middle = cell.bounds(driver.stage.panel_scroll()).center();
    slow_tap(driver, middle.x, middle.y);
}

fn open_settings(driver: &mut Driver) {
    driver.swipe(Point::new(233, 70), Point::new(233, 420), SLOW);
    driver.settle();
    driver.wait(ms(2_000));
}

fn close_settings(driver: &mut Driver) {
    driver.swipe(Point::new(233, 420), Point::new(233, 80), SLOW);
    driver.settle();
    driver.wait(ms(2_000));
}

/// Every screen and state, paced for a viewer who has not seen the device.
fn tour(driver: &mut Driver) {
    use Outcome::Answered;
    say("SELF-TEST. EACH PART OF THE BOARD IS TICKED OFF AS IT ANSWERS.");
    boot(driver, [
        (Part::Power, Answered, 150),
        (Part::Clock, Answered, 250),
        (Part::Touch, Answered, 500),
        (Part::Motion, Answered, 600),
        (Part::Magnet, Answered, 750),
        (Part::Gnss, Answered, 1_300),
    ]);
    driver.motion(facing(37.0));
    driver.wait(ms(1_700).saturating_sub(driver.now()));
    say("EVERY PART ANSWERED, SO THE IDENTITY AND THE LOGO CARD PLAY.");
    while driver.stage.starting_up() {
        driver.wait(ms(100));
    }
    say("THE CLOCK FACE. THE BLUE ICON MEANS GNSS SET THE TIME, AND THE ZONE CAME FROM THE POSITION.");
    driver.wait(ms(5_000));

    show(driver, "RTC. NO FIX SINCE START-UP, SO THE TIME IS THE BOARD'S OWN CLOCK.", |s| {
        s.clock.set_from_gnss = false;
    });
    show(driver, "MANUAL. A ZONE CHOSEN BY HAND IN SETTINGS, HERE NEW YORK.", |s| {
        s.zone = ZoneState {
            mode: ZoneMode::Manual,
            zone: octowhere_ui::tz::DATABASE.find("America/New_York").map(|zone| zone.id),
        };
    });
    show(driver, "STOPPED. THE CLOCK STOPPED SINCE IT WAS LAST SET, SO ITS TIME IS UNRELIABLE.", |s| {
        s.clock.stopped = true;
    });
    show(driver, "NO ZONE. NO FIX HAS PLACED THE DEVICE YET, SO THE FACE SHOWS UTC.", |s| {
        s.zone = ZoneState::default();
    });
    show(driver, "NO DATA. THE CLOCK COULD NOT BE READ.", |s| s.clock.utc = None);
    show(driver, "BACK TO A GNSS FIX.", |_| {});

    let battery = |percent, charging| Some(Battery { present: true, percent, millivolts: 3_900, charging, usb: charging });
    show(driver, "THE BATTERY, RIGHT OF THE MINUTES. HERE 64%, ON BATTERY ALONE.", |s| {
        s.battery = battery(64, false);
    });
    show(driver, "LOW BATTERY, AT 15% OR LESS.", |s| s.battery = battery(12, false));
    show(driver, "CHARGING. THE LEVEL CRAWLS UPWARD.", |s| s.battery = battery(12, true));
    show(driver, "NO BATTERY READING.", |s| s.battery = None);
    show(driver, "BACK ON USB, CHARGING.", |_| {});

    say("SWIPE LEFT FOR THE COMPASS.");
    slow_page_left(driver);
    say("THE HEADING. THE DIAL TURNS WITH THE DEVICE OVER A STILL BLUE FIELD.");
    driver.wait(ms(2_000));
    driver.motion_over(ms(2_000), |t| facing(37.0 + 90.0 * ease(t)));
    driver.wait(HOLD);
    say("CALIBRATING. TURN THE DEVICE EVERY WAY UNTIL THE COUNT FILLS.");
    driver.motion(calibrating(0));
    driver.wait(ms(1_500));
    driver.motion_over(ms(3_000), |t| calibrating((t * 99.0) as u8));
    driver.wait(ms(1_500));
    say("CALIBRATED. THE HEADING RETURNS.");
    driver.motion(facing(127.0));
    driver.wait(HOLD);
    say("INTERFERENCE. A MAGNETIC DISTURBANCE MAKES THE HEADING UNRELIABLE.");
    driver.motion(disturbed(127.0));
    driver.wait(HOLD);
    driver.motion(facing(127.0));
    driver.wait(ms(2_000));
    say("TOP EDGE UP. HELD NEAR VERTICAL, THE COMPASS HAS NO HEADING TO GIVE.");
    driver.motion(top_edge_up());
    driver.wait(HOLD);
    driver.motion(facing(127.0));
    driver.wait(ms(2_000));
    say("NO DATA. THE MOTION SENSORS STOPPED ANSWERING.");
    driver.motion(Motion { compass: CompassView::default() });
    driver.wait(HOLD);
    say("THEY ANSWER AGAIN.");
    driver.motion(facing(127.0));
    driver.wait(HOLD);
    say("SWIPE RIGHT FOR THE CLOCK.");
    slow_page_right(driver);
    driver.wait(ms(2_000));

    say("DRAG DOWN FROM EITHER FACE FOR SETTINGS.");
    open_settings(driver);
    say("BRIGHTNESS. THE PANEL FOLLOWS THE DRAG, AND A TAP KEEPS THE LEVEL.");
    tap_row(driver, Cell::Brightness);
    driver.swipe(Point::new(330, 280), Point::new(150, 280), ms(1_500));
    driver.wait(ms(1_500));
    slow_tap(driver, 233, 378);
    say("ZONE. DRAG TO AN OFFSET, TAP FOR ITS ZONES, AND TAP ONE TO SET IT BY HAND.");
    tap_row(driver, Cell::Zone);
    driver.swipe(Point::new(233, 300), Point::new(233, 255), SLOW);
    driver.wait(ms(1_500));
    slow_tap(driver, 233, 250);
    // The sensor task carries the stored zone from then on, as the firmware's does.
    let stored = driver.stroke(&[Point::new(233, 250)]).iter().find_map(|update| update.store);
    let Some(Store::ManualZone(zone)) = stored else { panic!("no zone was stored: {stored:?}") };
    let zone = ZoneState { mode: ZoneMode::Manual, zone: Some(zone) };
    let mut sensors = dublin_now(driver);
    sensors.zone = zone;
    driver.sensors(sensors);
    driver.wait(ms(1_500));
    say("THE CLOCK NOW SHOWS AMSTERDAM'S TIME, MARKED MANUAL.");
    close_settings(driver);
    driver.wait(HOLD);
    say("TIMEOUT, SET TO 15 SECONDS, AND ALWAYS ON, SWITCHED ON.");
    open_settings(driver);
    tap_row(driver, Cell::Timeout);
    driver.swipe(Point::new(233, 250), Point::new(233, 360), SLOW);
    driver.wait(ms(1_500));
    slow_tap(driver, 233, 258);
    tap_row(driver, Cell::AlwaysOn);
    close_settings(driver);

    say("AFTER 15 SECONDS UNTOUCHED THE SCREEN DIMS, THEN RESTS ON THE ALWAYS-ON FACE.");
    driver.wait(ms(21_000));
    say("THE ALWAYS-ON FACE: THE TIME AND THE BATTERY, REDRAWN ONCE A MINUTE.");
    driver.wait(HOLD);
    show(driver, "ALWAYS ON, STOPPED.", |s| {
        s.zone = zone;
        s.clock.stopped = true;
    });
    show(driver, "ALWAYS ON, NO ZONE, IN UTC.", |s| s.zone = ZoneState::default());
    show(driver, "ALWAYS ON, NO DATA.", |s| s.clock.utc = None);
    show(driver, "ALWAYS ON, LOCAL TIME AGAIN.", |s| s.zone = zone);
    say("A TOUCH WAKES THE SCREEN.");
    slow_tap(driver, 233, 300);
    driver.wait(ms(2_000));

    say("THE ZONE BACK TO AUTOMATIC, FROM THE GNSS POSITION.");
    open_settings(driver);
    tap_row(driver, Cell::Zone);
    tap(driver, 233, 342);
    let sensors = dublin_now(driver);
    driver.sensors(sensors);
    driver.wait(ms(1_500));
    close_settings(driver);
    driver.wait(HOLD);
    say("DEVICE. AT THE END OF ITS PAGE, REPLAY START-UP.");
    open_settings(driver);
    tap_row(driver, Cell::Device);
    driver.swipe(Point::new(233, 440), Point::new(233, 80), ms(1_200));
    driver.wait(ms(2_000));
    slow_tap(driver, 233, 298);
    say("A REPLAY CAN DEMONSTRATE A PART FAILING. HERE, THE MAGNETOMETER.");
    driver.swipe(Point::new(233, 330), Point::new(233, 330 - 40 * 5 - 10), ms(1_500));
    driver.wait(ms(2_000));
    tap(driver, 233, 258);
    say("THE SELF-TEST MARKS THE PART FAILED, AND THE FAULT SCREEN NAMES IT. BOTH SAY IT IS A DEMO.");
    while driver.stage.starting_up() {
        driver.wait(ms(100));
    }
    say("THEN THE CLOCK, AS AFTER ANY START-UP.");
    driver.wait(ms(4_000));
}

fn tap(driver: &mut Driver, x: i32, y: i32) {
    driver.stroke(&[Point::new(x, y)]);
}

fn settings(driver: &mut Driver) {
    driver.stage = Stage::new(PeripheralState { firmware: "0.1.0", ..PeripheralState::default() });
    start(driver, Screen::Clock);
    driver.motion(calibrating(54));
    driver.wait(ms(1_200));
    settings_walk(driver);
}

/// From the clock face into the settings panel, through brightness and the zone picker, and
/// back to the face.
fn settings_walk(driver: &mut Driver) {
    // Down from the face, across the two settings pages and back.
    driver.swipe(Point::new(233, 70), Point::new(233, 420), ms(300));
    driver.settle();
    driver.wait(ms(900));
    driver.swipe(Point::new(380, 250), Point::new(80, 250), ms(300));
    driver.settle();
    driver.wait(ms(800));
    driver.swipe(Point::new(80, 250), Point::new(380, 250), ms(300));
    driver.settle();
    driver.wait(ms(600));
    // Brightness to 80 %, kept.
    tap(driver, 150, 190);
    driver.wait(ms(700));
    driver.swipe(Point::new(220, 285), Point::new(333, 285), ms(500));
    driver.wait(ms(600));
    tap(driver, 233, 250);
    driver.wait(ms(700));
    // The timeout to 5 MIN, kept, then ALWAYS ON on.
    tap(driver, 300, 260);
    driver.wait(ms(700));
    driver.swipe(Point::new(233, 300), Point::new(233, 250), ms(300));
    driver.wait(ms(600));
    tap(driver, 233, 250);
    driver.wait(ms(700));
    tap(driver, 300, 330);
    driver.wait(ms(700));
    // The zone picker: one offset later, its zones, and back out.
    tap(driver, 150, 115);
    driver.wait(ms(800));
    driver.swipe(Point::new(233, 300), Point::new(233, 255), ms(300));
    driver.wait(ms(600));
    tap(driver, 233, 250);
    driver.wait(ms(900));
    tap(driver, 120, 120);
    driver.wait(ms(500));
    tap(driver, 120, 120);
    driver.wait(ms(600));
    // Up to close, back to the clock.
    driver.swipe(Point::new(233, 420), Point::new(233, 80), ms(300));
    driver.settle();
    driver.wait(ms(1_000));
}

/// Boots with the clock running in Dublin, each part reporting at `at` ms from power-on.
fn boot(driver: &mut Driver, reports: [(Part, Outcome, u64); 6]) {
    driver.stage = Stage::starting(PeripheralState { firmware: "0.1.0", ..PeripheralState::default() });
    driver.run_clock();
    driver.sensors(dublin());
    for (part, outcome, at) in reports {
        driver.wait(ms(at).saturating_sub(driver.now()));
        driver.boot(part, outcome);
    }
}

fn startup(driver: &mut Driver) {
    use Outcome::Answered;
    boot(driver, [
        (Part::Power, Answered, 150),
        (Part::Clock, Answered, 250),
        (Part::Touch, Answered, 500),
        (Part::Motion, Answered, 600),
        (Part::Magnet, Answered, 750),
        (Part::Gnss, Answered, 1_300),
    ]);
    driver.wait(ms(5_900));
}

fn startup_failed(driver: &mut Driver) {
    use Outcome::{Answered, NoReply};
    boot(driver, [
        (Part::Power, Answered, 150),
        (Part::Clock, Answered, 250),
        (Part::Touch, Answered, 500),
        (Part::Motion, Answered, 600),
        (Part::Magnet, NoReply, 1_000),
        (Part::Gnss, Answered, 1_300),
    ]);
    driver.wait(ms(5_200));
}

/// Settles on `screen` with a 15 s timeout, and waits out the timeout and the dim.
fn rest(driver: &mut Driver, screen: Screen, always_on: bool) {
    driver.stage = Stage::new(PeripheralState {
        firmware: "0.1.0",
        timeout: Timeout::Seconds15,
        always_on,
        ..PeripheralState::default()
    });
    start(driver, screen);
    driver.wait(ms(22_000));
    tap(driver, 233, 300);
    driver.wait(ms(2_000));
}

fn rest_always_on(driver: &mut Driver) {
    rest(driver, Screen::Clock, true);
}

fn rest_off(driver: &mut Driver) {
    rest(driver, Screen::Compass, false);
}
