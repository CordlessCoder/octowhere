#![no_std]

use embedded_hal_async::{delay::DelayNs, i2c::I2c};
use nmea0183::{FixType, GGA, GPSQuality, GSA, GSV, Mode, ParseResult, RMC};

const CONFIG_ADDRESS: u8 = 0x50;
const DATA_ADDRESS: u8 = 0x54;
const WRITE_DATA_ADDRESS: u8 = 0x58;
const CONFIG_READ_NMEA_LENGTH: u32 = 0xAA51_0008;
const CONFIG_READ_DATA: u32 = 0xAA51_2000;
const CONFIG_WRITE_NMEA: u32 = 0xAA53_1000;
const CONFIG_READ_FREE_LENGTH: u32 = 0xAA51_0004;
const NMEA_LENGTH_BYTES: usize = 4;
const INTER_COMMAND_DELAY_MS: u32 = 10;
const MAX_I2C_RETRIES: usize = 20;

const ALP_ENABLE: &[u8] = b"$PAIR732,1*21\r\n";
const ALP_DISABLE: &[u8] = b"$PAIR732,0*20\r\n";
const SET_FIX_RATE_1_HZ: &[u8] = b"$PAIR050,1000*12\r\n";
const SET_NORMAL_NAVIGATION: &[u8] = b"$PAIR080,0*2E\r\n";

#[derive(Debug, PartialEq, Eq)]
pub enum GnssOperation {
    WriteConfig,
    ReadLength,
    ReadData,
    WriteData,
}

#[derive(Debug, PartialEq, Eq)]
pub enum GnssError<E> {
    I2c { operation: GnssOperation, error: E },
    BufferTooSmall { required: usize, available: usize },
}

/// The low-power policy requested from the receiver.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LowPowerMode {
    /// Continuous tracking with the receiver's normal duty cycle.
    Disabled,
    /// Adaptive Low Power mode.
    Adaptive,
}

/// The NMEA GGA fix-quality code.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum FixQuality {
    /// No position solution.
    #[default]
    NoFix,
    /// Autonomous GNSS solution.
    Autonomous,
    /// Differential GNSS solution.
    Differential,
    /// PPS-locked solution.
    Pps,
    /// Fixed RTK solution.
    Rtk,
    /// Float RTK solution.
    FloatRtk,
    /// Estimated solution.
    Estimated,
    /// Manually entered solution.
    Manual,
    /// Simulated solution.
    Simulated,
}

macro_rules! coordinate_newtype {
    ($(#[$meta:meta])* $name:ident, $inner:ty, $unit:literal) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
        pub struct $name($inner);

        impl $name {
            /// Creates a value from its integer representation.
            pub const fn new(value: $inner) -> Self {
                Self(value)
            }

            /// Returns the integer representation.
            pub const fn get(self) -> $inner {
                self.0
            }

            /// Returns the unit represented by this newtype.
            pub const fn unit() -> &'static str {
                $unit
            }
        }

        impl From<$name> for $inner {
            fn from(value: $name) -> Self {
                value.0
            }
        }
    };
}

coordinate_newtype!(
    /// Latitude in degrees multiplied by 10⁷.
    LatitudeE7,
    i32,
    "degrees × 10⁷"
);
coordinate_newtype!(
    /// Longitude in degrees multiplied by 10⁷.
    LongitudeE7,
    i32,
    "degrees × 10⁷"
);
coordinate_newtype!(
    /// Altitude in millimetres.
    AltitudeMm,
    i32,
    "mm"
);
coordinate_newtype!(
    /// Ground speed in millimetres per second.
    SpeedMmPerSecond,
    u32,
    "mm/s"
);
coordinate_newtype!(
    /// Course over ground in thousandths of a degree.
    CourseMilliDegrees,
    u32,
    "millidegrees"
);
coordinate_newtype!(
    /// Dilution of precision multiplied by 1000.
    DopMilli,
    u32,
    "DOP × 1000"
);
coordinate_newtype!(
    /// Number of satellites used in the solution.
    SatelliteCount,
    u8,
    "satellites"
);
coordinate_newtype!(
    /// Signal-to-noise ratio in decibels.
    SnrDb,
    u8,
    "dB"
);

