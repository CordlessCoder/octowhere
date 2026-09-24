//! Scenes for `--play` and `--record`: the screens driven on a fixed timeline with synthetic
//! readings, so a recording comes out the same every run.

use embedded_graphics::prelude::Point;
use octowhere_ui::ui::{
    screens::PeripheralState,
    stage::Stage,
    clock::{ClockState, DateTime, ZoneMode, ZoneState},
    compass::CompassView,
    gesture::Micros,
    screens::{Battery, Gnss, Screen},
    script::Driver,
    stage::{Motion, Sensors},
};

pub struct Scene {
    pub name: &'static str,
    pub about: &'static str,
    pub run: fn(&mut Driver),
}

pub const SCENES: &[Scene] = &[
    Scene {
        name: "clock-entry",
        about: "the clock face building in, then ticking",
        run: clock_entry,
    },
    Scene {
        name: "swipe-to-compass",
        about: "from the clock to the compass, a turn, and back",
        run: swipe_to_compass,
    },
    Scene {
        name: "compass-calibration",
        about: "the compass calibrating, finding its heading, turning, and meeting interference",
        run: compass_calibration,
    },
    Scene {
        name: "settings",
        about: "the settings panel: opening, scrolling, brightness, the zone picker, and closing",
        run: settings,
    },
    Scene {
        name: "tour",
        about: "the clock finding a fix, the compass calibration walk, and back",
        run: tour,
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

fn tour(driver: &mut Driver) {
    // Before a fix: the clock's own time, unconfirmed, and no zone, so the face shows UTC.
    let mut before = dublin();
    before.clock.set_from_gnss = false;
    before.clock.utc = before.clock.utc.map(|utc| utc - 3);
    before.zone = ZoneState::default();
    driver.stage.show(Screen::Clock);
    driver.run_clock();
    driver.sensors(before);
    driver.motion(calibrating(0));
    driver.wait(ms(3_000));

    // The fix confirms the time where the clock has run to, and places the device in Dublin.
    let mut fix = dublin();
    fix.clock.utc = driver.stage.peripherals().clock.clock.utc;
    driver.sensors(fix);
    driver.wait(ms(1_800));

    page_left(driver);
    compass_walk(driver);
    page_right(driver);
    driver.wait(ms(1_500));
}

fn tap(driver: &mut Driver, x: i32, y: i32) {
    driver.stroke(&[Point::new(x, y)]);
}

fn settings(driver: &mut Driver) {
    driver.stage = Stage::new(PeripheralState { firmware: "0.1.0", ..PeripheralState::default() });
    start(driver, Screen::Clock);
    driver.motion(calibrating(54));
    driver.wait(ms(1_200));
    // Down from the face, then along the grid and back.
    driver.swipe(Point::new(233, 70), Point::new(233, 420), ms(300));
    driver.settle();
    driver.wait(ms(900));
    driver.swipe(Point::new(380, 250), Point::new(200, 250), ms(300));
    driver.settle();
    driver.wait(ms(800));
    driver.swipe(Point::new(200, 250), Point::new(380, 250), ms(300));
    driver.settle();
    driver.wait(ms(600));
    // Brightness to 80 %, kept.
    tap(driver, 150, 300);
    driver.wait(ms(700));
    driver.swipe(Point::new(220, 285), Point::new(333, 285), ms(500));
    driver.wait(ms(600));
    tap(driver, 233, 250);
    driver.wait(ms(700));
    // The zone picker: one offset later, its zones, and back out.
    tap(driver, 150, 150);
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
