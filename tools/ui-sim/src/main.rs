//! A desktop window running the firmware's UI stage. The mouse is the touchscreen, and the
//! keyboard sets the compass readings the motion task would publish. From the repository root:
//!
//! ```text
//! cargo +stable run --release --manifest-path tools/ui-sim/Cargo.toml -- [--scale 2]
//! ```
//!
//! Keys:
//!
//! - Left / Right: heading down / up 5°, 1° with Shift. Space spins it.
//! - Up / Down: pitch; Q / E: roll, 5° a press, 1° with Shift.
//! - C: calibration through none, part-way and complete. T: top edge vertical or not, which
//!   withholds the heading.
//! - D: magnetic disturbance on or off. L: sensors live or silent.
//! - Z: the clock's zone, through unknown, automatic in Dublin, and chosen by hand in New York
//!   and Kolkata.
//! - Hold H: a hand covering the screen, which restarts calibration on the settled compass.
//! - Tab: next screen without the slide. P: save the window to `ui-sim-<n>.png` in the current
//!   directory. Esc: quit.
//!
//! Readings are synthetic. Drawing times in the title are the host's and say nothing about the
//! target.

use std::{
    fs::File,
    io::BufWriter,
    path::Path,
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use embedded_graphics::{
    pixelcolor::{Rgb888, RgbColor},
    prelude::Point,
};
use minifb::{Key, KeyRepeat, MouseButton, MouseMode, Scale, Window, WindowOptions};
use octowhere_ui::{
    board::{LCD_HEIGHT, LCD_WIDTH},
    chrome::{Clip, FB},
    ui::{
        clock::{ClockState, ZoneMode, ZoneState},
        compass::CompassView,
        prototypes::PeripheralState,
        stage::{Input, Motion, Sensors, Stage, Touch},
    },
};

const WIDTH: usize = LCD_WIDTH as usize;
const HEIGHT: usize = LCD_HEIGHT as usize;
/// The motion task's sample periods, fast while a compass screen shows.
const FAST_SAMPLE_US: u64 = 20_000;
const SLOW_SAMPLE_US: u64 = 250_000;
const SENSOR_PERIOD_US: u64 = 1_000_000;
/// What the panel's round glass hides is drawn in this, so the corners read as off-panel.
const OFF_PANEL: u32 = 0x1c1c1c;

/// The readings the keyboard controls.
struct Readings {
    heading: f32,
    pitch: i32,
    roll: i32,
    calibration: u8,
    disturbed: bool,
    live: bool,
    spinning: bool,
    vertical: bool,
    /// Which of [`ZONES`] the clock shows.
    zone: usize,
}

/// The zones Z steps through, and whether each was chosen by hand. `None` is automatic mode
/// before any fix.
const ZONES: [Option<(&str, ZoneMode)>; 4] = [
    None,
    Some(("Europe/Dublin", ZoneMode::Automatic)),
    Some(("America/New_York", ZoneMode::Manual)),
    Some(("Asia/Kolkata", ZoneMode::Manual)),
];

impl Readings {
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
            Key::Z => self.zone = (self.zone + 1) % ZONES.len(),
            _ => return false,
        }
        true
    }

    fn zone_state(&self) -> ZoneState {
        ZONES[self.zone].map_or(ZoneState::default(), |(name, mode)| ZoneState {
            mode,
            zone: octowhere_ui::tz::DATABASE.find(name).map(|zone| zone.id),
        })
    }

    fn motion(&self) -> Motion {
        Motion {
            imu_valid: self.live,
            compass: self.compass(),
            ..Motion::default()
        }
    }
}

