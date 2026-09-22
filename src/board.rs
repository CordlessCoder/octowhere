// Board constants for the Waveshare ESP32-S3-Touch-AMOLED-1.75.
//
// GPIO numbers are not here. esp-hal takes typed peripheral singletons, so a `u8` pin number can
// never reach it, and an unreferenced number drifts from the hardware without anything noticing.
// The binding sites in `main.rs` are the live record; `docs/hardware-notes.md` has the pin map.
//
// An I2C address lives with whoever passes it. Drivers in `src/peripherals/` own theirs privately;
// the ones below are here because an external crate takes the address as an argument.

use embedded_hal::delay::DelayNs;

pub const CACHE_LINE: usize = 64;

// Display, CO5300
pub const LCD_WIDTH: u16 = 466;
pub const LCD_HEIGHT: u16 = 466;
pub const LCD_COL_OFFSET: u16 = 6;
pub const LCD_ROW_OFFSET: u16 = 0;

// I2C
pub const I2C_FREQ_HZ: u32 = 400_000;
pub const I2C_POWER_SETTLE_MS: u64 = 80;

// TCA9554 expander, and the bit index of each of its eight lines
pub const TCA9554_I2C_ADDR: u8 = 0x20;
pub const EXIO_LORA_RESET: u8 = 0;
pub const EXIO_LORA_RX_SWITCH: u8 = 1;
pub const EXIO_LORA_TX_SWITCH: u8 = 2;
pub const EXIO_RTC_INT: u8 = 3;
pub const EXIO_SYS_OUT: u8 = 4;
pub const EXIO_AXP_IRQ: u8 = 5;
pub const EXIO_QMI_INT1: u8 = 6;
pub const EXIO_GPS_RESET: u8 = 7;

// IMU, QMI8658 through `ph-qmi8658`
pub const IMU_I2C_ADDR: u8 = 0x6B;

#[inline]
pub fn delay_ms(ms: u32) {
    esp_hal::delay::Delay::new().delay_ms(ms);
}

#[inline]
pub fn delay_us(us: u32) {
    esp_hal::delay::Delay::new().delay_us(us);
}

#[inline]
pub fn delay_ms_async(ms: u32) -> impl Future<Output = ()> {
    embassy_time::Timer::after_millis(ms as u64)
}

#[inline]
pub fn delay_us_async(us: u32) -> impl Future<Output = ()> {
    embassy_time::Timer::after_micros(us as u64)
}
