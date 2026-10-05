//! A driver for the CO5300 AMOLED controller over QSPI: its start-up, the window pixels go
//! into, the brightness, sleep and the tearing-effect (TE) line. It is generic over the bus,
//! the reset and TE pins and a delay, so it builds and tests on the host.

#![cfg_attr(not(test), no_std)]

use core::marker::PhantomData;

use embedded_graphics_core::pixelcolor::{Gray8, Rgb565, Rgb888};
use embedded_hal::digital::OutputPin;
use embedded_hal_async::{delay::DelayNs, digital::Wait};

/// The lines a transaction's address and data go on. Its instruction always goes on one.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Lanes {
    Single,
    Quad,
}

/// Half-duplex QSPI writes under chip select, as the controller takes them: an 8-bit
/// instruction on one line, then a 24-bit address and the data on the transaction's
/// [`Lanes`].
#[expect(
    async_fn_in_trait,
    reason = "a display runs on one executor, and esp-hal's drivers are not Send"
)]
pub trait Bus {
    type Error;

    /// Writes `data` after `instruction` and `address`, as one transaction.
    async fn write(
        &mut self,
        instruction: u8,
        address: u32,
        lanes: Lanes,
        data: &[u8],
    ) -> Result<(), Self::Error>;

    /// Opens a transaction with `instruction` and `address` and no data yet. Its data goes
    /// through [`stream`](Self::stream), on the same lanes, until [`end`](Self::end) closes it.
    async fn begin(
        &mut self,
        instruction: u8,
        address: u32,
        lanes: Lanes,
    ) -> Result<(), Self::Error>;

    /// Adds data to the open transaction: `fill` writes into the buffer it is lent and returns
    /// how many bytes it wrote. The bus may hold them back until its buffer fills.
    async fn stream(&mut self, fill: impl FnOnce(&mut [u8]) -> usize) -> Result<(), Self::Error>;

    /// Sends what the stream holds back and closes the transaction.
    async fn end(&mut self) -> Result<(), Self::Error>;
}

/// A write whose command sits in the address's middle byte, on one line.
const WRITE: u8 = 0x02;
/// As [`WRITE`], with the address and data on four lines.
const WRITE_QUAD: u8 = 0x12;

const CMD_SLPIN: u8 = 0x10;
const CMD_SLPOUT: u8 = 0x11;
const CMD_INVOFF: u8 = 0x20;
const CMD_DISPOFF: u8 = 0x28;
const CMD_DISPON: u8 = 0x29;
const CMD_CASET: u8 = 0x2A;
const CMD_PASET: u8 = 0x2B;
const CMD_RAMWR: u8 = 0x2C;
const CMD_TEON: u8 = 0x35;
const CMD_MADCTL: u8 = 0x36;
const CMD_PIXFMT: u8 = 0x3A;
const CMD_STESL: u8 = 0x44;
const CMD_BRIGHTNESS: u8 = 0x51;
const CMD_WCTRLD1: u8 = 0x53;
const CMD_WCE: u8 = 0x58;
const CMD_BRIGHTNESS_HBM: u8 = 0x63;
const CMD_SPIMODECTL: u8 = 0xC4;
const CMD_PAGE: u8 = 0xFE;

const MADCTL_RGB: u8 = 0x00;

const RESET_MS: u32 = 120;
const SLPOUT_MS: u32 = 120;
const SLPIN_MS: u32 = 120;
const DISPLAY_MS: u32 = 20;

/// The panel the controller drives.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Config {
    /// The panel's size in pixels, which windows are clipped to.
    pub width: u16,
    pub height: u16,
    /// Where the panel's first column and row sit in the controller's memory.
    pub column_offset: u16,
    pub row_offset: u16,
    /// The scan line the TE line pulses at, counted from the first line of vertical sync.
    pub te_line: u16,
}

mod sealed {
    pub trait Sealed {}
}

