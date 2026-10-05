// Streams a framebuffer to the CO5300 over DMA QSPI.

use embedded_graphics_core::geometry::Point;
use embedded_graphics_core::primitives::Rectangle;

use crate::drivers::co5300::{Co5300ColorMode, Co5300Display, DisplayError};
use crate::framebuffer::Framebuffer;

/// Sends a framebuffer, or a region of it, to the panel.
#[expect(
    async_fn_in_trait,
    reason = "only the display core calls it, from one executor"
)]
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

    /// Flush the panel pixels that show framebuffer region `area` with the picture moved by
    /// `shift`. Where the region reaches the framebuffer's edge, the panel pixels past it, which
    /// repeat that edge, go too.
    async fn flush_region(
        &mut self,
        display: &mut Co5300Display<'_, C>,
        area: Rectangle,
        shift: Point,
        debug_overlay: Option<Rectangle>,
    ) -> Result<(), DisplayError>;

    /// Flush the whole panel with the picture moved by `shift`.
    async fn flush_moved(
        &mut self,
        display: &mut Co5300Display<'_, C>,
        shift: Point,
        debug_damage: bool,
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
        area: Rectangle,
        shift: Point,
        debug_overlay: Option<Rectangle>,
    ) -> Result<(), DisplayError> {
        let Some(corner) = area.bottom_right() else {
            return Ok(());
        };
        let (width, height) = (WIDTH as i32, HEIGHT as i32);
        // A side at the framebuffer's edge takes the panel's edge with it.
        let reach = |from: i32, to: i32, offset: i32, length: i32| {
            (
                if from <= 0 { 0 } else { from + offset },
                if to >= length { length } else { to + offset },
            )
        };
        let (x0, x1) = reach(area.top_left.x, corner.x + 1, shift.x, width);
        let (y0, y1) = reach(area.top_left.y, corner.y + 1, shift.y, height);
        let overlay =
            debug_overlay.map(|overlay| Rectangle::new(overlay.top_left + shift, overlay.size));
        stream_shifted::<C, WIDTH, HEIGHT>(self.buffer(), display, (x0, y0, x1, y1), shift, overlay)
            .await
    }

    async fn flush_moved(
        &mut self,
        display: &mut Co5300Display<'_, C>,
        shift: Point,
        debug_damage: bool,
    ) -> Result<(), DisplayError> {
        if shift == Point::zero() {
            return self.flush(display, debug_damage).await;
        }
        #[cfg(feature = "damage-debug")]
        let overlay = debug_damage.then(|| {
            Rectangle::new(
                Point::zero(),
                embedded_graphics_core::geometry::Size::new(WIDTH as u32, HEIGHT as u32),
            )
        });
        #[cfg(not(feature = "damage-debug"))]
        let overlay = {
            _ = debug_damage;
            None
        };
        stream_shifted::<C, WIDTH, HEIGHT>(
            self.buffer(),
            display,
            (0, 0, WIDTH as i32, HEIGHT as i32),
            shift,
            overlay,
        )
        .await
    }
}

/// Streams panel columns `x0..x1` of rows `y0..y1`, widened to the controller's 2 × 2 grain,
/// each panel pixel taken from the framebuffer pixel `shift` before it, with the
/// framebuffer's edge repeated past it.
async fn stream_shifted<C: Co5300ColorMode, const WIDTH: usize, const HEIGHT: usize>(
    pixels: &[u8],
    display: &mut Co5300Display<'_, C>,
    (x0, y0, x1, y1): (i32, i32, i32, i32),
    shift: Point,
    debug_overlay: Option<Rectangle>,
) -> Result<(), DisplayError>
where
    C::Bytes: AsRef<[u8]>,
{
    let (width, height) = (WIDTH as i32, HEIGHT as i32);
    let even_down = |v: i32, length: i32| (v.clamp(0, length) & !1).min(length - 2);
    let even_up = |v: i32, length: i32| ((v.clamp(0, length) + 1) & !1).min(length);
    let (x0, y0) = (even_down(x0, width), even_down(y0, height));
    let (x1, y1) = (
        even_up(x1, width).max(x0 + 2),
        even_up(y1, height).max(y0 + 2),
    );
    let bpp = C::BYTES_PER_PIXEL;
    let span = (x1 - x0) as usize;
    let (left, from, right) = crate::ui::shift::columns(x0, x1, width, shift.x);
    let middle = (span - left - right) * bpp;
    let row_bytes = span * bpp;

    display.set_addr_window(x0 as u16, y0 as u16, span as u16, (y1 - y0) as u16)?;
    let mut stream = display.begin_stream_async().await?;
    let (mut y, mut done) = (y0, 0);
    while y < y1 {
        stream
            .flush_if_needed_and_get_buf_async(|buf| {
                let room = buf.len() / bpp * bpp;
                let mut written = 0;
                while written < room && y < y1 {
                    let source = (y - shift.y).clamp(0, height - 1) as usize;
                    let row = &pixels[source * WIDTH * bpp..(source + 1) * WIDTH * bpp];
                    let n = (room - written).min(row_bytes - done);
                    let out = &mut buf[written..written + n];
                    write_row(
                        out,
                        done,
                        left * bpp,
                        &row[from * bpp..from * bpp + middle],
                        &row[..bpp],
                        &row[(WIDTH - 1) * bpp..],
                    );
                    #[cfg(feature = "damage-debug")]
                    if let Some(overlay) = debug_overlay {
                        debug_region_chunk::<C>(
                            out,
                            x0 as usize + done / bpp,
                            y as usize,
                            overlay.top_left.x.max(0) as usize,
                            overlay.top_left.y.max(0) as usize,
                            overlay.size.width as usize,
                            overlay.size.height as usize,
                        );
                    }
                    written += n;
                    done += n;
                    if done == row_bytes {
                        done = 0;
                        y += 1;
                    }
                }
                written
            })
            .await?;
    }
    #[cfg(not(feature = "damage-debug"))]
    let _ = debug_overlay;
    stream.flush_buf_async(|_| 0).await?;
    stream.end()
}

/// Writes `out.len()` bytes of a panel row, starting `at` bytes into it. The row is `left`
/// bytes of `first` repeated, then `middle`, then `last` repeated to its end.
fn write_row(out: &mut [u8], at: usize, left: usize, middle: &[u8], first: &[u8], last: &[u8]) {
    let mut at = at;
    let mut out = out;
    while !out.is_empty() {
        let (part, offset) = if at < left {
            (first, (left - at).min(out.len()))
        } else if at < left + middle.len() {
            let start = at - left;
            let n = (middle.len() - start).min(out.len());
            out[..n].copy_from_slice(&middle[start..start + n]);
            out = &mut out[n..];
            at += n;
            continue;
        } else {
            (last, out.len())
        };
        for pixel in out[..offset].chunks_exact_mut(part.len()) {
            pixel.copy_from_slice(part);
        }
        out = &mut out[offset..];
        at += offset;
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
