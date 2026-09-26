//! Renders the clock and the compass in each of their states, to 466×466 PNGs through the
//! firmware's own drawing code. From the repository root:
//!
//! ```text
//! cargo +stable run --manifest-path crates/octowhere-ui/Cargo.toml \
//!   --target x86_64-unknown-linux-gnu --example render -- [out-dir]
//! ```
//!
//! The output directory defaults to `target/renders` under the crate. The readings are
//! synthetic fixtures, chosen to exercise the drawing rather than to look like a real fix.

use std::{fs::File, io::BufWriter, path::PathBuf};

use embedded_graphics::{
    pixelcolor::{Rgb888, RgbColor},
    prelude::Point,
};
use octowhere_ui::{
    board::{LCD_HEIGHT, LCD_WIDTH},
    chrome::FB,
    ui::{
        clock::{ClockState, DateTime, ZoneMode, ZoneState},
        compass::CompassView,
        rest::{Rest, Timeout},
        screens::{Battery, Gnss, PeripheralState, Screen},
        script::Driver,
        stage::{Input, Motion, Sensors, Stage, Touch},
        startup::{Outcome, Part, Report},
    },
};

fn main() {
    let out = std::env::args_os().nth(1).map_or_else(
        || PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/renders"),
        PathBuf::from,
    );
    std::fs::create_dir_all(&out).expect("creating the output directory");

    let calibrated = CompassView {
        live: true,
        calibration_percent: 100,
        heading_decidegrees: Some(470),
        pitch_deg: 5,
        roll_deg: -12,
        disturbed: false,
    };
    let mut frames: Vec<(String, Stage)> = Screen::ALL
        .iter()
        .map(|&screen| {
            let name = match screen {
                Screen::Clock => "clock",
                Screen::Compass => "compass-heading",
            };
            (name.into(), stage(screen, calibrated))
        })
        .collect();
    for (name, compass) in [
        ("no-data", CompassView::default()),
        (
            "calibrating",
            CompassView {
                heading_decidegrees: None,
                calibration_percent: 54,
                ..calibrated
            },
        ),
        (
            "top-edge-up",
            CompassView {
                heading_decidegrees: None,
                ..calibrated
            },
        ),
        (
            "interference",
            CompassView {
                disturbed: true,
                ..calibrated
            },
        ),
        (
            "interference-350",
            CompassView {
                disturbed: true,
                heading_decidegrees: Some(3500),
                ..calibrated
            },
        ),
        (
            "heading-359",
            CompassView {
                heading_decidegrees: Some(3599),
                ..calibrated
            },
        ),
    ] {
        frames.push((format!("compass-{name}"), stage(Screen::Compass, compass)));
    }

    let fixture = sensors();
    for (name, clock, zone) in [
        ("rtc", ClockState { set_from_gnss: false, ..fixture.clock }, fixture.zone),
        ("manual", fixture.clock, ZoneState { mode: ZoneMode::Manual, ..fixture.zone }),
        ("stopped", ClockState { stopped: true, ..fixture.clock }, fixture.zone),
        ("no-zone", fixture.clock, ZoneState { zone: None, ..fixture.zone }),
        ("no-data", ClockState { utc: None, ..fixture.clock }, fixture.zone),
        (
            "longest-zone",
            fixture.clock,
            ZoneState {
                mode: ZoneMode::Manual,
                zone: octowhere_ui::tz::DATABASE
                    .find("America/Argentina/Buenos_Aires")
                    .map(|zone| zone.id),
            },
        ),
    ] {
        // A trusted reading first, as the device would have had before a fault, then this one.
        let mut stage = stage_with(Screen::Clock, calibrated, fixture, 1_000_000);
        for now in [1_000_001, 2_000_000] {
            stage.step(Input {
                now,
                sensors: Some(Sensors { clock, zone, ..fixture }),
                ..Input::default()
            });
        }
        frames.push((format!("clock-{name}"), stage));
    }
    let charging = Battery { present: true, percent: 40, millivolts: 3900, charging: true, usb: true };
    for (name, battery) in [
        ("charging", Some(charging)),
        ("battery-low", Some(Battery { percent: 12, charging: false, usb: false, ..charging })),
        ("battery-unknown", None),
    ] {
        let stage = stage_with(Screen::Clock, calibrated, Sensors { battery, ..fixture }, 1_000_000);
        frames.push((format!("clock-{name}"), stage));
    }
    // 200 ms into the clock's entry: the icon building, the first line typing, the battery
    // hatch past its level.
    let mut entering = stage_at(Screen::Clock, calibrated, 200_000);
    entering.step(Input { now: 200_001, ..Input::default() });
    frames.push(("clock-entering".into(), entering));
    // 380 ms in: the wordmark holding, its first block standing, before it snaps in.
    let mut marking = stage_at(Screen::Clock, calibrated, 380_000);
    marking.step(Input { now: 380_001, ..Input::default() });
    frames.push(("clock-marking".into(), marking));
    let mut swiping = stage(Screen::Clock, calibrated);
    for (step, x) in [400, 340, 280].into_iter().enumerate() {
        swiping.step(Input {
            now: 1_000_000 + step as u64 * 16_667,
            touch: Some(Touch::Contacts([Some(Point::new(x, 233)), None])),
            ..Input::default()
        });
    }
    frames.push(("clock-swiping".into(), swiping));

    // Partway through the entry fades: the ring is in, the icon arriving, the dial not yet.
    let mut entering = stage_at(Screen::Compass, calibrated, 150_000);
    entering.step(Input {
        now: 150_001,
        ..Input::default()
    });
    frames.push(("compass-entering".into(), entering));
    // Halfway through the dial's sweep.
    let mut sweeping = stage_at(Screen::Compass, calibrated, 275_000);
    sweeping.step(Input {
        now: 275_001,
        ..Input::default()
    });
    frames.push(("compass-sweeping".into(), sweeping));
    // Dragged a third of the way to the next page, where the accents have nearly faded.
    let mut swiping = stage(Screen::Compass, calibrated);
    for (step, x) in [400, 340, 280, 245].into_iter().enumerate() {
        swiping.step(Input {
            now: 1_000_000 + step as u64 * 16_667,
            touch: Some(Touch::Contacts([Some(Point::new(x, 233)), None])),
            ..Input::default()
        });
    }
    frames.push(("compass-swiping".into(), swiping));

    frames.extend(settings_frames());
    frames.extend(startup_frames());
    frames.extend(always_on_frames());

    for (name, stage) in frames {
        let mut fb = FB::boxed();
        stage.draw(&mut *fb);
        let path = out.join(format!("{name}.png"));
        write_png(&fb, &path);
        println!("{}", path.display());
    }
}