/// A pixel format the controller takes, with its `COLMOD` value.
pub trait ColorMode: sealed::Sealed {
    const PIXFMT: u8;
}

impl sealed::Sealed for Rgb888 {}
impl ColorMode for Rgb888 {
    const PIXFMT: u8 = 0x77;
}

impl sealed::Sealed for Rgb565 {}
impl ColorMode for Rgb565 {
    const PIXFMT: u8 = 0x55;
}

impl sealed::Sealed for Gray8 {}
impl ColorMode for Gray8 {
    const PIXFMT: u8 = 0x11;
}

#[derive(Debug, Eq, PartialEq)]
pub enum Error<B, P> {
    Bus(B),
    Reset(P),
}

/// The controller, with pixels in format `C`.
pub struct Co5300<B, RST, TE, D, C> {
    bus: B,
    reset: RST,
    te: TE,
    delay: D,
    config: Config,
    color: PhantomData<C>,
}

impl<B: Bus, RST: OutputPin, TE: Wait, D: DelayNs, C: ColorMode> Co5300<B, RST, TE, D, C> {
    /// Resets the controller and starts it, with the panel on but dark: the brightness is 0
    /// until [`set_brightness`](Self::set_brightness), so a first frame can go in unseen.
    pub async fn new(
        bus: B,
        reset: RST,
        te: TE,
        delay: D,
        config: Config,
    ) -> Result<Self, Error<B::Error, RST::Error>> {
        let mut display = Self {
            bus,
            reset,
            te,
            delay,
            config,
            color: PhantomData,
        };
        display.hardware_reset().await.map_err(Error::Reset)?;
        display.start().await.map_err(Error::Bus)?;
        Ok(display)
    }

    async fn hardware_reset(&mut self) -> Result<(), RST::Error> {
        self.reset.set_low()?;
        self.delay.delay_ms(10).await;
        self.reset.set_high()?;
        self.reset.set_low()?;
        self.delay.delay_us(10).await;
        self.reset.set_high()?;
        self.delay.delay_ms(RESET_MS).await;
        Ok(())
    }

    async fn start(&mut self) -> Result<(), B::Error> {
        self.command(CMD_SLPOUT, &[]).await?;
        self.delay.delay_ms(SLPOUT_MS).await;
        self.command(CMD_PAGE, &[0x00]).await?;
        self.command(CMD_SPIMODECTL, &[0x80]).await?;
        self.command(CMD_WCTRLD1, &[0x20]).await?;
        self.command(CMD_BRIGHTNESS_HBM, &[0xFF]).await?;
        self.command(CMD_BRIGHTNESS, &[0x00]).await?;
        self.command(CMD_PIXFMT, &[C::PIXFMT]).await?;
        self.command(CMD_DISPON, &[]).await?;
        self.command(CMD_WCE, &[0x00]).await?;
        self.command(CMD_MADCTL, &[MADCTL_RGB]).await?;
        self.delay.delay_ms(10).await;
        self.command(CMD_INVOFF, &[]).await?;
        // A pulse at the scan line rather than a level through the blanking.
        self.command(CMD_TEON, &[0x00]).await?;
        self.command(CMD_STESL, &self.config.te_line.to_be_bytes())
            .await
    }

    async fn command(&mut self, command: u8, parameters: &[u8]) -> Result<(), B::Error> {
        self.bus
            .write(WRITE, u32::from(command) << 8, Lanes::Single, parameters)
            .await
    }

    /// Sets the window the next pixels go into, widened to the controller's 2 × 2 grain and
    /// clipped to the panel.
    pub async fn set_window(&mut self, x: u16, y: u16, w: u16, h: u16) -> Result<(), B::Error> {
        let (x, y, w, h) = even_window(&self.config, x, y, w, h);
        let x_start = x + self.config.column_offset;
        let y_start = y + self.config.row_offset;
        let columns = [x_start.to_be_bytes(), (x_start + w - 1).to_be_bytes()];
        let rows = [y_start.to_be_bytes(), (y_start + h - 1).to_be_bytes()];
        self.command(CMD_CASET, columns.as_flattened()).await?;
        self.command(CMD_PASET, rows.as_flattened()).await
    }

