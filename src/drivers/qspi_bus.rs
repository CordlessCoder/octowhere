// QSPI bus driver for CO5300 AMOLED display - DMA version
// Uses SpiDmaBus for large transfers via DMA

use esp_hal::Async;
use esp_hal::dma::{DmaTxBuf, EmptyBuf};
use esp_hal::gpio::Output;
use esp_hal::spi::master::{Address, Command, DataMode, SpiDma};

#[cfg(feature = "timing-log")]
use embassy_time::Instant;

use crate::board::{delay_ms, delay_ms_async};

pub struct QspiBus<'d> {
    pub(crate) spi: Option<SpiDma<'d, Async>>,
    tx: Option<DmaTxBuf>,
    pub(crate) cs: Output<'d>,
}

pub enum QSPIOperation {
    Delay(u32),
    Command(u8),
    CommandD8(u8, u8),
    CommandD16(u8, u16),
    CommandD16D16(u8, u16, u16),
}

// PERF: Should our DMA writes be async or blocking?
impl<'d> QspiBus<'d> {
    #[must_use]
    pub fn new(spi: SpiDma<'d, Async>, tx: DmaTxBuf, cs: Output<'d>) -> Self {
        Self {
            spi: Some(spi),
            tx: Some(tx),
            cs,
        }
    }

    #[inline]
    fn command_to_bytes(&mut self, cmd: &QSPIOperation) -> (u8, usize) {
        match cmd {
            QSPIOperation::Delay(_) => {
                unreachable!("command_to_bytes must never be called on a Delay command")
            }
            &QSPIOperation::Command(cmd) => (cmd, 0),
            &QSPIOperation::CommandD8(cmd, byte) => (cmd, {
                self.tx.as_mut().unwrap().as_mut_slice()[0] = byte;
                1
            }),
            &QSPIOperation::CommandD16(cmd, d) => {
                self.tx.as_mut().unwrap().as_mut_slice()[..2].copy_from_slice(&d.to_be_bytes());
                (cmd, 2)
            }
            &QSPIOperation::CommandD16D16(cmd, d1, d2) => {
                let data = [(d1 >> 8) as u8, d1 as u8, (d2 >> 8) as u8, d2 as u8];
                self.tx.as_mut().unwrap().as_mut_slice()[..data.len()].copy_from_slice(&data);
                (cmd, data.len())
            }
        }
    }

    pub async fn execute_async(&mut self, op: &QSPIOperation) -> Result<(), esp_hal::spi::Error> {
        let (cmd, bytes) = match op {
            &QSPIOperation::Delay(ms) => {
                delay_ms_async(ms).await;
                return Ok(());
            }
            QSPIOperation::Command(..)
            | QSPIOperation::CommandD8(..)
            | QSPIOperation::CommandD16(..)
            | QSPIOperation::CommandD16D16(..) => self.command_to_bytes(op),
        };
        #[cfg(feature = "timing-log")]
        let started = Instant::now();
        self.cs_low();
        let spi = self.spi.take().unwrap();
        if bytes == 0 {
            let transfer = spi
                .half_duplex_write_buffer(
                    DataMode::Single,
                    Command::_8Bit(0x02, DataMode::Single),
                    Address::_24Bit((cmd as u32) << 8, DataMode::Single),
                    0,
                    0,
                    EmptyBuf,
                )
                .map_err(|(error, spi, _)| {
                    self.spi = Some(spi);
                    self.cs_high();
                    error
                })?;
            let mut transfer = transfer;
            transfer.wait_for_done().await;
            let (spi, _) = transfer.wait();
            self.spi = Some(spi);
        } else {
            let tx = self.tx.take().unwrap();
            let transfer = spi
                .half_duplex_write_buffer(
                    DataMode::Single,
                    Command::_8Bit(0x02, DataMode::Single),
                    Address::_24Bit((cmd as u32) << 8, DataMode::Single),
                    0,
                    bytes,
                    tx,
                )
                .map_err(|(error, spi, tx)| {
                    self.spi = Some(spi);
                    self.tx = Some(tx);
                    self.cs_high();
                    error
                })?;
            let mut transfer = transfer;
            transfer.wait_for_done().await;
            let (spi, tx) = transfer.wait();
            self.spi = Some(spi);
            self.tx = Some(tx);
        }
        self.cs_high();
        #[cfg(feature = "timing-log")]
        defmt::info!(
            "qspi transfer path=async bytes={} us={}",
            bytes,
            started.elapsed().as_micros()
        );
        Ok(())
    }

