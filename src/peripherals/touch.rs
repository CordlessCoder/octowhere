use embedded_graphics::prelude::Size;
use embedded_hal::digital::OutputPin;
use embedded_hal_async::{digital::Wait, i2c::I2c};

use crate::peripherals::i2c_helper;

const CST9220_CHIP_ID: u16 = 0x9220;
const CST9217_CHIP_ID: u16 = 0x9217;

const REG_READ: u16 = 0xD000;
/// Written before each mode command; a read of `REG_COMMAND_ECHO` then answers `0x1E`.
const REG_COMMAND: u16 = 0xD11E;
const REG_COMMAND_ECHO: u16 = 0x0002;
const REG_GESTURE_MODE: u16 = 0xD104;
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
const CST9217_POWER_ON_SETTLE_MS: u32 = 100;

const MAX_FINGER_NUM: usize = 2;
const PROGRAM_PAGE_SIZE: u8 = 128;
const READ_BUF_SIZE: usize = MAX_FINGER_NUM * 5 + 5;

const ACK_VALUE: u8 = 0xAB;

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
        let max_x = width.saturating_sub(1);
        let max_y = height.saturating_sub(1);
        if self.mirror_x {
            point.x = max_x.saturating_sub(point.x);
        }
        if self.mirror_y {
            point.y = max_y.saturating_sub(point.y);
        }
        point.x = point.x.min(max_x);
        point.y = point.y.min(max_y);
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
    firmware: (u32, u32),
    config: Cst9217Config,
    last_raw: [u8; READ_BUF_SIZE],
}

#[derive(Debug)]
pub enum Cst9217Error<I2CError, ResetError = I2CError> {
    I2CError(I2CError),
    ResetError(ResetError),
    IDMismatch,
    NoFirmware,
    InvalidCheckcode,
}

impl<I2CError, ResetError> From<I2CError> for Cst9217Error<I2CError, ResetError> {
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
    /// A gesture the controller recognised, reported alone while it is in gesture mode.
    Gesture(Gesture),
    Points(heapless::Vec<TouchPoint, 2, u8>),
    /// The finger lifted, last found here. This is the position of the controller's last
    /// report, which a missed read leaves newer than the last one read.
    Lifted(TouchPoint),
    /// No report since the last read: the controller has not replaced the acknowledgement the
    /// last read left. Whatever the last report said still holds.
    Stale,
}
/// The gestures the controller reports, in panel terms.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gesture {
    Tap,
    SwipeLeft,
    SwipeRight,
    SwipeUp,
    SwipeDown,
}

impl Gesture {
    /// A lift report's byte 4, which carries the gesture in gesture mode. A double tap comes as
    /// two taps.
    fn from_code(code: u8) -> Option<Self> {
        match code & 0x70 {
            0x10 => Some(Gesture::Tap),
            0x30 => Some(Gesture::SwipeLeft),
            0x40 => Some(Gesture::SwipeDown),
            0x50 => Some(Gesture::SwipeRight),
            0x60 => Some(Gesture::SwipeUp),
            _ => None,
        }
    }
}

impl Default for TouchData {
    fn default() -> Self {
        TouchData::Points(heapless::Vec::new())
    }
}

