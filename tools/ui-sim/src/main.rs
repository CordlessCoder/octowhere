//! A desktop window running the firmware's UI stage. The mouse is the touchscreen, and the
//! keyboard sets the compass readings the motion task would publish:
//!
//! ```text
//! cargo run --release -p ui-sim -- [--scale 2] [--boards 3 [--alone] [--log]]
//! ```
//!
//! `--boards <n>` shows up to six devices side by side, each its own stage and its own mesh
//! node, unchanged from the firmware's, on `octowhere-sim`'s simulated air between them. They
//! start in one group, or each alone with `--alone`, to pair through the screens. A click on a
//! panel touches it and gives it the keyboard, as 1 to 6 do. A column beside them shows each
//! node's group, the air's speed and time, and the link matrix: a click on a cell steps the
//! link from its row's device to its column's through in reach, lossy and out of reach, with
//! Shift both ways. `[` and `]` run the air slower or faster, up to 120 times the host's clock,
//! though a finger or a key held keeps it at the host's pace, so that a press or a double tap
//! keeps its length on a device's clock, which is its node's. X resets the device with the
//! keyboard, which starts its node again from what it stored. Powering a device off stops its node. The nodes' warnings go to the terminal, and
//! with `--log` every line they log. One device alone has no air: a scripted mesh answers it,
//! whose other device pairs with whatever it asks.
//!
//! After `--`, `--play <scene>` loops a scene from `scenes.rs` in the window instead, and
//! `--record <scene> <out>` records one without a window, to GIF or MP4 by `<out>`'s extension.
//! Both step it on the firmware's frame period, so a recording is the same every run and shows
//! none of the host's speed. A scene that captions its steps records with a column beside the
//! panel for them. In a terminal, `--record` shows bars for the frames rendered and encoded,
//! against the scene's length from a first run that does not draw. `--scenes` lists them.
//! MP4 is encoded by `ffmpeg`, which must be on the path, as H.264 with full colour resolution
//! (4:4:4).
//!
//! A white disc marks where a finger is down, and fades as a ring for 300 ms after it lifts, so
//! a recording shows taps and drags.
//!
//! The panel shows only its inscribed circle. The window paints the corners outside it grey, and
//! screenshots and GIFs leave them transparent. An MP4 has no transparency and keeps the grey.
//! Pixels the circle's edge crosses blend the panel with the grey by how much of them it covers.
//! The board's PWR and BOOT keys show past the glass's right edge, where they sit on the device,
//! and light while held, by K and O in the window or by a scene's press.
//! `--unmasked`, or M in the window, shows the whole framebuffer instead, to see what is drawn
//! where nobody will see it.
//!
//! Keys:
//!
//! - Left / Right: heading down / up 5°, 1° with Shift. Space spins it.
//! - Up / Down: pitch; Q / E: roll, 5° a press, 1° with Shift.
//! - C: calibration through none, part-way and complete. T: the screen stood near vertical or
//!   not, which withholds the heading.
//! - D: magnetic disturbance on or off. L: sensors live or silent.
//! - Z: the clock's zone, through unknown, automatic in Dublin, and chosen by hand in New York
//!   and Kolkata. R: the clock, through set from GNSS, running unconfirmed, stopped and
//!   unreadable.
//! - B: the supply, through USB and charging, battery only, and USB with no battery. - / =:
//!   the battery's level down / up 5 %, 1 % with Shift. G: GNSS fix or none. N: the GNSS
//!   receiver through answering, being reset once, twice and three times after it stopped, and
//!   faulted, as the GNSS task reports it.
//! - Drag up from a face for the Events drawer, as on the device.
//! - Hold H: a hand covering the screen, which goes to the clock face.
//! - K: the power key. Released within a second it is a short press; held a second, a long one.
//!   Once powered off, held 512 ms it powers the board on, through the start-up. O: the BOOT
//!   key, the same way.
//! - Drag down from a face for the settings panel, as on the device.
//! - While the screen rests on the always-on face or is dark, a click is the touch controller's
//!   tap gesture, as its gesture mode reports one, so a double click wakes it.
//! - Tab: next screen without the slide. P: save the window to `ui-sim-<n>.png` in the current
//!   directory. V: start or stop recording it to `ui-sim-<n>.gif`, or `.mp4` with `--mp4`,
//!   which keeps the mask it started with. M: mask the corners or show them. Esc: quit.
//!
//! V samples the window every 20 ms of the host's time, so slow drawing shows in it.
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
    ui::{shift, stage::Stage},
};
use octowhere_ui_script::{self as script, Driver};

