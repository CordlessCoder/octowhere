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

/// The most parameter bytes any command here sends.
pub const MAX_PARAMETERS: usize = 4;

/// The least room a [`Bus::stream`] lends its `fill`, which holds a pixel in every
/// [`ColorMode`].
pub const STREAM_ROOM: usize = 256;

/// Half-duplex QSPI writes under chip select, as the controller takes them: an 8-bit
/// instruction on one line, then a 24-bit address and the data on the transaction's
/// [`Lanes`].
#[expect(
    async_fn_in_trait,
    reason = "a display runs on one executor, and esp-hal's drivers are not Send"
)]
pub trait Bus {
    type Error;

    /// Writes `data`, at most [`MAX_PARAMETERS`] bytes, after `instruction` and `address`, as
    /// one transaction.
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

    /// Adds data to the open transaction: `fill` writes into the buffer it is lent, at least
    /// [`STREAM_ROOM`] bytes, and returns how many bytes it wrote, no more than that buffer
    /// holds. The bus may hold them back until its buffer fills.
    async fn stream(&mut self, fill: impl FnOnce(&mut [u8]) -> usize) -> Result<(), Self::Error>;

    /// Sends what the stream holds back and closes the transaction.
    async fn end(&mut self) -> Result<(), Self::Error>;

    /// Closes the open transaction at once, without what the stream holds back, so that the
    /// next transaction is not taken as part of it.
    fn abandon(&mut self);

    /// Reads `buffer.len()` bytes after `instruction` and `address`, all on one line. The
    /// controller takes a read's clock cycle no shorter than 100 ns, so at 10 MHz at most.
    async fn read(
        &mut self,
        instruction: u8,
        address: u32,
        buffer: &mut [u8],
    ) -> Result<(), Self::Error>;
}

/// A write whose command sits in the address's middle byte, on one line.
const WRITE: u8 = 0x02;
/// As [`WRITE`], with the address and data on four lines.
const WRITE_QUAD: u8 = 0x12;
/// A read of the command in the address's middle byte, on one line.
const READ: u8 = 0x03;

const CMD_SLPIN: u8 = 0x10;
const CMD_SLPOUT: u8 = 0x11;
const CMD_PTLON: u8 = 0x12;
const CMD_NORON: u8 = 0x13;
const CMD_INVOFF: u8 = 0x20;
const CMD_DISPOFF: u8 = 0x28;
const CMD_DISPON: u8 = 0x29;
const CMD_CASET: u8 = 0x2A;
const CMD_PASET: u8 = 0x2B;
const CMD_RAMWR: u8 = 0x2C;
const CMD_PTLAR: u8 = 0x30;
const CMD_PTLAR_H: u8 = 0x31;
const CMD_TEOFF: u8 = 0x34;
const CMD_TEON: u8 = 0x35;
const CMD_MADCTL: u8 = 0x36;
const CMD_IDMOFF: u8 = 0x38;
const CMD_IDMON: u8 = 0x39;
const CMD_PIXFMT: u8 = 0x3A;
const CMD_STESL: u8 = 0x44;
const CMD_GSL: u8 = 0x45;
const CMD_DSTBON: u8 = 0x4F;
const CMD_BRIGHTNESS: u8 = 0x51;
const CMD_WCTRLD1: u8 = 0x53;
const CMD_WRACL: u8 = 0x55;
const CMD_WCE: u8 = 0x58;
const CMD_BRIGHTNESS_HBM: u8 = 0x63;
const CMD_HBM: u8 = 0x66;
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

/// What the TE line pulses for.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TeMode {
    /// The vertical blanking alone.
    VBlank,
    /// The vertical and each horizontal blanking.
    VAndHBlank,
}

