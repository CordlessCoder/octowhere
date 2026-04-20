// PCF85063A RTC driver
// Reference: OLEDS3Watch/components/bsp_extra/src/pcf85063a.c
// I2C address 0x51, BCD encoded time registers
use embedded_hal_async::i2c::I2c;

use crate::peripherals::i2c_helper;

const PCF85063A_ADDR: u8 = 0x51;

// Registers
const REG_CTRL1: u8 = 0x00;
#[expect(unused)]
const REG_CTRL2: u8 = 0x01;
const REG_SECONDS: u8 = 0x04;
const REG_MINUTES: u8 = 0x05;
const REG_HOURS: u8 = 0x06;
const REG_DAYS: u8 = 0x07;
const REG_WEEKDAYS: u8 = 0x08;
const REG_MONTHS: u8 = 0x09;
const REG_YEARS: u8 = 0x0A;

#[derive(Debug, Clone, Copy)]
pub struct DateTime {
    pub seconds: u8,
    pub minutes: u8,
    pub hours: u8,
    pub day: u8,
    pub weekday: u8,
    pub month: u8,
    pub year: u8, // 0-99 (2000-2099)
}

impl DateTime {
    pub fn new(year: u8, month: u8, day: u8, hours: u8, minutes: u8, seconds: u8) -> Self {
        Self {
            seconds,
            minutes,
            hours,
            day,
            weekday: 0,
            month,
            year,
        }
    }
}

pub struct Pcf85063aRtc<I> {
    i2c: I,
}

impl<I: I2c> Pcf85063aRtc<I> {
    pub fn new(i2c: I) -> Self {
        Self { i2c }
    }

    fn read_reg(&mut self, reg: u8) -> impl Future<Output = Result<u8, I::Error>> {
        i2c_helper::read_reg_byte(&mut self.i2c, PCF85063A_ADDR, reg)
    }

    fn write_reg(&mut self, reg: u8, val: u8) -> impl Future<Output = Result<(), I::Error>> {
        i2c_helper::write_reg(&mut self.i2c, PCF85063A_ADDR, reg, val)
    }

    /// Initialize RTC: ensure oscillator running, 24h mode.
    pub async fn init(&mut self) -> Result<(), I::Error> {
        let ctrl1 = self.read_reg(REG_CTRL1).await?;
        // Clear STOP bit (bit 5) to start oscillator
        // Clear 12_24 bit (bit 2) for 24-hour mode
        let new_ctrl1 = ctrl1 & !(0x20 | 0x04);
        if new_ctrl1 != ctrl1 {
            self.write_reg(REG_CTRL1, new_ctrl1).await?;
        }
        Ok(())
    }

    /// Read current date/time.
    pub async fn get_time(&mut self) -> Result<DateTime, I::Error> {
        // Read all time registers in one burst (7 bytes from 0x04)
        let mut buf = [0u8; 7];
        self.i2c
            .write_read(PCF85063A_ADDR, &[REG_SECONDS], &mut buf)
            .await?;

        Ok(DateTime {
            seconds: bcd_to_dec(buf[0] & 0x7F), // mask OS bit
            minutes: bcd_to_dec(buf[1] & 0x7F),
            hours: bcd_to_dec(buf[2] & 0x3F), // 24h mode
            day: bcd_to_dec(buf[3] & 0x3F),
            weekday: buf[4] & 0x07,
            month: bcd_to_dec(buf[5] & 0x1F),
            year: bcd_to_dec(buf[6]),
        })
    }

    /// Set date/time.
    pub async fn set_time(&mut self, dt: &DateTime) -> Result<(), I::Error> {
        // Stop oscillator
        let ctrl1 = self.read_reg(REG_CTRL1).await?;
        self.write_reg(REG_CTRL1, ctrl1 | 0x20).await?;

        // Write time registers
        self.write_reg(REG_SECONDS, dec_to_bcd(dt.seconds)).await?;
        self.write_reg(REG_MINUTES, dec_to_bcd(dt.minutes)).await?;
        self.write_reg(REG_HOURS, dec_to_bcd(dt.hours)).await?;
        self.write_reg(REG_DAYS, dec_to_bcd(dt.day)).await?;
        self.write_reg(REG_WEEKDAYS, dt.weekday).await?;
        self.write_reg(REG_MONTHS, dec_to_bcd(dt.month)).await?;
        self.write_reg(REG_YEARS, dec_to_bcd(dt.year)).await?;

        // Restart oscillator
        self.write_reg(REG_CTRL1, ctrl1 & !0x20).await?;
        Ok(())
    }
}

fn bcd_to_dec(bcd: u8) -> u8 {
    (bcd / 16) * 10 + (bcd % 16)
}

fn dec_to_bcd(dec: u8) -> u8 {
    ((dec / 10) * 16) | (dec % 10)
}