    pub async fn batch_async(&mut self, ops: &[QSPIOperation]) -> Result<(), esp_hal::spi::Error> {
        for command in ops {
            self.execute_async(command).await?;
        }
        Ok(())
    }

    pub fn execute(&mut self, op: &QSPIOperation) -> Result<(), esp_hal::spi::Error> {
        let (cmd, bytes) = match op {
            &QSPIOperation::Delay(ms) => {
                delay_ms(ms);
                return Ok(());
            }
            QSPIOperation::Command(..)
            | QSPIOperation::CommandD8(..)
            | QSPIOperation::CommandD16(..)
            | QSPIOperation::CommandD16D16(..) => self.command_to_bytes(op),
        };
        #[cfg(feature = "timing-log")]
        let started = Instant::now();
        self.cs_low();
        let spi = self.spi.take().unwrap();
        if bytes == 0 {
            let transfer = spi
                .half_duplex_write_buffer(
                    DataMode::Single,
                    Command::_8Bit(0x02, DataMode::Single),
                    Address::_24Bit((cmd as u32) << 8, DataMode::Single),
                    0,
                    0,
                    EmptyBuf,
                )
                .map_err(|(error, spi, _)| {
                    self.spi = Some(spi);
                    self.cs_high();
                    error
                })?;
            let (spi, _) = transfer.wait();
            self.spi = Some(spi);
        } else {
            let tx = self.tx.take().unwrap();
            let transfer = spi
                .half_duplex_write_buffer(
                    DataMode::Single,
                    Command::_8Bit(0x02, DataMode::Single),
                    Address::_24Bit((cmd as u32) << 8, DataMode::Single),
                    0,
                    bytes,
                    tx,
                )
                .map_err(|(error, spi, tx)| {
                    self.spi = Some(spi);
                    self.tx = Some(tx);
                    self.cs_high();
                    error
                })?;
            let (spi, tx) = transfer.wait();
            self.spi = Some(spi);
            self.tx = Some(tx);
        }
        self.cs_high();
        #[cfg(feature = "timing-log")]
        defmt::info!(
            "qspi transfer path=blocking bytes={} us={}",
            bytes,
            started.elapsed().as_micros()
        );
        Ok(())
    }

    pub fn batch(&mut self, ops: &[QSPIOperation]) -> Result<(), esp_hal::spi::Error> {
        for command in ops {
            self.execute(command)?;
        }
        Ok(())
    }

    #[inline]
    fn cs_low(&mut self) {
        self.cs.set_low();
    }
    #[inline]
    fn cs_high(&mut self) {
        self.cs.set_high();
    }

    /// Starts a quad write with `command`, leaving CS asserted for the data that follows.
    pub(crate) async fn begin_quad_write_async(&mut self, command: u8) -> Result<(), esp_hal::spi::Error> {
        self.cs_low();
        let spi = self.spi.take().unwrap();
        let transfer = spi
            .half_duplex_write_buffer(
                DataMode::Quad,
                Command::_8Bit(0x12, DataMode::Single),
                Address::_24Bit(u32::from(command) << 8, DataMode::Quad),
                0,
                0,
                EmptyBuf,
            )
            .map_err(|(error, spi, _)| {
                self.spi = Some(spi);
                self.cs_high();
                error
            })?;
        let mut transfer = transfer;
        transfer.wait_for_done().await;
        let (spi, _) = transfer.wait();
        self.spi = Some(spi);
        Ok(())
    }

    /// As [`begin_quad_write_async`](Self::begin_quad_write_async), blocking.
    pub(crate) fn begin_quad_write(&mut self, command: u8) -> Result<(), esp_hal::spi::Error> {
        self.cs_low();
        let spi = self.spi.take().unwrap();
        let transfer = spi
            .half_duplex_write_buffer(
                DataMode::Quad,
                Command::_8Bit(0x12, DataMode::Single),
                Address::_24Bit(u32::from(command) << 8, DataMode::Quad),
                0,
                0,
                EmptyBuf,
            )
            .map_err(|(error, spi, _)| {
                self.spi = Some(spi);
                self.cs_high();
                error
            })?;
        let (spi, _) = transfer.wait();
        self.spi = Some(spi);
        Ok(())
    }
}
