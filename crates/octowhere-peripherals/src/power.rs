// AXP2101 Power Management wrapper
// Reference: 05_LVGL_AXP2101_ADC_Data.ino
use embedded_hal_async::i2c::I2c;
use futures::TryFutureExt;

const AXP2101_ADDR: u8 = 0x34;

const REG_STATUS1: u8 = 0x00;
const REG_STATUS2: u8 = 0x01;
const REG_IC_TYPE: u8 = 0x03;
const REG_COMMON: u8 = 0x10;
const REG_POWER_ON_SOURCE: u8 = 0x20;
const REG_POWER_OFF_SOURCE: u8 = 0x21;
const REG_POWER_OFF_ENABLE: u8 = 0x22;
const REG_KEY_LEVELS: u8 = 0x27;
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

/// `REG_COMMON`: power every output off but the RTC LDO.
const SOFT_POWER_OFF: u8 = 1 << 0;
/// `REG_POWER_OFF_ENABLE`: holding the key past OFFLEVEL powers off, and does not restart.
const KEY_POWER_OFF: u8 = 1 << 1;
const KEY_RESTARTS: u8 = 1 << 0;
/// `REG_IRQ_ENABLE1` and `REG_IRQ_STATUS1`: the key's short and long press.
const SHORT_PRESS: u8 = 1 << 3;
const LONG_PRESS: u8 = 1 << 2;
/// `REG_KEY_LEVELS`: a long press is 1 s (IRQLEVEL), and powering on takes 512 ms (ONLEVEL).
/// OFFLEVEL, the forced power-off, is left as found.
const IRQ_LEVEL_MASK: u8 = 0b11 << 4;
const IRQ_LEVEL_1S: u8 = 0b00 << 4;
const ON_LEVEL_MASK: u8 = 0b11;
const ON_LEVEL_512MS: u8 = 0b01;

/// A press of the power key, as the PMIC tells it apart.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum PowerKey {
    Short,
    Long,
}

/// What powered the PMIC on, and what last powered it off, from `REG_POWER_ON_SOURCE` and
/// `REG_POWER_OFF_SOURCE`.
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct PowerSources {
    pub on: u8,
    pub off: u8,
}

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
        self.write_reg(REG_IRQ_ENABLE1, SHORT_PRESS | LONG_PRESS)
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

    /// Sets the key's long press and power-on times, and has a hold past OFFLEVEL power off, so
    /// the board can be switched off when the firmware does not answer.
    pub async fn configure_power_key(&mut self) -> Result<(), I::Error> {
        let levels = self.read_reg(REG_KEY_LEVELS).await?;
        self.write_reg(
            REG_KEY_LEVELS,
            levels & !(IRQ_LEVEL_MASK | ON_LEVEL_MASK) | IRQ_LEVEL_1S | ON_LEVEL_512MS,
        )
        .await?;
        let enable = self.read_reg(REG_POWER_OFF_ENABLE).await?;
        self.write_reg(REG_POWER_OFF_ENABLE, enable & !KEY_RESTARTS | KEY_POWER_OFF)
            .await
    }

    pub async fn power_sources(&mut self) -> Result<PowerSources, I::Error> {
        Ok(PowerSources {
            on: self.read_reg(REG_POWER_ON_SOURCE).await?,
            off: self.read_reg(REG_POWER_OFF_SOURCE).await?,
        })
    }

    /// Takes the key press latched since the last call. Two presses between calls give the
    /// long one.
    pub async fn take_key_press(&mut self) -> Result<Option<PowerKey>, I::Error> {
        let status = self.read_reg(REG_IRQ_STATUS1).await? & (SHORT_PRESS | LONG_PRESS);
        if status == 0 {
            return Ok(None);
        }
        self.write_reg(REG_IRQ_STATUS1, status).await?;
        Ok(Some(if status & LONG_PRESS != 0 {
            PowerKey::Long
        } else {
            PowerKey::Short
        }))
    }

    /// Powers every output off but the RTC LDO; the key powers the board on again. The
    /// interrupts go first, since the PMIC can be set to power on while its IRQ pin is low.
    /// Returns only if a write fails.
    pub async fn power_off(&mut self) -> Result<(), I::Error> {
        for enable in [REG_IRQ_ENABLE0, REG_IRQ_ENABLE1, REG_IRQ_ENABLE2] {
            self.write_reg(enable, 0x00).await?;
        }
        for status in [REG_IRQ_STATUS0, REG_IRQ_STATUS1, REG_IRQ_STATUS2] {
            self.write_reg(status, 0xFF).await?;
        }
        let common = self.read_reg(REG_COMMON).await?;
        self.write_reg(REG_COMMON, common | SOFT_POWER_OFF).await
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
