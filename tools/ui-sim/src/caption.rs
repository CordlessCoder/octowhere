//! The column a recording adds beside the panel, where a scene says what each step shows.

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

/// The column's width, and the text's margin inside it.
pub const COLUMN: usize = 360;
const MARGIN: i32 = 28;
const SIZE: u32 = 16;
const LINE: i32 = 22;

thread_local! {
    static SAID: Cell<Option<&'static str>> = const { Cell::new(None) };
    static SPEED: Cell<u64> = const { Cell::new(1) };
}

/// Plays the scene `speed` times as fast in a recording from the next step on, with a
/// fast-forward mark in the column while it is above 1.
pub fn fast_forward(speed: u64) {
    SPEED.set(speed.max(1));
}

pub fn speed() -> u64 {
    SPEED.get()
}

/// Shows `text` in the column from the next step on.
pub fn say(text: &'static str) {
    SAID.set(Some(text));
}

/// Empties the column, as a recording starts.
pub fn clear() {
    SAID.set(None);
    SPEED.set(1);
}

pub struct Column {
    shown: (Option<&'static str>, u64),
    pixels: Vec<u32>,
}

impl Column {
    pub fn new() -> Self {
        Self { shown: (None, 1), pixels: vec![OFF_PANEL; COLUMN * HEIGHT] }
    }

    /// Redraws the column if the scene has said something new since.
    pub fn follow(&mut self) {
        let shown = (SAID.get(), SPEED.get());
        if shown != self.shown {
            self.shown = shown;
            self.pixels = render(shown.0.unwrap_or(""));
            if shown.1 > 1 {
                fast_forward_mark(&mut self.pixels);
            }
        }
    }

    /// `panel`'s rows with the column's beside each.
    pub fn beside(&self, panel: &[u32]) -> Vec<u32> {
        panel
            .as_chunks::<WIDTH>().0.iter()
            .zip(self.pixels.as_chunks::<COLUMN>().0)
            .flat_map(|(panel, column)| panel.iter().chain(column).copied())
            .collect()
    }
}

/// `text` in white microtext, wrapped to the column and centred on the panel's middle row.
fn render(text: &str) -> Vec<u32> {
    let font = FontdueRenderer::new(FontdueRendererCtx::new_rc(), SIZE, chrome::WHITE, chrome::FONTS);
    let style = text::style(&font, chrome::WHITE, SIZE, chrome::FRAKTION);
    let mut lines: Vec<String> = Vec::new();
    for word in text.split(' ') {
        match lines.last_mut() {
            Some(line) if style.advance(&format!("{line} {word}")) <= (COLUMN as i32 - 2 * MARGIN) as f32 => {
                line.push(' ');
                line.push_str(word);
            }
            _ => lines.push(word.to_owned()),
        }
    }
    let mut fb = FB::boxed();
    let cap = text::cap(&style);
    let top = (HEIGHT as i32 - (LINE * (lines.len() as i32 - 1) + cap)) / 2;
    for (index, line) in lines.iter().enumerate() {
        let baseline = top + cap + LINE * index as i32;
        style.draw_on_baseline(line, Point::new(MARGIN, baseline), &mut *fb).expect("drawing a caption");
    }
    // White on black, so each channel is the text's coverage; blend it over the surround.
    let mut pixels = Vec::with_capacity(COLUMN * HEIGHT);
    for y in 0..HEIGHT {
        for x in 0..COLUMN {
            let color = Rgb888::from(fb.pixel(Point::new(x as i32, y as i32)).expect("inside the buffer"));
            let [_, r, g, b] = OFF_PANEL.to_be_bytes();
            let mix = |under: u8, over: u8| (u16::from(under) + (255 - u16::from(under)) * u16::from(over) / 255) as u8;
            pixels.push(u32::from_be_bytes([0, mix(r, color.r()), mix(g, color.g()), mix(b, color.b())]));
        }
    }
    pixels
}

/// Two white triangles pointing right, centred above the caption.
fn fast_forward_mark(pixels: &mut [u32]) {
    const HALF: i32 = 14;
    let (left, top) = (COLUMN as i32 / 2 - HALF, HEIGHT as i32 / 2 - 90);
    for triangle in 0..2 {
        // Each widens from a point at its top to its full depth at its middle row and back.
        for y in 0..2 * HALF {
            for x in 0..HALF - (y - HALF).abs() {
                let (x, y) = (left + triangle * HALF + x, top + y);
                pixels[y as usize * COLUMN + x as usize] = 0x00d2_d3d6;
            }
        }
    }
}