/// The settings panel and the screens it opens, reached by the gestures that reach them.
fn settings_frames() -> Vec<(String, Stage)> {
    let calibrating = CompassView {
        live: true,
        calibration_percent: 54,
        heading_decidegrees: None,
        pitch_deg: 5,
        roll_deg: -12,
        disturbed: false,
    };
    let start = || {
        let mut driver = Driver::on(Screen::Clock);
        driver.stage = Stage::new(PeripheralState { firmware: "0.1.0", ..PeripheralState::default() });
        driver.stage.show(Screen::Clock);
        driver.sensors(sensors());
        driver.motion(Motion { compass: calibrating });
        driver.wait(600_000);
        driver
    };
    let open = || {
        let mut driver = start();
        driver.swipe(Point::new(233, 80), Point::new(233, 420), 250_000);
        driver.settle();
        driver.wait(600_000);
        driver
    };
    let tap = |driver: &mut Driver, x, y| {
        driver.stroke(&[Point::new(x, y)]);
        driver.wait(400_000);
    };
    // DEVICE is on the second settings page.
    let open_device = |driver: &mut Driver| {
        driver.swipe(Point::new(380, 250), Point::new(80, 250), 300_000);
        driver.settle();
        tap(driver, 300, 330);
    };
    let mut frames = Vec::new();

    let mut pulling = start();
    for y in [80, 120, 180, 240, 280] {
        pulling.touch(Some(Point::new(233, y)));
    }
    frames.push(("panel-pulling".into(), pulling.stage));
    frames.push(("panel-rest".into(), open().stage));
    let mut always_on = open();
    tap(&mut always_on, 300, 330);
    frames.push(("panel-always-on".into(), always_on.stage));
    let mut end = open();
    end.swipe(Point::new(380, 250), Point::new(80, 250), 300_000);
    end.settle();
    end.wait(400_000);
    frames.push(("panel-end".into(), end.stage));
    let mut scrolling = open();
    for x in [380, 360, 330, 320] {
        scrolling.touch(Some(Point::new(x, 250)));
    }
    frames.push(("panel-scrolling".into(), scrolling.stage));

    let mut brightness = open();
    tap(&mut brightness, 150, 190);
    frames.push(("settings-brightness".into(), brightness.stage));
    let mut timeout = open();
    tap(&mut timeout, 300, 260);
    frames.push(("settings-timeout".into(), timeout.stage));
    let mut device = open();
    open_device(&mut device);
    frames.push(("panel-device".into(), device.stage));
    let mut device_end = open();
    open_device(&mut device_end);
    device_end.swipe(Point::new(233, 440), Point::new(233, 80), 300_000);
    frames.push(("panel-device-end".into(), device_end.stage));
    let mut clear = open();
    open_device(&mut clear);
    clear.swipe(Point::new(233, 440), Point::new(233, 80), 300_000);
    tap(&mut clear, 233, 342);
    frames.push(("settings-clear".into(), clear.stage));
    let chooser = |steps: i32| {
        let mut driver = open();
        open_device(&mut driver);
        driver.swipe(Point::new(233, 440), Point::new(233, 80), 300_000);
        tap(&mut driver, 233, 298);
        if steps > 0 {
            driver.swipe(Point::new(233, 330), Point::new(233, 330 - 40 * steps - 10), 300_000);
            driver.wait(400_000);
        }
        driver
    };
    frames.push(("replay-chooser".into(), chooser(0).stage));
    frames.push(("replay-chooser-magnet".into(), chooser(5).stage));
    let mut demo = chooser(4);
    tap(&mut demo, 233, 258);
    demo.wait(1_100_000);
    frames.push(("replay-demo-selftest".into(), demo.stage));
    let mut demo = chooser(4);
    tap(&mut demo, 233, 258);
    demo.wait(2_000_000);
    frames.push(("replay-demo-fault".into(), demo.stage));
    let mut picker = open();
    tap(&mut picker, 150, 115);
    frames.push(("picker-offset".into(), picker.stage));
    let mut zone = open();
    tap(&mut zone, 150, 115);
    tap(&mut zone, 233, 250);
    frames.push(("picker-zone".into(), zone.stage));
    frames
}

