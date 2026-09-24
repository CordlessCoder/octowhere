//! A desktop window running the firmware's UI stage. The mouse is the touchscreen, and the
//! keyboard sets the compass readings the motion task would publish. From the repository root:
//!
//! ```text
//! cargo +stable run --release --manifest-path tools/ui-sim/Cargo.toml -- [--scale 2]
//! ```
//!
//! After `--`, `--play <scene>` loops a scene from `scenes.rs` in the window instead, and
//! `--record <scene> <out.gif>` records one without a window. Both step it on the firmware's
//! frame period, so a recording is the same every run and shows none of the host's speed.
//! `--scenes` lists them.
//!
//! The panel shows only its inscribed circle. The window paints the corners outside it grey, and
//! screenshots and recordings leave them transparent. `--unmasked`, or M in the window, shows the
//! whole framebuffer instead, to see what is drawn where nobody will see it.
//!
//! Keys:
//!
//! - Left / Right: heading down / up 5°, 1° with Shift. Space spins it.
//! - Up / Down: pitch; Q / E: roll, 5° a press, 1° with Shift.
//! - C: calibration through none, part-way and complete. T: top edge vertical or not, which
//!   withholds the heading.
//! - D: magnetic disturbance on or off. L: sensors live or silent.
//! - Z: the clock's zone, through unknown, automatic in Dublin, and chosen by hand in New York
//!   and Kolkata. R: the clock, through set from GNSS, running unconfirmed, stopped and
//!   unreadable.
//! - Hold H: a hand covering the screen, which restarts calibration on the settled compass.
//! - Tab: next screen without the slide. P: save the window to `ui-sim-<n>.png` in the current
//!   directory. V: start or stop recording it to `ui-sim-<n>.gif`, which keeps the mask it
//!   started with. M: mask the corners or show them. Esc: quit.
//!
//! V samples the window every 20 ms of the host's time, so slow drawing shows in it. For a
//! video of any recording, `ffmpeg -i ui-sim-1.gif -pix_fmt yuv420p ui-sim-1.mp4`.
//!
//! Readings are synthetic. Drawing times in the title are the host's and say nothing about the
//! target.

use std::{
    fs::File,
    io::BufWriter,
    path::{Path, PathBuf},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
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
        screens::PeripheralState,
        script::{self, Driver},
        stage::{Input, Motion, Sensors, Stage, Touch},
    },
};

mod record;
mod scenes;

const WIDTH: usize = LCD_WIDTH as usize;
const HEIGHT: usize = LCD_HEIGHT as usize;
/// The motion task's sample periods, fast while a compass screen shows.
const FAST_SAMPLE_US: u64 = 20_000;
const SLOW_SAMPLE_US: u64 = 250_000;
const SENSOR_PERIOD_US: u64 = 1_000_000;
/// What the panel's round glass hides is shown in this in the window, so the corners read as
/// off-panel.
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
    /// Which of [`CLOCKS`] the clock reads.
    clock: usize,
}

/// The clock's states R steps through: whether GNSS has set it, whether it stopped, and whether
/// it can be read.
const CLOCKS: [(bool, bool, bool); 4] =
    [(true, false, true), (false, false, true), (false, true, true), (false, false, false)];

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
            Key::R => self.clock = (self.clock + 1) % CLOCKS.len(),
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
            compass: self.compass(),
        }
    }
}

/// The framebuffer and what the window shows of it.
struct Panel {
    fb: Box<FB>,
    pixels: Vec<u32>,
    /// Whether each pixel shows on the panel.
    mask: Vec<bool>,
    masked: bool,
}

impl Panel {
    fn new(masked: bool) -> Self {
        Self {
            fb: FB::boxed(),
            pixels: vec![OFF_PANEL; WIDTH * HEIGHT],
            mask: panel_mask(),
            masked,
        }
    }

    fn set_masked(&mut self, masked: bool) {
        self.masked = masked;
        to_pixels(&self.fb, self.masked.then_some(&self.mask), &mut self.pixels);
    }

    /// Which pixels an image of the panel should leave out.
    fn knock_out(&self) -> Option<&[bool]> {
        self.masked.then_some(&self.mask)
    }

