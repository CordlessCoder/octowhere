// PSRAM Framebuffer for CO5300 display
// 466x466 RGB565 = 434.312 kB
// Draws to RAM, then flushes entire screen via DMA QSPI

use crate::drivers::co5300::Co5300ColorMode;
use crate::drivers::co5300::Co5300Display;
use crate::drivers::co5300::DisplayError;
use crate::util::{fill_buf_repeat, widening_copy};
use alloc::alloc::Allocator;
use alloc::boxed::Box;
use core::marker::PhantomData;
use embedded_graphics_core::draw_target::DrawTarget;
use embedded_graphics_core::geometry::{OriginDimensions, Size};
use embedded_graphics_core::prelude::*;
use embedded_graphics_core::primitives::Rectangle;

#[repr(align(32))]
#[derive(Clone)]
pub struct Framebuffer<
    const UPSCALE: usize,
    const N: usize,
    const WIDTH: usize,
    const HEIGHT: usize,
    C: Co5300ColorMode,
> where
    <C as embedded_graphics::pixelcolor::raw::ToBytes>::Bytes: core::convert::AsRef<[u8]>,
{
    buf: [u8; N],
    color: PhantomData<C>,
}

/// Calculates the required buffer size.
///
/// This function is a workaround for current limitations in Rust const generics.
/// It can be used to calculate the `N` parameter based on the size and color type of the framebuffer.
pub const fn buffer_size<C: Co5300ColorMode>(width: usize, height: usize) -> usize
where
    <C as embedded_graphics::pixelcolor::raw::ToBytes>::Bytes: core::convert::AsRef<[u8]>,
{
    width * height * C::BYTES_PER_PIXEL
}

impl<
    const UPSCALE: usize,
    const N: usize,
    const WIDTH: usize,
    const HEIGHT: usize,
    C: Co5300ColorMode,
> Framebuffer<UPSCALE, N, WIDTH, HEIGHT, C>
where
    <C as embedded_graphics::pixelcolor::raw::ToBytes>::Bytes: core::convert::AsRef<[u8]>,
{
    const PIXEL_COUNT: usize = WIDTH * HEIGHT;
    const BUFFER_SIZE: usize = buffer_size::<C>(WIDTH, HEIGHT);

    /// Static assertion that N is correct.
    // MSRV: remove N when constant generic expressions are stabilized
    #[expect(unused)]
    const CHECK_N: () = assert!(
        N == Self::BUFFER_SIZE,
        "Invalid N: it must be equal to the output of buffer_size for the given width and height"
    );

    #[must_use]
    pub fn alloc<A: Allocator>(alloc: A) -> Box<Self, A> {
        unsafe {
            // Initialize in-place on the heap
            let mut alloc = Box::new_uninit_in(alloc);
            core::ptr::write_bytes(alloc.as_mut_ptr(), 0, 1);
            alloc.assume_init()
        }
    }

    /// Clear the entire framebuffer with a color.
    pub fn clear_color(&mut self, color: C) {
        let raw = color.to_be_bytes();
        fill_buf_repeat(self.buf.as_mut_slice(), raw.as_ref(), Self::PIXEL_COUNT);
    }

    /// Set a single pixel
    ///
    /// PERF: no panic for speed?
    #[inline]
    pub fn set_pixel(&mut self, x: usize, y: usize, color: C) {
        if x < WIDTH && y < HEIGHT {
            let idx = y * WIDTH + x;
            unsafe {
                self.buf
                    .get_unchecked_mut(idx * C::BYTES_PER_PIXEL..)
                    .get_unchecked_mut(..C::BYTES_PER_PIXEL)
                    .copy_from_slice(color.to_be_bytes().as_ref());
            }
        }
    }

    /// Fill a rectangular region.
    pub fn fill_rect(&mut self, x: usize, y: usize, w: usize, h: usize, raw: &[u8]) {
        let x_end = (x + w).min(WIDTH);
        let y_end = (y + h).min(HEIGHT);
        for row in y..y_end {
            let start = row * WIDTH + x;
            let end = row * WIDTH + x_end;
            fill_buf_repeat(
                &mut self.buf[start * raw.len()..end * raw.len()],
                raw,
                end - start,
            );
        }
    }

    // /// VSync flush for watchface / menus.
    // pub async fn flush_vsync(&self, display: &mut Co5300Display<'_, C>) {
    //     display.wait_for_vsync().await;
    //     self.flush(display).await;
    // }

    /// Flush the entire framebuffer to the display via DMA QSPI.
    pub async fn flush(
        &mut self,
        display: &mut Co5300Display<'_, C>,
        mut post_write: impl FnMut(&mut [u8]),
    ) {
        display.set_addr_window(0, 0, (WIDTH * UPSCALE) as u16, (HEIGHT * UPSCALE) as u16);
        let mut stream = display.begin_stream_async().await;

        let mut rows = self.buf.chunks_exact_mut(WIDTH * C::BYTES_PER_PIXEL);
        let mut row = rows.next().unwrap_or(&mut []);
        let mut repetition = 0;
        let mut skip = 0;
        let mut keep_going = true;
        while keep_going {
            stream
                .flush_if_needed_and_get_buf_async(|mut buf| {
                    let mut new = 0;
                    loop {
                        let mut rem = &mut row[skip..];
                        let pre_scale_chunk = (buf.len() / UPSCALE / C::BYTES_PER_PIXEL
                            * C::BYTES_PER_PIXEL)
                            .min(rem.len());
                        skip += pre_scale_chunk;
                        if pre_scale_chunk == 0 {
                            break;
                        }
                        let captured = rem.split_off_mut(..pre_scale_chunk).unwrap();
                        widening_copy::<UPSCALE>(
                            buf.split_off_mut(..pre_scale_chunk * UPSCALE).unwrap(),
                            captured,
                            C::BYTES_PER_PIXEL,
                        );
                        if repetition == const { UPSCALE - 1 } {
                            post_write(captured);
                        }

                        new += pre_scale_chunk * UPSCALE;
                        if rem.is_empty() {
                            skip = 0;
                            repetition += 1;
                            if repetition >= UPSCALE {
                                let Some(next_row) = rows.next() else {
                                    keep_going = false;
                                    break;
                                };
                                row = next_row;
                                repetition = 0;
                            }
                        }
                    }
                    new
                })
                .await;
        }
        stream.flush_buf_async(|_| 0).await;
        stream.end();
    }

    /// Flush only a rectangular region (dirty rect optimization).
    pub async fn flush_region(
        &mut self,
        display: &mut Co5300Display<'_, C>,
        x: u16,
        y: u16,
        w: u16,
        h: u16,
        mut post_write: impl FnMut(&mut [u8]),
    ) {
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

        // PERF: Buffer as much as possible into stream.buf() before streaming, instead of doing it
        // per-row(buffer fits anywhere between 2 and 8k pixels depending on pixel type)

        display.set_addr_window(x0 as u16, y0 as u16, flush_w as u16, flush_h as u16);
        let mut stream = display.begin_stream_async().await;
        let mut rows = self
            .buf
            .chunks_exact_mut(WIDTH * C::BYTES_PER_PIXEL)
            .skip(y0)
            .take(flush_h)
            .map(|row| &mut row[x0 * C::BYTES_PER_PIXEL..(x0 + flush_w) * C::BYTES_PER_PIXEL]);
        let mut row = rows.next().unwrap_or(&mut []);
        let mut repetition = 0;
        let mut skip = 0;
        let mut keep_going = true;
        while keep_going {
            stream
                .flush_if_needed_and_get_buf_async(|mut buf| {
                    let mut new = 0;
                    loop {
                        let mut rem = &mut row[skip..];
                        let pre_scale_chunk = (buf.len() / UPSCALE / C::BYTES_PER_PIXEL
                            * C::BYTES_PER_PIXEL)
                            .min(rem.len());
                        skip += pre_scale_chunk;
                        if pre_scale_chunk == 0 {
                            break;
                        }
                        let captured = rem.split_off_mut(..pre_scale_chunk).unwrap();
                        widening_copy::<UPSCALE>(
                            buf.split_off_mut(..pre_scale_chunk * UPSCALE).unwrap(),
                            captured,
                            C::BYTES_PER_PIXEL,
                        );
                        if repetition == const { UPSCALE - 1 } {
                            post_write(captured);
                        }

                        new += pre_scale_chunk * UPSCALE;
                        if rem.is_empty() {
                            skip = 0;
                            repetition += 1;
                            if repetition >= UPSCALE {
                                let Some(next_row) = rows.next() else {
                                    keep_going = false;
                                    break;
                                };
                                row = next_row;
                                repetition = 0;
                            }
                        }
                    }
                    new
                })
                .await;
        }
        stream.flush_buf_async(|_| 0).await;
        stream.end();
    }

    /// Get raw buffer for direct access.
    pub fn buffer(&self) -> &[u8] {
        &self.buf
    }

    /// Get mutable raw buffer for direct access (snapshot restore).
    pub fn buffer_mut(&mut self) -> &mut [u8] {
        &mut self.buf
    }
}

