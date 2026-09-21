// AXP2101 Power Management wrapper
// Reference: 05_LVGL_AXP2101_ADC_Data.ino
use embedded_hal_async::i2c::I2c;
use futures::TryFutureExt;

const AXP2101_ADDR: u8 = 0x34;

const REG_STATUS1: u8 = 0x00;
const REG_STATUS2: u8 = 0x01;
const REG_IC_TYPE: u8 = 0x03;
const REG_VBAT_H: u8 = 0x34;
const REG_VBAT_L: u8 = 0x35;
const REG_TS_H: u8 = 0x36;
const REG_TS_L: u8 = 0x37;
const REG_VBUS_H: u8 = 0x38;
const REG_VBUS_L: u8 = 0x39;
const REG_VSYS_H: u8 = 0x3A;
const REG_VSYS_L: u8 = 0x3B;
const REG_DC_ONOFF: u8 = 0x80; // DC output on/off + DVM control
const REG_DC_VOL0: u8 = 0x82; // DCDC1 voltage setting
const REG_LDO_ONOFF0: u8 = 0x90; // ALDO1-4 on/off control
const REG_LDO_VOL0: u8 = 0x92; // ALDO1 voltage setting
const REG_ADC_ENABLE: u8 = 0x30;
const REG_IRQ_ENABLE0: u8 = 0x40;
const REG_IRQ_ENABLE1: u8 = 0x41;
const REG_IRQ_ENABLE2: u8 = 0x42;
const REG_IRQ_STATUS0: u8 = 0x48;
const REG_IRQ_STATUS1: u8 = 0x49;
const REG_IRQ_STATUS2: u8 = 0x4A;
const REG_BAT_PERCENT: u8 = 0xA4;
const REG_CHG_STATUS: u8 = 0x01;
const AXP2101_CHIP_ID: u8 = 0x4A;

const ADC_VBAT: u8 = 1 << 0;
const ADC_TS: u8 = 1 << 1;
const ADC_VBUS: u8 = 1 << 2;
const ADC_VSYS: u8 = 1 << 3;
const ADC_DIE_TEMPERATURE: u8 = 1 << 4;

#[derive(Debug)]
pub enum Axp2101Error<E> {
    I2c(E),
    WrongChipId(u8),
}

pub struct Axp2101Power<I> {
    i2c: I,
}

impl<I: I2c> Axp2101Power<I> {
    #[must_use]
    pub fn new(i2c: I) -> Self {
        Self { i2c }
    }

    fn read_reg(&mut self, reg: u8) -> impl Future<Output = Result<u8, I::Error>> {
        super::i2c_helper::read_reg_byte(&mut self.i2c, AXP2101_ADDR, reg)
    }

    fn write_reg(&mut self, reg: u8, val: u8) -> impl Future<Output = Result<(), I::Error>> {
        super::i2c_helper::write_reg(&mut self.i2c, AXP2101_ADDR, reg, val)
    }

    /// Initialize monitoring without changing the board's power-rail state.
    pub async fn init(&mut self) -> Result<(), Axp2101Error<I::Error>> {
        let chip_id = self.read_chip_id().await.map_err(Axp2101Error::I2c)?;
        if chip_id != AXP2101_CHIP_ID {
            return Err(Axp2101Error::WrongChipId(chip_id));
        }

        self.write_reg(REG_IRQ_ENABLE0, 0x00)
            .await
            .map_err(Axp2101Error::I2c)?;
        self.write_reg(REG_IRQ_ENABLE1, 0x00)
            .await
            .map_err(Axp2101Error::I2c)?;
        self.write_reg(REG_IRQ_ENABLE2, 0x00)
            .await
            .map_err(Axp2101Error::I2c)?;
        self.write_reg(REG_IRQ_STATUS0, 0xFF)
            .await
            .map_err(Axp2101Error::I2c)?;
        self.write_reg(REG_IRQ_STATUS1, 0xFF)
            .await
            .map_err(Axp2101Error::I2c)?;
        self.write_reg(REG_IRQ_STATUS2, 0xFF)
            .await
            .map_err(Axp2101Error::I2c)?;

        // Leave TS disabled because the board has no battery temperature input.
        self.write_reg(
            REG_ADC_ENABLE,
            ADC_VBAT | ADC_VBUS | ADC_VSYS | ADC_DIE_TEMPERATURE,
        )
        .await
        .map_err(Axp2101Error::I2c)?;

        Ok(())
    }

