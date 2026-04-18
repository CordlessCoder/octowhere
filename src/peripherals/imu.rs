// QMI8658 6-axis IMU driver (Accelerometer + Gyroscope)
// Reference: SensorLib/src/SensorQMI8658.hpp
// I2C address 0x6B
use embedded_hal_async::i2c::I2c;

const QMI8658_ADDR: u8 = 0x6B;

// Registers
const REG_WHO_AM_I: u8 = 0x00;
const REG_CTRL1: u8 = 0x02; // Serial interface and sensor enable
const REG_CTRL2: u8 = 0x03; // Accelerometer settings
const REG_CTRL3: u8 = 0x04; // Gyroscope settings
const REG_CTRL5: u8 = 0x06; // Low-pass filter
const REG_CTRL7: u8 = 0x08; // Enable sensors
const REG_AX_L: u8 = 0x35; // Accel X low byte
const REG_GX_L: u8 = 0x3B; // Gyro X low byte
const REG_TEMP_L: u8 = 0x33; // Temperature low byte

const QMI8658_WHO_AM_I: u8 = 0x05; // Expected chip ID

#[derive(Debug, Clone, Copy, Default)]
pub struct AccelData {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct GyroData {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

pub struct Qmi8658Imu<I> {
    i2c: I,
    accel_scale: f32,
    gyro_scale: f32,
}

impl<I: I2c> Qmi8658Imu<I> {
    pub fn new(i2c: I) -> Self {
        Self {
            i2c,
            accel_scale: 1.0 / 4096.0, // ±8g default
            gyro_scale: 1.0 / 64.0,    // ±512 dps default
        }
    }

    async fn read_reg(&mut self, reg: u8) -> Result<u8, I::Error> {
        let mut buf = [0u8];
        self.i2c.write_read(QMI8658_ADDR, &[reg], &mut buf).await?;
        Ok(buf[0])
    }

    async fn write_reg(&mut self, reg: u8, val: u8) -> Result<(), I::Error> {
        self.i2c.write(QMI8658_ADDR, &[reg, val]).await
    }

    async fn read_regs(&mut self, reg: u8, buf: &mut [u8]) -> Result<(), I::Error> {
        self.i2c.write_read(QMI8658_ADDR, &[reg], buf).await
    }

    /// Initialize the IMU. Returns true if chip ID matches.
    pub async fn init(&mut self) -> Result<bool, I::Error> {
        let id = self.read_reg(REG_WHO_AM_I).await?;
        if id != QMI8658_WHO_AM_I {
            return Ok(false);
        }

        // Reset: CTRL1 bit 6 = soft reset (auto-clear)
        self.write_reg(REG_CTRL1, 0x40).await?;
        // Wait for reset (no delay available here, just write next configs)

        // CTRL1: address auto-increment enabled
        self.write_reg(REG_CTRL1, 0x60).await?;

        // CTRL2: Accelerometer ODR=119Hz, Full scale=±8g
        // ODR[3:0]=0b0101 (119Hz), FS[6:4]=0b010 (±8g)
        self.write_reg(REG_CTRL2, 0x25).await?;
        self.accel_scale = 8.0 / 32768.0; // ±8g

        // CTRL3: Gyroscope ODR=119Hz, Full scale=±512dps
        // ODR[3:0]=0b0101 (119Hz), FS[6:4]=0b011 (±512dps)
        self.write_reg(REG_CTRL3, 0x35).await?;
        self.gyro_scale = 512.0 / 32768.0; // ±512dps

        // CTRL5: Low-pass filter enabled for both accel and gyro
        self.write_reg(REG_CTRL5, 0x11).await?;

        // CTRL7: Enable accelerometer and gyroscope
        // Bit 0: accel enable, Bit 1: gyro enable
        self.write_reg(REG_CTRL7, 0x03).await?;

        Ok(true)
    }

    /// Power down accelerometer + gyroscope. Call when sensors are not needed.
    /// Reduces idle current significantly (gyro alone draws ~1.5 mA).
    pub fn power_down(&mut self) -> impl Future<Output = Result<(), I::Error>> {
        self.write_reg(REG_CTRL7, 0x00)
    }

    /// Power up accelerometer + gyroscope. Call before reading.
    pub fn power_up(&mut self) -> impl Future<Output = Result<(), I::Error>> {
        self.write_reg(REG_CTRL7, 0x03)
    }

    /// Read accelerometer data in g.
    pub async fn read_accel(&mut self) -> Result<AccelData, I::Error> {
        let mut buf = [0u8; 6];
        self.read_regs(REG_AX_L, &mut buf).await?;

        let x = i16::from_le_bytes([buf[0], buf[1]]) as f32 * self.accel_scale;
        let y = i16::from_le_bytes([buf[2], buf[3]]) as f32 * self.accel_scale;
        let z = i16::from_le_bytes([buf[4], buf[5]]) as f32 * self.accel_scale;

        Ok(AccelData { x, y, z })
    }

    /// Read gyroscope data in degrees per second.
    pub async fn read_gyro(&mut self) -> Result<GyroData, I::Error> {
        let mut buf = [0u8; 6];
        self.read_regs(REG_GX_L, &mut buf).await?;

        let x = i16::from_le_bytes([buf[0], buf[1]]) as f32 * self.gyro_scale;
        let y = i16::from_le_bytes([buf[2], buf[3]]) as f32 * self.gyro_scale;
        let z = i16::from_le_bytes([buf[4], buf[5]]) as f32 * self.gyro_scale;

        Ok(GyroData { x, y, z })
    }

    /// Read chip temperature in °C.
    pub async fn read_temperature(&mut self) -> Result<f32, I::Error> {
        let mut buf = [0u8; 2];
        self.read_regs(REG_TEMP_L, &mut buf).await?;
        let raw = i16::from_le_bytes([buf[0], buf[1]]);
        Ok(raw as f32 / 256.0)
    }

    /// Read chip ID.
    pub fn read_chip_id(&mut self) -> impl Future<Output = Result<u8, I::Error>> {
        self.read_reg(REG_WHO_AM_I)
    }
}
