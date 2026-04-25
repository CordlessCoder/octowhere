use embedded_graphics::prelude::Size;
use embedded_hal::digital::OutputPin;
use embedded_hal_async::i2c::I2c;
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
const READ_BUF_SIZE: usize = MAX_FINGER_NUM * 5 + 5;

const ACK_VALUE: u8 = 0xAB;

// TODO: Impl Lpscan mode

#[repr(u8)]
#[derive(Debug, Clone, Copy)]
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

#[derive(Debug, Clone, Copy, Default)]
pub struct Cst9217Config {
    pub swap_xy: bool,
    pub scale_x: Option<f32>,
    pub scale_y: Option<f32>,
    pub mirror_x: bool,
    pub mirror_y: bool,
}

impl Cst9217Config {
    fn apply(&self, width: u16, height: u16, point: &mut TouchPoint) {
        if self.swap_xy {
            core::mem::swap(&mut point.x, &mut point.y);
        }
        if let Some(scale) = self.scale_x {
            point.x = (point.x as f32 * scale) as u16;
        }
        if let Some(scale) = self.scale_y {
            point.y = (point.y as f32 * scale) as u16;
        }
        if self.mirror_x {
            point.x = width.saturating_sub(point.x);
        }
        if self.mirror_y {
            point.y = height.saturating_sub(point.y);
        }
        point.x = point.x.min(width);
        point.y = point.y.min(width);
    }
}

pub struct Cst9217<I, INT, RST, DELAY> {
    i2c: I,
    addr: u8,
    reset: RST,
    int: INT,
    delay: DELAY,
    width: u16,
    height: u16,
    config: Cst9217Config,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TouchPoint {
    pub x: u16,
    pub y: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TouchData {
    CoverGesture,
    Points(heapless::Vec<TouchPoint, 5, u8>),
}

impl<I: I2c, RST: OutputPin, INT, DELAY: embedded_hal_async::delay::DelayNs>
    Cst9217<I, INT, RST, DELAY>
{
    pub fn new(i2c: I, reset: RST, int: INT, delay: DELAY) -> Self {
        Self {
            i2c,
            addr: 0x5A,
            int,
            reset,
            delay,
            width: 0,
            height: 0,
            config: Default::default(),
        }
    }
    pub fn with_address(mut self, addr: u8) -> Self {
        self.addr = addr;
        self
    }
    pub fn set_config(&mut self, config: Cst9217Config) {
        self.config = config;
    }

    pub async fn init(&mut self) -> Result<(), Cst9217Error<I::Error>> {
        i2c_helper::write_wide_reg(&mut self.i2c, self.addr, REG_DEBUG_MODE, 0x01).await?;
        self.delay.delay_ms(10).await;
        let mut buf = [0u8; 4];
        self.i2c
            .write_read(self.addr, &REG_CHECKCODE.to_be_bytes(), &mut buf)
            .await?;
        let checkcode = u32::from_le_bytes(buf);
        if (checkcode & 0xffff0000) != 0xCACA0000 {
            log!(Level::Error, "Firmware info read error");
            return Err(Cst9217Error::InvalidCheckcode);
        }

        self.i2c
            .write_read(self.addr, &REG_RESOLUTION.to_be_bytes(), &mut buf)
            .await?;
        self.width = u16::from_le_bytes(buf[0..2].try_into().unwrap());
        self.height = u16::from_le_bytes(buf[2..4].try_into().unwrap());

        self.i2c
            .write_read(self.addr, &REG_PROJECT_ID.to_be_bytes(), &mut buf)
            .await
            .unwrap();
        let _touch_project_id = u16::from_le_bytes(buf[0..2].try_into().unwrap());
        let chip_id = u16::from_le_bytes(buf[2..4].try_into().unwrap());
        if chip_id != CST9217_CHIP_ID {
            return Err(Cst9217Error::IDMismatch);
        }

        let mut buf = [0u8; 8];
        self.i2c
            .write_read(self.addr, &REG_VERSION.to_be_bytes(), &mut buf)
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
    pub async fn sleep(&mut self) -> Result<(), I::Error> {
        self.set_mode(Cst9217RunMode::DebugInfo).await?;
        self.i2c
            .write(self.addr, &REG_SLEEP_MODE.to_be_bytes())
            .await
    }
    pub async fn set_mode(&mut self, mode: Cst9217RunMode) -> Result<(), I::Error> {
        let write = match mode {
            Cst9217RunMode::Normal => REG_NORMAL_MODE,
            Cst9217RunMode::DebugDiff => REG_DIFF_MODE,
            Cst9217RunMode::DebugRawdata => REG_RAW_MODE,
            Cst9217RunMode::DebugInfo => REG_DEBUG_MODE,
            Cst9217RunMode::Factory => REG_FACTORY_MODE,
            _ => unimplemented!(),
        };
        self.i2c.write(self.addr, &write.to_be_bytes()).await
    }

    pub async fn read_touch_data(&mut self) -> Result<TouchData, I::Error> {
        let mut buf = [0u8; READ_BUF_SIZE];
        self.i2c
            .write_read(self.addr, &REG_READ.to_be_bytes(), &mut buf)
            .await?;
        let ack = [
            REG_READ.to_be_bytes()[0],
            REG_READ.to_be_bytes()[1],
            CST92XX_ACK,
        ];
        self.i2c.write(self.addr, &ack).await?;
        // Check for cover screen gesture
        if buf[4] >> 7 == 1 {
            return Ok(TouchData::CoverGesture);
        }
        let mut points = heapless::Vec::new();

        let point_count = buf[5] & 0x7F;
        if point_count > MAX_FINGER_NUM as u8 || point_count == 0 {
            return Ok(TouchData::Points(points));
        }

        for i in 0..point_count {
            let data_idx = (i * 5) + if i == 0 { 0 } else { 2 };
            let data = &buf[data_idx as usize..][..4];
            let id = data[0] >> 4;
            let event = data[0] & 0x0F;
            let x = ((data[1] as u16) << 4) | (data[3] >> 4) as u16;
            let y = ((data[2] as u16) << 4) | (data[3] & 0x0F) as u16;
            if event == 0x06 && id < MAX_FINGER_NUM as u8 {
                let mut point = TouchPoint { x, y };
                self.config.apply(self.width, self.height, &mut point);
                _ = points.push(point);
            }
        }
        // NOTE: Should we care about this?
        // Swap XY or mirroring coordinates,if set
        // updateXY(_touchPoints);
        Ok(TouchData::Points(points))
    }
    pub fn resolution(&self) -> Size {
        Size {
            width: self.width as u32,
            height: self.height as u32,
        }
    }
    // pub fn is_pressed(&self) -> bool {
    //     todo!()
    // }

    // pub fn getSuppor
}
