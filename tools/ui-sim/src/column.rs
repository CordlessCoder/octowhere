//! The column beside several devices: the air's speed and time, each device's group as its
//! node sees it, and the link matrix, which a click edits.

use embedded_graphics::{
    Drawable, Pixel,
    mono_font::{
        MonoTextStyle,
        ascii::{FONT_7X13, FONT_10X20},
    },
    pixelcolor::{Rgb888, RgbColor},
    prelude::{DrawTarget, OriginDimensions, Point, Size},
    text::{Baseline, Text},
};

use octowhere_sim::air::{Reach, SPEEDS};

use crate::{HEIGHT, OFF_PANEL, air::Air, device::Device};

pub const WIDTH: usize = 300;
const MARGIN: i32 = 14;
const LINE: i32 = 15;
/// A link's cell in the matrix.
const CELL: i32 = 22;

const TEXT: Rgb888 = Rgb888::new(0xd2, 0xd3, 0xd6);
const DIM: Rgb888 = Rgb888::new(0x6b, 0x6e, 0x75);
const FOCUS: Rgb888 = Rgb888::new(0xb5, 0xd3, 0x37);

/// Pixels of `width` a row, as the window holds them, to draw on.
pub struct Canvas<'a> {
    pub pixels: &'a mut [u32],
    pub width: usize,
}

impl OriginDimensions for Canvas<'_> {
    fn size(&self) -> Size {
        Size::new(self.width as u32, (self.pixels.len() / self.width) as u32)
    }
}

impl DrawTarget for Canvas<'_> {
    type Color = Rgb888;
    type Error = core::convert::Infallible;

    fn draw_iter<I: IntoIterator<Item = Pixel<Rgb888>>>(
        &mut self,
        pixels: I,
    ) -> Result<(), Self::Error> {
        let height = self.pixels.len() / self.width;
        for Pixel(point, color) in pixels {
            let (Ok(x), Ok(y)) = (usize::try_from(point.x), usize::try_from(point.y)) else {
                continue;
            };
            if x < self.width && y < height {
                self.pixels[y * self.width + x] = rgb(color);
            }
        }
        Ok(())
    }
}

fn rgb(color: Rgb888) -> u32 {
    u32::from_be_bytes([0, color.r(), color.g(), color.b()])
}

/// Writes `text` with its top left at `at`.
pub fn write(canvas: &mut Canvas, text: &str, at: Point, color: Rgb888) {
    let style = MonoTextStyle::new(&FONT_7X13, color);
    let _ = Text::with_baseline(text, at, style, Baseline::Top).draw(canvas);
}

/// The box device `n`'s number takes in its panel's corner.
pub const NUMBER: Size = Size::new(10, 20);

/// Writes device `n`'s number large at `at`, in the focus colour when it has the keyboard.
pub fn number(canvas: &mut Canvas, n: usize, at: Point, focused: bool) {
    let style = MonoTextStyle::new(&FONT_10X20, if focused { FOCUS } else { DIM });
    let _ = Text::with_baseline(&(n + 1).to_string(), at, style, Baseline::Top).draw(canvas);
}

fn fill(canvas: &mut Canvas, at: Point, size: Size, color: Rgb888) {
    for y in at.y..at.y + size.height as i32 {
        for x in at.x..at.x + size.width as i32 {
            if let (Ok(x), Ok(y)) = (usize::try_from(x), usize::try_from(y))
                && x < canvas.width
            {
                canvas.pixels[y * canvas.width + x] = rgb(color);
            }
        }
    }
}

/// Where the matrix's first cell sits in the column, for `count` devices.
fn matrix_origin(count: usize) -> Point {
    Point::new(MARGIN + 3 * 7, matrix_top(count) + 2 * LINE + 4)
}

fn matrix_top(count: usize) -> i32 {
    MARGIN + 3 * LINE + count as i32 * 2 * LINE + LINE
}

/// The link whose cell `point`, in the column, falls on: from the row's device to the column's.
pub fn link_at(point: Point, count: usize) -> Option<(usize, usize)> {
    let offset = point - matrix_origin(count);
    let (row, column) = (offset.y.div_euclid(CELL), offset.x.div_euclid(CELL));
    let range = 0..count as i32;
    (range.contains(&row) && range.contains(&column) && row != column)
        .then_some((row as usize, column as usize))
}