/// The dimensionality of the latest navigation solution.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum GnssFixType {
    /// No position solution is available.
    #[default]
    NoFix,
    /// A two-dimensional position solution is available.
    Fix2D,
    /// A three-dimensional position solution is available.
    Fix3D,
}

/// Satellite and dilution data reported while the receiver is acquiring a fix.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct GnssSignal {
    pub fix_type: GnssFixType,
    pub satellites_in_view: SatelliteCount,
    pub satellites_used: SatelliteCount,
    pub satellites_with_signal: SatelliteCount,
    pub strongest_snr: Option<SnrDb>,
    pub pdop: Option<DopMilli>,
    pub hdop: Option<DopMilli>,
    pub vdop: Option<DopMilli>,
}

/// UTC date and time reported by the receiver.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct GnssDateTime {
    pub year: u16,
    pub month: u8,
    pub day: u8,
    pub weekday: u8,
    pub hours: u8,
    pub minutes: u8,
    pub seconds: u8,
    pub milliseconds: u16,
}

/// The latest position solution assembled from RMC and GGA sentences.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct GnssFix {
    pub latitude: LatitudeE7,
    pub longitude: LongitudeE7,
    pub altitude: Option<AltitudeMm>,
    pub speed: Option<SpeedMmPerSecond>,
    pub course: Option<CourseMilliDegrees>,
    pub satellites: Option<SatelliteCount>,
    pub hdop: Option<DopMilli>,
    pub quality: FixQuality,
}

/// Receiver state assembled from the latest valid NMEA sentences.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct GnssState {
    pub fix: Option<GnssFix>,
    pub utc: Option<GnssDateTime>,
    pub signal: GnssSignal,
}

/// Sentence type that changed the parsed receiver state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NmeaUpdate {
    Rmc,
    Gga,
    Gsa,
    Gsv,
    PairAck(PairAck),
}

/// Acknowledgement returned for a proprietary `PAIR` command.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PairAck {
    pub command: u16,
    pub status: PairAckStatus,
}

/// Result reported by the receiver for a proprietary `PAIR` command.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PairAckStatus {
    /// The command was accepted by the receiver.
    Accepted,
    /// The command was received and is still being processed.
    Processing,
    /// The receiver could not send the command to the GNSS service.
    Failed,
    /// The receiver does not implement the command.
    Unsupported,
    /// One or more command parameters were invalid.
    InvalidParameter,
    /// The GNSS service is busy and the command can be retried.
    Busy,
    /// A newer firmware returned an unrecognised status code.
    Unknown(u8),
}

struct PairAckParser {
    line: [u8; 32],
    len: usize,
}

impl PairAckParser {
    const fn new() -> Self {
        Self {
            line: [0; 32],
            len: 0,
        }
    }

    fn push(&mut self, byte: u8) -> Option<PairAck> {
        if byte == b'$' {
            self.len = 0;
        }
        if self.len == self.line.len() {
            self.len = 0;
            return None;
        }
        self.line[self.len] = byte;
        self.len += 1;
        if byte != b'\n' {
            return None;
        }

        let ack = parse_pair_ack(&self.line[..self.len]);
        self.len = 0;
        ack
    }
}