mod air;
mod buttons;
mod caption;
mod column;
mod device;
mod record;
mod scenes;

use air::Air;
use device::{Controls, Device, Readings};

pub const WIDTH: usize = LCD_WIDTH as usize;
pub const HEIGHT: usize = LCD_HEIGHT as usize;
/// What the panel's round glass hides is shown in this in the window, so the corners read as
/// off-panel.
pub const OFF_PANEL: u32 = 0x1c1c1c;
/// The gap between two devices' panels in the window.
const GAP: usize = 8;
/// The most devices the window holds.
const MOST: usize = 6;

/// The framebuffer and what the window shows of it.
pub struct Panel {
    fb: Box<FB>,
    /// What the window shows of the framebuffer, before the touch marker.
    base: Vec<u32>,
    /// `base` with the touch marker over it.
    pub pixels: Vec<u32>,
    /// Whether each pixel shows on the panel, even in part.
    mask: Vec<bool>,
    /// The pixels the glass's edge crosses, and how much of each it shows, out of 255.
    edge: Vec<(usize, u8)>,
    /// The board's keys past the glass, which of them are held, the pixels past the glass they
    /// reach, and whether each pixel shows anything: the glass or a key.
    buttons: buttons::Buttons,
    held: [bool; 2],
    beyond: Vec<usize>,
    shown: Vec<bool>,
    pub masked: bool,
    /// Where a finger is down, or where one lifted and when, while its marker fades.
    contact: Option<Point>,
    lifted: Option<(Point, u64)>,
    /// The display's level against the stored one, which the window shows by scaling colours.
    light: f32,
    /// How far the display moves the picture, as the panel's flush would.
    shift: Point,
}

/// The touch marker's radius, and how long it fades after a lift, in microseconds.
const MARKER_RADIUS: f32 = 16.0;
const MARKER_FADE: u64 = 300_000;

impl Panel {
    pub fn new(masked: bool) -> Self {
        let coverage = panel_coverage();
        let buttons = buttons::Buttons::new();
        let beyond = buttons
            .reach()
            .filter(|&index| coverage[index] == 0)
            .collect();
        let shown = coverage
            .iter()
            .enumerate()
            .map(|(index, &glass)| glass > 0 || buttons.reaches(index))
            .collect();
        Self {
            fb: FB::boxed(),
            base: vec![OFF_PANEL; WIDTH * HEIGHT],
            pixels: vec![OFF_PANEL; WIDTH * HEIGHT],
            mask: coverage.iter().map(|&shown| shown > 0).collect(),
            edge: coverage
                .iter()
                .enumerate()
                .filter(|&(_, &shown)| shown > 0 && shown < 255)
                .map(|(index, &shown)| (index, shown))
                .collect(),
            buttons,
            held: [false; 2],
            beyond,
            shown,
            masked,
            contact: None,
            lifted: None,
            light: 1.0,
            shift: Point::zero(),
        }
    }

    /// Forgets the finger and its fading mark, as a device's clock starts again from zero.
    pub fn forget_touch(&mut self) {
        (self.contact, self.lifted) = (None, None);
    }

    pub fn set_masked(&mut self, masked: bool) {
        self.masked = masked;
        self.refresh();
        self.compose(0);
    }

    /// Follows the stage's contact, and puts a marker where it is: a disc while a finger is
    /// down, and a ring that fades after it lifts, so a one-step tap still shows in a recording.
    pub fn touch(&mut self, stage: &Stage, now: u64, held: [bool; 2]) {
        self.held = held;
        // The stage reports the contact on its unshifted picture; the finger is on the panel.
        let contact = stage.contact().map(|point| point + stage.shift());
        if let (None, Some(was)) = (contact, self.contact) {
            self.lifted = Some((was, now));
        }
        if contact.is_some() {
            self.lifted = None;
        }
        self.contact = contact;
        let stored = stage.peripherals().brightness.max(1);
        self.light = (f32::from(stage.shown_level()) / f32::from(stored)).min(1.0);
        self.compose(now);
    }