    /// Read battery voltage in millivolts.
    pub async fn get_battery_voltage(&mut self) -> Result<u16, I::Error> {
        self.read_adc_mv(REG_VBAT_H).await
    }

    /// Read VBUS voltage in millivolts.
    pub async fn get_vbus_voltage(&mut self) -> Result<u16, I::Error> {
        self.read_adc_mv(REG_VBUS_H).await
    }

    /// Read system voltage in millivolts.
    pub async fn get_system_voltage(&mut self) -> Result<u16, I::Error> {
        self.read_adc_mv(REG_VSYS_H).await
    }

    async fn read_adc_mv(&mut self, high_reg: u8) -> Result<u16, I::Error> {
        let mut data = [0u8; 2];
        self.i2c
            .write_read(AXP2101_ADDR, &[high_reg], &mut data)
            .await?;
        // VBAT, VBUS and VSYS use 1 mV per ADC count (14-bit, high byte first).
        Ok((((data[0] as u16) & 0x3F) << 8) | data[1] as u16)
    }

    /// Read battery percentage (0-100).
    pub fn get_battery_percent(&mut self) -> impl Future<Output = Result<u8, I::Error>> {
        self.read_reg(REG_BAT_PERCENT)
    }

    /// Check if charging.
    pub fn is_charging(&mut self) -> impl Future<Output = Result<bool, I::Error>> {
        self.read_reg(REG_CHG_STATUS).map_ok(is_charging_status)
    }

    /// Check if VBUS (USB) is connected.
    pub fn is_vbus_in(&mut self) -> impl Future<Output = Result<bool, I::Error>> {
        self.read_reg(REG_STATUS1).map_ok(|status| {
            status & 0x20 != 0 // Bit 5: VBUS good
        })
    }

    /// Check the charger’s battery-presence result.
    pub fn is_battery_present(&mut self) -> impl Future<Output = Result<bool, I::Error>> {
        self.read_reg(REG_STATUS1)
            .map_ok(|status| status & (1 << 3) != 0)
    }

    /// Read chip ID to verify communication.
    pub fn read_chip_id(&mut self) -> impl Future<Output = Result<u8, I::Error>> {
        self.read_reg(REG_IC_TYPE)
    }

    /// Read raw STATUS2 (charge / WLTF / BATFET states).
    pub fn read_status2(&mut self) -> impl Future<Output = Result<u8, I::Error>> {
        self.read_reg(REG_STATUS2)
    }

    /// Disable power ADC channels we don't actively use on the watchface
    /// (TS pin + die temp) to shave a few hundred µA off ADC refresh.
    /// Keep VBAT+VBUS+VSYS enabled so battery UI still works.
    pub fn trim_adc_channels(&mut self) -> impl Future<Output = Result<(), I::Error>> {
        // Keep VBAT, VBUS and VSYS enabled; disable TS and die temperature.
        self.write_reg(REG_ADC_ENABLE, ADC_VBAT | ADC_VBUS | ADC_VSYS)
    }
}

fn is_charging_status(status: u8) -> bool {
    matches!(status & 0x07, 0b001..=0b011)
}

const ADC_ENABLE_INIT: u8 = ADC_VBAT | ADC_VBUS | ADC_VSYS | ADC_DIE_TEMPERATURE;
const ADC_ENABLE_TRIMMED: u8 = ADC_VBAT | ADC_VBUS | ADC_VSYS;

#[cfg(test)]
mod tests {
    use super::{ADC_ENABLE_INIT, ADC_ENABLE_TRIMMED, ADC_TS, is_charging_status};

    #[test]
    fn adc_masks_match_axp2101_register_bits() {
        assert_eq!(ADC_ENABLE_INIT, 0x1D);
        assert_eq!(ADC_ENABLE_TRIMMED, 0x0D);
        assert_eq!(ADC_ENABLE_INIT & ADC_TS, 0);
        assert_eq!(ADC_ENABLE_TRIMMED & ADC_TS, 0);
    }

    #[test]
    fn charging_status_uses_status2_low_bits() {
        assert!(!is_charging_status(0b000));
        assert!(is_charging_status(0b001));
        assert!(is_charging_status(0b010));
        assert!(is_charging_status(0b011));
        assert!(!is_charging_status(0b100));
        assert!(!is_charging_status(0b101));
        assert!(!is_charging_status(0b111));
        assert!(is_charging_status(0b1010_0010));
    }
}
