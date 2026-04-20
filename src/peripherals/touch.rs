use embedded_hal_async::i2c::I2c;

// /* CST9217 registers */
// #define ESP_LCD_TOUCH_CST9217_DATA_REG 0xD000
// #define ESP_LCD_TOUCH_CST9217_PROJECT_ID_REG 0xD204
// #define ESP_LCD_TOUCH_CST9217_CMD_MODE_REG   0xD101
// #define ESP_LCD_TOUCH_CST9217_CHECKCODE_REG  0xD1FC
// #define ESP_LCD_TOUCH_CST9217_RESOLUTION_REG 0xD1F8
//
// /* CST9217 parameters */
// #define CST9217_MAX_TOUCH_POINTS 1
// #define CST9217_DATA_LENGTH (CST9217_MAX_TOUCH_POINTS * 5 + 5)

const CST9217_ID: u16 = 0x9217;
const CST9217_ACK_VALUE: u8 = 0xAB;

#[repr(u8)]
pub enum Cst9217RunMode {
    Normal = 0x00,
    LowPower = 0x01,
    DeepSleep = 0x02,
    Wakeup = 0x03,
    DebugDiff = 0x04,
    DebugRawdata = 0x05,
    Factory = 0x06,
    DebugInfo = 0x07,
    UpdateFw = 0x08,
    FactoryHighdrv = 0x10,
    FactoryLowdrv = 0x11,
    FactoryShort = 0x12,
    Lpscan = 0x13,
}

pub struct Cst9217<I> {
    i2c: I,
}

impl<I: I2c> Cst9217<I> {
    pub fn new(i2c: I) -> Self {
        Self { i2c }
    }

    pub fn reset(&self) {}
    pub fn sleep(&self) {}
    pub fn wakeup(&self) {}
    pub fn idle(&self) {}

    pub fn is_pressed(&self) -> bool {
        todo!()
    }

    // pub fn getSuppor
}
