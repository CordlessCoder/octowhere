//! Times framebuffer flushes on the display core, with no wait for TE: a whole frame unshifted
//! and at each shifted position, and regions through the flush from before pixel shift and the
//! current one, unshifted and shifted. Logs each case's spread in µs, round after round.

use defmt::info;
use embassy_time::Instant;
use embedded_graphics::prelude::*;
use embedded_graphics::primitives::Rectangle;
use octowhere::{
    board,
    chrome::{Color, FB},
    drivers::{
        co5300::{Co5300Display, DisplayError},
        framebuffer::Flush as _,
    },
    ui::shift::POSITIONS,
};

const RUNS: usize = 64;

/// A region of the frame, named for what redraws it.
const REGIONS: [(&str, Rectangle); 5] = [
    ("band", rect(0, 198, 466, 120)),
    ("gauge", rect(262, 281, 178, 30)),
    ("compass", rect(83, 83, 300, 300)),
    ("small", rect(200, 200, 40, 40)),
    ("rows", rect(0, 232, 466, 2)),
];

const fn rect(x: i32, y: i32, w: u32, h: u32) -> Rectangle {
    Rectangle::new(Point::new(x, y), Size::new(w, h))
}

fn report(case: &str, region: &str, shift: Point, samples: &mut [u32; RUNS]) {
    samples.sort_unstable();
    info!(
        "[FLUSHBENCH] case={=str} region={=str} shift={},{} n={} min={} median={} p90={} max={}",
        case,
        region,
        shift.x,
        shift.y,
        RUNS,
        samples[0],
        samples[RUNS / 2],
        samples[RUNS * 9 / 10],
        samples[RUNS - 1],
    );
}

pub async fn run(display: &mut Co5300Display<'_, Color>, fb: &mut FB) -> ! {
    // Content does not change what a copy costs, but fill it so the rows are not all zero.
    for (i, byte) in fb.buffer_mut().iter_mut().enumerate() {
        *byte = (i * 7 + i / 932) as u8;
    }
    let mut samples = [0u32; RUNS];
    for round in 0.. {
        info!("[FLUSHBENCH] round {}", round);
        for &(x, y) in &POSITIONS {
            let shift = Point::new(i32::from(x), i32::from(y));
            for sample in &mut samples {
                let start = Instant::now();
                fb.flush_moved(display, shift, false).await.unwrap();
                *sample = start.elapsed().as_micros() as u32;
            }
            report("full", "frame", shift, &mut samples);
        }
        for (name, area) in REGIONS {
            for sample in &mut samples {
                let start = Instant::now();
                flush_region_before(fb, display, area).await.unwrap();
                *sample = start.elapsed().as_micros() as u32;
            }
            report("before", name, Point::zero(), &mut samples);
            for shift in [Point::zero(), Point::new(3, 0), Point::new(2, 2)] {
                for sample in &mut samples {
                    let start = Instant::now();
                    fb.flush_region(display, area, shift, None).await.unwrap();
                    *sample = start.elapsed().as_micros() as u32;
                }
                report("region", name, shift, &mut samples);
            }
        }
        octowhere::settings::hold_display_core_if_asked();
    }
    unreachable!()
}

/// The region flush as it was before pixel shift, from `f5bceb3`.
async fn flush_region_before(
    fb: &mut FB,
    display: &mut Co5300Display<'_, Color>,
    area: Rectangle,
) -> Result<(), DisplayError> {
    const WIDTH: usize = board::LCD_WIDTH as usize;
    const HEIGHT: usize = board::LCD_HEIGHT as usize;
    const BPP: usize = 2;
    let (x, y, w, h) = (
        area.top_left.x as usize,
        area.top_left.y as usize,
        area.size.width as usize,
        area.size.height as usize,
    );
    let mut x0 = x.min(WIDTH - 1);
    let mut y0 = y.min(HEIGHT - 1);
    let mut x1 = (x + w).min(WIDTH);
    let mut y1 = (y + h).min(HEIGHT);
    x0 &= !1;
    y0 &= !1;
    if x1 & 1 != 0 && x1 < WIDTH {
        x1 += 1;
    }
    if y1 & 1 != 0 && y1 < HEIGHT {
        y1 += 1;
    }
    if x1 <= x0 {
        x1 = (x0 + 2).min(WIDTH);
    }
    if y1 <= y0 {
        y1 = (y0 + 2).min(HEIGHT);
    }
    let flush_w = (x1 - x0).max(2).min(WIDTH - x0);
    let flush_h = (y1 - y0).max(2).min(HEIGHT - y0);
    display.set_addr_window(x0 as u16, y0 as u16, flush_w as u16, flush_h as u16)?;
    let mut stream = display.begin_stream_async().await?;
    let mut rows = fb
        .buffer_mut()
        .chunks_exact_mut(WIDTH * BPP)
        .skip(y0)
        .take(flush_h)
        .map(|row| &mut row[x0 * BPP..(x0 + flush_w) * BPP]);
    let mut row = rows.next().unwrap_or(&mut []);
    let mut keep_going = true;
    while keep_going {
        stream
            .flush_if_needed_and_get_buf_async(|mut buf| {
                let mut new = 0;
                loop {
                    let chunk = (buf.len() / BPP * BPP).min(row.len());
                    if chunk == 0 {
                        break;
                    }
                    let captured = row.split_off_mut(..chunk).unwrap();
                    let dma_chunk = buf.split_off_mut(..chunk).unwrap();
                    dma_chunk.copy_from_slice(captured);
                    new += chunk;
                    if row.is_empty() {
                        let Some(next_row) = rows.next() else {
                            keep_going = false;
                            break;
                        };
                        row = next_row;
                    }
                }
                new
            })
            .await?;
    }
    stream.flush_buf_async(|_| 0).await?;
    stream.end()
}