    fn compose(&mut self, now: u64) {
        self.pixels.copy_from_slice(&self.base);
        if self.light < 1.0 {
            let panel = self
                .pixels
                .iter_mut()
                .zip(&self.mask)
                .filter(|(_, on)| **on || !self.masked);
            for (pixel, _) in panel {
                let [_, r, g, b] = pixel.to_be_bytes();
                let dim = |channel: u8| (f32::from(channel) * self.light) as u8;
                *pixel = u32::from_be_bytes([0, dim(r), dim(g), dim(b)]);
            }
        }
        // After the dim, which does not reach past the glass.
        if self.masked {
            for &index in &self.beyond {
                self.pixels[index] = self.buttons.outside(index, self.held);
            }
            for &(index, shown) in &self.edge {
                let [_, gr, gg, gb] = self.buttons.outside(index, self.held).to_be_bytes();
                let [_, r, g, b] = self.pixels[index].to_be_bytes();
                let mix = |panel: u8, grey: u8| {
                    ((u32::from(panel) * u32::from(shown)
                        + u32::from(grey) * (255 - u32::from(shown))
                        + 127)
                        / 255) as u8
                };
                self.pixels[index] = u32::from_be_bytes([0, mix(r, gr), mix(g, gg), mix(b, gb)]);
            }
        }
        let (center, fill, ring) = match (self.contact, self.lifted) {
            (Some(point), _) => (point, 0.45, 0.9),
            (None, Some((point, at))) if now.saturating_sub(at) < MARKER_FADE => {
                let left = 1.0 - now.saturating_sub(at) as f32 / MARKER_FADE as f32;
                (point, 0.0, 0.9 * left)
            }
            _ => return,
        };
        // White, with a dark outline so it shows on white too.
        let reach = MARKER_RADIUS as i32 + 4;
        for y in (center.y - reach).max(0)..(center.y + reach).min(HEIGHT as i32) {
            for x in (center.x - reach).max(0)..(center.x + reach).min(WIDTH as i32) {
                let distance = ((x - center.x) as f32).hypot((y - center.y) as f32);
                let band = |radius: f32| (1.0 - (distance - radius).abs() / 1.5).clamp(0.0, 1.0);
                let inside = (MARKER_RADIUS - distance + 0.5).clamp(0.0, 1.0);
                let light = (ring * band(MARKER_RADIUS)).max(fill * inside);
                let dark = ring * band(MARKER_RADIUS + 2.0) * (1.0 - light);
                if light <= 0.0 && dark <= 0.0 {
                    continue;
                }
                let index = y as usize * WIDTH + x as usize;
                let [_, r, g, b] = self.pixels[index].to_be_bytes();
                let mix = |channel: u8| {
                    let lit = f32::from(channel) + (255.0 - f32::from(channel)) * light;
                    (lit * (1.0 - dark)) as u8
                };
                self.pixels[index] = u32::from_be_bytes([0, mix(r), mix(g), mix(b)]);
            }
        }
    }

    /// Takes what the window shows from the framebuffer again.
    fn refresh(&mut self) {
        let knock_out = self.masked.then_some(&self.mask[..]);
        to_pixels(&self.fb, knock_out, self.shift, &mut self.base);
    }

    /// Which pixels an image of the panel should leave out.
    pub fn knock_out(&self) -> Option<&[bool]> {
        self.masked.then_some(&self.shown)
    }

