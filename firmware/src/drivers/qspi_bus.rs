// The CO5300's QSPI bus on esp-hal's SPI DMA, streaming pixels through two DMA buffers.

use co5300::{Bus, Lanes};
use esp_hal::Async;
use esp_hal::dma::{DmaRxBuf, DmaTxBuf, EmptyBuf};
use esp_hal::gpio::Output;
use esp_hal::spi::master::{Address, Command, DataMode, SpiDma};

#[cfg(feature = "timing-log")]
use embassy_time::Instant;

/// Every transfer spins rather than awaits: waking from the interrupt added tens of
/// microseconds a chunk, and the display core has nothing else to run meanwhile.
pub struct QspiBus<'d> {
    spi: Option<SpiDma<'d, Async>>,
    /// A command's parameters, and a read's reply.
    command: Option<DmaTxBuf>,
    reply: Option<DmaRxBuf>,
    cs: Output<'d>,
    /// The stream's buffer filling, and the one the last chunk went from.
    active: Option<DmaTxBuf>,
    swap: Option<DmaTxBuf>,
    buffered: usize,
}

impl<'d> QspiBus<'d> {
    #[must_use]
    pub fn new(
        spi: SpiDma<'d, Async>,
        command: DmaTxBuf,
        reply: DmaRxBuf,
        stream: DmaTxBuf,
        swap: DmaTxBuf,
        cs: Output<'d>,
    ) -> Self {
        Self {
            spi: Some(spi),
            command: Some(command),
            reply: Some(reply),
            cs,
            active: Some(stream),
            swap: Some(swap),
            buffered: 0,
        }
    }

    /// Sends the buffered bytes while `fill_swap_with` fills the other buffer, which then
    /// becomes the one filling.
    fn send(
        &mut self,
        fill_swap_with: impl FnOnce(&mut [u8]) -> usize,
    ) -> Result<(), esp_hal::spi::Error> {
        if self.buffered == 0 {
            self.buffered = fill_swap_with(self.active.as_mut().unwrap().as_mut_slice());
            return Ok(());
        }
        let buffered = self.buffered;
        let active = self.active.take().unwrap();
        let mut swap = self.swap.take().unwrap();
        let spi = self.spi.take().unwrap();
        let transfer = match spi.half_duplex_write_buffer(
            DataMode::Quad,
            Command::None,
            Address::None,
            0,
            buffered,
            active,
        ) {
            Ok(transfer) => transfer,
            Err((error, spi, active)) => {
                self.spi = Some(spi);
                self.active = Some(active);
                self.swap = Some(swap);
                self.cs.set_high();
                return Err(error);
            }
        };
        let new = fill_swap_with(swap.as_mut_slice());
        let (spi, active) = transfer.wait();
        self.spi = Some(spi);
        self.active = Some(swap);
        self.swap = Some(active);
        self.buffered = new;
        Ok(())
    }
}

/// Which line the controls bench reads replies on.
pub static READ_ON_SIO1: core::sync::atomic::AtomicBool =
    core::sync::atomic::AtomicBool::new(false);

fn mode(lanes: Lanes) -> DataMode {
    match lanes {
        Lanes::Single => DataMode::Single,
        Lanes::Quad => DataMode::Quad,
    }
}