    /// Starts the pixels for the window last set, from its top-left corner. They open with
    /// `RAMWR`, which resets the write position, so each run of pixels must follow its own
    /// [`set_window`](Self::set_window) rather than continue an earlier one.
    pub async fn pixels(&mut self) -> Result<Pixels<'_, B>, B::Error> {
        self.bus
            .begin(WRITE_QUAD, u32::from(CMD_RAMWR) << 8, Lanes::Quad)
            .await?;
        Ok(Pixels { bus: &mut self.bus })
    }

    /// Sets the brightness, from 0, dark, to 255.
    pub async fn set_brightness(&mut self, level: u8) -> Result<(), B::Error> {
        self.command(CMD_BRIGHTNESS, &[level]).await
    }

    /// Takes the controller out of sleep and turns the panel on.
    pub async fn display_on(&mut self) -> Result<(), B::Error> {
        self.command(CMD_SLPOUT, &[]).await?;
        self.delay.delay_ms(SLPOUT_MS).await;
        self.command(CMD_DISPON, &[]).await?;
        self.delay.delay_ms(DISPLAY_MS).await;
        Ok(())
    }

    /// Turns the panel off and puts the controller to sleep.
    pub async fn display_off(&mut self) -> Result<(), B::Error> {
        self.command(CMD_DISPOFF, &[]).await?;
        self.delay.delay_ms(DISPLAY_MS).await;
        self.command(CMD_SLPIN, &[]).await?;
        self.delay.delay_ms(SLPIN_MS).await;
        Ok(())
    }

    /// Waits for the TE pulse, when the scan reaches [`Config::te_line`]. It waits for the
    /// edge: a write started later in the pulse, or after it, has less of the lead.
    pub fn wait_for_te(&mut self) -> impl Future<Output = Result<(), TE::Error>> {
        self.te.wait_for_rising_edge()
    }
}

/// Pixels going into the window last set. [`finish`](Self::finish) must close them: until it
/// does, the bus holds the transaction open.
#[must_use]
pub struct Pixels<'a, B: Bus> {
    bus: &'a mut B,
}

impl<B: Bus> Pixels<'_, B> {
    /// Adds the bytes `fill` writes into the buffer it is lent, and returns how many it wrote.
    pub async fn fill(&mut self, fill: impl FnOnce(&mut [u8]) -> usize) -> Result<(), B::Error> {
        self.bus.stream(fill).await
    }

    pub async fn finish(self) -> Result<(), B::Error> {
        self.bus.end().await
    }
}

/// The window `x, y, w, h` on the 2 × 2 grain: its corner down to even, its far edges up to
/// even, clipped to the panel and at least 2 × 2.
fn even_window(config: &Config, x: u16, y: u16, w: u16, h: u16) -> (u16, u16, u16, u16) {
    let (width, height) = (usize::from(config.width), usize::from(config.height));
    let x0 = usize::from(x).min(width.saturating_sub(1)) & !1;
    let y0 = usize::from(y).min(height.saturating_sub(1)) & !1;
    let mut x1 = (usize::from(x) + usize::from(w)).min(width);
    let mut y1 = (usize::from(y) + usize::from(h)).min(height);
    if x1 & 1 != 0 && x1 < width {
        x1 += 1;
    }
    if y1 & 1 != 0 && y1 < height {
        y1 += 1;
    }
    if x1 <= x0 {
        x1 = (x0 + 2).min(width);
    }
    if y1 <= y0 {
        y1 = (y0 + 2).min(height);
    }
    (x0 as u16, y0 as u16, (x1 - x0) as u16, (y1 - y0) as u16)
}

#[cfg(test)]
mod tests;
