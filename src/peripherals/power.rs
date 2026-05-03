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

    /// Initialize the AXP2101: enable power rails, disable IRQs, enable ADC.
    pub async fn init(&mut self) -> Result<(), I::Error> {
        // CRITICAL: Enable power rails for display
        // DC1 = 3300mV (main 3.3V rail)
        // DCDC1 voltage: reg 0x82, value = (3300 - 1500) / 100 = 18
        self.write_reg(REG_DC_VOL0, 18).await?;
        // Enable DC1: reg 0x80, bit 0 = DC1 enable
        let dc_ctrl = self.read_reg(REG_DC_ONOFF).await?;
        self.write_reg(REG_DC_ONOFF, dc_ctrl | 0x01).await?;

        // ALDO1 = 3300mV (display/peripheral power)
        // ALDO1 voltage: reg 0x92, value = (3300 - 500) / 100 = 28
        self.write_reg(REG_LDO_VOL0, 28).await?;
        // Enable ALDO1: reg 0x90, bit 0 = ALDO1 enable
        let ldo_ctrl = self.read_reg(REG_LDO_ONOFF0).await?;
        self.write_reg(REG_LDO_ONOFF0, ldo_ctrl | 0x01).await?;

        // === Disable unused rails (save power) ===
        // (Skip for now - don't touch what's already configured by bootloader)

        self.write_reg(REG_IRQ_ENABLE0, 0x00).await?;
        self.write_reg(REG_IRQ_ENABLE1, 0x00).await?;
        self.write_reg(REG_IRQ_ENABLE2, 0x00).await?;
        self.write_reg(REG_IRQ_STATUS0, 0xFF).await?;
        self.write_reg(REG_IRQ_STATUS1, 0xFF).await?;
        self.write_reg(REG_IRQ_STATUS2, 0xFF).await?;

        // ADC_ENABLE bit layout (AXP2101):
        //   bit 0 = VBAT
        //   bit 1 = TS
        //   bit 2 = VBUS
        //   bit 3 = VSYS
        //   bit 4 = die temperature
        // 0b00011101 = VBAT+VBUS+VSYS+TEMP.
        self.write_reg(REG_ADC_ENABLE, 0b0001_1101).await?;

        Ok(())
    }

    /// Read battery voltage in millivolts.
    pub async fn get_battery_voltage(&mut self) -> Result<u16, I::Error> {
        let high = self.read_reg(REG_VBAT_H).await? as u16;
        let low = self.read_reg(REG_VBAT_L).await? as u16;
        // 14-bit ADC, 1.1mV per LSB
        Ok(((high << 8) | low) & 0x3FFF)
    }

    /// Read VBUS voltage in millivolts.
    pub async fn get_vbus_voltage(&mut self) -> Result<u16, I::Error> {
        let high = self.read_reg(REG_VBUS_H).await? as u16;
        let low = self.read_reg(REG_VBUS_L).await? as u16;
        Ok(((high << 8) | low) & 0x3FFF)
    }

    /// Read system voltage in millivolts.
    pub async fn get_system_voltage(&mut self) -> Result<u16, I::Error> {
        let high = self.read_reg(REG_VSYS_H).await? as u16;
        let low = self.read_reg(REG_VSYS_L).await? as u16;
        Ok(((high << 8) | low) & 0x3FFF)
    }

    /// Read battery percentage (0-100).
    pub fn get_battery_percent(&mut self) -> impl Future<Output = Result<u8, I::Error>> {
        self.read_reg(REG_BAT_PERCENT)
    }

    /// Check if charging.
    pub fn is_charging(&mut self) -> impl Future<Output = Result<bool, I::Error>> {
        self.read_reg(REG_CHG_STATUS).map_ok(|status| {
            // Bits [7:5] = charger status, 001/010/011 = charging
            let chg = (status >> 5) & 0x07;
            (1..=3).contains(&chg)
        })
    }

    /// Check if VBUS (USB) is connected.
    pub fn is_vbus_in(&mut self) -> impl Future<Output = Result<bool, I::Error>> {
        self.read_reg(REG_STATUS1).map_ok(|status| {
            status & 0x20 != 0 // Bit 5: VBUS present
        })
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
        // ADC_ENABLE bit layout (AXP2101):
        //   bit 0 = VBAT
        //   bit 1 = TS
        //   bit 2 = VBUS
        //   bit 3 = VSYS
        //   bit 4 = die temperature
        // Previous init wrote 0b00011101 = VBAT+VBUS+VSYS+TEMP.
        // Drop TEMP (bit 4) to 0b00001101 = VBAT+VBUS+VSYS only.
        self.write_reg(REG_ADC_ENABLE, 0b00001101)
    }
}
