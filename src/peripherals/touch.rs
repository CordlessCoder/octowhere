use axp2101_embedded::AXP2101_CHIP_ID;
use embedded_hal::digital::OutputPin;
use embedded_hal_async::{digital::Wait, i2c::I2c};
use esp_println::dbg;
use log::{Level, log};

use crate::peripherals::i2c_helper;

const CST9220_CHIP_ID: u16 = 0x9220;
const CST9217_CHIP_ID: u16 = 0x9217;

const REG_READ: u16 = 0xD000;
const REG_DEBUG_MODE: u16 = 0xD101;
const REG_SLEEP_MODE: u16 = 0xD105;
const REG_DIS_LOW_POWER_SCAN_MODE: u16 = 0xD106;
const REG_NORMAL_MODE: u16 = 0xD109;
const REG_RAW_MODE: u16 = 0xD10A;
const REG_DIFF_MODE: u16 = 0xD10D;
const REG_BASE_LINE_MODE: u16 = 0xD10E;
const REG_LOW_POWER_MODE: u16 = 0xD10F;
const REG_FACTORY_MODE: u16 = 0xD114;

const REG_RESOLUTION: u16 = 0xD1F8;
const REG_VERSION: u16 = 0xD208;
const REG_CHECKCODE: u16 = 0xD1FC;
const REG_PROJECT_ID: u16 = 0xD204;

const CST92XX_BOOT_ADDRESS: u8 = 0x5A;
const CST92XX_ACK: u8 = 0xAB;
const CST92XX_MEM_SIZE: u32 = 0x007F80;

const MAX_FINGER_NUM: usize = 2;
const PROGRAM_PAGE_SIZE: u8 = 128;

const ADDR: u8 = 0x5A;
const ACK_VALUE: u8 = 0xAB;

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

pub struct Cst9217<I, INT, RST, DELAY> {
    i2c: I,
    reset: RST,
    int: INT,
    delay: DELAY,
}

#[derive(Debug)]
pub enum Cst9217Error<I2CError> {
    I2CError(I2CError),
    IDMismatch,
    NoFirmware,
    InvalidCheckcode,
}

impl<I2CError> From<I2CError> for Cst9217Error<I2CError> {
    fn from(value: I2CError) -> Self {
        Cst9217Error::I2CError(value)
    }
}

impl<I: I2c, RST: OutputPin, INT, DELAY: embedded_hal_async::delay::DelayNs>
    Cst9217<I, INT, RST, DELAY>
{
    pub fn new(i2c: I, reset: RST, int: INT, delay: DELAY) -> Self {
        Self {
            i2c,
            int,
            reset,
            delay,
        }
    }

    pub async fn init(&mut self) -> Result<(), Cst9217Error<I::Error>> {
        i2c_helper::write_wide_reg(&mut self.i2c, ADDR, REG_DEBUG_MODE, 0x01).await?;
        self.delay.delay_ms(10).await;
        let mut buf = [0u8; 4];
        self.i2c
            .write_read(ADDR, &REG_CHECKCODE.to_be_bytes(), &mut buf)
            .await?;
        let checkcode = u32::from_le_bytes(buf);
        if (checkcode & 0xffff0000) != 0xCACA0000 {
            log!(Level::Error, "Firmware info read error");
            return Err(Cst9217Error::InvalidCheckcode);
        }

        self.i2c
            .write_read(ADDR, &REG_RESOLUTION.to_be_bytes(), &mut buf)
            .await?;
        let width = u16::from_le_bytes(buf[0..2].try_into().unwrap());
        let height = u16::from_le_bytes(buf[2..4].try_into().unwrap());
        // TODO: Use checkcode, width, height

        self.i2c
            .write_read(ADDR, &REG_PROJECT_ID.to_be_bytes(), &mut buf)
            .await
            .unwrap();
        let touch_project_id = u16::from_le_bytes(buf[0..2].try_into().unwrap());
        let chip_id = u16::from_le_bytes(buf[2..4].try_into().unwrap());
        if chip_id != CST9217_CHIP_ID {
            return Err(Cst9217Error::IDMismatch);
        }

        let mut buf = [0u8; 8];
        self.i2c
            .write_read(ADDR, &REG_VERSION.to_be_bytes(), &mut buf)
            .await?;

        let fw_version = u32::from_le_bytes(buf[0..4].try_into().unwrap());
        let checksum = u32::from_le_bytes(buf[4..8].try_into().unwrap());

        log!(
            Level::Info,
            "Chip IC version: 0x{fw_version:x}, checksum: 0x{checksum:0>16x}"
        );

        if fw_version == 0xA5A5A5A5 {
            log!(Level::Error, "Chip ic don't have firmware");
            return Err(Cst9217Error::NoFirmware);
        }

        Ok(())
    }

    pub async fn reset(&mut self) -> Result<(), RST::Error> {
        self.reset.set_low()?;
        self.delay.delay_ms(10).await;
        self.reset.set_high()?;
        self.delay.delay_ms(30).await;
        Ok(())
    }
    // pub fn sleep(&mut self) -> impl Future<Output = Result<(), I::Error>> {
    //     todo!("Need to implement set_mode")
    // }
    pub fn wakeup(&self) {}
    pub fn idle(&self) {}

    pub fn is_pressed(&self) -> bool {
        todo!()
    }

    // pub fn getSuppor
}
