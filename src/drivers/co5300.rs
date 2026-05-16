// CO5300 AMOLED display driver
// Translated from Arduino_CO5300.h/.cpp
// Resolution: 410x502, col_offset=22, RGB565

use core::marker::PhantomData;

use embedded_graphics::pixelcolor::raw::ToBytes;
use embedded_graphics::pixelcolor::{Gray8, Rgb565, Rgb888};
use embedded_graphics_core::geometry::{OriginDimensions, Size};
use embedded_graphics_core::prelude::*;

use esp_hal::dma::DmaTxBuf;
use esp_hal::gpio::{Input, Output};
use esp_hal::spi::master::{Address, Command, DataMode};

use crate::board::{self, delay_ms, delay_ms_async, delay_us, delay_us_async};
use crate::chrome::RgbColorExt;
use crate::drivers::qspi_bus::{QSPIOperation, QspiBus};
use crate::util::fill_buf_repeat;

pub const X_OFFS: u16 = 6;
pub const Y_OFFS: u16 = 0;

// CO5300 commands (from Arduino_CO5300.h)
// const CMD_SWRESET: u8 = 0x01;
const CMD_SLPOUT: u8 = 0x11;
const CMD_SLPIN: u8 = 0x10;
const CMD_INVOFF: u8 = 0x20;
// const CMD_INVON: u8 = 0x21;
const CMD_DISPOFF: u8 = 0x28;
const CMD_DISPON: u8 = 0x29;
const CMD_CASET: u8 = 0x2A;
const CMD_PASET: u8 = 0x2B;
const CMD_RAMWR: u8 = 0x2C;
const CMD_MADCTL: u8 = 0x36;
const CMD_PIXFMT: u8 = 0x3A;
const CMD_SPIMODECTL: u8 = 0xC4;
const CMD_WCTRLD1: u8 = 0x53;
const CMD_BRIGHTNESS: u8 = 0x51;
const CMD_BRIGHTNESS_HBM: u8 = 0x63;
const CMD_WCE: u8 = 0x58;
// const CMD_ALLPOFF: u8 = 0x22; // All pixels off
// const CMD_ALLPON: u8 = 0x23; // All pixels on

// MADCTL flags
const MADCTL_RGB: u8 = 0x00;

// Delays
const RST_DELAY_MS: u32 = 120;
const SLPOUT_DELAY_MS: u32 = 120;
const SLPIN_DELAY_MS: u32 = 120;

pub struct Co5300Display<'d, C> {
    pub bus: QspiBus<'d>,
    reset: Output<'d>,
    width: u16,
    height: u16,
    col_offset: u16,
    row_offset: u16,
    pub te_pin: Input<'d>,
    color: PhantomData<C>,
    swap: DmaTxBuf,
}

#[derive(Debug)]
pub enum DisplayError {
    BusError,
}

/// Turn display on (exit sleep + display ON).
/// MIPI DCS order: SLPOUT -> 120ms -> DISPON -> 20ms.
static CO5300_EXIT_SLEEP: [QSPIOperation; 4] = [
    QSPIOperation::Command(CMD_DISPON),
    QSPIOperation::Delay(20),
    QSPIOperation::Command(CMD_SLPOUT),
    QSPIOperation::Delay(SLPOUT_DELAY_MS),
];
static CO5300_ENTER_SLEEP: [QSPIOperation; 4] = [
    QSPIOperation::Command(CMD_DISPOFF),
    QSPIOperation::Delay(20),
    QSPIOperation::Command(CMD_SLPIN),
    QSPIOperation::Delay(SLPIN_DELAY_MS),
];

// // 16-bit RGB565
// QSPIOperation::CommandD8(CMD_PIXFMT, 0x55),
// // // 24-bit RGB888
// // QSPIOperation::CommandD8(CMD_PIXFMT, 0x77),

static CO5300_INIT: [QSPIOperation; 13] = [
    QSPIOperation::Command(CMD_SLPOUT),
    QSPIOperation::Delay(SLPOUT_DELAY_MS),
    // Set command page 0
    QSPIOperation::CommandD8(0xFE, 0x00),
    // SPI mode control
    QSPIOperation::CommandD8(CMD_SPIMODECTL, 0x80),
    // Write CTRL Display Brightness
    QSPIOperation::CommandD8(CMD_WCTRLD1, 0x20),
    // High Brightness Mode max
    QSPIOperation::CommandD8(CMD_BRIGHTNESS_HBM, 0xFF),
    // Brightness 80%
    QSPIOperation::CommandD8(CMD_BRIGHTNESS, 0xD0),
    // Display on
    QSPIOperation::Command(CMD_DISPON),
    // Contrast enhancement off
    QSPIOperation::CommandD8(CMD_WCE, 0x00),
    // Set MADCTL for correct color order (RGB, no rotation)
    QSPIOperation::CommandD8(CMD_MADCTL, MADCTL_RGB),
    QSPIOperation::Delay(10),
    // Inversion off (standard for this panel)
    QSPIOperation::Command(CMD_INVOFF),
    // Enable Tearing Effect output on CO5300 (TE pin = GPIO13)
    // Command 0x35 = TEARON, param 0x00 = VBlank only
    QSPIOperation::CommandD8(0x35, 0x00),
];

