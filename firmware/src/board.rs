// Board constants for the Waveshare ESP32-S3-Touch-AMOLED-1.75.
//
// GPIO numbers are not here. esp-hal takes typed peripheral singletons, so a `u8` pin number can
// never reach it, and an unreferenced number drifts from the hardware without anything noticing.
// The binding sites in `main.rs` are the live record; `docs/hardware-notes.md` has the pin map.
//
// An I2C address lives with whoever passes it. Drivers in `src/peripherals/` own theirs privately;
// the ones below are here because an external crate takes the address as an argument.

// Display, CO5300
pub use octowhere_ui::board::{LCD_HEIGHT, LCD_WIDTH};
/// The panel as its controller drives it.
///
/// TE pulses at scan line 150. A full flush takes about as long as the panel's scan, so one
/// started at the blanking races the scan and tears wherever it falls behind. Starting this far
/// behind the scan keeps the whole write on the far side of it: the lead must exceed the scan
/// time less the shortest full flush, and the flush must end before the next scan reaches the
/// rows it is writing.
pub const DISPLAY: co5300::Config = co5300::Config {
    width: LCD_WIDTH,
    height: LCD_HEIGHT,
    column_offset: 6,
    row_offset: 0,
    te_line: 150,
};

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