fn parse_pair_ack(line: &[u8]) -> Option<PairAck> {
    let line = line.strip_suffix(b"\r\n")?;
    let prefix = b"$PAIR001,";
    if !line.starts_with(prefix) {
        return None;
    }
    let star = line.iter().position(|&byte| byte == b'*')?;
    if star + 3 != line.len() {
        return None;
    }
    let checksum = line[1..star]
        .iter()
        .fold(0, |checksum, byte| checksum ^ byte);
    if checksum != (hex_digit(line[star + 1])? << 4 | hex_digit(line[star + 2])?) {
        return None;
    }

    let fields = &line[prefix.len()..star];
    let comma = fields.iter().position(|&byte| byte == b',')?;
    let command = decimal(&fields[..comma])?;
    let status = match decimal(&fields[comma + 1..])? {
        0 => PairAckStatus::Accepted,
        1 => PairAckStatus::Processing,
        2 => PairAckStatus::Failed,
        3 => PairAckStatus::Unsupported,
        4 => PairAckStatus::InvalidParameter,
        5 => PairAckStatus::Busy,
        status => PairAckStatus::Unknown(status as u8),
    };
    Some(PairAck { command, status })
}

fn decimal(bytes: &[u8]) -> Option<u16> {
    if bytes.is_empty() {
        return None;
    }
    let mut value = 0u16;
    for &byte in bytes {
        if !byte.is_ascii_digit() {
            return None;
        }
        value = value.checked_mul(10)?.checked_add((byte - b'0') as u16)?;
    }
    Some(value)
}

fn hex_digit(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}

/// Incrementally parses RMC and GGA sentences into fixed-point receiver state.
pub struct NmeaParser {
    parser: nmea0183::Parser,
    pair_ack: PairAckParser,
    state: GnssState,
}

impl NmeaParser {
    pub fn new() -> Self {
        Self {
            parser: nmea0183::Parser::new().sentence_filter(
                nmea0183::Sentence::RMC
                    | nmea0183::Sentence::GGA
                    | nmea0183::Sentence::GSA
                    | nmea0183::Sentence::GSV,
            ),
            pair_ack: PairAckParser::new(),
            state: GnssState::default(),
        }
    }

    pub fn state(&self) -> GnssState {
        self.state
    }

    pub fn push(&mut self, byte: u8) -> Result<Option<NmeaUpdate>, &'static str> {
        let pair_ack = self.pair_ack.push(byte).map(NmeaUpdate::PairAck);
        let Some(result) = self.parser.parse_from_byte(byte) else {
            return Ok(pair_ack);
        };
        let result = match result {
            Ok(result) => result,
            Err("Source is not supported!" | "Unsupported sentence type.") => return Ok(pair_ack),
            Err(error) => return Err(error),
        };