impl<I: I2c, RST, INT, DELAY> Cst9217<I, INT, RST, DELAY> {
    #[must_use]
    pub fn new(i2c: I, reset: RST, int: INT, delay: DELAY) -> Self {
        Self {
            i2c,
            addr: 0x5A,
            int,
            reset,
            delay,
            width: 0,
            height: 0,
            firmware: (0, 0),
            config: Default::default(),
            last_raw: [0; READ_BUF_SIZE],
        }
    }
    pub fn with_address(mut self, addr: u8) -> Self {
        self.addr = addr;
        self
    }
    pub fn set_config(&mut self, config: Cst9217Config) {
        self.config = config;
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
        self.last_raw = buf;
        match report_kind(&buf) {
            Report::Stale => return Ok(TouchData::Stale),
            Report::Lifted if let Some(gesture) = Gesture::from_code(buf[4]) => {
                return Ok(TouchData::Gesture(gesture));
            }
            Report::Lifted => {
                let (_, _, mut point) = decode_point(&buf, 0);
                self.config.apply(self.width, self.height, &mut point);
                return Ok(TouchData::Lifted(point));
            }
            Report::Fresh => {}
        }
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
            let (id, event, mut point) = decode_point(&buf, i.into());
            if event == 0x06 && id < MAX_FINGER_NUM as u8 {
                self.config.apply(self.width, self.height, &mut point);
                _ = points.push(point);
            }
        }
        Ok(TouchData::Points(points))
    }
    /// The bytes the last read returned.
    pub fn last_raw(&self) -> &[u8; READ_BUF_SIZE] {
        &self.last_raw
    }

    /// The controller's firmware version and checksum, as `init` read them.
    pub fn firmware(&self) -> (u32, u32) {
        self.firmware
    }

    pub fn resolution(&self) -> Size {
        Size {
            width: self.width as u32,
            height: self.height as u32,
        }
    }
}
impl<I: I2c, RST: OutputPin, INT, DELAY: embedded_hal_async::delay::DelayNs>
    Cst9217<I, INT, RST, DELAY>
{
    pub async fn init(&mut self) -> Result<(), Cst9217Error<I::Error, RST::Error>> {
        self.delay.delay_ms(CST9217_POWER_ON_SETTLE_MS).await;
        self.start().await
    }

    /// Has the controller report only the gestures it recognises, one INT pulse each, instead
    /// of every contact. [`Self::leave_gesture_mode`] undoes it.
    pub async fn enter_gesture_mode(&mut self) -> Result<(), I::Error> {
        // From the debug mode `start` leaves it in, gesture mode goes on reporting contacts.
        self.command(REG_NORMAL_MODE).await?;
        self.command(REG_GESTURE_MODE).await
    }

    /// Resets the controller, since no mode command leaves gesture mode, and starts it as `init`
    /// does. Normal mode is not used after: it ends some reads early, and debug mode does not.
    pub async fn leave_gesture_mode(&mut self) -> Result<(), Cst9217Error<I::Error, RST::Error>> {
        self.reset().await.map_err(Cst9217Error::ResetError)?;
        self.start().await
    }

    /// Sends a mode command, after the exchange Hynitron's driver makes before each one. The
    /// command goes even if the exchange is not answered, as that driver's does.
    pub async fn command(&mut self, command: u16) -> Result<(), I::Error> {
        for _ in 0..3 {
            if self
                .i2c
                .write(self.addr, &REG_COMMAND.to_be_bytes())
                .await
                .is_err()
            {
                self.delay.delay_ms(1).await;
                continue;
            }
            self.delay.delay_ms(1).await;
            let mut echo = [0u8; 2];
            let answered = self
                .i2c
                .write_read(self.addr, &REG_COMMAND_ECHO.to_be_bytes(), &mut echo)
                .await;
            if answered.is_ok() && echo[1] == 0x1E {
                break;
            }
        }
        self.i2c.write(self.addr, &command.to_be_bytes()).await
    }

    /// Puts the controller in the debug mode every read so far has been made in, and reads its
    /// resolution and firmware.
    async fn start(&mut self) -> Result<(), Cst9217Error<I::Error, RST::Error>> {
        if i2c_helper::write_wide_reg(&mut self.i2c, self.addr, REG_DEBUG_MODE, 0x01)
            .await
            .is_err()
        {
            // ACK failure may mean that the sensor needs a reset
            self.reset().await.map_err(Cst9217Error::ResetError)?;
            i2c_helper::write_wide_reg(&mut self.i2c, self.addr, REG_DEBUG_MODE, 0x01).await?;
        }
        self.delay.delay_ms(10).await;
        let mut buf = [0u8; 4];
        self.i2c
            .write_read(self.addr, &REG_CHECKCODE.to_be_bytes(), &mut buf)
            .await?;
        let checkcode = u32::from_le_bytes(buf);
        if (checkcode & 0xffff0000) != 0xCACA0000 {
            return Err(Cst9217Error::InvalidCheckcode);
        }

        self.i2c
            .write_read(self.addr, &REG_RESOLUTION.to_be_bytes(), &mut buf)
            .await?;
        self.width = u16::from_le_bytes(buf[0..2].try_into().unwrap());
        self.height = u16::from_le_bytes(buf[2..4].try_into().unwrap());

        self.i2c
            .write_read(self.addr, &REG_PROJECT_ID.to_be_bytes(), &mut buf)
            .await?;
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

        if fw_version == 0xA5A5A5A5 {
            return Err(Cst9217Error::NoFirmware);
        }
        self.firmware = (fw_version, checksum);

        Ok(())
    }

    pub async fn reset(&mut self) -> Result<(), RST::Error> {
        self.reset.set_low()?;
        self.delay.delay_ms(10).await;
        self.reset.set_high()?;
        self.delay.delay_ms(CST9217_POWER_ON_SETTLE_MS).await;
        Ok(())
    }
}