impl Bus for QspiBus<'_> {
    type Error = esp_hal::spi::Error;

    async fn write(
        &mut self,
        instruction: u8,
        address: u32,
        lanes: Lanes,
        data: &[u8],
    ) -> Result<(), Self::Error> {
        #[cfg(feature = "timing-log")]
        let started = Instant::now();
        let mode = mode(lanes);
        self.cs.set_low();
        let spi = self.spi.take().unwrap();
        if data.is_empty() {
            let transfer = spi
                .half_duplex_write_buffer(
                    mode,
                    Command::_8Bit(u16::from(instruction), DataMode::Single),
                    Address::_24Bit(address, mode),
                    0,
                    0,
                    EmptyBuf,
                )
                .map_err(|(error, spi, _)| {
                    self.spi = Some(spi);
                    self.cs.set_high();
                    error
                })?;
            let (spi, _) = transfer.wait();
            self.spi = Some(spi);
        } else {
            let mut buffer = self.command.take().unwrap();
            buffer.as_mut_slice()[..data.len()].copy_from_slice(data);
            let transfer = spi
                .half_duplex_write_buffer(
                    mode,
                    Command::_8Bit(u16::from(instruction), DataMode::Single),
                    Address::_24Bit(address, mode),
                    0,
                    data.len(),
                    buffer,
                )
                .map_err(|(error, spi, buffer)| {
                    self.spi = Some(spi);
                    self.command = Some(buffer);
                    self.cs.set_high();
                    error
                })?;
            let (spi, buffer) = transfer.wait();
            self.spi = Some(spi);
            self.command = Some(buffer);
        }
        self.cs.set_high();
        #[cfg(feature = "timing-log")]
        defmt::info!(
            "qspi transfer bytes={} us={}",
            data.len(),
            started.elapsed().as_micros()
        );
        Ok(())
    }

    async fn begin(
        &mut self,
        instruction: u8,
        address: u32,
        lanes: Lanes,
    ) -> Result<(), Self::Error> {
        let mode = mode(lanes);
        self.buffered = 0;
        self.cs.set_low();
        let spi = self.spi.take().unwrap();
        let transfer = spi
            .half_duplex_write_buffer(
                mode,
                Command::_8Bit(u16::from(instruction), DataMode::Single),
                Address::_24Bit(address, mode),
                0,
                0,
                EmptyBuf,
            )
            .map_err(|(error, spi, _)| {
                self.spi = Some(spi);
                self.cs.set_high();
                error
            })?;
        let (spi, _) = transfer.wait();
        self.spi = Some(spi);
        Ok(())
    }

    async fn stream(&mut self, fill: impl FnOnce(&mut [u8]) -> usize) -> Result<(), Self::Error> {
        let active = self.active.as_mut().unwrap();
        if self.buffered > active.len().saturating_sub(256) {
            return self.send(fill);
        }
        let new = fill(&mut active.as_mut_slice()[self.buffered..]);
        self.buffered += new;
        Ok(())
    }

    async fn end(&mut self) -> Result<(), Self::Error> {
        let sent = self.send(|_| 0);
        self.cs.set_high();
        sent
    }

    /// The reply comes on SIO0, where the controller's read diagram has it. Nothing has read
    /// the panel yet to show the module wires it there rather than to SIO1.
    async fn read(
        &mut self,
        instruction: u8,
        address: u32,
        buffer: &mut [u8],
    ) -> Result<(), Self::Error> {
        self.cs.set_low();
        let mut spi = self.spi.take().unwrap();
        let read = esp_hal::spi::master::Config::default()
            .with_frequency(esp_hal::time::Rate::from_mhz(5))
            .with_mode(esp_hal::spi::Mode::_0);
        spi.apply_config(&read).unwrap();
        let reply = self.reply.take().unwrap();
        let transfer = spi
            .half_duplex_read_buffer(
                if READ_ON_SIO1.load(core::sync::atomic::Ordering::Relaxed) {
                    DataMode::SingleTwoDataLines
                } else {
                    DataMode::Single
                },
                Command::_8Bit(u16::from(instruction), DataMode::Single),
                Address::_24Bit(address, DataMode::Single),
                0,
                buffer.len(),
                reply,
            )
            .map_err(|(error, spi, reply)| {
                self.spi = Some(spi);
                self.reply = Some(reply);
                self.cs.set_high();
                error
            })?;
        let (mut spi, reply) = transfer.wait();
        let write = esp_hal::spi::master::Config::default()
            .with_frequency(esp_hal::time::Rate::from_mhz(80))
            .with_mode(esp_hal::spi::Mode::_0);
        spi.apply_config(&write).unwrap();
        let received = reply.number_of_received_bytes();
        let copied = reply.read_received_data(buffer);
        defmt::info!("[CTRLBENCH] read received={} copied={}", received, copied);
        self.spi = Some(spi);
        self.reply = Some(reply);
        self.cs.set_high();
        Ok(())
    }
}