        match result {
            ParseResult::RMC(Some(rmc)) => {
                self.update_rmc(&rmc);
                Ok(Some(NmeaUpdate::Rmc))
            }
            ParseResult::RMC(None) => {
                self.state.fix = None;
                self.state.utc = None;
                Ok(Some(NmeaUpdate::Rmc))
            }
            ParseResult::GGA(Some(gga)) => {
                self.update_gga(&gga);
                Ok(Some(NmeaUpdate::Gga))
            }
            ParseResult::GGA(None) => {
                self.state.fix = None;
                Ok(Some(NmeaUpdate::Gga))
            }
            ParseResult::GSA(Some(gsa)) => {
                self.update_gsa(&gsa);
                Ok(Some(NmeaUpdate::Gsa))
            }
            ParseResult::GSA(None) => {
                self.state.signal = GnssSignal::default();
                Ok(Some(NmeaUpdate::Gsa))
            }
            ParseResult::GSV(Some(gsv)) => {
                self.update_gsv(&gsv);
                Ok(Some(NmeaUpdate::Gsv))
            }
            ParseResult::GSV(None) => Ok(Some(NmeaUpdate::Gsv)),
            _ => Ok(pair_ack),
        }
    }

    fn update_rmc(&mut self, rmc: &RMC) {
        if !rmc.mode.is_valid() {
            self.state.fix = None;
            self.state.utc = None;
            return;
        }

        self.state.utc = Some(convert_datetime(&rmc.datetime));
        self.state.fix = Some(GnssFix {
            latitude: latitude_e7(&rmc.latitude),
            longitude: longitude_e7(&rmc.longitude),
            altitude: None,
            speed: Some(SpeedMmPerSecond::new((rmc.speed.as_mps() * 1_000.0) as u32)),
            course: rmc
                .course
                .as_ref()
                .map(|course| CourseMilliDegrees::new((course.degrees * 1_000.0) as u32)),
            satellites: None,
            hdop: None,
            quality: rmc_quality(&rmc.mode),
        });
    }

    fn update_gsa(&mut self, gsa: &GSA) {
        self.state.signal.fix_type = match gsa.fix_type {
            FixType::NoFix => GnssFixType::NoFix,
            FixType::Fix2D => GnssFixType::Fix2D,
            FixType::Fix3D => GnssFixType::Fix3D,
        };
        self.state.signal.satellites_used =
            SatelliteCount::new(gsa.get_fix_satellites_prn().len().min(u8::MAX as usize) as u8);
        self.state.signal.pdop = Some(dop_milli(gsa.pdop));
        self.state.signal.hdop = Some(dop_milli(gsa.hdop));
        self.state.signal.vdop = Some(dop_milli(gsa.vdop));
    }

    fn update_gsv(&mut self, gsv: &GSV) {
        if gsv.message_number == 1 {
            self.state.signal.satellites_with_signal = SatelliteCount::default();
            self.state.signal.strongest_snr = None;
        }
        self.state.signal.satellites_in_view = SatelliteCount::new(gsv.sat_in_view);
        let mut with_signal = self.state.signal.satellites_with_signal.get();
        let mut strongest = self.state.signal.strongest_snr;
        for satellite in gsv.get_in_view_satellites() {
            if let Some(snr) = satellite.snr {
                with_signal = with_signal.saturating_add(1);
                if strongest.is_none_or(|current| snr > current.get()) {
                    strongest = Some(SnrDb::new(snr));
                }
            }
        }
        self.state.signal.satellites_with_signal = SatelliteCount::new(with_signal);
        self.state.signal.strongest_snr = strongest;
    }

    fn update_gga(&mut self, gga: &GGA) {
        let quality = gps_quality(&gga.gps_quality);
        if quality == FixQuality::NoFix {
            self.state.fix = None;
            return;
        }

        let previous = self.state.fix.unwrap_or_default();
        self.state.fix = Some(GnssFix {
            latitude: latitude_e7(&gga.latitude),
            longitude: longitude_e7(&gga.longitude),
            altitude: gga
                .altitude
                .as_ref()
                .map(|altitude| AltitudeMm::new((altitude.meters * 1_000.0) as i32)),
            speed: previous.speed,
            course: previous.course,
            satellites: Some(SatelliteCount::new(gga.sat_in_use)),
            hdop: Some(dop_milli(gga.hdop)),
            quality,
        });
    }
}

impl Default for NmeaParser {
    fn default() -> Self {
        Self::new()
    }
}

fn convert_datetime(datetime: &nmea0183::datetime::DateTime) -> GnssDateTime {
    let mut seconds = datetime.time.seconds as u8;
    let mut milliseconds = ((datetime.time.seconds - seconds as f32) * 1_000.0 + 0.5) as u16;
    if milliseconds >= 1_000 {
        if seconds < 59 {
            seconds += 1;
            milliseconds = 0;
        } else {
            milliseconds = 999;
        }
    }
    GnssDateTime {
        year: datetime.date.year,
        month: datetime.date.month,
        day: datetime.date.day,
        weekday: weekday(datetime.date.year, datetime.date.month, datetime.date.day),
        hours: datetime.time.hours,
        minutes: datetime.time.minutes,
        seconds,
        milliseconds,
    }
}

fn latitude_e7(latitude: &nmea0183::coords::Latitude) -> LatitudeE7 {
    LatitudeE7::new((latitude.as_f64() * 10_000_000.0) as i32)
}