mod sealed {
    pub trait Sealed {}
}

pub trait Co5300ColorMode: ToBytes + PixelColor + sealed::Sealed + RgbColorExt
where
    Self::Bytes: AsRef<[u8]>,
{
    const BYTES_PER_PIXEL: usize;
    const MODE_BYTE: u8;
}

impl sealed::Sealed for Rgb888 {}

impl Co5300ColorMode for Rgb888 {
    const MODE_BYTE: u8 = 0x77;
    const BYTES_PER_PIXEL: usize = 3;
}

impl sealed::Sealed for Rgb565 {}

impl Co5300ColorMode for Rgb565 {
    const MODE_BYTE: u8 = 0x55;
    const BYTES_PER_PIXEL: usize = 2;
}

impl sealed::Sealed for Gray8 {}

impl Co5300ColorMode for Gray8 {
    const MODE_BYTE: u8 = 0x11;
    const BYTES_PER_PIXEL: usize = 1;
}

impl<'d, C: Co5300ColorMode> Co5300Display<'d, C>
where
    <C as embedded_graphics::pixelcolor::raw::ToBytes>::Bytes: core::convert::AsRef<[u8]>,
{
    pub async fn new(
        bus: QspiBus<'d>,
        reset: Output<'d>,
        te_pin: Input<'d>,
        swap: DmaTxBuf,
    ) -> Self {
        let mut disp = Self {
            bus,
            reset,
            te_pin,
            width: board::LCD_WIDTH,
            height: board::LCD_HEIGHT,
            col_offset: board::LCD_COL_OFFSET,
            row_offset: board::LCD_ROW_OFFSET,
            swap,
            color: PhantomData,
        };
        disp.hw_reset_async().await;
        disp.bus.batch_async(&CO5300_INIT).await;
        disp.apply_color();

        disp
    }
    fn apply_color(&mut self) {
        self.bus
            .execute(&QSPIOperation::CommandD8(CMD_PIXFMT, C::MODE_BYTE));
    }
    pub fn with_color_format<NC>(self) -> Co5300Display<'d, NC>
    where
        NC: Co5300ColorMode,
        <NC as embedded_graphics::pixelcolor::raw::ToBytes>::Bytes: core::convert::AsRef<[u8]>,
    {
        let Self {
            bus,
            reset,
            width,
            height,
            col_offset,
            row_offset,
            te_pin,
            color: _,
            swap,
        } = self;
        let mut new = Co5300Display {
            bus,
            reset,
            width,
            height,
            col_offset,
            row_offset,
            te_pin,
            swap,
            color: PhantomData,
        };
        new.apply_color();
        new
    }

    pub async fn hw_reset_async(&mut self) {
        // Hardware reset
        self.reset.set_high();
        delay_us_async(10).await;
        self.reset.set_low();
        delay_us_async(10).await;
        self.reset.set_high();
        delay_ms_async(10).await;
    }

    pub fn hw_reset(&mut self) {
        // Hardware reset
        self.reset.set_high();
        delay_us(10);
        self.reset.set_low();
        delay_us(10);
        self.reset.set_high();
        delay_ms(RST_DELAY_MS);
    }

    /// Set the address window for pixel writes.
    pub fn set_addr_window(&mut self, x: u16, y: u16, w: u16, h: u16) {
        let x_start = x + self.col_offset;
        let x_end = x_start + w - 1;
        let y_start = y + self.row_offset;
        let y_end = y_start + h - 1;

        self.bus.batch(&[
            QSPIOperation::CommandD16D16(CMD_CASET, x_start, x_end),
            QSPIOperation::CommandD16D16(CMD_PASET, y_start, y_end),
            QSPIOperation::Command(CMD_RAMWR),
        ]);
    }

    /// Fill the entire screen with a single color.
    pub fn fill_screen(&mut self, color: C) {
        let raw = color.to_be_bytes();
        self.set_addr_window(0, 0, self.width, self.height);
        let total = self.width as usize * self.height as usize;
        self.write_repeat(raw.as_ref(), total);
    }

    pub fn write_repeat(&mut self, data: &[u8], count: usize) {
        if count == 0 {
            return;
        }
        let mut stream = self.begin_stream();
        let buf = stream.buf_remaining();
        let chunk_size = (buf.len() / data.len()).min(count);
        assert!(chunk_size > 0);
        fill_buf_repeat(buf, data, chunk_size);
        let mut remaining = count;
        while remaining > 0 {
            let n = remaining.min(chunk_size);
            let bytes = n * data.len();

            stream.write(bytes);
            stream.flush_buf();

            remaining -= n;
        }
        stream.end();
    }

    /// Fill a rectangular area with a solid color.
    pub fn write_pixels_area(&mut self, x: u16, y: u16, w: u16, h: u16, color: C) {
        if w == 0 || h == 0 {
            return;
        };
        let bytes = color.to_be_bytes();
        self.set_addr_window(x, y, w, h);
        self.write_repeat(bytes.as_ref(), w as usize * h as usize);
    }

    pub fn wait_for_vsync(&mut self) -> impl Future<Output = ()> {
        self.te_pin.wait_for_high()
    }

    /// Set display brightness (0x00 = off, 0xD0 = default, 0xFF = max).
    pub fn set_brightness(&mut self, brightness: u8) {
        self.bus
            .execute(&QSPIOperation::CommandD8(CMD_BRIGHTNESS, brightness));
    }

    /// Turn display on (exit sleep + display ON).
    /// MIPI DCS order: SLPOUT -> 120ms -> DISPON -> 20ms.
    pub fn display_on(&mut self) -> impl Future<Output = ()> {
        self.bus.batch_async(&CO5300_EXIT_SLEEP)
    }

    /// Turn display off (DISPOFF + enter sleep).
    /// MIPI DCS order: DISPOFF -> 20ms -> SLPIN -> 120ms.
    pub fn display_off(&mut self) -> impl Future<Output = ()> {
        self.bus.batch_async(&CO5300_ENTER_SLEEP)
    }

    pub async fn begin_stream_async<'r>(&'r mut self) -> PixelStream<'r, 'd, C> {
        self.bus.cs.set_low();
        self.bus
            .spi
            .half_duplex_write_and_wait(
                DataMode::Quad,
                Command::_8Bit(0x12, DataMode::Single),
                Address::_24Bit(0x003C00, DataMode::Quad),
                0,
                0,
                &mut self.bus.tx,
            )
            .await
            .unwrap();
        PixelStream {
            disp: self,
            buffered: 0,
        }
    }
    pub fn begin_stream<'r>(&'r mut self) -> PixelStream<'r, 'd, C> {
        self.bus.cs.set_low();
        self.bus
            .spi
            .half_duplex_write_and_block(
                DataMode::Quad,
                Command::_8Bit(0x12, DataMode::Single),
                Address::_24Bit(0x003C00, DataMode::Quad),
                0,
                0,
                &mut self.bus.tx,
            )
            .unwrap();
        PixelStream {
            disp: self,
            buffered: 0,
        }
    }
}