fn main() {
    let scale = match std::env::args().skip_while(|arg| arg != "--scale").nth(1).as_deref() {
        None | Some("1") => Scale::X1,
        Some("2") => Scale::X2,
        Some("4") => Scale::X4,
        Some(other) => panic!("--scale takes 1, 2 or 4, not {other}"),
    };
    let mut window = Window::new(
        "octowhere",
        WIDTH,
        HEIGHT,
        WindowOptions {
            scale,
            ..WindowOptions::default()
        },
    )
    .expect("opening the window");
    window.set_target_fps(60);

    let mut stage = Stage::new(PeripheralState {
        pmic_valid: true,
        tca_valid: true,
        lora_valid: true,
        ..PeripheralState::default()
    });
    let mut readings = Readings {
        heading: 37.0,
        pitch: 0,
        roll: 0,
        calibration: 100,
        disturbed: false,
        live: true,
        spinning: false,
        vertical: false,
        zone: 1,
    };
    let mut fb = FB::boxed();
    let mut pixels = vec![0u32; WIDTH * HEIGHT];
    let mask = panel_mask();
    let start = Instant::now();
    let (mut next_motion, mut next_sensors) = (0, 0);
    let mut samples_fast = false;
    let mut readings_changed = true;
    let mut redraw = true;
    let mut screenshots = 0;
    let mut zone_changed = false;

    while window.is_open() && !window.is_key_down(Key::Escape) {
        let now = start.elapsed().as_micros() as u64 + 1;
        let shift = window.is_key_down(Key::LeftShift) || window.is_key_down(Key::RightShift);
        for key in window.get_keys_pressed(KeyRepeat::Yes) {
            match key {
                Key::Tab => {
                    stage.show(stage.screen().next());
                    redraw = true;
                }
                Key::P => {
                    screenshots += 1;
                    let path = format!("ui-sim-{screenshots}.png");
                    write_png(&pixels, Path::new(&path));
                    println!("saved {path}");
                }
                key => {
                    let zone = readings.zone;
                    readings_changed |= readings.press(key, shift);
                    zone_changed |= readings.zone != zone;
                }
            }
        }
        if readings.spinning {
            readings.heading += 0.5;
            readings_changed = true;
        }

        let motion_due = now >= next_motion || readings_changed;
        if motion_due {
            next_motion = now + if samples_fast { FAST_SAMPLE_US } else { SLOW_SAMPLE_US };
            readings_changed = false;
        }
        let sensors_due = now >= next_sensors || zone_changed;
        if sensors_due {
            next_sensors = now + SENSOR_PERIOD_US;
            zone_changed = false;
        }
        let contact = window
            .get_mouse_pos(MouseMode::Discard)
            .filter(|_| window.get_mouse_down(MouseButton::Left))
            .map(|(x, y)| Point::new(x as i32, y as i32));
        let update = stage.step(Input {
            now,
            touch: Some(if window.is_key_down(Key::H) {
                Touch::Cover
            } else {
                Touch::Contacts([contact, None])
            }),
            motion: motion_due.then(|| readings.motion()),
            sensors: sensors_due.then(|| sensors(readings.zone_state())),
        });
        samples_fast = update.samples_fast;
        if update.recalibrate {
            readings.calibration = 0;
            readings_changed = true;
        }
        if let Some(record) = update.pose {
            println!("pose {} logged from {} samples", record.pose + 1, record.samples);
        }

        let changed = stage.changed();
        if redraw || !changed.is_empty() {
            // One buffer, so each step repaints only its own damage, as the firmware's buffers
            // would with the step before added.
            let drawing = Instant::now();
            let pixels_drawn = if redraw || changed.is_full() {
                stage.draw(&mut *fb);
                LCD_WIDTH as u32 * LCD_HEIGHT as u32
            } else {
                stage.draw(&mut Clip::new(&mut *fb, changed));
                changed.pixels()
            };
            let took = drawing.elapsed();
            window.set_title(&format!(
                "octowhere: {:?}, {} px drawn in {:.2} ms on the host",
                stage.screen(),
                pixels_drawn,
                took.as_secs_f64() * 1e3
            ));
            to_pixels(&fb, &mask, &mut pixels);
            redraw = false;
        }
        window
            .update_with_buffer(&pixels, WIDTH, HEIGHT)
            .expect("updating the window");
    }
}

/// Whether each pixel's centre falls on the round panel.
fn panel_mask() -> Vec<bool> {
    let radius = WIDTH as f32 / 2.0;
    (0..WIDTH * HEIGHT)
        .map(|index| {
            let x = (index % WIDTH) as f32 + 0.5 - radius;
            let y = (index / WIDTH) as f32 + 0.5 - radius;
            x * x + y * y <= radius * radius
        })
        .collect()
}

fn to_pixels(fb: &FB, mask: &[bool], pixels: &mut [u32]) {
    for (index, (pixel, &on_panel)) in pixels.iter_mut().zip(mask).enumerate() {
        *pixel = if on_panel {
            let point = Point::new((index % WIDTH) as i32, (index / WIDTH) as i32);
            let color = Rgb888::from(fb.pixel(point).expect("inside the panel"));
            u32::from_be_bytes([0, color.r(), color.g(), color.b()])
        } else {
            OFF_PANEL
        };
    }
}

/// Synthetic power and GNSS readings, with the host's UTC clock.
fn sensors(zone: ZoneState) -> Sensors {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs());
    Sensors {
        battery_mv: Some(3_912),
        vbus_mv: Some(5_020),
        vsys_mv: Some(3_890),
        gnss_bytes: 512,
        gnss_fix: true,
        lora_irq: 0,
        clock: ClockState {
            utc: Some(seconds as i64),
            set_from_gnss: true,
            stopped: false,
        },
        zone,
    }
}

/// Saves what the window shows, off-panel corners included.
fn write_png(pixels: &[u32], path: &Path) {
    let pixels: Vec<u8> = pixels
        .iter()
        .flat_map(|pixel| {
            let [_, r, g, b] = pixel.to_be_bytes();
            [r, g, b]
        })
        .collect();
    let file = BufWriter::new(File::create(path).expect("creating the PNG"));
    let mut encoder = png::Encoder::new(file, WIDTH as u32, HEIGHT as u32);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().expect("writing the PNG header");
    writer.write_image_data(&pixels).expect("writing the PNG");
}