#[derive(Debug, PartialEq, Eq)]
enum Report {
    Fresh,
    /// Written as a finger lifts, before anything else.
    Lifted,
    Stale,
}

/// The `index`th point of a report: its id, its event and where it is, before the config applies.
fn decode_point(buf: &[u8; READ_BUF_SIZE], index: usize) -> (u8, u8, TouchPoint) {
    let data = &buf[index * 5 + if index == 0 { 0 } else { 2 }..][..4];
    let x = (u16::from(data[1]) << 4) | u16::from(data[3] >> 4);
    let y = (u16::from(data[2]) << 4) | u16::from(data[3] & 0x0F);
    (data[0] >> 4, data[0] & 0x0F, TouchPoint { x, y })
}

/// A read before the controller writes its next report returns our acknowledgement in byte 0.
/// A still finger is reported less often than the frame loop polls, so these arrive mid-contact.
fn report_kind(buf: &[u8; READ_BUF_SIZE]) -> Report {
    match buf[0] {
        _ if buf[6] != CST92XX_ACK => Report::Stale,
        CST92XX_ACK => Report::Stale,
        0 => Report::Lifted,
        _ => Report::Fresh,
    }
}

impl<I: I2c, RST, INT: Wait, DELAY> Cst9217<I, INT, RST, DELAY> {
    /// Waits for the controller to signal a report. INT pulses low for about 2 ms each time,
    /// longer than a read, so waiting for the low level would read one report several times. An
    /// edge that comes while nothing waits is missed.
    pub fn wait_for_touch(&mut self) -> impl Future<Output = Result<(), INT::Error>> {
        self.int.wait_for_falling_edge()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CST92XX_ACK, Cst9217Config, Gesture, READ_BUF_SIZE, Report, TouchPoint, decode_point,
        report_kind,
    };
    use embedded_hal::digital::OutputPin;
    use embedded_hal_async::{
        delay::DelayNs,
        i2c::{ErrorKind, ErrorType, I2c, Operation},
    };

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum FakeError {
        Bus,
        Reset,
    }

    impl embedded_hal::i2c::Error for FakeError {
        fn kind(&self) -> ErrorKind {
            ErrorKind::Other
        }
    }

    impl embedded_hal::digital::Error for FakeError {
        fn kind(&self) -> embedded_hal::digital::ErrorKind {
            embedded_hal::digital::ErrorKind::Other
        }
    }

    struct FakeI2c {
        fail_project: bool,
        fail_first_write: bool,
    }

    impl ErrorType for FakeI2c {
        type Error = FakeError;
    }

    impl I2c for FakeI2c {
        async fn transaction(
            &mut self,
            _address: u8,
            operations: &mut [Operation<'_>],
        ) -> Result<(), Self::Error> {
            let Some(operation) = operations.first_mut() else {
                return Ok(());
            };
            match operation {
                Operation::Write(bytes) if *bytes == [0xD1, 0x01, 0x01] => {
                    if self.fail_first_write {
                        self.fail_first_write = false;
                        return Err(FakeError::Bus);
                    }
                }
                Operation::Read(_) => {}
                Operation::Write(_) => {}
            }
            if let [Operation::Write(write), Operation::Read(read)] = operations {
                match write {
                    [0xD1, 0xFC] => read.copy_from_slice(&[1, 0, 0xCA, 0xCA]),
                    [0xD1, 0xF8] => read.copy_from_slice(&[0xD2, 0x01, 0x2C, 0x01]),
                    [0xD2, 0x04] if self.fail_project => return Err(FakeError::Bus),
                    [0xD2, 0x04] => read.fill(0),
                    _ => {}
                }
            }
            Ok(())
        }
    }

    struct FakeReset {
        fail_low: bool,
    }