pub struct PixelStream<'r, 'd, C: Co5300ColorMode>
where
    <C as embedded_graphics::pixelcolor::raw::ToBytes>::Bytes: core::convert::AsRef<[u8]>,
{
    pub(crate) disp: &'r mut Co5300Display<'d, C>,
    pub(crate) buffered: usize,
}

impl<C: Co5300ColorMode> PixelStream<'_, '_, C>
where
    <C as embedded_graphics::pixelcolor::raw::ToBytes>::Bytes: core::convert::AsRef<[u8]>,
{
    pub async fn flush_if_needed_and_get_buf_async(
        &mut self,
        fill_buf: impl FnOnce(&mut [u8]) -> usize,
    ) {
        if self.should_flush() {
            self.flush_buf_async(fill_buf).await;
            return;
        }
        let new = fill_buf(self.buf_remaining());
        self.write(new);
    }
    pub fn flush_if_needed_and_get_buf(&mut self) -> &mut [u8] {
        if self.should_flush() {
            self.flush_buf();
        }
        self.buf_remaining()
    }
    pub fn buf_remaining(&mut self) -> &mut [u8] {
        &mut self.disp.bus.tx.as_mut_slice()[self.buffered..]
    }
    #[inline(always)]
    pub fn write(&mut self, bytes: usize) {
        self.buffered += bytes;
    }
    #[inline(always)]
    pub fn buffered(&self) -> usize {
        self.buffered
    }
    #[inline(always)]
    pub fn should_flush(&self) -> bool {
        self.buffered > self.disp.bus.tx.len().saturating_sub(256)
    }
    pub async fn flush_buf_async(&mut self, fill_swap_with: impl FnOnce(&mut [u8]) -> usize) {
        if self.buffered == 0 {
            self.buffered = fill_swap_with(self.buf_remaining());
            return;
        }
        let Self { disp, buffered } = self;
        let mut new = 0;
        embassy_futures::join::join(
            disp.bus.spi.half_duplex_write_and_wait(
                DataMode::Quad,
                Command::None,
                Address::None,
                0,
                *buffered,
                &mut disp.bus.tx,
            ),
            async {
                let buf = disp.swap.as_mut_slice();
                new = fill_swap_with(buf);
            },
        )
        .await
        .0
        .unwrap();
        core::mem::swap(&mut self.disp.swap, &mut self.disp.bus.tx);
        self.buffered = new;
    }
    pub fn flush_buf(&mut self) {
        if self.buffered == 0 {
            return;
        }
        self.disp
            .bus
            .spi
            .half_duplex_write_and_block(
                DataMode::Quad,
                Command::None,
                Address::None,
                0,
                self.buffered,
                &mut self.disp.bus.tx,
            )
            .unwrap();
        self.buffered = 0;
    }
    pub fn end(mut self) {
        self.flush_buf();
    }
}

