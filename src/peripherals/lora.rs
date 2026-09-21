use embedded_hal::digital::OutputPin;
use embedded_hal_async::spi::SpiBus;

const REG_FIFO: u8 = 0x00;
const REG_OP_MODE: u8 = 0x01;
const REG_FRF_MSB: u8 = 0x06;
const REG_IRQ_FLAGS: u8 = 0x12;
const REG_VERSION: u8 = 0x42;

const MODE_LONG_RANGE: u8 = 1 << 7;
const MODE_SLEEP: u8 = 0x00;
const MODE_STDBY: u8 = 0x01;
const SX1272_VERSION: u8 = 0x22;
const XTAL_HZ: u64 = 32_000_000;
const FRF_SCALE: u64 = 1 << 19;

#[derive(Debug, PartialEq, Eq)]
pub enum LoraError<SpiError, PinError> {
    Spi(SpiError),
    ChipSelect(PinError),
    UnexpectedVersion(u8),
}

pub struct Sx1272<SPI, CS> {
    spi: SPI,
    cs: CS,
}

impl<SPI, CS> Sx1272<SPI, CS>
where
    SPI: SpiBus<u8>,
    CS: OutputPin,
{
    pub fn new(spi: SPI, mut cs: CS) -> Result<Self, LoraError<SPI::Error, CS::Error>> {
        cs.set_high().map_err(LoraError::ChipSelect)?;
        Ok(Self { spi, cs })
    }

    pub async fn identify(&mut self) -> Result<(), LoraError<SPI::Error, CS::Error>> {
        let version = self.read_register(REG_VERSION).await?;
        if version != SX1272_VERSION {
            return Err(LoraError::UnexpectedVersion(version));
        }
        Ok(())
    }

    pub async fn probe_registers(&mut self) -> Result<[u8; 4], LoraError<SPI::Error, CS::Error>> {
        Ok([
            self.read_register(REG_OP_MODE).await?,
            self.read_register(REG_FRF_MSB).await?,
            self.read_register(REG_IRQ_FLAGS).await?,
            self.read_register(REG_VERSION).await?,
        ])
    }

    pub async fn set_lora_standby(&mut self) -> Result<(), LoraError<SPI::Error, CS::Error>> {
        self.write_register(REG_OP_MODE, MODE_LONG_RANGE | MODE_STDBY)
            .await
    }

    pub async fn set_sleep(&mut self) -> Result<(), LoraError<SPI::Error, CS::Error>> {
        self.write_register(REG_OP_MODE, MODE_LONG_RANGE | MODE_SLEEP)
            .await
    }

    pub async fn set_frequency_hz(
        &mut self,
        frequency_hz: u32,
    ) -> Result<(), LoraError<SPI::Error, CS::Error>> {
        let frf = (u64::from(frequency_hz) * FRF_SCALE / XTAL_HZ) as u32;
        self.write_registers(REG_FRF_MSB, &frf.to_be_bytes()[1..])
            .await
    }

    pub async fn irq_flags(&mut self) -> Result<u8, LoraError<SPI::Error, CS::Error>> {
        self.read_register(REG_IRQ_FLAGS).await
    }

    pub async fn clear_irq_flags(
        &mut self,
        flags: u8,
    ) -> Result<(), LoraError<SPI::Error, CS::Error>> {
        self.write_register(REG_IRQ_FLAGS, flags).await
    }

    pub async fn write_fifo(
        &mut self,
        payload: &[u8],
    ) -> Result<(), LoraError<SPI::Error, CS::Error>> {
        self.write_registers(REG_FIFO, payload).await
    }

    async fn read_register(
        &mut self,
        register: u8,
    ) -> Result<u8, LoraError<SPI::Error, CS::Error>> {
        let mut data = [register & 0x7F, 0];
        self.transaction(&mut data).await?;
        Ok(data[1])
    }

    async fn write_register(
        &mut self,
        register: u8,
        value: u8,
    ) -> Result<(), LoraError<SPI::Error, CS::Error>> {
        self.write_registers(register, &[value]).await
    }

    async fn write_registers(
        &mut self,
        register: u8,
        values: &[u8],
    ) -> Result<(), LoraError<SPI::Error, CS::Error>> {
        self.cs.set_low().map_err(LoraError::ChipSelect)?;
        if let Err(error) = self.spi.write(&[register | 0x80]).await {
            let _ = self.cs.set_high();
            return Err(LoraError::Spi(error));
        }
        let result = self.spi.write(values).await;
        let cs_result = self.cs.set_high();
        result.map_err(LoraError::Spi)?;
        cs_result.map_err(LoraError::ChipSelect)
    }

    async fn transaction(
        &mut self,
        data: &mut [u8],
    ) -> Result<(), LoraError<SPI::Error, CS::Error>> {
        self.cs.set_low().map_err(LoraError::ChipSelect)?;
        let result = self.spi.transfer_in_place(data).await;
        let cs_result = self.cs.set_high();
        result.map_err(LoraError::Spi)?;
        cs_result.map_err(LoraError::ChipSelect)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn eu868_frequency_word_is_calculated_from_the_32_mhz_crystal() {
        let frf = (868_000_000u64 * (1 << 19) / 32_000_000) as u32;
        assert_eq!(frf, 0xD9_0000);
    }
}
