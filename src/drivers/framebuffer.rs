// Streams a framebuffer to the CO5300 over DMA QSPI.

use embedded_graphics_core::primitives::Rectangle;

use crate::drivers::co5300::{Co5300ColorMode, Co5300Display, DisplayError};
use crate::framebuffer::Framebuffer;

/// Sends a framebuffer, or a region of it, to the panel.
#[expect(async_fn_in_trait, reason = "only the display core calls it, from one executor")]
pub trait Flush<C: Co5300ColorMode>
where
    C::Bytes: AsRef<[u8]>,
{
    /// Flush the entire framebuffer to the display via DMA QSPI.
    async fn flush(
        &mut self,
        display: &mut Co5300Display<'_, C>,
        debug_damage: bool,
    ) -> Result<(), DisplayError>;

    /// Flush the entire framebuffer through the blocking pixel-stream path.
    fn flush_blocking(
        &mut self,
        display: &mut Co5300Display<'_, C>,
        debug_damage: bool,
    ) -> Result<(), DisplayError>;

    /// Flush only a rectangular region (dirty rect optimization).
    async fn flush_region(
        &mut self,
        display: &mut Co5300Display<'_, C>,
        x: u16,
        y: u16,
        w: u16,
        h: u16,
        debug_overlay: Option<Rectangle>,
    ) -> Result<(), DisplayError>;
}

impl<const N: usize, const WIDTH: usize, const HEIGHT: usize, C: Co5300ColorMode> Flush<C>
    for Framebuffer<N, WIDTH, HEIGHT, C>
where
    C::Bytes: AsRef<[u8]>,
{
    async fn flush(
        &mut self,
        display: &mut Co5300Display<'_, C>,
        debug_damage: bool,
    ) -> Result<(), DisplayError> {
        display.set_addr_window(0, 0, WIDTH as u16, HEIGHT as u16)?;
        let mut stream = display.begin_stream_async().await?;
        let mut remaining = &mut self.buffer_mut()[..];
        let mut offset = 0;

        while !remaining.is_empty() {
            stream
                .flush_if_needed_and_get_buf_async(|mut buf| {
                    let chunk = buf.len().min(remaining.len());
                    let captured = remaining.split_off_mut(..chunk).unwrap();
                    buf[..chunk].copy_from_slice(captured);
                    #[cfg(feature = "damage-debug")]
                    if debug_damage {
                        debug_full_chunk::<C, WIDTH, HEIGHT>(&mut buf[..chunk], offset);
                    }
                    offset += chunk;
                    chunk
                })
                .await?;
        }
        stream.flush_buf_async(|_| 0).await?;
        stream.end()
    }

    fn flush_blocking(
        &mut self,
        display: &mut Co5300Display<'_, C>,
        debug_damage: bool,
    ) -> Result<(), DisplayError> {
        display.set_addr_window(0, 0, WIDTH as u16, HEIGHT as u16)?;
        let mut stream = display.begin_stream()?;
        let mut remaining = &mut self.buffer_mut()[..];
        let mut offset = 0;

        while !remaining.is_empty() {
            let chunk = {
                let buf = stream.flush_if_needed_and_get_buf()?;
                let chunk = buf.len().min(remaining.len());
                let captured = remaining.split_off_mut(..chunk).unwrap();
                buf[..chunk].copy_from_slice(captured);
                #[cfg(feature = "damage-debug")]
                if debug_damage {
                    debug_full_chunk::<C, WIDTH, HEIGHT>(&mut buf[..chunk], offset);
                }
                offset += chunk;
                chunk
            };
            stream.write(chunk);
        }
        stream.flush_buf()?;
        stream.end()
    }

    async fn flush_region(
        &mut self,
        display: &mut Co5300Display<'_, C>,
        x: u16,
        y: u16,
        w: u16,
        h: u16,
        debug_overlay: Option<Rectangle>,
    ) -> Result<(), DisplayError> {
        if w == 0 || h == 0 {
            return Ok(());
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

        display.set_addr_window(x0 as u16, y0 as u16, flush_w as u16, flush_h as u16)?;
        let mut stream = display.begin_stream_async().await?;
        let mut rows = self
            .buffer_mut()
            .chunks_exact_mut(WIDTH * C::BYTES_PER_PIXEL)
            .skip(y0)
            .take(flush_h)
            .map(|row| &mut row[x0 * C::BYTES_PER_PIXEL..(x0 + flush_w) * C::BYTES_PER_PIXEL]);
        let mut row = rows.next().unwrap_or(&mut []);
        let mut row_x = x0;
        let mut row_y = y0;
        let mut keep_going = true;
        while keep_going {
            stream
                .flush_if_needed_and_get_buf_async(|mut buf| {
                    let mut new = 0;
                    loop {
                        let pre_scale_chunk =
                            (buf.len() / C::BYTES_PER_PIXEL * C::BYTES_PER_PIXEL).min(row.len());
                        if pre_scale_chunk == 0 {
                            break;
                        }
                        let captured = row.split_off_mut(..pre_scale_chunk).unwrap();
                        let dma_chunk = buf.split_off_mut(..pre_scale_chunk).unwrap();
                        dma_chunk.copy_from_slice(captured);
                        #[cfg(feature = "damage-debug")]
                        if let Some(overlay) = debug_overlay {
                            debug_region_chunk::<C>(
                                dma_chunk,
                                row_x,
                                row_y,
                                overlay.top_left.x as usize,
                                overlay.top_left.y as usize,
                                overlay.size.width as usize,
                                overlay.size.height as usize,
                            );
                        }

                        new += pre_scale_chunk;
                        row_x += pre_scale_chunk / C::BYTES_PER_PIXEL;
                        if row.is_empty() {
                            let Some(next_row) = rows.next() else {
                                keep_going = false;
                                break;
                            };
                            row = next_row;
                            row_x = x0;
                            row_y += 1;
                        }
                    }
                    new
                })
                .await?;
        }
        stream.flush_buf_async(|_| 0).await?;
        stream.end()
    }
}

#[cfg(feature = "damage-debug")]
fn debug_full_chunk<C: Co5300ColorMode, const WIDTH: usize, const HEIGHT: usize>(
    pixels: &mut [u8],
    offset: usize,
) where
    C::Bytes: AsRef<[u8]>,
{
    let bpp = C::BYTES_PER_PIXEL;
    let first_pixel = offset / bpp;
    for (index, pixel) in pixels.chunks_exact_mut(bpp).enumerate() {
        let position = first_pixel + index;
        let x = position % WIDTH;
        let y = position / WIDTH;
        if x == 0 || x == WIDTH - 1 || y == 0 || y == HEIGHT - 1 {
            pixel.fill(u8::MAX);
        }
    }
}

#[cfg(feature = "damage-debug")]
fn debug_region_chunk<C: Co5300ColorMode>(
    pixels: &mut [u8],
    start_x: usize,
    y: usize,
    region_x: usize,
    region_y: usize,
    region_w: usize,
    region_h: usize,
) where
    C::Bytes: AsRef<[u8]>,
{
    let bpp = C::BYTES_PER_PIXEL;
    let right = region_x + region_w;
    let bottom = region_y + region_h;
    for (index, pixel) in pixels.chunks_exact_mut(bpp).enumerate() {
        let x = start_x + index;
        let in_x = region_x <= x && x < right;
        let in_y = region_y <= y && y < bottom;
        let horizontal = in_x && (y == region_y || y == bottom - 1);
        let vertical = in_y && (x == region_x || x == right - 1);
        if horizontal || vertical {
            pixel.fill(u8::MAX);
        }
    }
}
