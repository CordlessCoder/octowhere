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

const CTRL1_STOP: u8 = 1 << 5;
const CTRL1_12_24: u8 = 1 << 1;

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
    #[must_use]
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

    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.seconds < 60
            && self.minutes < 60
            && self.hours < 24
            && self.weekday < 7
            && (1..=12).contains(&self.month)
            && (1..=days_in_month(self.year, self.month)).contains(&self.day)
    }
}

#[derive(Debug)]
pub enum RtcError<E> {
    I2c(E),
    InvalidDateTime,
}

pub struct Pcf85063aRtc<I> {
    i2c: I,
    oscillator_stopped: bool,
}

impl<I: I2c> Pcf85063aRtc<I> {
    pub fn new(i2c: I) -> Self {
        Self {
            i2c,
            oscillator_stopped: false,
        }
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
        let new_ctrl1 = ctrl1 & !(CTRL1_STOP | CTRL1_12_24);
        if new_ctrl1 != ctrl1 {
            self.write_reg(REG_CTRL1, new_ctrl1).await?;
        }
        Ok(())
    }

    /// Read current date/time.
    pub async fn get_time(&mut self) -> Result<DateTime, RtcError<I::Error>> {
        // Read all time registers in one burst (7 bytes from 0x04)
        let mut buf = [0u8; 7];
        self.i2c
            .write_read(PCF85063A_ADDR, &[REG_SECONDS], &mut buf)
            .await
            .map_err(RtcError::I2c)?;
        self.oscillator_stopped = buf[0] & 0x80 != 0;

        let dt = DateTime {
            seconds: bcd_to_dec_checked(buf[0] & 0x7F),
            minutes: bcd_to_dec_checked(buf[1] & 0x7F),
            hours: bcd_to_dec_checked(buf[2] & 0x3F),
            day: bcd_to_dec_checked(buf[3] & 0x3F),
            weekday: buf[4] & 0x07,
            month: bcd_to_dec_checked(buf[5] & 0x1F),
            year: bcd_to_dec_checked(buf[6]),
        };
        if dt.is_valid() {
            Ok(dt)
        } else {
            Err(RtcError::InvalidDateTime)
        }
    }

    #[must_use]
    pub fn oscillator_stopped(&self) -> bool {
        self.oscillator_stopped
    }

    #[must_use]
    pub fn time_valid(&self) -> bool {
        !self.oscillator_stopped
    }

    /// Set date/time.
    pub async fn set_time(&mut self, dt: &DateTime) -> Result<(), RtcError<I::Error>> {
        if !dt.is_valid() {
            return Err(RtcError::InvalidDateTime);
        }

        // Stop oscillator
        let ctrl1 = self.read_reg(REG_CTRL1).await.map_err(RtcError::I2c)?;
        self.write_reg(REG_CTRL1, ctrl1 | CTRL1_STOP)
            .await
            .map_err(RtcError::I2c)?;

        let data = [
            REG_SECONDS,
            dec_to_bcd(dt.seconds),
            dec_to_bcd(dt.minutes),
            dec_to_bcd(dt.hours),
            dec_to_bcd(dt.day),
            dt.weekday,
            dec_to_bcd(dt.month),
            dec_to_bcd(dt.year),
        ];
        let write_result = self
            .i2c
            .write(PCF85063A_ADDR, &data)
            .await
            .map_err(RtcError::I2c);

        // Restart oscillator
        let restart_result = self
            .write_reg(REG_CTRL1, ctrl1 & !CTRL1_STOP)
            .await
            .map_err(RtcError::I2c);
        write_result?;
        restart_result?;
        self.oscillator_stopped = false;
        Ok(())
    }
}

fn bcd_to_dec(bcd: u8) -> u8 {
    (bcd / 16) * 10 + (bcd % 16)
}

fn bcd_to_dec_checked(bcd: u8) -> u8 {
    if bcd & 0x0F > 9 || bcd >> 4 > 9 {
        0xFF
    } else {
        bcd_to_dec(bcd)
    }
}

fn days_in_month(year: u8, month: u8) -> u8 {
    match month {
        2 if year.is_multiple_of(4) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        _ => 0,
    }
}

fn dec_to_bcd(dec: u8) -> u8 {
    ((dec / 10) * 16) | (dec % 10)
}

#[cfg(test)]
mod tests {
    use super::{CTRL1_12_24, CTRL1_STOP, DateTime, bcd_to_dec_checked, days_in_month};

    #[test]
    fn init_clears_stop_and_selects_24_hour_mode() {
        let ctrl1 = 0xFF;
        let configured = ctrl1 & !(CTRL1_STOP | CTRL1_12_24);
        assert_eq!(configured, 0xDD);
        assert_eq!(configured & (1 << 2), 1 << 2);
    }

    #[test]
    fn month_lengths_include_leap_years() {
        assert_eq!(days_in_month(24, 2), 29);
        assert_eq!(days_in_month(25, 2), 28);
        assert_eq!(days_in_month(25, 4), 30);
        assert_eq!(days_in_month(25, 1), 31);
    }

    #[test]
    fn invalid_bcd_and_calendar_values_are_rejected() {
        assert_eq!(bcd_to_dec_checked(0x6A), 0xFF);
        assert!(!DateTime::new(25, 2, 29, 0, 0, 0).is_valid());
        assert!(DateTime::new(24, 2, 29, 0, 0, 0).is_valid());
    }
}