/// The start-up sequence: the self-test as parts answer, the identity and the card by frame, and
/// the fault screen. Every part reports a tenth of a second after the last.
fn startup_frames() -> Vec<(String, Stage)> {
    let boot = |failing: Option<Part>, parts: usize, now: u64| {
        let mut stage = Stage::starting(PeripheralState { firmware: "0.1.0", ..PeripheralState::default() });
        stage.step(Input { now: 1, sensors: Some(sensors()), ..Input::default() });
        for (i, part) in Part::ALL.into_iter().take(parts).enumerate() {
            let outcome = if failing == Some(part) { Outcome::NoReply } else { Outcome::Answered };
            stage.step(Input { now: 100_000 * (i as u64 + 1), boot: Some(Report { part, outcome }), ..Input::default() });
        }
        stage.step(Input { now, ..Input::default() });
        stage
    };
    // The last glyph lands at 720 ms, and the hold ends 200 ms later.
    let frame = |n: u64| 920_000 + (n * 1_000_000).div_ceil(30);
    let mut frames = vec![
        ("startup-selftest".to_string(), boot(None, 0, 50_000)),
        ("startup-selftest-building".into(), boot(None, 3, 345_000)),
        ("startup-selftest-passed".into(), boot(None, 6, 850_000)),
        ("startup-selftest-failing".into(), boot(Some(Part::Magnet), 5, 850_000)),
        ("startup-selftest-failed".into(), boot(Some(Part::Magnet), 6, 850_000)),
    ];
    // The identity's frames 0–119, then the card's 120–138, then the clock.
    for n in [0, 3, 6, 12, 13, 20, 24, 30, 38, 42, 45, 51, 53, 56, 66, 77, 119, 120, 123, 129, 133, 137, 138, 139] {
        frames.push((format!("startup-frame-{n:02}"), boot(None, 6, frame(n))));
    }
    // With a failure the hold is 300 ms, from 720 ms.
    for n in [0, 30] {
        frames.push((format!("startup-fault-{n:02}"), boot(Some(Part::Magnet), 6, frame(n) + 100_000)));
    }
    frames
}