fn longitude_e7(longitude: &nmea0183::coords::Longitude) -> LongitudeE7 {
    LongitudeE7::new((longitude.as_f64() * 10_000_000.0) as i32)
}

fn dop_milli(value: f32) -> DopMilli {
    DopMilli::new((value * 1_000.0) as u32)
}

fn rmc_quality(mode: &Mode) -> FixQuality {
    match mode {
        Mode::Autonomous => FixQuality::Autonomous,
        Mode::Differential => FixQuality::Differential,
        Mode::Estimated => FixQuality::Estimated,
        Mode::Manual => FixQuality::Manual,
        Mode::Simulator => FixQuality::Simulated,
        Mode::NotValid => FixQuality::NoFix,
    }
}

fn gps_quality(quality: &GPSQuality) -> FixQuality {
    match quality {
        GPSQuality::NoFix => FixQuality::NoFix,
        GPSQuality::GPS => FixQuality::Autonomous,
        GPSQuality::DGPS => FixQuality::Differential,
        GPSQuality::PPS => FixQuality::Pps,
        GPSQuality::RTK => FixQuality::Rtk,
        GPSQuality::FRTK => FixQuality::FloatRtk,
        GPSQuality::Estimated => FixQuality::Estimated,
        GPSQuality::Manual => FixQuality::Manual,
        GPSQuality::Simulated => FixQuality::Simulated,
    }
}

fn weekday(year: u16, month: u8, day: u8) -> u8 {
    const MONTH_OFFSETS: [i32; 12] = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
    let mut year = year as i32;
    if month < 3 {
        year -= 1;
    }
    ((year + year / 4 - year / 100 + year / 400 + MONTH_OFFSETS[month as usize - 1] + day as i32)
        % 7) as u8
}

pub struct Lc76g<I, D> {
    i2c: I,
    delay: D,
}