/// How strongly the controller raises the picture's contrast for sunlight.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Sunlight {
    Low = 0,
    Medium = 1,
    High = 2,
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

    /// The datasheet asks for one low pulse over 10 µs. The second, and the edge between them,
    /// come from the vendor's driver this one was first translated from.
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
        // The user command set, which every command here is from.
        self.command(CMD_PAGE, &[0x00]).await?;
        // Pixels written over SPI go to the frame memory (SPI_WRAM).
        self.command(CMD_SPIMODECTL, &[0x80]).await?;
        // The brightness block on (BCTRL), without dimming between levels. Without it the
        // brightness set is ignored.
        self.command(CMD_WCTRLD1, &[0x20]).await?;
        self.set_hbm_brightness(0xFF).await?;
        self.set_brightness(0).await?;
        self.command(CMD_PIXFMT, &[C::PIXFMT]).await?;
        self.command(CMD_DISPON, &[]).await?;
        self.set_sunlight(None).await?;
        self.command(CMD_MADCTL, &[MADCTL_RGB]).await?;
        self.delay.delay_ms(10).await;
        self.command(CMD_INVOFF, &[]).await?;
        self.te_on(TeMode::VBlank).await?;
        self.set_te_line(self.config.te_line).await
    }

    async fn command(&mut self, command: u8, parameters: &[u8]) -> Result<(), B::Error> {
        self.bus
            .write(WRITE, u32::from(command) << 8, Lanes::Single, parameters)
            .await
    }

    /// Sets the window the next pixels go into: `x, y, w, h` widened to the controller's 2 × 2
    /// grain and clipped to the panel, at least one cell. Returns that window, which the pixels
    /// must fill.
    pub async fn set_window(&mut self, x: u16, y: u16, w: u16, h: u16) -> Result<Window, B::Error> {
        let window = even_window(&self.config, x, y, w, h);
        let x_start = window.x + self.config.column_offset;
        let y_start = window.y + self.config.row_offset;
        let columns = [
            x_start.to_be_bytes(),
            (x_start + window.width - 1).to_be_bytes(),
        ];
        let rows = [
            y_start.to_be_bytes(),
            (y_start + window.height - 1).to_be_bytes(),
        ];
        self.command(CMD_CASET, columns.as_flattened()).await?;
        self.command(CMD_PASET, rows.as_flattened()).await?;
        Ok(window)
    }

    /// Starts the pixels for the window last set, from its top-left corner. They open with
    /// `RAMWR`, which resets the write position, so each run of pixels must follow its own
    /// [`set_window`](Self::set_window) rather than continue an earlier one.
    pub async fn pixels(&mut self) -> Result<Pixels<'_, B>, B::Error> {
        self.bus
            .begin(WRITE_QUAD, u32::from(CMD_RAMWR) << 8, Lanes::Quad)
            .await?;
        Ok(Pixels {
            bus: &mut self.bus,
            open: true,
        })
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

    /// Turns the TE line on. It does nothing while the line is already on, so a change of mode
    /// needs [`te_off`](Self::te_off) first.
    pub async fn te_on(&mut self, mode: TeMode) -> Result<(), B::Error> {
        let mode = match mode {
            TeMode::VBlank => 0x00,
            TeMode::VAndHBlank => 0x01,
        };
        self.command(CMD_TEON, &[mode]).await
    }

    /// Turns the TE line off, holding it low.
    pub async fn te_off(&mut self) -> Result<(), B::Error> {
        self.command(CMD_TEOFF, &[]).await
    }

    /// Has the TE line pulse once a frame, when the scan reaches `line`, from the frame after
    /// this one; 0 pulses for the vertical blanking instead.
    pub async fn set_te_line(&mut self, line: u16) -> Result<(), B::Error> {
        self.command(CMD_STESL, &line.to_be_bytes()).await
    }

    /// The scan line the controller is updating, counted from the first line of vertical sync.
    /// It means nothing while the controller sleeps.
    pub async fn scan_line(&mut self) -> Result<u16, B::Error> {
        let mut line = [0; 2];
        self.bus
            .read(READ, u32::from(CMD_GSL) << 8, &mut line)
            .await?;
        Ok(u16::from_be_bytes(line))
    }

    /// Sets the rows and columns [`partial_mode`](Self::partial_mode) shows, from `first` to
    /// `last` inclusive, in panel coordinates. A `last` before `first` wraps round the panel.
    pub async fn set_partial_area(
        &mut self,
        (first_column, last_column): (u16, u16),
        (first_row, last_row): (u16, u16),
    ) -> Result<(), B::Error> {
        let (columns, rows) = (self.config.column_offset, self.config.row_offset);
        let columns = [
            (first_column + columns).to_be_bytes(),
            (last_column + columns).to_be_bytes(),
        ];
        let rows = [
            (first_row + rows).to_be_bytes(),
            (last_row + rows).to_be_bytes(),
        ];
        self.command(CMD_PTLAR_H, columns.as_flattened()).await?;
        self.command(CMD_PTLAR, rows.as_flattened()).await
    }

    /// Shows only the partial area, leaving the rest of the panel dark.
    pub async fn partial_mode(&mut self) -> Result<(), B::Error> {
        self.command(CMD_PTLON, &[]).await
    }

    /// Shows the whole panel again after [`partial_mode`](Self::partial_mode).
    pub async fn normal_mode(&mut self) -> Result<(), B::Error> {
        self.command(CMD_NORON, &[]).await
    }

    /// In idle mode the panel shows eight colours, from the top bit of each channel.
    pub async fn set_idle(&mut self, idle: bool) -> Result<(), B::Error> {
        self.command(if idle { CMD_IDMON } else { CMD_IDMOFF }, &[])
            .await
    }

    /// Puts the controller in deep standby, where it holds nothing. Only
    /// [`leave_deep_standby`](Self::leave_deep_standby) brings it back.
    pub async fn deep_standby(&mut self) -> Result<(), B::Error> {
        self.command(CMD_DSTBON, &[0x01]).await
    }

    /// Brings the controller out of deep standby by its reset line and starts it again, dark,
    /// as [`new`](Self::new) does. The picture has to be sent again.
    pub async fn leave_deep_standby(&mut self) -> Result<(), Error<B::Error, RST::Error>> {
        self.hardware_reset().await.map_err(Error::Reset)?;
        self.start().await.map_err(Error::Bus)
    }

    /// Turns high-brightness mode on or off. While it is on, the panel takes its level from
    /// [`set_hbm_brightness`](Self::set_hbm_brightness).
    pub async fn set_hbm(&mut self, on: bool) -> Result<(), B::Error> {
        self.command(CMD_HBM, &[if on { 0x02 } else { 0x00 }]).await
    }

    /// Sets the level high-brightness mode shows, from 0 to 255. The start-up sets 255.
    pub async fn set_hbm_brightness(&mut self, level: u8) -> Result<(), B::Error> {
        self.command(CMD_BRIGHTNESS_HBM, &[level]).await
    }

    /// Raises the picture's contrast for sunlight, or with `None` stops, as the start-up leaves
    /// it.
    pub async fn set_sunlight(&mut self, level: Option<Sunlight>) -> Result<(), B::Error> {
        let value = level.map_or(0x00, |level| 0x04 | level as u8);
        self.command(CMD_WCE, &[value]).await
    }

    /// Turns the controller's automatic current limit on or off.
    pub async fn set_current_limit(&mut self, on: bool) -> Result<(), B::Error> {
        self.command(CMD_WRACL, &[if on { 0x03 } else { 0x00 }])
            .await
    }
}