impl<C: Co5300ColorMode> Drop for PixelStream<'_, '_, C>
where
    <C as embedded_graphics::pixelcolor::raw::ToBytes>::Bytes: core::convert::AsRef<[u8]>,
{
    fn drop(&mut self) {
        self.disp.bus.cs.set_high();
    }
}

impl<C: Co5300ColorMode> OriginDimensions for Co5300Display<'_, C>
where
    <C as embedded_graphics::pixelcolor::raw::ToBytes>::Bytes: core::convert::AsRef<[u8]>,
{
    fn size(&self) -> Size {
        Size::new(self.width as u32, self.height as u32)
    }
}

// impl<C: Co5300ColorMode + PixelColor> DrawTarget for Co5300Display<'_, C>
// where
//     <C as embedded_graphics::pixelcolor::raw::ToBytes>::Bytes: core::convert::AsRef<[u8]>,
// {
//     type Color = C;
//     type Error = DisplayError;
//
//     fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
//     where
//         I: IntoIterator<Item = Pixel<Self::Color>>,
//     {
//         // CO5300 requires minimum 2x2 pixel writes.
//         // Draw each pixel as a 2x2 block.
//         for Pixel(coord, color) in pixels.into_iter() {
//             if coord.x >= 0
//                 && coord.x < self.width as i32
//                 && coord.y >= 0
//                 && coord.y < self.height as i32
//             {
//                 // Write 2x2 block (4 pixels)
//                 self.write_pixels_area(coord.x as u16, coord.y as u16, 2, 2, color);
//             }
//         }
//         Ok(())
//     }
//
//     fn fill_contiguous<I>(&mut self, area: &Rectangle, colors: I) -> Result<(), Self::Error>
//     where
//         I: IntoIterator<Item = Self::Color>,
//     {
//         let area = area.intersection(&Rectangle::new(
//             Point::zero(),
//             Size::new(self.width as u32, self.height as u32),
//         ));
//
//         if area.size.width == 0 || area.size.height == 0 {
//             return Ok(());
//         }
//
//         self.set_addr_window(
//             area.top_left.x as u16,
//             area.top_left.y as u16,
//             area.size.width as u16,
//             area.size.height as u16,
//         );
//
//         let mut stream = self.begin_stream();
//         for color in colors.into_iter() {
//             let raw = color.to_be_bytes();
//             let raw = raw.as_ref();
//             stream.flush_if_needed_and_get_buf()[..C::BYTES_PER_PIXEL].copy_from_slice(raw);
//             stream.write(C::BYTES_PER_PIXEL);
//         }
//         stream.flush_buf();
//         stream.end();
//
//         Ok(())
//     }
//
//     fn fill_solid(&mut self, area: &Rectangle, color: Self::Color) -> Result<(), Self::Error> {
//         let area = area.intersection(&Rectangle::new(
//             Point::zero(),
//             Size::new(self.width as u32, self.height as u32),
//         ));
//
//         self.write_pixels_area(
//             area.top_left.x as u16,
//             area.top_left.y as u16,
//             area.size.width as u16,
//             area.size.height as u16,
//             color,
//         );
//         Ok(())
//     }
// }
