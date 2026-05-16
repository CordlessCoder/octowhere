// Board pin definitions for Waveshare ESP32-S3-Touch-AMOLED-1.75
// Reference: pin_config.h from Waveshare Arduino examples

use embedded_hal::delay::DelayNs;

pub const CACHE_LINE: usize = 64;

// Display, CO5300
pub const LCD_SDIO0: u8 = 4;
pub const LCD_SDIO1: u8 = 5;
pub const LCD_SDIO2: u8 = 6;
pub const LCD_SDIO3: u8 = 7;
pub const LCD_SCLK: u8 = 38;
pub const LCD_CS: u8 = 12;
pub const LCD_RESET: u8 = 39;

pub const LCD_WIDTH: u16 = 466;
pub const LCD_HEIGHT: u16 = 466;
pub const LCD_COL_OFFSET: u16 = 6;
pub const LCD_ROW_OFFSET: u16 = 0;
// Display TE (Tearing Effect sync)
pub const LCD_TE: u8 = 13;

// I2C
pub const I2C_SDA: u8 = 15;
pub const I2C_SCL: u8 = 14;
pub const I2C_FREQ_HZ: u32 = 400_000;

// TOUCH
pub const TP_INT: u8 = 11;
pub const TP_RESET: u8 = 40;
pub const TP_I2C_ADDR: u8 = 0x38;

// Power: AXP2101
pub const PMIC_I2C_ADDR: u8 = 0x34;

// IMU
pub const IMU_I2C_ADDR: u8 = 0x6B;

// RTC
pub const RTC_I2C_ADDR: u8 = 0x51;

// SD
pub const SD_CLK: u8 = 2;
pub const SD_CMD: u8 = 1;
pub const SD_DATA: u8 = 3;
pub const SD_CS: u8 = 41;

// // Audio Codec: ES8311
// // #define MCLKPIN             42
// // #define BCLKPIN              9
// // #define WSPIN               45
// // #define DOPIN               10
// // #define DIPIN                8
// // #define PA                  46
// //
// // #define I2S_DI_IO 10
// // #define I2S_WS_IO 45
// // #define I2S_DO_IO 8
// pub const I2S_MCLK: u8 = 16;
// pub const I2S_SCLK: u8 = 9;  // BCLK
// pub const I2S_LRCK: u8 = 45;  // WS
// pub const I2S_DSDIN: u8 = 40; // DAC data in (speaker)
// pub const I2S_ASDOUT: u8 = 42; // ADC data out (microphone)
// pub const PA_CTRL: u8 = 46;   // Power amplifier enable

// IMU Interrupt
pub const IMU_INT: u8 = 21;

// RTC Interrupt
pub const RTC_INT: u8 = 39;

// Buttons
pub const BOOT_BUTTON: u8 = 0;
pub const PWR_BUTTON: u8 = 10;

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
