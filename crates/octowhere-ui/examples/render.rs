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
            "heading-359",
            CompassView {
                heading_decidegrees: Some(3599),
                ..calibrated
            },
        ),
    ] {
        frames.push((format!("compass-{name}"), stage(Screen::Compass, compass)));
    }

    // Partway through the entry fades: the ring is in, the icon arriving, the dial not yet.
    let mut entering = stage_at(Screen::Compass, calibrated, 150_000);
    entering.step(Input {
        now: 150_001,
        ..Input::default()
    });
    frames.push(("compass-entering".into(), entering));
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

/// A stage on `screen` that settled at 1 µs and has stepped to `now`.
fn stage_at(screen: Screen, compass: CompassView, now: u64) -> Stage {
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
        sensors: Some(Sensors {
            battery_mv: Some(3_912),
            vbus_mv: Some(5_020),
            vsys_mv: Some(3_890),
            gnss_bytes: 512,
            gnss_fix: true,
            lora_irq: 0,
            clock: ClockState {
                hours: 14,
                minutes: 7,
                seconds: 32,
                day: 24,
                month: 9,
                year: 26,
                valid: true,
            },
        }),
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