impl<I, D> Lc76g<I, D>
where
    I: I2c,
    D: DelayNs,
{
    /// Creates a driver with an async I²C device and an injected delay source.
    pub fn new(i2c: I, delay: D) -> Self {
        Self { i2c, delay }
    }

    /// Selects the receiver's low-power policy.
    pub async fn set_low_power_mode(
        &mut self,
        mode: LowPowerMode,
    ) -> Result<(), GnssError<I::Error>> {
        match mode {
            LowPowerMode::Disabled => self.write_nmea(ALP_DISABLE).await,
            LowPowerMode::Adaptive => {
                self.write_nmea(SET_NORMAL_NAVIGATION).await?;
                self.write_nmea(SET_FIX_RATE_1_HZ).await?;
                self.write_nmea(ALP_ENABLE).await
            }
        }
    }

    /// Requests the module's adaptive low-power mode.
    pub async fn enable_alp_mode(&mut self) -> Result<(), GnssError<I::Error>> {
        self.set_low_power_mode(LowPowerMode::Adaptive).await
    }

    /// Requests normal continuous tracking mode.
    pub async fn disable_alp_mode(&mut self) -> Result<(), GnssError<I::Error>> {
        self.set_low_power_mode(LowPowerMode::Disabled).await
    }

    /// Reads all currently buffered NMEA data into `buffer`.
    pub async fn read_nmea<'a>(
        &mut self,
        buffer: &'a mut [u8],
    ) -> Result<&'a [u8], GnssError<I::Error>> {
        let available = self.read_nmea_length().await?;
        if available == 0 {
            return Ok(&buffer[..0]);
        }
        if available > buffer.len() {
            return Err(GnssError::BufferTooSmall {
                required: available,
                available: buffer.len(),
            });
        }

        self.write_config(CONFIG_READ_DATA, available as u32)
            .await?;
        self.command_delay().await;
        self.read_data(&mut buffer[..available]).await?;
        Ok(&buffer[..available])
    }

    /// Reads at most `buffer.len()` bytes of currently buffered NMEA data.
    pub async fn read_nmea_chunk<'a>(
        &mut self,
        buffer: &'a mut [u8],
    ) -> Result<&'a [u8], GnssError<I::Error>> {
        let available = self.read_nmea_length().await?;
        let read_len = available.min(buffer.len());
        if read_len == 0 {
            return Ok(&buffer[..0]);
        }

        self.write_config(CONFIG_READ_DATA, read_len as u32).await?;
        self.command_delay().await;
        self.read_data(&mut buffer[..read_len]).await?;
        Ok(&buffer[..read_len])
    }

    pub async fn write_nmea(&mut self, data: &[u8]) -> Result<(), GnssError<I::Error>> {
        let free = self.read_buffer_length(CONFIG_READ_FREE_LENGTH).await?;
        if data.len() > free {
            return Err(GnssError::BufferTooSmall {
                required: data.len(),
                available: free,
            });
        }

        self.write_config(CONFIG_WRITE_NMEA, data.len() as u32)
            .await?;
        self.command_delay().await;
        for attempt in 0..MAX_I2C_RETRIES {
            match self.i2c.write(WRITE_DATA_ADDRESS, data).await {
                Ok(()) => return Ok(()),
                Err(error) if attempt + 1 == MAX_I2C_RETRIES => {
                    return Err(GnssError::I2c {
                        operation: GnssOperation::WriteData,
                        error,
                    });
                }
                Err(_) => self.command_delay().await,
            }
        }
        unreachable!()
    }

    async fn read_nmea_length(&mut self) -> Result<usize, GnssError<I::Error>> {
        self.read_buffer_length(CONFIG_READ_NMEA_LENGTH).await
    }

    async fn read_buffer_length(&mut self, command: u32) -> Result<usize, GnssError<I::Error>> {
        self.write_config(command, NMEA_LENGTH_BYTES as u32).await?;
        self.command_delay().await;
        let mut length = [0; NMEA_LENGTH_BYTES];
        self.read_data(&mut length).await?;
        Ok(u32::from_le_bytes(length) as usize)
    }

    async fn read_data(&mut self, data: &mut [u8]) -> Result<(), GnssError<I::Error>> {
        for attempt in 0..MAX_I2C_RETRIES {
            match self.i2c.read(DATA_ADDRESS, data).await {
                Ok(()) => return Ok(()),
                Err(error) if attempt + 1 == MAX_I2C_RETRIES => {
                    return Err(GnssError::I2c {
                        operation: if data.len() == NMEA_LENGTH_BYTES {
                            GnssOperation::ReadLength
                        } else {
                            GnssOperation::ReadData
                        },
                        error,
                    });
                }
                Err(_) => self.command_delay().await,
            }
        }
        unreachable!()
    }

    async fn write_config(&mut self, command: u32, length: u32) -> Result<(), GnssError<I::Error>> {
        let mut request = [0; 8];
        request[..4].copy_from_slice(&command.to_le_bytes());
        request[4..].copy_from_slice(&length.to_le_bytes());
        for attempt in 0..MAX_I2C_RETRIES {
            match self.i2c.write(CONFIG_ADDRESS, &request).await {
                Ok(()) => return Ok(()),
                Err(error) if attempt + 1 == MAX_I2C_RETRIES => {
                    return Err(GnssError::I2c {
                        operation: GnssOperation::WriteConfig,
                        error,
                    });
                }
                Err(_) => self.command_delay().await,
            }
        }
        unreachable!()
    }

    async fn command_delay(&mut self) {
        self.delay.delay_ms(INTER_COMMAND_DELAY_MS).await;
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ALP_DISABLE, ALP_ENABLE, CONFIG_READ_DATA, CONFIG_READ_FREE_LENGTH,
        CONFIG_READ_NMEA_LENGTH, GnssDateTime, NmeaParser, NmeaUpdate, PairAck, PairAckStatus,
    };

    #[test]
    fn commands_match_quectel_protocol() {
        assert_eq!(
            CONFIG_READ_NMEA_LENGTH.to_le_bytes(),
            [0x08, 0x00, 0x51, 0xAA]
        );
        assert_eq!(CONFIG_READ_DATA.to_le_bytes(), [0x00, 0x20, 0x51, 0xAA]);
        assert_eq!(
            CONFIG_READ_FREE_LENGTH.to_le_bytes(),
            [0x04, 0x00, 0x51, 0xAA]
        );
        assert_eq!(ALP_ENABLE, b"$PAIR732,1*21\r\n");
        assert_eq!(ALP_DISABLE, b"$PAIR732,0*20\r\n");
    }

    #[test]
    fn parser_decodes_rmc_position_and_utc() {
        let mut parser = NmeaParser::new();
        let mut update = None;
        for byte in b"$GPRMC,125504.049,A,5542.2389,N,03741.6063,E,0.06,25.82,200906,,,A*56\r\n" {
            update = parser.push(*byte).unwrap().or(update);
        }

        assert_eq!(update, Some(NmeaUpdate::Rmc));
        assert_eq!(
            parser.state().utc,
            Some(GnssDateTime {
                year: 2006,
                month: 9,
                day: 20,
                weekday: 3,
                hours: 12,
                minutes: 55,
                seconds: 4,
                milliseconds: 49,
            })
        );
        let fix = parser.state().fix.unwrap();
        assert!((557_000_000..558_000_000).contains(&fix.latitude.get()));
        assert!((376_000_000..377_000_000).contains(&fix.longitude.get()));
        assert_eq!(fix.speed.map(|value| value.get()), Some(30));
        assert_eq!(fix.course.map(|value| value.get()), Some(25_820));
    }

    #[test]
    fn parser_merges_gga_quality_and_measurements() {
        let mut parser = NmeaParser::new();
        for byte in b"$GPRMC,125504.049,A,5542.2389,N,03741.6063,E,0.06,25.82,200906,,,A*56\r\n" {
            parser.push(*byte).unwrap();
        }
        for byte in b"$GPGGA,123519,4807.038,N,01131.000,E,1,08,0.9,545.4,M,46.9,M,,*47\r\n" {
            parser.push(*byte).unwrap();
        }

        let fix = parser.state().fix.unwrap();
        assert_eq!(fix.altitude.map(|value| value.get()), Some(545_400));
        assert_eq!(fix.satellites.map(|value| value.get()), Some(8));
        assert_eq!(fix.hdop.map(|value| value.get()), Some(900));
    }

    #[test]
    fn parser_ignores_proprietary_acknowledgements() {
        let mut parser = NmeaParser::new();
        let mut update = None;
        for byte in b"$PAIR001,732,0*3D\r\n" {
            update = parser.push(*byte).unwrap().or(update);
        }
        assert_eq!(
            update,
            Some(NmeaUpdate::PairAck(PairAck {
                command: 732,
                status: PairAckStatus::Accepted,
            }))
        );
    }

    #[test]
    fn parser_rejects_a_bad_checksum_and_recovers() {
        let mut parser = NmeaParser::new();
        let mut error = None;
        for byte in b"$GPGGA,123519,4807.038,N,01131.000,E,1,08,0.9,545.4,M,46.9,M,,*00\r\n" {
            if let Err(value) = parser.push(*byte) {
                error = Some(value);
            }
        }
        assert_eq!(error, Some("Checksum error!"));

        for byte in b"$GPGGA,123519,4807.038,N,01131.000,E,1,08,0.9,545.4,M,46.9,M,,*47\r\n" {
            parser.push(*byte).unwrap();
        }
        assert_eq!(
            parser
                .state()
                .fix
                .unwrap()
                .satellites
                .map(|value| value.get()),
            Some(8)
        );
    }
}