    /// Draws the stage's damage, or all of it when `whole`. Says how many pixels it drew and
    /// how long that took, or `None` when there was nothing to draw.
    fn draw(&mut self, stage: &Stage, whole: bool) -> Option<(u32, Duration)> {
        let changed = stage.changed();
        if !whole && changed.is_empty() {
            return None;
        }
        // One buffer, so each step repaints only its own damage, as the firmware's buffers
        // would with the step before added.
        let drawing = Instant::now();
        let pixels_drawn = if whole || changed.is_full() {
            stage.draw(&mut *self.fb);
            LCD_WIDTH as u32 * LCD_HEIGHT as u32
        } else {
            stage.draw(&mut Clip::new(&mut *self.fb, changed));
            changed.pixels()
        };
        let took = drawing.elapsed();
        to_pixels(&self.fb, self.masked.then_some(&self.mask), &mut self.pixels);
        Some((pixels_drawn, took))
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let after = |flag: &str| args.iter().position(|arg| arg == flag).map(|at| &args[at + 1..]);
    let masked = after("--unmasked").is_none();
    if after("--scenes").is_some() {
        for scene in scenes::SCENES {
            println!("{:20} {}", scene.name, scene.about);
        }
        return;
    }
    if let Some(rest) = after("--record") {
        let [name, out, ..] = rest else { panic!("--record takes a scene and an output path") };
        record(scenes::find(name), PathBuf::from(out), masked);
        return;
    }
    let scene = after("--play").map(|rest| {
        scenes::find(rest.first().expect("--play takes a scene"))
    });
    let scale = match after("--scale").and_then(|rest| rest.first()).map(String::as_str) {
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
    match scene {
        Some(scene) => play(&mut window, scene, masked),
        None => interact(window, masked),
    }
}

/// Steps `scene` on the firmware's frame period and records it to `path`.
fn record(scene: &scenes::Scene, path: PathBuf, masked: bool) {
    let mut panel = Panel::new(masked);
    let mut recording: Option<record::Recording> = None;
    let mut driver = Driver::new();
    driver.observe(|stage, now| match &mut recording {
        None => {
            panel.draw(stage, true);
            recording = Some(record::Recording::start(
                path.clone(),
                now,
                &panel.pixels,
                panel.knock_out(),
            ));
        }
        Some(recording) => {
            // Samples due before this step show the step before.
            recording.sample(now - 1, &panel.pixels);
            panel.draw(stage, false);
            recording.sample(now, &panel.pixels);
        }
    });
    (scene.run)(&mut driver);
    drop(driver);
    let recording = recording.expect("the scene took no steps");
    recording.finish().join().expect("encoding the recording");
}

/// Plays `scene` in the window at its own pace, over and over, until the window closes.
fn play(window: &mut Window, scene: &scenes::Scene, masked: bool) {
    // The scene paces itself.
    window.set_target_fps(0);
    window.set_title(&format!("octowhere: {}", scene.name));
    let show = |window: &mut Window, pixels: &[u32]| {
        if !window.is_open() || window.is_key_down(Key::Escape) {
            std::process::exit(0);
        }
        window.update_with_buffer(pixels, WIDTH, HEIGHT).expect("updating the window");
    };
    loop {
        let mut panel = Panel::new(masked);
        let start = Instant::now();
        let mut driver = Driver::new();
        driver.observe(|stage, now| {
            panel.draw(stage, now == script::FRAME);
            let due = start + Duration::from_micros(now);
            thread::sleep(due.saturating_duration_since(Instant::now()));
            show(window, &panel.pixels);
        });
        (scene.run)(&mut driver);
        drop(driver);
        // A second on the last frame before it starts again.
        let end = Instant::now() + Duration::from_secs(1);
        while Instant::now() < end {
            show(window, &panel.pixels);
            thread::sleep(Duration::from_millis(16));
        }
    }
}

/// The mouse and keyboard drive the stage, on the host's clock.
fn interact(mut window: Window, masked: bool) {
    window.set_target_fps(60);

    let mut stage = Stage::new(PeripheralState::default());
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
        clock: 0,
    };
    let mut panel = Panel::new(masked);
    let start = Instant::now();
    let (mut next_motion, mut next_sensors) = (0, 0);
    let mut samples_fast = false;
    let mut readings_changed = true;
    let mut redraw = true;
    let mut screenshots = 0;
    let mut recording: Option<record::Recording> = None;
    let (mut recordings, mut encoders) = (0, Vec::new());
    let mut drawn = String::new();
    let mut title_changed = false;
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
                    write_png(&panel.pixels, panel.knock_out(), Path::new(&path));
                    println!("saved {path}");
                }
                Key::V => {
                    match recording.take() {
                        Some(finished) => encoders.push(finished.finish()),
                        None => {
                            recordings += 1;
                            let path = PathBuf::from(format!("ui-sim-{recordings}.gif"));
                            println!("recording {}", path.display());
                            recording = Some(record::Recording::start(
                                path,
                                now,
                                &panel.pixels,
                                panel.knock_out(),
                            ));
                        }
                    }
                    title_changed = true;
                }
                Key::M => {
                    panel.set_masked(!panel.masked);
                    title_changed = true;
                }
                key => {
                    let clock = (readings.zone, readings.clock);
                    readings_changed |= readings.press(key, shift);
                    zone_changed |= (readings.zone, readings.clock) != clock;
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
            sensors: sensors_due.then(|| sensors(readings.zone_state(), CLOCKS[readings.clock])),
        });
        samples_fast = update.samples_fast;
        if update.recalibrate {
            readings.calibration = 0;
            readings_changed = true;
        }

        if let Some((pixels_drawn, took)) = panel.draw(&stage, redraw) {
            drawn = format!(
                "octowhere: {:?}, {} px drawn in {:.2} ms on the host",
                stage.screen(),
                pixels_drawn,
                took.as_secs_f64() * 1e3
            );
            title_changed = true;
            redraw = false;
        }
        if title_changed {
            let recording = if recording.is_some() { " [recording]" } else { "" };
            let unmasked = if panel.masked { "" } else { " [unmasked]" };
            window.set_title(&format!("{drawn}{unmasked}{recording}"));
            title_changed = false;
        }
        if let Some(recording) = &mut recording {
            recording.sample(now, &panel.pixels);
        }
        window
            .update_with_buffer(&panel.pixels, WIDTH, HEIGHT)
            .expect("updating the window");
    }
    encoders.extend(recording.map(record::Recording::finish));
    for encoder in encoders {
        encoder.join().expect("encoding a recording");
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

/// What the window shows of `fb`, with the corners in `knock_out` grey.
fn to_pixels(fb: &FB, knock_out: Option<&[bool]>, pixels: &mut [u32]) {
    for (index, pixel) in pixels.iter_mut().enumerate() {
        *pixel = if knock_out.is_none_or(|mask| mask[index]) {
            let point = Point::new((index % WIDTH) as i32, (index / WIDTH) as i32);
            let color = Rgb888::from(fb.pixel(point).expect("inside the panel"));
            u32::from_be_bytes([0, color.r(), color.g(), color.b()])
        } else {
            OFF_PANEL
        };
    }
}

/// The host's UTC clock, in the state R selects.
fn sensors(zone: ZoneState, (gnss, stopped, readable): (bool, bool, bool)) -> Sensors {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs());
    Sensors {
        clock: ClockState {
            utc: readable.then_some(seconds as i64),
            set_from_gnss: gnss,
            stopped,
        },
        zone,
    }
}

/// Saves what the window shows, with the pixels `knock_out` leaves out transparent.
fn write_png(pixels: &[u32], knock_out: Option<&[bool]>, path: &Path) {
    let pixels: Vec<u8> = pixels
        .iter()
        .enumerate()
        .flat_map(|(index, pixel)| {
            let [_, r, g, b] = pixel.to_be_bytes();
            let shown = knock_out.is_none_or(|mask| mask[index]);
            [r, g, b, if shown { 0xff } else { 0 }]
        })
        .collect();
    let file = BufWriter::new(File::create(path).expect("creating the PNG"));
    let mut encoder = png::Encoder::new(file, WIDTH as u32, HEIGHT as u32);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().expect("writing the PNG header");
    writer.write_image_data(&pixels).expect("writing the PNG");
}
