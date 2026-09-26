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
}

/// Shows `text` in the column from the next step on.
pub fn say(text: &'static str) {
    SAID.set(Some(text));
}

/// Empties the column, as a recording starts.
pub fn clear() {
    SAID.set(None);
}

pub struct Column {
    shown: Option<&'static str>,
    pixels: Vec<u32>,
}

impl Column {
    pub fn new() -> Self {
        Self { shown: None, pixels: vec![OFF_PANEL; COLUMN * HEIGHT] }
    }

    /// Redraws the column if the scene has said something new since.
    pub fn follow(&mut self) {
        let said = SAID.get();
        if said != self.shown {
            self.shown = said;
            self.pixels = render(said.unwrap_or(""));
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