    impl embedded_hal::digital::ErrorType for FakeReset {
        type Error = FakeError;
    }

    impl OutputPin for FakeReset {
        fn set_low(&mut self) -> Result<(), Self::Error> {
            if self.fail_low {
                Err(FakeError::Reset)
            } else {
                Ok(())
            }
        }

        fn set_high(&mut self) -> Result<(), Self::Error> {
            Ok(())
        }
    }

    struct FakeDelay;

    impl DelayNs for FakeDelay {
        async fn delay_ns(&mut self, _ns: u32) {}
    }

    fn block_on<F: Future>(future: F) -> F::Output {
        let waker = std::task::Waker::noop();
        let mut context = std::task::Context::from_waker(waker);
        let mut future = core::pin::pin!(future);
        loop {
            match Future::poll(future.as_mut(), &mut context) {
                core::task::Poll::Ready(value) => return value,
                core::task::Poll::Pending => std::thread::yield_now(),
            }
        }
    }

    #[test]
    fn clamps_points_to_the_last_pixel() {
        let config = Cst9217Config::default();
        let mut point = TouchPoint { x: 500, y: 500 };
        config.apply(466, 300, &mut point);
        assert_eq!(point, TouchPoint { x: 465, y: 299 });
    }

    #[test]
    fn mirrors_around_the_pixel_range() {
        let config = Cst9217Config {
            mirror_x: true,
            mirror_y: true,
            ..Default::default()
        };
        let mut point = TouchPoint { x: 0, y: 0 };
        config.apply(466, 300, &mut point);
        assert_eq!(point, TouchPoint { x: 465, y: 299 });
    }

    #[test]
    fn tells_a_lift_from_a_read_before_the_next_report() {
        let mut report = [0u8; READ_BUF_SIZE];
        report[6] = CST92XX_ACK;
        assert_eq!(report_kind(&report), Report::Lifted);

        report[0] = CST92XX_ACK;
        assert_eq!(report_kind(&report), Report::Stale);

        report[0] = 0x06;
        assert_eq!(report_kind(&report), Report::Fresh);
        report[6] = 0;
        assert_eq!(report_kind(&report), Report::Stale);
    }

    #[test]
    fn a_lift_report_keeps_the_last_position() {
        let report = [
            0x00, 0x0b, 0x0e, 0xa5, 0x00, 0x01, 0xab, 0x00, 0x00, 0x00, 0x3c, 0x00, 0xab, 0x80,
            0x00,
        ];
        assert_eq!(report_kind(&report), Report::Lifted);
        assert_eq!(
            decode_point(&report, 0),
            (0, 0, TouchPoint { x: 0xba, y: 0xe5 })
        );
    }

    #[test]
    fn a_gesture_comes_in_a_lift_report() {
        let mut report = [
            0x00, 0x0b, 0x10, 0x79, 0x10, 0x01, 0xab, 0x10, 0x00, 0x00, 0x3c, 0x00, 0xab, 0x80,
            0x00,
        ];
        assert_eq!(report_kind(&report), Report::Lifted);
        assert_eq!(Gesture::from_code(report[4]), Some(Gesture::Tap));
        report[4] = 0x30;
        assert_eq!(Gesture::from_code(report[4]), Some(Gesture::SwipeLeft));
        report[4] = 0x00;
        assert_eq!(Gesture::from_code(report[4]), None);
    }

    #[test]
    fn init_returns_project_id_bus_failure() {
        let mut touch = super::Cst9217::new(
            FakeI2c {
                fail_project: true,
                fail_first_write: false,
            },
            FakeReset { fail_low: false },
            (),
            FakeDelay,
        );
        let error = block_on(touch.init()).unwrap_err();
        assert!(matches!(
            error,
            super::Cst9217Error::I2CError(FakeError::Bus)
        ));
    }

    #[test]
    fn init_returns_reset_failure_after_ack_error() {
        let mut touch = super::Cst9217::new(
            FakeI2c {
                fail_project: false,
                fail_first_write: true,
            },
            FakeReset { fail_low: true },
            (),
            FakeDelay,
        );
        let error = block_on(touch.init()).unwrap_err();
        assert!(matches!(
            error,
            super::Cst9217Error::ResetError(FakeError::Reset)
        ));
    }
}