impl<
    const UPSCALE: usize,
    const N: usize,
    const WIDTH: usize,
    const HEIGHT: usize,
    C: Co5300ColorMode,
> OriginDimensions for Framebuffer<UPSCALE, N, WIDTH, HEIGHT, C>
where
    <C as embedded_graphics::pixelcolor::raw::ToBytes>::Bytes: core::convert::AsRef<[u8]>,
{
    fn size(&self) -> Size {
        Size::new(WIDTH as u32, HEIGHT as u32)
    }
}

impl<
    const UPSCALE: usize,
    const N: usize,
    const WIDTH: usize,
    const HEIGHT: usize,
    C: Co5300ColorMode,
> DrawTarget for Framebuffer<UPSCALE, N, WIDTH, HEIGHT, C>
where
    <C as embedded_graphics::pixelcolor::raw::ToBytes>::Bytes: core::convert::AsRef<[u8]>,
{
    type Color = C;
    type Error = DisplayError;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        for Pixel(coord, color) in pixels.into_iter() {
            self.set_pixel(coord.x as usize, coord.y as usize, color);
        }
        Ok(())
    }

    #[inline]
    fn fill_contiguous<I>(&mut self, area: &Rectangle, colors: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Self::Color>,
    {
        let visible = self.bounding_box().intersection(area);
        if visible.size.width == 0 || visible.size.height == 0 {
            return Ok(());
        }

        let x = area.top_left.x as usize;
        let y = area.top_left.y as usize;
        let w = x + area.size.width as usize;
        let mut row = y;
        let mut col = x;

        for color in colors.into_iter() {
            self.set_pixel(col, row, color);
            col += 1;
            if col >= w {
                col = x;
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
        self.fill_rect(
            area.top_left.x as usize,
            area.top_left.y as usize,
            area.size.width as usize,
            area.size.height as usize,
            color.to_be_bytes().as_ref(),
        );
        Ok(())
    }
}
