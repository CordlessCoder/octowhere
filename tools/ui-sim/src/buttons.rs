//! The board's two keys, drawn past the glass's right edge where they sit on the device: PWR
//! at 45° clockwise from the top and BOOT at 135°, each an arc of the bezel with its name beyond
//! it, thicker and lime while held. A scene holds one with [`hold`], so a recording shows the
//! press as well as what it does.

use std::cell::Cell;

use embedded_graphics::{
    pixelcolor::{Rgb888, RgbColor},
    prelude::Point,
};
use octowhere_ui::{
    chrome::{self, FB, FontdueRenderer, FontdueRendererCtx},
    ui::text,
};

use crate::{HEIGHT, OFF_PANEL, WIDTH};

thread_local! {
    static HELD: Cell<[bool; 2]> = const { Cell::new([false; 2]) };
}

#[derive(Clone, Copy, Debug)]
pub enum Button {
    Power,
    #[expect(
        dead_code,
        reason = "no scene presses BOOT, which has no behaviour yet"
    )]
    Boot,
}

/// Each key's place, clockwise from the top in degrees, and its name.
const KEYS: [(f32, &str); 2] = [(45.0, "PWR"), (135.0, "BOOT")];
/// How far either side of its place an arc runs, in degrees.
const HALF_SPAN: f32 = 9.0;
/// The glass's radius, where an arc starts, and where it ends idle and held.
const GLASS: f32 = 233.0;
const IDLE_TO: f32 = 237.5;
const HELD_TO: f32 = 240.0;
/// How far out a key's name is centred, and its size.
const NAME_AT: f32 = 262.0;
const NAME_SIZE: u32 = 12;
/// Samples a pixel's side, for the arcs' coverage.
const SAMPLES: usize = 4;

/// Holds `button` down, or lets it go, from the next step on.
pub fn hold(button: Button, down: bool) {
    let mut held = HELD.get();
    held[button as usize] = down;
    HELD.set(held);
}

/// Which keys a scene holds down.
pub fn held() -> [bool; 2] {
    HELD.get()
}

/// Lets both keys go, as a scene starts.
pub fn clear() {
    HELD.set([false; 2]);
}

/// How much of one pixel a key covers, out of 255: its arc idle and held, and its name.
#[derive(Clone, Copy, Default)]
struct Cover {
    idle: u8,
    held: u8,
    name: u8,
}

/// Each pixel's cover by the two keys, worked out once.
pub struct Buttons {
    /// Indices into `covers`, by pixel; `None` where neither key reaches.
    slots: Vec<Option<(usize, usize)>>,
    covers: [Vec<Cover>; 2],
}

impl Buttons {
    pub fn new() -> Self {
        let mut slots = vec![None; WIDTH * HEIGHT];
        let mut covers = [Vec::new(), Vec::new()];
        for (key, &(place, name)) in KEYS.iter().enumerate() {
            let names = name_coverage(place, name);
            for index in 0..WIDTH * HEIGHT {
                let (x, y) = ((index % WIDTH) as f32, (index / WIDTH) as f32);
                let cover = Cover {
                    idle: arc_coverage(place, IDLE_TO, x, y),
                    held: arc_coverage(place, HELD_TO, x, y),
                    name: names[index],
                };
                if cover.idle > 0 || cover.held > 0 || cover.name > 0 {
                    slots[index] = Some((key, covers[key].len()));
                    covers[key].push(cover);
                }
            }
        }
        Self { slots, covers }
    }

    /// Whether a key reaches the pixel at `index` in either state.
    pub fn reaches(&self, index: usize) -> bool {
        self.slots[index].is_some()
    }

    /// The pixels a key reaches, in either state.
    pub fn reach(&self) -> impl Iterator<Item = usize> + '_ {
        (0..self.slots.len()).filter(|&index| self.reaches(index))
    }

    /// What the pixel at `index` shows past the glass with the keys `held` down: the off-panel
    /// grey with the key's arc and name over it.
    pub fn outside(&self, index: usize, held: [bool; 2]) -> u32 {
        let Some((key, slot)) = self.slots[index] else {
            return OFF_PANEL;
        };
        let cover = self.covers[key][slot];
        let (arc, color) = if held[key] {
            (cover.held, Rgb888::from(chrome::LIME))
        } else {
            (cover.idle, Rgb888::from(chrome::GRAY))
        };
        let [_, gr, gg, gb] = OFF_PANEL.to_be_bytes();
        let shown = arc.max(cover.name);
        let mix = |grey: u8, ink: u8| {
            ((u32::from(grey) * (255 - u32::from(shown)) + u32::from(ink) * u32::from(shown) + 127)
                / 255) as u8
        };
        u32::from_be_bytes([
            0,
            mix(gr, color.r()),
            mix(gg, color.g()),
            mix(gb, color.b()),
        ])
    }
}

/// How much of the pixel at (`x`, `y`) the arc at `place` covers when it reaches out to `to`.
fn arc_coverage(place: f32, to: f32, x: f32, y: f32) -> u8 {
    let center = WIDTH as f32 / 2.0;
    let mut inside = 0;
    for sy in 0..SAMPLES {
        for sx in 0..SAMPLES {
            let dx = x + (sx as f32 + 0.5) / SAMPLES as f32 - center;
            let dy = y + (sy as f32 + 0.5) / SAMPLES as f32 - center;
            let radius = dx.hypot(dy);
            // Clockwise from the top.
            let angle = dx.atan2(-dy).to_degrees();
            if (GLASS..=to).contains(&radius) && (angle - place).abs() <= HALF_SPAN {
                inside += 1;
            }
        }
    }
    (inside * 255 / (SAMPLES * SAMPLES)) as u8
}

/// The coverage of `name` set upright in PP Fraktion Mono, centred `NAME_AT` out at `place`.
fn name_coverage(place: f32, name: &str) -> Vec<u8> {
    let font = FontdueRenderer::new(
        FontdueRendererCtx::new_rc(),
        NAME_SIZE,
        chrome::WHITE,
        chrome::FONTS,
    );
    let style = text::style(&font, chrome::WHITE, NAME_SIZE, chrome::FRAKTION);
    let (sin, cos) = place.to_radians().sin_cos();
    let center = WIDTH as f32 / 2.0;
    let (x, y) = (center + sin * NAME_AT, center - cos * NAME_AT);
    let left = (x - style.advance(name) / 2.0).round() as i32;
    let baseline = (y + text::cap(&style) as f32 / 2.0).round() as i32;
    let mut fb = FB::boxed();
    style
        .draw_on_baseline(name, Point::new(left, baseline), &mut *fb)
        .expect("drawing a key's name");
    // White on black, so a channel is the coverage.
    (0..WIDTH * HEIGHT)
        .map(|index| {
            let point = Point::new((index % WIDTH) as i32, (index / WIDTH) as i32);
            Rgb888::from(fb.pixel(point).expect("inside the buffer")).r()
        })
        .collect()
}
