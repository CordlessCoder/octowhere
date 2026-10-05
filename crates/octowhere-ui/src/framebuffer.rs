//! A framebuffer the size of the panel, holding pixels in the panel's byte order.

use alloc::boxed::Box;
use core::marker::PhantomData;
use embedded_graphics::pixelcolor::raw::ToBytes;
use embedded_graphics::pixelcolor::{Gray8, Rgb565, Rgb888};
use embedded_graphics_core::draw_target::DrawTarget;
use embedded_graphics_core::geometry::{OriginDimensions, Size};
use embedded_graphics_core::prelude::*;
use embedded_graphics_core::primitives::Rectangle;

use crate::chrome::RgbColorExt;
use crate::ui::geometry::for_each_visible_color;

/// A colour type a framebuffer can store, as big-endian bytes.
pub trait PixelFormat: ToBytes + PixelColor + RgbColorExt
where
    Self::Bytes: AsRef<[u8]>,
{
    const BYTES_PER_PIXEL: usize;
}

impl PixelFormat for Rgb888 {
    const BYTES_PER_PIXEL: usize = 3;
}

impl PixelFormat for Rgb565 {
    const BYTES_PER_PIXEL: usize = 2;
}

impl PixelFormat for Gray8 {
    const BYTES_PER_PIXEL: usize = 1;
}

/// The most `fill_buf_repeat` copies at once. Each copy reads from the start of `buf`, so a small
/// source stays in cache while a large destination, such as a PSRAM framebuffer, is written.
const REPEAT_BLOCK: usize = 256;

/// Writes `data` into `buf` `n` times over and returns the part written.
#[inline]
pub fn fill_buf_repeat<'b>(buf: &'b mut [u8], data: &[u8], n: usize) -> &'b mut [u8] {
    let bytes = data.len() * n;
    let buf = &mut buf[..bytes];
    if bytes == 0 {
        return buf;
    }
    buf[..data.len()].copy_from_slice(data);
    // Whole copies of `data`, so every copy lands in phase.
    let block = REPEAT_BLOCK.max(data.len()) / data.len() * data.len();
    let mut filled = data.len();
    while filled < bytes {
        let len = filled.min(block).min(bytes - filled);
        buf.copy_within(..len, filled);
        filled += len;
    }
    buf
}

#[repr(align(64))]
#[repr(C)]
#[derive(Clone)]
pub struct Framebuffer<const N: usize, const WIDTH: usize, const HEIGHT: usize, C: PixelFormat>
where
    <C as embedded_graphics::pixelcolor::raw::ToBytes>::Bytes: core::convert::AsRef<[u8]>,
{
    buf: [u8; N],
    color: PhantomData<C>,
}

/// Uses word stores for the aligned middle to fill 2-byte pixels faster.
fn fill_pairs(bytes: &mut [u8], pixel: [u8; 2]) {
    // SAFETY: every bit pattern is a valid `u32`, so viewing aligned bytes as words is sound.
    let (head, words, tail) = unsafe { bytes.align_to_mut::<u32>() };
    // `word` repeats the slice's bytes as they fall from the end of the head, so the head matches
    // its last bytes and the tail its first.
    let lead = if head.len() % 2 == 0 {
        pixel
    } else {
        [pixel[1], pixel[0]]
    };
    let word = [lead[0], lead[1], lead[0], lead[1]];
    head.copy_from_slice(&word[4 - head.len()..]);
    words.fill(u32::from_ne_bytes(word));
    tail.copy_from_slice(&word[..tail.len()]);
}

/// Calculates the required buffer size.
///
/// This function is a workaround for current limitations in Rust const generics.
/// It can be used to calculate the `N` parameter based on the size and color type of the framebuffer.
pub const fn buffer_size<C: PixelFormat>(width: usize, height: usize) -> usize
where
    <C as embedded_graphics::pixelcolor::raw::ToBytes>::Bytes: core::convert::AsRef<[u8]>,
{
    width * height * C::BYTES_PER_PIXEL
}

impl<const N: usize, const WIDTH: usize, const HEIGHT: usize, C: PixelFormat>
    Framebuffer<N, WIDTH, HEIGHT, C>
