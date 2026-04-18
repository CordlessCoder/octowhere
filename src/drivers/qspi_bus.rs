// QSPI bus driver for CO5300 AMOLED display - DMA version
// Uses SpiDmaBus for large transfers via DMA

use esp_hal::Async;
use esp_hal::dma::DmaTxBuf;
use esp_hal::gpio::Output;
use esp_hal::spi::master::{Address, Command, DataMode, SpiDma};

use crate::board::{delay_ms, delay_ms_async};

pub struct QspiBus<'d> {
    pub(crate) spi: SpiDma<'d, Async>,
    pub(crate) tx: DmaTxBuf,
    pub(crate) cs: Output<'d>,
}

pub enum QSPIOperation {
    Delay(u32),
    Command(u8),
    CommandD8(u8, u8),
    CommandD16D16(u8, u16, u16),
}

// PERF: Should our DMA writes be async or blocking?
impl<'d> QspiBus<'d> {
    pub fn new(spi: SpiDma<'d, Async>, tx: DmaTxBuf, cs: Output<'d>) -> Self {
        Self { spi, tx, cs }
    }

    #[inline]
    fn command_to_bytes(&mut self, cmd: &QSPIOperation) -> (u8, usize) {
        match cmd {
            QSPIOperation::Delay(_) => {
                unreachable!("command_to_bytes must never be called on a Delay command")
            }
            &QSPIOperation::Command(cmd) => (cmd, 0),
            &QSPIOperation::CommandD8(cmd, byte) => (cmd, {
                self.tx.as_mut_slice()[0] = byte;
                1
            }),
            &QSPIOperation::CommandD16D16(cmd, d1, d2) => {
                let data = [(d1 >> 8) as u8, d1 as u8, (d2 >> 8) as u8, d2 as u8];
                self.tx.as_mut_slice()[..data.len()].copy_from_slice(&data);
                (cmd, data.len())
            }
        }
    }

    pub async fn execute_async(&mut self, op: &QSPIOperation) {
        let (cmd, bytes) = match op {
            &QSPIOperation::Delay(ms) => {
                delay_ms_async(ms).await;
                return;
            }
            QSPIOperation::Command(..)
            | QSPIOperation::CommandD8(..)
            | QSPIOperation::CommandD16D16(..) => self.command_to_bytes(op),
        };
        self.cs_low();
        self.spi
            .half_duplex_write_and_wait(
                DataMode::Single,
                Command::_8Bit(0x02, DataMode::Single),
                Address::_24Bit((cmd as u32) << 8, DataMode::Single),
                0,
                bytes,
                &mut self.tx,
            )
            .await
            .unwrap();
        self.cs_high();
    }

    pub async fn batch_async(&mut self, ops: &[QSPIOperation]) {
        for command in ops {
            self.execute_async(command).await;
        }
    }

    pub fn execute(&mut self, op: &QSPIOperation) {
        let (cmd, bytes) = match op {
            &QSPIOperation::Delay(ms) => {
                delay_ms(ms);
                return;
            }
            QSPIOperation::Command(..)
            | QSPIOperation::CommandD8(..)
            | QSPIOperation::CommandD16D16(..) => self.command_to_bytes(op),
        };
        self.cs_low();
        self.spi
            .half_duplex_write_and_block(
                DataMode::Single,
                Command::_8Bit(0x02, DataMode::Single),
                Address::_24Bit((cmd as u32) << 8, DataMode::Single),
                0,
                bytes,
                &mut self.tx,
            )
            .unwrap();
        self.cs_high();
    }

    pub fn batch(&mut self, ops: &[QSPIOperation]) {
        for command in ops {
            self.execute(command);
        }
    }

    #[inline]
    fn cs_low(&mut self) {
        self.cs.set_low();
    }
    #[inline]
    fn cs_high(&mut self) {
        self.cs.set_high();
    }

    // pub async fn stream_pixels(&mut self, pixels: &[u16]) {
    //     if pixels.is_empty() {
    //         return;
    //     }
    //     let max_px = self.tx.len() / 2;
    //     let mut remaining = pixels;
    //     while !remaining.is_empty() {
    //         let scratch = self.tx.as_mut_slice();
    //         let n = remaining.len().min(max_px);
    //         for (i, &px) in remaining.iter().enumerate().take(n) {
    //             scratch[i * 2] = (px >> 8) as u8;
    //             scratch[i * 2 + 1] = px as u8;
    //         }
    //         let _ = self
    //             .spi
    //             .half_duplex_write_and_wait(
    //                 DataMode::Quad,
    //                 Command::None,
    //                 Address::None,
    //                 0,
    //                 n * 2,
    //                 &mut self.tx,
    //             )
    //             .await
    //             .unwrap();
    //         remaining = &remaining[n..];
    //     }
    // }
    //
    // pub async fn write_pixels_async(&mut self, pixels: &[u16]) {
    //     if pixels.is_empty() {
    //         return;
    //     }
    //     self.cs_low();
    //     let max_px = self.tx.len() / 2;
    //     let mut remaining = pixels;
    //     let mut first = true;
    //     while !remaining.is_empty() {
    //         let scratch = self.tx.as_mut_slice();
    //         let n = remaining.len().min(max_px);
    //         for (i, &px) in remaining[..n].iter().enumerate() {
    //             scratch[i * 2] = (px >> 8) as u8;
    //             scratch[i * 2 + 1] = px as u8;
    //         }
    //         if first {
    //             self.spi
    //                 .half_duplex_write_and_wait(
    //                     DataMode::Quad,
    //                     Command::_8Bit(0x32, DataMode::Single),
    //                     Address::_24Bit(0x003C00, DataMode::Single),
    //                     0,
    //                     n * 2,
    //                     &mut self.tx,
    //                 )
    //                 .await
    //                 .unwrap();
    //             first = false;
    //         } else {
    //             self.spi
    //                 .half_duplex_write_and_wait(
    //                     DataMode::Quad,
    //                     Command::None,
    //                     Address::None,
    //                     0,
    //                     n * 2,
    //                     &mut self.tx,
    //                 )
    //                 .await
    //                 .unwrap();
    //         }
    //         remaining = &remaining[n..];
    //     }
    //     self.cs_high();
    // }
}
