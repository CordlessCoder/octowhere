use embassy_time::{Duration, Timer};
use embedded_hal_async::i2c::I2c;

const BMM350_ADDR: u8 = 0x14;
const CHIP_ID: u8 = 0x33;

const REG_CHIP_ID: u8 = 0x00;
const REG_PMU_CMD_AGGR_SET: u8 = 0x04;
const REG_PMU_CMD_AXIS_EN: u8 = 0x05;
const REG_PMU_CMD: u8 = 0x06;
const REG_INT_CTRL: u8 = 0x2E;
const REG_INT_STATUS: u8 = 0x30;
const REG_MAG_X_XLSB: u8 = 0x31;
const REG_OTP_CMD: u8 = 0x50;
const REG_OTP_DATA_MSB: u8 = 0x52;
const REG_OTP_DATA_LSB: u8 = 0x53;
const REG_OTP_STATUS: u8 = 0x55;
const REG_CMD: u8 = 0x7E;

const CMD_SOFT_RESET: u8 = 0xB6;
const PMU_NORMAL: u8 = 0x01;
const PMU_ODR_UPDATE: u8 = 0x02;
const PMU_BR: u8 = 0x07;
const PMU_FGR: u8 = 0x05;
const INT_DATA_READY_ENABLE: u8 = 1 << 7;
const INT_DATA_READY: u8 = 1 << 2;
const OTP_DIRECT_READ: u8 = 0x20;
const OTP_POWER_OFF: u8 = 0x80;
const OTP_DONE: u8 = 1;
const OTP_ERROR_MASK: u8 = 0xE0;
const OTP_WORDS: u8 = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RawMagneticData {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub temperature: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MagneticData {
    pub raw: RawMagneticData,
    pub sensor_time: u32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CompensatedMagneticData {
    pub x_microtesla: f32,
    pub y_microtesla: f32,
    pub z_microtesla: f32,
    pub temperature_celsius: f32,
    pub sensor_time: u32,
}

#[derive(Debug)]
pub enum MagnetometerError<E> {
    I2c(E),
    InvalidChipId(u8),
    OtpTimeout(u8),
    OtpError(u8),
}

#[derive(Clone, Copy)]
struct Compensation {
    offset: [f32; 3],
    sensitivity: [f32; 3],
    temperature_offset: f32,
    temperature_sensitivity: f32,
    temperature_coefficient: [f32; 3],
    temperature_scale: [f32; 3],
    temperature_reference: f32,
    cross_axis: [f32; 4],
}

pub struct Bmm350<I> {
    i2c: I,
    compensation: Option<Compensation>,
}

impl<I: I2c> Bmm350<I> {
    pub fn new(i2c: I) -> Self {
        Self {
            i2c,
            compensation: None,
        }
    }

    pub async fn init(&mut self) -> Result<(), MagnetometerError<I::Error>> {
        self.write_reg(REG_CMD, CMD_SOFT_RESET)
            .await
            .map_err(MagnetometerError::I2c)?;
        Timer::after(Duration::from_millis(24)).await;

        let chip_id = self
            .read_reg(REG_CHIP_ID)
            .await
            .map_err(MagnetometerError::I2c)?;
        if chip_id != CHIP_ID {
            return Err(MagnetometerError::InvalidChipId(chip_id));
        }

        self.compensation = Some(self.load_compensation().await?);
        self.write_reg(REG_OTP_CMD, OTP_POWER_OFF)
            .await
            .map_err(MagnetometerError::I2c)?;
        self.magnetic_reset()
            .await
            .map_err(MagnetometerError::I2c)?;

        Ok(())
    }

    /// Starts 100 Hz normal mode with two-sample averaging and all axes enabled.
    pub async fn start_normal_mode(&mut self) -> Result<(), I::Error> {
        self.write_reg(REG_PMU_CMD_AGGR_SET, 0x14).await?;
        self.write_reg(REG_PMU_CMD_AXIS_EN, 0x07).await?;
        self.write_reg(REG_INT_CTRL, INT_DATA_READY_ENABLE).await?;
        self.write_reg(REG_PMU_CMD, PMU_ODR_UPDATE).await?;
        Timer::after(Duration::from_millis(1)).await;
        self.write_reg(REG_PMU_CMD, PMU_NORMAL).await?;
        Timer::after(Duration::from_millis(38)).await;
        Ok(())
    }

    async fn magnetic_reset(&mut self) -> Result<(), I::Error> {
        self.write_reg(REG_PMU_CMD, PMU_BR).await?;
        Timer::after(Duration::from_millis(14)).await;
        self.write_reg(REG_PMU_CMD, PMU_FGR).await?;
        Timer::after(Duration::from_millis(18)).await;
        Ok(())
    }

    pub async fn data_ready(&mut self) -> Result<bool, I::Error> {
        Ok(self.read_reg(REG_INT_STATUS).await? & INT_DATA_READY != 0)
    }

    /// Reads one coherent sample. The sensor requires a burst read for this block.
    pub async fn read_data(&mut self) -> Result<MagneticData, I::Error> {
        let mut transfer = [0u8; 17];
        self.i2c
            .write_read(BMM350_ADDR, &[REG_MAG_X_XLSB], &mut transfer)
            .await?;
        let data = &transfer[2..];

        Ok(MagneticData {
            raw: RawMagneticData {
                x: decode_signed_24(data[0], data[1], data[2]),
                y: decode_signed_24(data[3], data[4], data[5]),
                z: decode_signed_24(data[6], data[7], data[8]),
                temperature: decode_signed_24(data[9], data[10], data[11]),
            },
            sensor_time: u32::from_le_bytes([data[12], data[13], data[14], 0]),
        })
    }

    #[must_use]
    pub fn compensate(&self, data: &MagneticData) -> Option<CompensatedMagneticData> {
        let compensation = self.compensation?;
        let raw = [
            data.raw.x as f32 * 0.007069979,
            data.raw.y as f32 * 0.007069979,
            data.raw.z as f32 * 0.007174964,
        ];
        let temperature = data.raw.temperature as f32 * 0.000981282 - 25.49;
        let temperature = (1.0 + compensation.temperature_sensitivity) * temperature
            + compensation.temperature_offset;
        let delta_temperature = temperature - compensation.temperature_reference;
        let mut magnetic = [0.0; 3];

        for axis in 0..3 {
            magnetic[axis] = (raw[axis] * (1.0 + compensation.sensitivity[axis])
                + compensation.offset[axis]
                + compensation.temperature_coefficient[axis] * delta_temperature)
                / (1.0 + compensation.temperature_scale[axis] * delta_temperature);
        }

        let denominator = 1.0 - compensation.cross_axis[1] * compensation.cross_axis[0];
        let x = (magnetic[0] - compensation.cross_axis[0] * magnetic[1]) / denominator;
        let y = (magnetic[1] - compensation.cross_axis[1] * magnetic[0]) / denominator;
        let z = magnetic[2]
            + (magnetic[0]
                * (compensation.cross_axis[1] * compensation.cross_axis[3]
                    - compensation.cross_axis[2])
                - magnetic[1]
                    * (compensation.cross_axis[3]
                        - compensation.cross_axis[0] * compensation.cross_axis[2]))
                / denominator;

        Some(CompensatedMagneticData {
            x_microtesla: x,
            y_microtesla: y,
            z_microtesla: z,
            temperature_celsius: temperature,
            sensor_time: data.sensor_time,
        })
    }

    async fn load_compensation(&mut self) -> Result<Compensation, MagnetometerError<I::Error>> {
        let mut otp = [0u16; OTP_WORDS as usize];
        for word in 0..OTP_WORDS {
            self.write_reg(REG_OTP_CMD, OTP_DIRECT_READ | word)
                .await
                .map_err(MagnetometerError::I2c)?;

            let mut status = 0;
            for _ in 0..100 {
                Timer::after(Duration::from_micros(300)).await;
                status = self
                    .read_reg(REG_OTP_STATUS)
                    .await
                    .map_err(MagnetometerError::I2c)?;
                if status & OTP_ERROR_MASK != 0 {
                    return Err(MagnetometerError::OtpError(status));
                }
                if status & OTP_DONE != 0 {
                    break;
                }
            }
            if status & OTP_DONE == 0 {
                return Err(MagnetometerError::OtpTimeout(word));
            }

            let msb = self
                .read_reg(REG_OTP_DATA_MSB)
                .await
                .map_err(MagnetometerError::I2c)?;
            let lsb = self
                .read_reg(REG_OTP_DATA_LSB)
                .await
                .map_err(MagnetometerError::I2c)?;
            otp[word as usize] = u16::from_be_bytes([msb, lsb]);
        }

        Ok(compensation_from_otp(&otp))
    }

    async fn read_reg(&mut self, reg: u8) -> Result<u8, I::Error> {
        let mut transfer = [0u8; 3];
        self.i2c
            .write_read(BMM350_ADDR, &[reg], &mut transfer)
            .await?;
        Ok(transfer[2])
    }

    async fn write_reg(&mut self, reg: u8, value: u8) -> Result<(), I::Error> {
        self.i2c.write(BMM350_ADDR, &[reg, value]).await
    }
}

fn compensation_from_otp(otp: &[u16; OTP_WORDS as usize]) -> Compensation {
    let offset_x = sign_extend(otp[0x0E] & 0x0FFF, 12);
    let offset_y = sign_extend(((otp[0x0E] >> 12) << 8) | (otp[0x0F] & 0x00FF), 12);
    let offset_z = sign_extend((otp[0x0F] & 0x0F00) | (otp[0x10] & 0x00FF), 12);
    let signed_byte = |value: u16| sign_extend(value & 0xFF, 8) as f32;

    Compensation {
        offset: [offset_x as f32, offset_y as f32, offset_z as f32],
        sensitivity: [
            signed_byte(otp[0x10] >> 8) / 256.0,
            signed_byte(otp[0x11]) / 256.0,
            signed_byte(otp[0x11] >> 8) / 256.0,
        ],
        temperature_offset: signed_byte(otp[0x0D]) / 5.0,
        temperature_sensitivity: signed_byte(otp[0x0D] >> 8) / 512.0,
        temperature_coefficient: [
            signed_byte(otp[0x12]) / 32.0,
            signed_byte(otp[0x13]) / 32.0,
            signed_byte(otp[0x14]) / 32.0,
        ],
        temperature_scale: [
            signed_byte(otp[0x12] >> 8) / 16384.0,
            signed_byte(otp[0x13] >> 8) / 16384.0,
            signed_byte(otp[0x14] >> 8) / 16384.0,
        ],
        temperature_reference: sign_extend(otp[0x18], 16) as f32 / 512.0 + 23.0,
        cross_axis: [
            signed_byte(otp[0x15]) / 800.0,
            signed_byte(otp[0x15] >> 8) / 800.0,
            signed_byte(otp[0x16]) / 800.0,
            signed_byte(otp[0x16] >> 8) / 800.0,
        ],
    }
}

fn sign_extend(value: u16, bits: u32) -> i32 {
    let sign = 1i32 << (bits - 1);
    let mask = (1i32 << bits) - 1;
    let value = i32::from(value) & mask;
    (value ^ sign) - sign
}

fn decode_signed_24(lsb: u8, mid: u8, msb: u8) -> i32 {
    let value = u32::from_le_bytes([lsb, mid, msb, 0]);
    if value & (1 << 23) != 0 {
        (value | 0xFF00_0000) as i32
    } else {
        value as i32
    }
}

#[cfg(test)]
mod tests {
    use super::{compensation_from_otp, decode_signed_24, sign_extend};

    #[test]
    fn decodes_signed_24_bit_samples() {
        assert_eq!(decode_signed_24(0x34, 0x12, 0x00), 0x1234);
        assert_eq!(decode_signed_24(0xCC, 0xED, 0xFF), -0x1234);
        assert_eq!(decode_signed_24(0xFF, 0xFF, 0x7F), 0x7F_FFFF);
        assert_eq!(decode_signed_24(0x00, 0x00, 0x80), -0x80_0000);
    }

    #[test]
    fn sign_extension_preserves_otp_width() {
        assert_eq!(sign_extend(0x7f, 8), 127);
        assert_eq!(sign_extend(0x80, 8), -128);
        assert_eq!(sign_extend(0x800, 12), -2048);
        assert_eq!(sign_extend(0xffff, 16), -1);
    }

    #[test]
    fn zero_trim_data_has_zero_compensation() {
        let compensation = compensation_from_otp(&[0; 32]);
        assert_eq!(compensation.offset, [0.0; 3]);
        assert_eq!(compensation.sensitivity, [0.0; 3]);
        assert_eq!(compensation.temperature_reference, 23.0);
    }
}