/// Draws the column into `pixels`, `WIDTH` × `HEIGHT`.
pub fn draw(pixels: &mut [u32], air: &Air, devices: &[Device], focus: usize) {
    pixels.fill(OFF_PANEL);
    let mut canvas = Canvas {
        pixels,
        width: WIDTH,
    };
    let canvas = &mut canvas;
    let count = devices.len();
    let utc = air.paced.sim.utc_us().div_euclid(1_000_000);
    let elapsed = (air.paced.sim.now_us() / 1_000_000) as i64;
    write(
        canvas,
        &format!("AIR x{}  {}", SPEEDS[air.paced.speed], hms(elapsed)),
        Point::new(MARGIN, MARGIN),
        TEXT,
    );
    write(
        canvas,
        &format!("UTC {}", hms(utc.rem_euclid(86_400))),
        Point::new(MARGIN, MARGIN + LINE),
        DIM,
    );
    for (n, device) in devices.iter().enumerate() {
        let top = MARGIN + 3 * LINE + n as i32 * 2 * LINE;
        let color = if n == focus { FOCUS } else { TEXT };
        let view = device.stage.mesh();
        write(
            canvas,
            &format!("{} {}", n + 1, view.name),
            Point::new(MARGIN, top),
            color,
        );
        let state = if device.powered_off {
            "powered off".to_string()
        } else {
            match (&view.group, air.paced.sim.key(n)) {
                (Some(group), stored) => {
                    let members = group.members().count();
                    let heard = group
                        .members()
                        .filter(|(_, member)| member.heard.is_some())
                        .count();
                    let generation = stored.map_or(0, |(_, generation)| generation);
                    format!(
                        "id {} gen {generation} {members} members, {heard} heard",
                        group.own
                    )
                }
                (None, _) => "no group".to_string(),
            }
        };
        write(
            canvas,
            &format!("  {state}"),
            Point::new(MARGIN, top + LINE),
            DIM,
        );
    }
    let top = matrix_top(count);
    write(
        canvas,
        "LINKS  row sends, column hears",
        Point::new(MARGIN, top),
        TEXT,
    );
    let origin = matrix_origin(count);
    for n in 0..count {
        let label = (n + 1).to_string();
        let along = n as i32 * CELL + CELL / 2 - 3;
        write(
            canvas,
            &label,
            Point::new(origin.x + along, origin.y - LINE),
            DIM,
        );
        write(
            canvas,
            &label,
            Point::new(MARGIN, origin.y + along - 6),
            DIM,
        );
    }
    for from in 0..count {
        for to in 0..count {
            let at = origin + Point::new(to as i32 * CELL, from as i32 * CELL) + Point::new(2, 2);
            let size = Size::new(CELL as u32 - 4, CELL as u32 - 4);
            if from == to {
                continue;
            }
            fill(canvas, at, size, Rgb888::new(0x30, 0x34, 0x3a));
            let inner = Size::new(size.width - 2, size.height - 2);
            fill(canvas, at + Point::new(1, 1), inner, Rgb888::BLACK);
            match air.paced.reach(from, to) {
                Reach::InReach => fill(
                    canvas,
                    at + Point::new(3, 3),
                    Size::new(inner.width - 4, inner.height - 4),
                    TEXT,
                ),
                Reach::Lossy => fill(
                    canvas,
                    at + Point::new(3, 3),
                    Size::new((inner.width - 4) / 2, inner.height - 4),
                    TEXT,
                ),
                Reach::None => {}
            }
        }
    }
    let help = [
        "click a link: in reach, lossy, none",
        "1-6 keyboard  [ ] speed  X reset",
    ];
    // At the foot, or under the matrix where six devices leave no room.
    let below = origin.y + count as i32 * CELL + LINE / 2;
    let bottom = (HEIGHT as i32 - MARGIN - help.len() as i32 * LINE).max(below);
    for (line, text) in help.iter().enumerate() {
        write(
            canvas,
            text,
            Point::new(MARGIN, bottom + line as i32 * LINE),
            DIM,
        );
    }
}

fn hms(seconds: i64) -> String {
    format!(
        "{:02}:{:02}:{:02}",
        seconds / 3600,
        seconds / 60 % 60,
        seconds % 60
    )
}
