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
        stage::{Input, Motion, Sensors, Stage},
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
        heading_decidegrees: Some(372),
        pitch_deg: 4,
        roll_deg: -7,
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
                live: true,
                calibration_percent: 40,
                ..CompassView::default()
            },
        ),
        (
            "disturbed",
            CompassView {
                disturbed: true,
                ..calibrated
            },
        ),
    ] {
        frames.push((format!("compass-{name}"), stage(Screen::Compass, compass)));
    }

    for (name, stage) in frames {
        let mut fb = FB::boxed();
        stage.draw(&mut *fb);
        let path = out.join(format!("{name}.png"));
        write_png(&fb, &path);
        println!("{}", path.display());
    }
}

fn stage(screen: Screen, compass: CompassView) -> Stage {
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