where
    <C as embedded_graphics::pixelcolor::raw::ToBytes>::Bytes: core::convert::AsRef<[u8]>,
{
    const BUFFER_SIZE: usize = buffer_size::<C>(WIDTH, HEIGHT);

    /// Static assertion that N is correct.
    // MSRV: remove N when constant generic expressions are stabilized
    const CHECK_N: () = assert!(
        N == Self::BUFFER_SIZE,
        "Invalid N: it must be equal to the output of buffer_size for the given width and height"
    );

    #[cfg(feature = "allocator-api")]
    #[must_use]
    pub fn alloc<A: core::alloc::Allocator>(alloc: A) -> Box<Self, A> {
        let _: () = Self::CHECK_N;
        // SAFETY: the fields are a byte array and a marker, so all zeroes is a valid value.
        unsafe { Box::new_zeroed_in(alloc).assume_init() }
    }

    /// A black framebuffer on the global heap.
    #[must_use]
    pub fn boxed() -> Box<Self> {
        let _: () = Self::CHECK_N;
        // SAFETY: the fields are a byte array and a marker, so all zeroes is a valid value.
        unsafe { Box::new_zeroed().assume_init() }
    }

    /// Set a single pixel
    ///
    /// PERF: no panic for speed?
    #[inline]
    fn set_pixel(&mut self, x: usize, y: usize, color: C) {
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
    fn fill_rect(&mut self, x: usize, y: usize, w: usize, h: usize, raw: &[u8]) {
        let x_end = (x + w).min(WIDTH);
        let y_end = (y + h).min(HEIGHT);
        for row in y..y_end {
            let start = row * WIDTH + x;
            let end = row * WIDTH + x_end;
            let bytes = &mut self.buf[start * raw.len()..end * raw.len()];
            if let &[first, second] = raw {
                fill_pairs(bytes, [first, second]);
            } else {
                fill_buf_repeat(bytes, raw, end - start);
            }
        }
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

impl<const N: usize, const WIDTH: usize, const HEIGHT: usize, C: PixelFormat> OriginDimensions
    for Framebuffer<N, WIDTH, HEIGHT, C>
where
    <C as embedded_graphics::pixelcolor::raw::ToBytes>::Bytes: core::convert::AsRef<[u8]>,
{
    fn size(&self) -> Size {
        Size::new(WIDTH as u32, HEIGHT as u32)
    }
}

impl<const N: usize, const WIDTH: usize, const HEIGHT: usize, C: PixelFormat> DrawTarget
    for Framebuffer<N, WIDTH, HEIGHT, C>
where
    <C as embedded_graphics::pixelcolor::raw::ToBytes>::Bytes: core::convert::AsRef<[u8]>,
{
    type Color = C;
    type Error = core::convert::Infallible;

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
        for_each_visible_color(
            Size::new(WIDTH as u32, HEIGHT as u32),
            *area,
            colors,
            |x, y, color| self.set_pixel(x, y, color),
        );
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

#[cfg(test)]
mod tests {
    use super::{fill_buf_repeat, fill_pairs};

    #[test]
    fn fill_pairs_preserves_pixel_phase_at_every_alignment() {
        let pixel = [0x12, 0x34];
        for prefix in 0..4 {
            for len in 0..40 {
                let mut storage = [0xa5; 44];
                let bytes = &mut storage[prefix..prefix + len];
                fill_pairs(bytes, pixel);

                for (index, byte) in bytes.iter().enumerate() {
                    assert_eq!(*byte, pixel[index % 2], "prefix={prefix}, len={len}");
                }
                assert!(storage[..prefix].iter().all(|&byte| byte == 0xa5));
                assert!(storage[prefix + len..].iter().all(|&byte| byte == 0xa5));
            }
        }
    }

    #[test]
    fn fill_buf_repeat_writes_only_the_repeats_into_a_longer_buffer() {
        for data in [&[7u8][..], &[1, 2], &[1, 2, 3], &[9, 8, 7, 6, 5]] {
            for n in [0, 1, 15, 16, 70, 300] {
                let mut buf = [0u8; 2048];
                let bytes = data.len() * n;
                let written = fill_buf_repeat(&mut buf, data, n);
                assert_eq!(written.len(), bytes, "data={data:?}, n={n}");
                for (index, byte) in buf[..bytes].iter().enumerate() {
                    assert_eq!(*byte, data[index % data.len()], "data={data:?}, n={n}");
                }
                assert!(
                    buf[bytes..].iter().all(|&byte| byte == 0),
                    "data={data:?}, n={n}"
                );
            }
        }
    }
}