/// A window of the panel, in panel coordinates.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Window {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

/// Pixels going into the window last set. [`finish`](Self::finish) sends the last of them and
/// closes the transaction; dropped before it, they close it without what the bus held back.
#[must_use]
pub struct Pixels<'a, B: Bus> {
    bus: &'a mut B,
    open: bool,
}

impl<B: Bus> Pixels<'_, B> {
    /// Adds the bytes `fill` writes into the buffer it is lent, and returns how many it wrote.
    pub async fn fill(&mut self, fill: impl FnOnce(&mut [u8]) -> usize) -> Result<(), B::Error> {
        self.bus.stream(fill).await
    }

    pub async fn finish(mut self) -> Result<(), B::Error> {
        let ended = self.bus.end().await;
        self.open = false;
        ended
    }
}

impl<B: Bus> Drop for Pixels<'_, B> {
    fn drop(&mut self) {
        if self.open {
            self.bus.abandon();
        }
    }
}

/// The window `x, y, w, h` on the 2 × 2 grain: its corner down to even, its far edges up to
/// even, clipped to the panel and at least 2 × 2.
fn even_window(config: &Config, x: u16, y: u16, w: u16, h: u16) -> Window {
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
    Window {
        x: x0 as u16,
        y: y0 as u16,
        width: (x1 - x0) as u16,
        height: (y1 - y0) as u16,
    }
}

#[cfg(test)]
mod tests;