/// The always-on face in each of the clock's states, reached by waiting out a 15 s timeout and
/// the dim after it.
fn always_on_frames() -> Vec<(String, Stage)> {
    let fixture = sensors();
    [
        ("local", fixture.clock, fixture.zone),
        ("stopped", ClockState { stopped: true, ..fixture.clock }, fixture.zone),
        ("no-zone", fixture.clock, ZoneState { zone: None, ..fixture.zone }),
        ("no-data", ClockState { utc: None, ..fixture.clock }, fixture.zone),
    ]
    .into_iter()
    .map(|(name, clock, zone)| {
        let mut stage = Stage::new(PeripheralState {
            firmware: "0.1.0",
            timeout: Timeout::Seconds15,
            always_on: true,
            ..PeripheralState::default()
        });
        stage.show(Screen::Clock);
        stage.step(Input { now: 1, sensors: Some(fixture), ..Input::default() });
        stage.step(Input { now: 2, sensors: Some(Sensors { clock, zone, ..fixture }), ..Input::default() });
        for now in [16_000_000, 21_000_000] {
            stage.step(Input { now, ..Input::default() });
        }
        assert_eq!(stage.rest(), Rest::AlwaysOn);
        (format!("always-on-{name}"), stage)
    })
    .collect()
}

fn stage(screen: Screen, compass: CompassView) -> Stage {
    stage_at(screen, compass, 1_000_000)
}

fn stage_at(screen: Screen, compass: CompassView, now: u64) -> Stage {
    stage_with(screen, compass, sensors(), now)
}

/// 13:07:42 on Thu 24 Sep 2026 in Europe/Dublin, found automatically, with GNSS having set the
/// clock.
fn sensors() -> Sensors {
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
        battery: Some(Battery { present: true, percent: 87, millivolts: 4020, charging: false, usb: false }),
        gnss: Gnss { fix: true, in_use: 9, in_view: 14, position: Some((533_498_000, -62_603_000)) },
    }
}

/// A stage on `screen` that settled at 1 µs and has stepped to `now`.
fn stage_with(screen: Screen, compass: CompassView, sensors: Sensors, now: u64) -> Stage {
    let mut stage = Stage::new(PeripheralState { firmware: "0.1.0", ..PeripheralState::default() });
    stage.show(screen);
    stage.step(Input {
        now: 1,
        motion: Some(Motion { compass }),
        sensors: Some(sensors),
        ..Input::default()
    });
    stage.step(Input {
        now,
        ..Input::default()
    });
    stage
}

fn write_png(fb: &FB, path: &std::path::Path) {
    let (width, height) = (u32::from(LCD_WIDTH), u32::from(LCD_HEIGHT));
    let pixels: Vec<u8> = (0..height as i32)
        .flat_map(|y| (0..width as i32).map(move |x| Point::new(x, y)))
        .flat_map(|point| {
            let color = Rgb888::from(fb.pixel(point).expect("inside the panel"));
            [color.r(), color.g(), color.b()]
        })
        .collect();
    let file = BufWriter::new(File::create(path).expect("creating the PNG"));
    let mut encoder = png::Encoder::new(file, width, height);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().expect("writing the PNG header");
    writer.write_image_data(&pixels).expect("writing the PNG");
}
