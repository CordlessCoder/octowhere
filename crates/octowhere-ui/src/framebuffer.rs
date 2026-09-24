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

const CHUNK_SIZE: usize = 32;

/// Writes `data` into `buf` `n` times over and returns the part written.
#[inline]
pub fn fill_buf_repeat<'b>(mut buf: &'b mut [u8], data: &[u8], mut n: usize) -> &'b mut [u8] {
    if n == 0 {
        return &mut [];
    }
    let bytes = data.len() * n;
    assert!(buf.len() >= bytes);
    if bytes >= 4 * CHUNK_SIZE && CHUNK_SIZE.is_multiple_of(data.len()) {
        let mut arr = [0; CHUNK_SIZE];
        arr.chunks_exact_mut(data.len())
            .for_each(|chunk| chunk.copy_from_slice(data));
        let mut chunks = buf.chunks_exact_mut(arr.len());
        chunks
            .by_ref()
            .take(n / (CHUNK_SIZE / data.len()))
            .for_each(|chunk| {
                chunk.copy_from_slice(&arr);
            });
        n %= CHUNK_SIZE / data.len();
        buf = chunks.into_remainder();
    }
    buf.chunks_exact_mut(data.len()).take(n).for_each(|chunk| {
        chunk.copy_from_slice(data);
    });

    &mut buf[..data.len() * n]
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

/// Fills `bytes` with a repeated 2-byte pixel in whole words. A row that starts at an arbitrary
/// pixel is not word-aligned, and a byte-wise copy into it runs well below the word rate.
fn fill_pairs(bytes: &mut [u8], pixel: [u8; 2]) {
    // SAFETY: every bit pattern is a valid `u32`, so viewing aligned bytes as words is sound.
    let (head, words, tail) = unsafe { bytes.align_to_mut::<u32>() };
    let word = u32::from_ne_bytes([pixel[0], pixel[1], pixel[0], pixel[1]]);
    // The buffer is word-aligned and pixels are 2 bytes, so the ends are whole pixels.
    for end in [head, tail] {
        end.as_chunks_mut::<2>().0.fill(pixel);
    }
    words.fill(word);
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
    const PIXEL_COUNT: usize = WIDTH * HEIGHT;
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