    /// Draws the stage's damage, or all of it when `whole`. Says how many pixels it drew and
    /// how long that took, or `None` when there was nothing to draw.
    pub fn draw(&mut self, stage: &Stage, whole: bool) -> Option<(u32, Duration)> {
        let changed = stage.changed();
        let moved = stage.shift() != self.shift;
        self.shift = stage.shift();
        if !whole && changed.is_empty() {
            if moved {
                self.refresh();
                self.compose(0);
            }
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
        self.refresh();
        self.compose(0);
        Some((pixels_drawn, took))
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let after = |flag: &str| {
        args.iter()
            .position(|arg| arg == flag)
            .map(|at| &args[at + 1..])
    };
    let masked = after("--unmasked").is_none();
    if after("--scenes").is_some() {
        for scene in scenes::SCENES {
            println!("{:20} {}", scene.name, scene.about);
        }
        return;
    }
    if let Some(rest) = after("--record") {
        let [name, out, ..] = rest else {
            panic!("--record takes a scene and an output path")
        };
        let out = PathBuf::from(out);
        assert!(
            record::Format::of(&out).is_some(),
            "--record writes a .gif or an .mp4"
        );
        record(scenes::find(name), out, masked);
        return;
    }
    let scene =
        after("--play").map(|rest| scenes::find(rest.first().expect("--play takes a scene")));
    let count = after("--boards").map_or(1, |rest| {
        rest.first()
            .and_then(|count| count.parse().ok())
            .filter(|count| (1..=MOST).contains(count))
            .unwrap_or_else(|| panic!("--boards takes 1 to {MOST}"))
    });
    let scale = match after("--scale")
        .and_then(|rest| rest.first())
        .map(String::as_str)
    {
        None | Some("1") => Scale::X1,
        Some("2") => Scale::X2,
        Some("4") => Scale::X4,
        Some(other) => panic!("--scale takes 1, 2 or 4, not {other}"),
    };
    let layout = Layout { count };
    let mut window = Window::new(
        "octowhere",
        layout.width(),
        HEIGHT,
        WindowOptions {
            scale,
            ..WindowOptions::default()
        },
    )
    .expect("opening the window");
    match scene {
        Some(scene) => play(&mut window, scene, masked),
        None => interact(
            window,
            layout,
            Options {
                masked,
                extension: if after("--mp4").is_some() {
                    "mp4"
                } else {
                    "gif"
                },
                in_group: after("--alone").is_none(),
                every_line: after("--log").is_some(),
            },
        ),
    }
}

/// How long `scene` records for on the recording's own clock, from a run that steps the stage
/// without drawing it.
fn recorded_length(scene: &scenes::Scene) -> u64 {
    caption::clear();
    buttons::clear();
    let (mut first, mut played, mut last) = (None, 0, 0);
    let mut driver = Driver::new();
    driver.observe(|_, now| {
        played += (now - last) / caption::speed();
        last = now;
        first.get_or_insert(played);
    });
    (scene.run)(&mut driver);
    drop(driver);
    played - first.unwrap_or(played)
}

/// Steps `scene` on the firmware's frame period and records it to `path`, with bars for the
/// frames rendered and encoded.
fn record(scene: &scenes::Scene, path: PathBuf, masked: bool) {
    let mut progress = Some(record::Progress::new(recorded_length(scene)));
    let mut panel = Panel::new(masked);
    let mut column = scene.captioned.then(caption::Column::new);
    caption::clear();
    buttons::clear();
    let width = WIDTH + column.as_ref().map_or(0, |_| caption::COLUMN);
    let frame = |panel: &Panel, column: &mut Option<caption::Column>| match column {
        Some(column) => {
            column.follow();
            column.beside(&panel.pixels)
        }
        None => panel.pixels.clone(),
    };
    let mut recording: Option<record::Recording> = None;
    // The recording's own clock, which runs behind the stage's while the scene fast-forwards.
    let (mut played, mut last) = (0, 0);
    let mut driver = Driver::new();
    driver.observe(|stage, now| {
        played += (now - last) / caption::speed();
        last = now;
        let now = played;
        match &mut recording {
            None => {
                panel.draw(stage, true);
                panel.touch(stage, now, buttons::held());
                // The column shows in every frame.
                let knock_out = panel.knock_out().map(|mask| {
                    mask.as_chunks::<WIDTH>()
                        .0
                        .iter()
                        .flat_map(|row| {
                            row.iter()
                                .copied()
                                .chain(std::iter::repeat_n(true, width - WIDTH))
                        })
                        .collect::<Vec<_>>()
                });
                recording = Some(record::Recording::start(
                    path.clone(),
                    now,
                    width,
                    &frame(&panel, &mut column),
                    knock_out.as_deref(),
                    progress.take(),
                ));
            }
            Some(recording) => {
                // Samples due before this step show the step before.
                recording.sample(now - 1, &frame(&panel, &mut column));
                panel.draw(stage, false);
                panel.touch(stage, now, buttons::held());
                recording.sample(now, &frame(&panel, &mut column));
            }
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
        window
            .update_with_buffer(pixels, WIDTH, HEIGHT)
            .expect("updating the window");
    };
    loop {
        let mut panel = Panel::new(masked);
        buttons::clear();
        let start = Instant::now();
        let (mut played, mut last) = (0, 0);
        let mut driver = Driver::new();
        driver.observe(|stage, now| {
            panel.draw(stage, now == script::FRAME);
            played += (now - last) / caption::speed();
            last = now;
            panel.touch(stage, played, buttons::held());
            let due = start + Duration::from_micros(played);
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

/// Where each device's panel sits in the window, and the column after them when there are
/// several.
#[derive(Clone, Copy)]
struct Layout {
    count: usize,
}

impl Layout {
    fn width(self) -> usize {
        if self.count == 1 {
            return WIDTH;
        }
        self.column_x() + column::WIDTH
    }

    fn panel_x(self, n: usize) -> usize {
        n * (WIDTH + GAP)
    }

    fn column_x(self) -> usize {
        self.count * (WIDTH + GAP)
    }

    /// The panel `point` falls on, and where on it.
    fn panel_at(self, point: Point) -> Option<(usize, Point)> {
        let x = usize::try_from(point.x).ok()?;
        let n = x / (WIDTH + GAP);
        let along = x - self.panel_x(n);
        (n < self.count && along < WIDTH && (0..HEIGHT as i32).contains(&point.y))
            .then(|| (n, Point::new(along as i32, point.y)))
    }
}

struct Options {
    masked: bool,
    /// What V records to: "gif" or "mp4".
    extension: &'static str,
    /// Whether several devices start in one group.
    in_group: bool,
    /// Whether every line the nodes log goes to the terminal.
    every_line: bool,
}

/// The mouse and keyboard drive the devices, on the host's clock.
fn interact(mut window: Window, layout: Layout, options: Options) {
    window.set_target_fps(60);
    let count = layout.count;
    let width = layout.width();
    let host_utc = || {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_secs() as i64)
    };
    let mut air =
        (count > 1).then(|| Air::new(count, options.in_group, host_utc(), options.every_line));
    let mut devices: Vec<Device> = (0..count)
        .map(|n| {
            let label = if count > 1 {
                format!("[{}] ", n + 1)
            } else {
                String::new()
            };
            Device::new(
                options.masked,
                Readings::new(octowhere_sim::air::position(n)),
                label,
                air.is_none(),
            )
        })
        .collect();
    let mut frame = vec![OFF_PANEL; width * HEIGHT];
    let mut column = vec![OFF_PANEL; column::WIDTH * HEIGHT];
    let start = Instant::now();
    let mut focus = 0;
    // The device the mouse went down on, which has the finger until it lifts.
    let mut touched: Option<usize> = None;
    let mut was_down = false;
    let mut screenshots = 0;
    let mut recording: Option<record::Recording> = None;
    let (mut recordings, mut encoders) = (0, Vec::new());
    let mut title = String::new();

    while window.is_open() && !window.is_key_down(Key::Escape) {
        let wall = start.elapsed().as_micros() as u64 + 1;
        // A finger or a key held keeps the air at the host's pace, so presses keep their length.
        let input = window.get_mouse_down(MouseButton::Left)
            || [Key::K, Key::O, Key::H]
                .iter()
                .any(|&key| window.is_key_down(key));
        if let Some(air) = &mut air {
            air.advance(wall, input);
        }
        let utc_s = air.as_ref().map_or_else(host_utc, |air| air.paced.utc_s());
        let shift = window.is_key_down(Key::LeftShift) || window.is_key_down(Key::RightShift);
        for key in window.get_keys_pressed(KeyRepeat::Yes) {
            let digit = [
                Key::Key1,
                Key::Key2,
                Key::Key3,
                Key::Key4,
                Key::Key5,
                Key::Key6,
            ]
            .iter()
            .position(|&digit| digit == key);
            match (key, digit) {
                (_, Some(n)) if n < count && !input => {
                    devices[focus].release_controls();
                    focus = n;
                }
                (Key::Tab, _) => {
                    let device = &mut devices[focus];
                    device.stage.show(device.stage.screen().next());
                    device.redraw = true;
                }
                (Key::P, _) => {
                    screenshots += 1;
                    let path = format!("ui-sim-{screenshots}.png");
                    let mask = frame_mask(&devices, layout);
                    write_png(&frame, width, mask.as_deref(), Path::new(&path));
                    println!("saved {path}");
                }
                (Key::V, _) => match recording.take() {
                    Some(finished) => encoders.push(finished.finish()),
                    None => {
                        recordings += 1;
                        let path =
                            PathBuf::from(format!("ui-sim-{recordings}.{}", options.extension));
                        println!("recording {}", path.display());
                        let mask = frame_mask(&devices, layout);
                        recording = Some(record::Recording::start(
                            path,
                            wall,
                            width,
                            &frame,
                            mask.as_deref(),
                            None,
                        ));
                    }
                },
                (Key::M, _) => {
                    for device in &mut devices {
                        device.panel.set_masked(!device.panel.masked);
                    }
                }
                (Key::LeftBracket | Key::RightBracket, _) => {
                    if let Some(air) = &mut air {
                        air.paced.faster(key == Key::RightBracket);
                    }
                }
                (Key::X, _) if air.is_some() => {
                    println!("[{}] reset", focus + 1);
                    devices[focus].reset();
                    if let Some(air) = &mut air {
                        air.paced.sim.restart(focus);
                    }
                }
                (key, _) => {
                    devices[focus].press(key, shift);
                }
            }
        }

        let pointer = window
            .get_mouse_pos(MouseMode::Discard)
            .map(|(x, y)| Point::new(x as i32, y as i32));
        let down = window.get_mouse_down(MouseButton::Left);
        if down
            && !was_down
            && let Some(pointer) = pointer
        {
            if let Some((n, _)) = layout.panel_at(pointer) {
                if n != focus {
                    devices[focus].release_controls();
                }
                touched = Some(n);
                focus = n;
            } else if let Some(air) = &air {
                let inside = pointer - Point::new(layout.column_x() as i32, 0);
                if let Some((from, to)) = column::link_at(inside, count) {
                    let reach = air.paced.reach(from, to).next();
                    air.paced.set_reach(from, to, reach);
                    if shift {
                        air.paced.set_reach(to, from, reach);
                    }
                }
            }
        }
        if !down {
            touched = None;
        }
        was_down = down;

        for (n, device) in devices.iter_mut().enumerate() {
            let now = air
                .as_ref()
                .map_or(wall, |air| air.paced.sim.clock(n) as u64 + 1);
            let contact = touched
                .filter(|&touched| touched == n)
                .and(pointer)
                .and_then(|pointer| layout.panel_at(pointer))
                .filter(|&(on, _)| on == n)
                .map(|(_, at)| at);
            let held = if n == focus {
                Controls {
                    power: window.is_key_down(Key::K),
                    boot: window.is_key_down(Key::O),
                    cover: window.is_key_down(Key::H),
                }
            } else {
                Controls::default()
            };
            if let Some(air) = &air {
                air.sense(n, &device.readings);
                if let Some((view, seen)) = air.paced.sim.view_since(n, device.seen) {
                    device.seen = seen;
                    device.stage.set_mesh(view);
                }
                let seen = device.messages_seen;
                let sim = &air.paced.sim;
                let mut taken = None;
                device.stage.update_messages(|messages| {
                    taken = sim.messages_since(n, seen, messages);
                    taken.is_some()
                });
                if let Some(seen) = taken {
                    device.messages_seen = seen;
                }
            }
            let stepped = device.step(now, utc_s, contact, held);
            if let Some(air) = &mut air {
                if let Some(request) = stepped.mesh {
                    air.paced.sim.command(n, request.into());
                }
                match stepped.powered {
                    Some(false) => air.paced.sim.power_off(n),
                    Some(true) => air.paced.sim.restart(n),
                    None => {}
                }
            }
            device
                .panel
                .touch(&device.stage, now, [held.power, held.boot]);
        }

        for (n, device) in devices.iter().enumerate() {
            let x = layout.panel_x(n);
            for (row, pixels) in device.panel.pixels.chunks(WIDTH).enumerate() {
                frame[row * width + x..][..WIDTH].copy_from_slice(pixels);
            }
        }
        if let Some(air) = &air {
            let mut canvas = column::Canvas {
                pixels: &mut frame,
                width,
            };
            for n in 0..count {
                column::number(&mut canvas, n, number_corner(layout, n), n == focus);
            }
            column::draw(&mut column, air, &devices, focus);
            let x = layout.column_x();
            for (row, pixels) in column.chunks(column::WIDTH).enumerate() {
                frame[row * width + x..][..column::WIDTH].copy_from_slice(pixels);
            }
        }

        let shown = format!(
            "octowhere: {}{}{}{}",
            if count > 1 {
                format!("{}, ", focus + 1)
            } else {
                String::new()
            },
            devices[focus].drawn.as_deref().unwrap_or("starting"),
            if devices[focus].panel.masked {
                ""
            } else {
                " [unmasked]"
            },
            if recording.is_some() {
                " [recording]"
            } else {
                ""
            },
        );
        if shown != title {
            window.set_title(&shown);
            title = shown;
        }
        if let Some(recording) = &mut recording {
            recording.sample(wall, &frame);
        }
        window
            .update_with_buffer(&frame, width, HEIGHT)
            .expect("updating the window");
    }
    encoders.extend(recording.map(record::Recording::finish));
    for encoder in encoders {
        encoder.join().expect("encoding a recording");
    }
}

/// Where device `n`'s number sits, in its panel's top left corner, past the glass.
fn number_corner(layout: Layout, n: usize) -> Point {
    Point::new(layout.panel_x(n) as i32 + 10, 8)
}

/// Which pixels of the window an image of it should leave out: each panel's corners, while
/// they are masked.
fn frame_mask(devices: &[Device], layout: Layout) -> Option<Vec<bool>> {
    let masks: Vec<&[bool]> = devices
        .iter()
        .map(|device| device.panel.knock_out())
        .collect::<Option<_>>()?;
    let width = layout.width();
    let mut shown = vec![true; width * HEIGHT];
    for (n, mask) in masks.iter().enumerate() {
        let x = layout.panel_x(n);
        for (row, pixels) in mask.chunks(WIDTH).enumerate() {
            shown[row * width + x..][..WIDTH].copy_from_slice(pixels);
        }
        if layout.count > 1 {
            let corner = number_corner(layout, n);
            for row in corner.y..corner.y + column::NUMBER.height as i32 {
                let start = row as usize * width + corner.x as usize;
                shown[start..][..column::NUMBER.width as usize].fill(true);
            }
        }
    }
    Some(shown)
}

/// Whether each pixel's centre falls on the round panel.
/// How much of each pixel the round glass shows, out of 255, from how far its centre lies
/// inside the circle.
fn panel_coverage() -> Vec<u8> {
    let radius = WIDTH as f32 / 2.0;
    (0..WIDTH * HEIGHT)
        .map(|index| {
            let x = (index % WIDTH) as f32 + 0.5 - radius;
            let y = (index / WIDTH) as f32 + 0.5 - radius;
            let inside = (radius - x.hypot(y) + 0.5).clamp(0.0, 1.0);
            (inside * 255.0).round() as u8
        })
        .collect()
}

/// What the window shows of `fb`, with the corners in `knock_out` grey.
/// What the panel shows of `fb`, moved by `shift` with its edge repeated past it, as the
/// display's flush sends it.
fn to_pixels(fb: &FB, knock_out: Option<&[bool]>, shift: Point, pixels: &mut [u32]) {
    for (index, pixel) in pixels.iter_mut().enumerate() {
        *pixel = if knock_out.is_none_or(|mask| mask[index]) {
            let point = Point::new(
                shift::source((index % WIDTH) as i32, shift.x, WIDTH as i32),
                shift::source((index / WIDTH) as i32, shift.y, HEIGHT as i32),
            );
            let color = Rgb888::from(fb.pixel(point).expect("inside the panel"));
            u32::from_be_bytes([0, color.r(), color.g(), color.b()])
        } else {
            OFF_PANEL
        };
    }
}

/// Saves what the window shows, with the pixels `knock_out` leaves out transparent.
fn write_png(pixels: &[u32], width: usize, knock_out: Option<&[bool]>, path: &Path) {
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
    let mut encoder = png::Encoder::new(file, width as u32, HEIGHT as u32);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().expect("writing the PNG header");
    writer.write_image_data(&pixels).expect("writing the PNG");
}
