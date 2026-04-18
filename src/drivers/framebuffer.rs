// PSRAM Framebuffer for CO5300 display
// 466x466 RGB565 = 434.312 kB
// Draws to RAM, then flushes entire screen via DMA QSPI

use alloc::boxed::Box;
use embedded_graphics::pixelcolor::Rgb888;
use embedded_graphics_core::draw_target::DrawTarget;
use embedded_graphics_core::geometry::{OriginDimensions, Size};
use embedded_graphics_core::pixelcolor::Rgb565;
use embedded_graphics_core::pixelcolor::raw::RawU16;
use embedded_graphics_core::prelude::*;
use embedded_graphics_core::primitives::Rectangle;

use crate::board;
use crate::drivers::co5300::Co5300Display;
use crate::drivers::co5300::DisplayError;
use crate::util::fill_buf_repeat;

const WIDTH: usize = board::LCD_WIDTH as usize;
const HEIGHT: usize = board::LCD_HEIGHT as usize;
pub const BYTES_PER_PIXEL: usize = 3;
const PIXEL_COUNT: usize = WIDTH * HEIGHT;

pub type Color = Rgb888;

pub struct Framebuffer {
    buf: Box<[u8; PIXEL_COUNT * BYTES_PER_PIXEL]>,
}

impl Default for Framebuffer {
    fn default() -> Self {
        Self::new()
    }
}

impl Framebuffer {
    /// Allocate framebuffer in PSRAM (via global allocator).
    pub fn new() -> Self {
        unsafe {
            // Initialize in-place on the heap
            let mut buf = Box::new_uninit();
            core::ptr::write_bytes(buf.as_mut_ptr(), 0, 1);
            Self {
                buf: buf.assume_init(),
            }
        }
    }

    /// Clear the entire framebuffer with a color.
    pub fn clear_color(&mut self, color: Color) {
        let raw = color.to_be_bytes();
        fill_buf_repeat(self.buf.as_mut_slice(), &raw, PIXEL_COUNT);
    }

    /// Set a single pixel
    ///
    /// PERF: no panic for speed?
    #[inline(always)]
    pub fn set_pixel(&mut self, x: usize, y: usize, color: Color) {
        if x < WIDTH && y < HEIGHT {
            let idx = y * WIDTH + x;
            self.buf[idx * BYTES_PER_PIXEL..][..BYTES_PER_PIXEL]
                .copy_from_slice(&color.to_be_bytes());
        }
    }

    /// Fill a rectangular region.
    pub fn fill_rect(&mut self, x: usize, y: usize, w: usize, h: usize, color: u16) {
        let x_end = (x + w).min(WIDTH);
        let y_end = (y + h).min(HEIGHT);
        let raw = color.to_be_bytes();
        for row in y..y_end {
            let start = row * WIDTH + x;
            let end = row * WIDTH + x_end;
            fill_buf_repeat(
                &mut self.buf[start * BYTES_PER_PIXEL..end * BYTES_PER_PIXEL],
                &raw,
                end - start,
            );
        }
    }

    /// VSync flush for watchface / menus. Same as swap_and_flush but kept distinct for clarity.
    pub fn flush_vsync(&self, display: &mut Co5300Display) {
        let te = display.te();
        for _ in 0..400 {
            if te.is_high() {
                break;
            }
        }
        self.flush(display);
    }

    /// Flush the entire framebuffer to the display via DMA QSPI.
    pub fn flush(&self, display: &mut Co5300Display) {
        display.set_addr_window(0, 0, WIDTH as u16, HEIGHT as u16);
        let mut stream = display.begin_stream();
        let mut remaining = &self.buf[..];
        while !remaining.is_empty() {
            let buf = stream.buf();
            let chunk = buf.len().min(remaining.len());
            let captured = remaining.split_off(..chunk).unwrap();
            buf[..chunk].copy_from_slice(captured);
            stream.stream_pixels(chunk);
        }
    }

    /// Flush only a rectangular region (dirty rect optimization).
    pub fn flush_region(&self, display: &mut Co5300Display, x: u16, y: u16, w: u16, h: u16) {
        if w == 0 || h == 0 {
            return;
        }

        // The CO5300 is happier with even-aligned partial writes.
        // Expand the dirty rect to an even 2x2-aligned region before streaming rows.
        let mut x0 = (x as usize).min(WIDTH.saturating_sub(1));
        let mut y0 = (y as usize).min(HEIGHT.saturating_sub(1));
        let mut x1 = ((x as usize).saturating_add(w as usize)).min(WIDTH);
        let mut y1 = ((y as usize).saturating_add(h as usize)).min(HEIGHT);

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

        display.set_addr_window(x0 as u16, y0 as u16, flush_w as u16, flush_h as u16);
        let mut stream = display.begin_stream();
        for row in y0..(y0 + flush_h) {
            let start = row * WIDTH + x0;
            let end = start + flush_w;
            let line = &self.buf[start * BYTES_PER_PIXEL..end * BYTES_PER_PIXEL];
            stream.buf()[..line.len()].copy_from_slice(line);
            stream.stream_pixels(line.len());
        }
        stream.end();
    }

    /// Get raw buffer for direct access.
    pub fn buffer(&self) -> &[u8] {
        &self.buf[..]
    }

    /// Get mutable raw buffer for direct access (snapshot restore).
    pub fn buffer_mut(&mut self) -> &mut [u8] {
        &mut self.buf[..]
    }
}

impl OriginDimensions for Framebuffer {
    fn size(&self) -> Size {
        Size::new(WIDTH as u32, HEIGHT as u32)
    }
}

impl DrawTarget for Framebuffer {
    type Color = Rgb565;
    type Error = DisplayError;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        for Pixel(coord, color) in pixels.into_iter() {
            if coord.x >= 0 && coord.x < WIDTH as i32 && coord.y >= 0 && coord.y < HEIGHT as i32 {
                let raw = color.to_be_bytes();
                let idx = coord.y as usize * WIDTH + coord.x as usize;
                self.buf[idx * BYTES_PER_PIXEL..][..BYTES_PER_PIXEL].copy_from_slice(&raw);
            }
        }
        Ok(())
    }

    fn fill_contiguous<I>(&mut self, area: &Rectangle, colors: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Self::Color>,
    {
        let area = area.intersection(&Rectangle::new(
            Point::zero(),
            Size::new(WIDTH as u32, HEIGHT as u32),
        ));
        if area.size.width == 0 || area.size.height == 0 {
            return Ok(());
        }

        let x = area.top_left.x as usize;
        let y = area.top_left.y as usize;
        let w = area.size.width as usize;
        let mut row = y;
        let mut col = 0;

        for color in colors.into_iter() {
            if col < w && row < HEIGHT {
                let raw = color.to_be_bytes();
                let idx = row * WIDTH + x + col;
                self.buf[idx * BYTES_PER_PIXEL..][..BYTES_PER_PIXEL].copy_from_slice(&raw);
            }
            col += 1;
            if col >= w {
                col = 0;
                row += 1;
            }
        }
        Ok(())
    }

    fn fill_solid(&mut self, area: &Rectangle, color: Self::Color) -> Result<(), Self::Error> {
        let area = area.intersection(&Rectangle::new(
            Point::zero(),
            Size::new(WIDTH as u32, HEIGHT as u32),
        ));
        if area.size.width == 0 || area.size.height == 0 {
            return Ok(());
        }
        let raw = RawU16::from(color).into_inner();
        self.fill_rect(
            area.top_left.x as usize,
            area.top_left.y as usize,
            area.size.width as usize,
            area.size.height as usize,
            raw,
        );
        Ok(())
    }
}
