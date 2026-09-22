#[cfg(feature = "defmt")]
use defmt::debug;

#[cfg(not(feature = "sync"))]
pub use embedded_hal_async::delay::DelayNs;
#[cfg(not(feature = "sync"))]
pub use embedded_hal_async::spi::SpiDevice;

#[cfg(feature = "sync")]
pub use embedded_hal::delay::DelayNs;
#[cfg(feature = "sync")]
pub use embedded_hal::spi::SpiDevice;

use crate::error::Sx127xError;
use embedded_hal::spi::Operation;

pub struct Sx127xSpi<SPI> {
    pub spi: SPI,
}
impl<SPI: SpiDevice> Sx127xSpi<SPI> {
    pub fn new(spi: SPI) -> Self {
        Self { spi }
    }

    /// Gets the byte from the register at `addr` over SPI.
    ///
    /// See: datasheet section 2.2
    #[maybe_async::maybe_async]
    pub async fn read(&mut self, addr: u8) -> Result<u8, Sx127xError<SPI::Error>> {
        // #[cfg(feature = "defmt")]
        // debug!("Sx127xSpi::read: {}", addr);
        let mut read = [0; 2];
        // 1 wnr bit (0 for read) + 7 bit addr
        let write = [addr & 0x7f, 0];
        self.spi
            .transfer(&mut read, &write)
            .await
            .map_err(Sx127xError::SPI)?;
        Ok(read[1])
    }

    /// Reads a contiguous range of registers while keeping chip select asserted.
    #[maybe_async::maybe_async]
    pub async fn read_burst(
        &mut self,
        addr: u8,
        buffer: &mut [u8],
    ) -> Result<(), Sx127xError<SPI::Error>> {
        let command = [addr & 0x7f];
        self.spi
            .transaction(&mut [Operation::Write(&command), Operation::Read(buffer)])
            .await
            .map_err(Sx127xError::SPI)
    }

    /// Writes the `data` byte to the register at `addr` over SPI.
    ///
    /// See: datasheet section 2.2
    #[maybe_async::maybe_async]
    pub async fn write(&mut self, addr: u8, data: u8) -> Result<(), Sx127xError<SPI::Error>> {
        #[cfg(feature = "defmt")]
        debug!("Sx127xSpi::write: 0x{:x} 0x{:x}", addr, data);
        // 1 wnr bit (1 for write) + 7 bit addr
        let buf = [addr | 0x80, data];
        self.spi.write(&buf).await.map_err(Sx127xError::SPI)
    }

    /// Writes a contiguous range of registers while keeping chip select asserted.
    #[maybe_async::maybe_async]
    pub async fn write_burst(
        &mut self,
        addr: u8,
        data: &[u8],
    ) -> Result<(), Sx127xError<SPI::Error>> {
        let command = [addr | 0x80];
        self.spi
            .transaction(&mut [Operation::Write(&command), Operation::Write(data)])
            .await
            .map_err(Sx127xError::SPI)
    }
}
