//! Renders every screen, and the compass in each of its states, to 466×466 PNGs through the
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
        clock::{DateTime, ZoneMode, ZoneState},
        compass::CompassView,
        prototypes::{ClockState, PeripheralState, Screen},
        stage::{Input, Motion, Sensors, Stage, Touch},
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
        .map(|&screen| (format!("{screen:?}").to_lowercase(), stage(screen, calibrated)))
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
        let sensors = Sensors { clock, zone, ..fixture };
        frames.push((format!("clock-{name}"), stage_with(Screen::Clock, calibrated, sensors, 1_000_000)));
    }
    // 200 ms into the clock's entry: the icon half built, the plate landing.
    let mut entering = stage_at(Screen::Clock, calibrated, 200_000);
    entering.step(Input { now: 200_001, ..Input::default() });
    frames.push(("clock-entering".into(), entering));
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

    for (name, stage) in frames {
        let mut fb = FB::boxed();
        stage.draw(&mut *fb);
        let path = out.join(format!("{name}.png"));
        write_png(&fb, &path);
        println!("{}", path.display());
    }
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
        battery_mv: Some(3_912),
        vbus_mv: Some(5_020),
        vsys_mv: Some(3_890),
        gnss_bytes: 512,
        gnss_fix: true,
        lora_irq: 0,
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
    }
}

/// A stage on `screen` that settled at 1 µs and has stepped to `now`.
fn stage_with(screen: Screen, compass: CompassView, sensors: Sensors, now: u64) -> Stage {
    let mut stage = Stage::new(PeripheralState {
        pmic_valid: true,
        tca_valid: true,
        lora_valid: true,
        ..PeripheralState::default()
    });
    stage.show(screen);
    stage.step(Input {
        now: 1,
        motion: Some(Motion {
            accel_micro_ms2: [120_000, -340_000, 9_790_000],
            gyro_micro_rad_s: [1_200, -800, 300],
            imu_valid: true,
            magnetic_microtesla: [21_400, -3_100, -38_900],
            compass,
        }),
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
