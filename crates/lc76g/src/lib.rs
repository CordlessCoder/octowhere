#![no_std]

use core::fmt::Write;

use embedded_hal_async::{delay::DelayNs, i2c::I2c};
use heapless::{String, Vec};
use nmea0183::{FixType, GGA, GPSQuality, GSA, GSV, Mode, ParseResult, RMC, Source};

pub use nmea0183::{Sentence as StandardSentence, SentenceMask as StandardSentenceMask};

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
const PAIR_COMMAND_CAPACITY: usize = 120;
const PAIR_MESSAGE_FIELDS_CAPACITY: usize = PAIR_COMMAND_CAPACITY - 8;

const ALP_ENABLE: &[u8] = b"$PAIR732,1*21\r\n";
const ALP_DISABLE: &[u8] = b"$PAIR732,0*20\r\n";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
    PairCommand(PairCommandError),
}

/// Error returned while constructing a proprietary PAIR command.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PairCommandError {
    /// The command identifier does not fit the three decimal digits required
    /// by the LC76G protocol.
    InvalidCommandId,
    /// The encoded command would exceed the maximum NMEA sentence length.
    TooLong,
}

/// A checked, checksummed LC76G proprietary command.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PairCommand {
    bytes: Vec<u8, PAIR_COMMAND_CAPACITY>,
}

impl PairCommand {
    /// Returns the complete command including checksum and line ending.
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }
}

/// Builder for a checksummed LC76G proprietary command.
pub struct PairCommandBuilder {
    bytes: Vec<u8, PAIR_COMMAND_CAPACITY>,
}

impl PairCommandBuilder {
    /// Starts a command with a three-digit PAIR command identifier.
    pub fn new(command_id: u16) -> Result<Self, PairCommandError> {
        if command_id > 999 {
            return Err(PairCommandError::InvalidCommandId);
        }

        let mut prefix = String::<8>::new();
        write!(&mut prefix, "$PAIR{command_id:03}").map_err(|_| PairCommandError::TooLong)?;
        let mut bytes = Vec::new();
        bytes
            .extend_from_slice(prefix.as_bytes())
            .map_err(|_| PairCommandError::TooLong)?;
        Ok(Self { bytes })
    }

    /// Appends a field already encoded as ASCII.
    pub fn field_bytes(&mut self, field: &[u8]) -> Result<(), PairCommandError> {
        if self.bytes.len() + 1 + field.len() + 5 > PAIR_COMMAND_CAPACITY {
            return Err(PairCommandError::TooLong);
        }
        self.bytes
            .push(b',')
            .map_err(|_| PairCommandError::TooLong)?;
        self.bytes
            .extend_from_slice(field)
            .map_err(|_| PairCommandError::TooLong)?;
        Ok(())
    }

    /// Appends an unsigned decimal field.
    pub fn field_u32(&mut self, value: u32) -> Result<(), PairCommandError> {
        let mut field = String::<10>::new();
        write!(&mut field, "{value}").map_err(|_| PairCommandError::TooLong)?;
        self.field_bytes(field.as_bytes())
    }

    /// Appends a signed decimal field.
    pub fn field_i32(&mut self, value: i32) -> Result<(), PairCommandError> {
        let mut field = String::<11>::new();
        write!(&mut field, "{value}").map_err(|_| PairCommandError::TooLong)?;
        self.field_bytes(field.as_bytes())
    }

    /// Appends a boolean field using the receiver's `0` or `1` encoding.
    pub fn field_bool(&mut self, value: bool) -> Result<(), PairCommandError> {
        self.field_u32(value as u32)
    }

    /// Finishes the command by appending its checksum and line ending.
    pub fn finish(mut self) -> Result<PairCommand, PairCommandError> {
        if self.bytes.len() + 5 > PAIR_COMMAND_CAPACITY {
            return Err(PairCommandError::TooLong);
        }
        self.bytes
            .push(b'*')
            .map_err(|_| PairCommandError::TooLong)?;
        let checksum = self.bytes[1..self.bytes.len() - 1]
            .iter()
            .fold(0, |checksum, byte| checksum ^ byte);
        self.bytes
            .push(hex_digit_to_ascii(checksum >> 4))
            .map_err(|_| PairCommandError::TooLong)?;
        self.bytes
            .push(hex_digit_to_ascii(checksum & 0x0F))
            .map_err(|_| PairCommandError::TooLong)?;
        self.bytes
            .extend_from_slice(b"\r\n")
            .map_err(|_| PairCommandError::TooLong)?;
        Ok(PairCommand { bytes: self.bytes })
    }
}

/// The low-power policy requested from the receiver.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LowPowerMode {
    /// Continuous tracking with the receiver's normal duty cycle.
    Disabled,
    /// Adaptive Low Power mode.
    Adaptive,
}

/// Position-fix interval accepted by PAIR050, in milliseconds.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct FixIntervalMs(u16);

impl FixIntervalMs {
    /// The receiver's documented default and one-hertz interval.
    pub const ONE_SECOND: Self = Self(1_000);

    /// Creates an interval in the receiver's supported 100–1000 ms range.
    pub const fn new(milliseconds: u16) -> Option<Self> {
        if milliseconds >= 100 && milliseconds <= 1_000 {
            Some(Self(milliseconds))
        } else {
            None
        }
    }

    /// Returns the interval in milliseconds.
    pub const fn milliseconds(self) -> u16 {
        self.0
    }
}

/// Minimum signal-to-noise ratio accepted for satellites in use.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct MinimumSnrDb(u8);

impl MinimumSnrDb {
    /// Creates a threshold in the receiver's supported 9–37 dB range.
    pub const fn new(decibels: u8) -> Option<Self> {
        if decibels >= 9 && decibels <= 37 {
            Some(Self(decibels))
        } else {
            None
        }
    }

    /// Returns the threshold in decibels.
    pub const fn decibels(self) -> u8 {
        self.0
    }
}

/// Satellite constellation search configuration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GnssSearchMode {
    gps: bool,
    glonass: bool,
    galileo: bool,
    bds: bool,
    qzss: bool,
}

impl GnssSearchMode {
    /// Creates a search mode from the five constellation enable flags.
    pub const fn new(gps: bool, glonass: bool, galileo: bool, bds: bool, qzss: bool) -> Self {
        Self {
            gps,
            glonass,
            galileo,
            bds,
            qzss,
        }
    }

    /// Searches GPS satellites.
    pub const fn gps(self) -> bool {
        self.gps
    }

    /// Searches GLONASS satellites.
    pub const fn glonass(self) -> bool {
        self.glonass
    }

    /// Searches Galileo satellites.
    pub const fn galileo(self) -> bool {
        self.galileo
    }

    /// Searches BeiDou satellites.
    pub const fn bds(self) -> bool {
        self.bds
    }

    /// Searches QZSS satellites.
    pub const fn qzss(self) -> bool {
        self.qzss
    }
}

/// Static-navigation speed threshold in decimetres per second.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct StaticNavigationThreshold(u8);

impl StaticNavigationThreshold {
    /// Disables static navigation.
    pub const DISABLED: Self = Self(0);

    /// Creates a threshold in the receiver's supported 0–20 dm/s range.
    pub const fn new(decimetres_per_second: u8) -> Option<Self> {
        if decimetres_per_second <= 20 {
            Some(Self(decimetres_per_second))
        } else {
            None
        }
    }

    /// Returns the threshold in decimetres per second.
    pub const fn decimetres_per_second(self) -> u8 {
        self.0
    }
}

/// Satellite elevation mask in degrees.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ElevationMaskDegrees(i8);

impl ElevationMaskDegrees {
    /// Creates a mask in the receiver's supported -90–90 degree range.
    pub const fn new(degrees: i8) -> Option<Self> {
        if degrees >= -90 && degrees <= 90 {
            Some(Self(degrees))
        } else {
            None
        }
    }

    /// Returns the mask in degrees.
    pub const fn degrees(self) -> i8 {
        self.0
    }
}

/// Navigation model used by the receiver.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NavigationMode {
    /// General-purpose navigation.
    Normal,
    /// Running and walking navigation.
    Fitness,
    /// Reserved receiver mode 2.
    Reserved2,
    /// Reserved receiver mode 3.
    Reserved3,
    /// Reserved receiver mode 4.
    Reserved4,
    /// Drone navigation.
    Drone,
    /// Reserved receiver mode 6.
    Reserved6,
    /// Swimming navigation.
    Swimming,
}

impl NavigationMode {
    const fn wire_value(self) -> u8 {
        match self {
            Self::Normal => 0,
            Self::Fitness => 1,
            Self::Reserved2 => 2,
            Self::Reserved3 => 3,
            Self::Reserved4 => 4,
            Self::Drone => 5,
            Self::Reserved6 => 6,
            Self::Swimming => 7,
        }
    }
}

/// Active interference cancellation state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AicMode {
    Disabled,
    Enabled,
}

impl AicMode {
    const fn wire_value(self) -> u8 {
        match self {
            Self::Disabled => 0,
            Self::Enabled => 1,
        }
    }
}

/// Binary debug-log output mode.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DebugLogOutput {
    Disabled,
    Full,
    Lite,
}

impl DebugLogOutput {
    const fn wire_value(self) -> u8 {
        match self {
            Self::Disabled => 0,
            Self::Full => 1,
            Self::Lite => 2,
        }
    }
}

/// Standard NMEA sentence types whose output rate can be configured.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NmeaSentence {
    /// Geographic position, altitude and fix-quality data.
    Gga,
    /// Geographic position and fix status data.
    Gll,
    /// Fix dimensionality, active satellites and dilution of precision.
    Gsa,
    /// Satellites in view and their signal levels.
    Gsv,
    /// Recommended minimum position, speed and time data.
    Rmc,
    /// Course and speed relative to the ground.
    Vtg,
    /// UTC date and time.
    Zda,
    /// Residuals of the position solution.
    Grs,
    /// Pseudorange noise statistics.
    Gst,
    /// Combined GNSS fix data.
    Gns,
}

impl NmeaSentence {
    /// All sentence types accepted by the LC76G's PAIR062/PAIR063 commands.
    pub const ALL: [Self; 10] = [
        Self::Gga,
        Self::Gll,
        Self::Gsa,
        Self::Gsv,
        Self::Rmc,
        Self::Vtg,
        Self::Zda,
        Self::Grs,
        Self::Gst,
        Self::Gns,
    ];

    const fn command_id(self) -> u8 {
        match self {
            Self::Gga => 0,
            Self::Gll => 1,
            Self::Gsa => 2,
            Self::Gsv => 3,
            Self::Rmc => 4,
            Self::Vtg => 5,
            Self::Zda => 6,
            Self::Grs => 7,
            Self::Gst => 8,
            Self::Gns => 9,
        }
    }

    const fn from_command_id(command_id: u8) -> Option<Self> {
        Some(match command_id {
            0 => Self::Gga,
            1 => Self::Gll,
            2 => Self::Gsa,
            3 => Self::Gsv,
            4 => Self::Rmc,
            5 => Self::Vtg,
            6 => Self::Zda,
            7 => Self::Grs,
            8 => Self::Gst,
            9 => Self::Gns,
            _ => return None,
        })
    }
}

/// Rate at which a standard NMEA sentence is emitted.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NmeaOutputRate(u8);

impl NmeaOutputRate {
    /// Disables this sentence type.
    pub const DISABLED: Self = Self(0);

    /// Emits this sentence type at every position fix.
    pub const EVERY_FIX: Self = Self(1);

    /// Creates a rate from the receiver's wire representation.
    pub const fn from_interval(fixes: u8) -> Option<Self> {
        if fixes <= 20 { Some(Self(fixes)) } else { None }
    }

    /// Creates a rate that emits once every `fixes` position fixes.
    ///
    /// The receiver accepts values from 1 through 20. `None` represents an
    /// out-of-range value.
    pub const fn every(fixes: u8) -> Option<Self> {
        if fixes == 0 {
            None
        } else {
            Self::from_interval(fixes)
        }
    }

    /// Returns the interval, or `None` when output is disabled.
    pub const fn interval(self) -> Option<u8> {
        match self.0 {
            0 => None,
            fixes => Some(fixes),
        }
    }
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

#[derive(Clone, Copy)]
struct GsvState {
    valid: bool,
    satellites_in_view: u8,
    satellites_with_signal: u8,
    strongest_snr: Option<u8>,
}

impl GsvState {
    const EMPTY: Self = Self {
        valid: false,
        satellites_in_view: 0,
        satellites_with_signal: 0,
        strongest_snr: None,
    };
}

/// Event emitted for parsed receiver data or a proprietary response.
#[derive(Clone, Debug, PartialEq)]
pub enum NmeaUpdate {
    /// A sentence parsed by the underlying NMEA parser.
    Sentence(ParseResult),
    /// A valid NMEA frame not understood by the underlying parser.
    Raw(RawNmeaSentence),
    /// An acknowledgement returned for a proprietary `PAIR` command.
    PairAck(PairAck),
    /// A proprietary response that does not have a specialized representation.
    Pair(PairMessage),
    /// The receiver reported the configured output rate for a sentence type.
    NmeaOutputRate {
        sentence: NmeaSentence,
        rate: NmeaOutputRate,
    },
}

/// A checksummed NMEA frame preserved without interpreting its fields.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RawNmeaSentence {
    bytes: Vec<u8, PAIR_COMMAND_CAPACITY>,
}

impl RawNmeaSentence {
    /// Returns the complete frame including its checksum and line ending.
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }
}

/// Acknowledgement returned for a proprietary `PAIR` command.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PairAck {
    pub command: u16,
    pub status: PairAckStatus,
}

/// A checksummed proprietary response with its fields preserved as ASCII.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PairMessage {
    command: u16,
    fields: [u8; PAIR_MESSAGE_FIELDS_CAPACITY],
    fields_len: u8,
}

impl PairMessage {
    /// Returns the PAIR command identifier.
    pub const fn command(self) -> u16 {
        self.command
    }

    /// Returns the comma-separated response fields without the checksum.
    pub fn fields(&self) -> &[u8] {
        &self.fields[..self.fields_len as usize]
    }
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
    line: Vec<u8, PAIR_COMMAND_CAPACITY>,
}

struct RawNmeaParser {
    line: Vec<u8, PAIR_COMMAND_CAPACITY>,
}

impl RawNmeaParser {
    const fn new() -> Self {
        Self { line: Vec::new() }
    }

    fn push(&mut self, byte: u8) -> Option<RawNmeaSentence> {
        if byte == b'$' {
            self.line.clear();
        }
        if self.line.push(byte).is_err() {
            self.line.clear();
            return None;
        }
        if byte != b'\n' {
            return None;
        }
        let bytes = core::mem::take(&mut self.line);
        Some(RawNmeaSentence { bytes })
    }
}

enum PairUpdate {
    Ack(PairAck),
    Message(PairMessage),
    NmeaOutputRate {
        sentence: NmeaSentence,
        rate: NmeaOutputRate,
    },
}

impl PairAckParser {
    const fn new() -> Self {
        Self { line: Vec::new() }
    }

    fn push(&mut self, byte: u8) -> Option<PairUpdate> {
        if byte == b'$' {
            self.line.clear();
        }
        if self.line.push(byte).is_err() {
            self.line.clear();
            return None;
        }
        if byte != b'\n' {
            return None;
        }

        let ack = parse_pair_update(&self.line);
        self.line.clear();
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

fn parse_pair_update(line: &[u8]) -> Option<PairUpdate> {
    parse_pair_ack(line)
        .map(PairUpdate::Ack)
        .or_else(|| {
            parse_pair_nmea_output_rate(line)
                .map(|(sentence, rate)| PairUpdate::NmeaOutputRate { sentence, rate })
        })
        .or_else(|| parse_pair_message(line).map(PairUpdate::Message))
}

fn parse_pair_message(line: &[u8]) -> Option<PairMessage> {
    let line = line.strip_suffix(b"\r\n")?;
    if !line.starts_with(b"$PAIR") {
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

    let command_end = line[..star]
        .iter()
        .position(|&byte| byte == b',')
        .unwrap_or(star);
    let command = decimal(&line[5..command_end])?;
    let fields_start = (command_end < star)
        .then_some(command_end + 1)
        .unwrap_or(star);
    let fields = &line[fields_start..star];
    if fields.len() > PAIR_MESSAGE_FIELDS_CAPACITY {
        return None;
    }
    let mut message = PairMessage {
        command,
        fields: [0; PAIR_MESSAGE_FIELDS_CAPACITY],
        fields_len: fields.len() as u8,
    };
    message.fields[..fields.len()].copy_from_slice(fields);
    Some(message)
}

fn parse_pair_nmea_output_rate(line: &[u8]) -> Option<(NmeaSentence, NmeaOutputRate)> {
    let line = line.strip_suffix(b"\r\n")?;
    let prefix = b"$PAIR063,";
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
    let sentence = NmeaSentence::from_command_id(u8::try_from(decimal(&fields[..comma])?).ok()?)?;
    let rate = NmeaOutputRate::from_interval(u8::try_from(decimal(&fields[comma + 1..])?).ok()?)?;
    Some((sentence, rate))
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

/// Incrementally parses standard NMEA sentences into receiver events and state.
pub struct NmeaParser {
    parser: nmea0183::Parser,
    pair_ack: PairAckParser,
    raw: RawNmeaParser,
    state: GnssState,
    gsv: [GsvState; 5],
}

impl NmeaParser {
    /// Creates a parser for every sentence type supported by `nmea0183`.
    pub fn new() -> Self {
        Self::with_sentence_filter(
            nmea0183::Sentence::RMC
                | nmea0183::Sentence::GGA
                | nmea0183::Sentence::GSA
                | nmea0183::Sentence::GSV
                | nmea0183::Sentence::GLL
                | nmea0183::Sentence::VTG
                | nmea0183::Sentence::ZDA,
        )
    }

    /// Creates a parser that emits only the selected standard sentence types.
    pub fn with_sentence_filter(filter: StandardSentenceMask) -> Self {
        Self {
            parser: nmea0183::Parser::new().sentence_filter(filter),
            pair_ack: PairAckParser::new(),
            raw: RawNmeaParser::new(),
            state: GnssState::default(),
            gsv: [GsvState::EMPTY; 5],
        }
    }

    /// Returns the latest state projected from the parsed fix sentences.
    pub fn state(&self) -> GnssState {
        self.state
    }

    /// Feeds one byte from the receiver's NMEA stream into the parser.
    pub fn push(&mut self, byte: u8) -> Result<Option<NmeaUpdate>, &'static str> {
        let raw = self.raw.push(byte).map(NmeaUpdate::Raw);
        let pair_update = self.pair_ack.push(byte).map(|update| match update {
            PairUpdate::Ack(ack) => NmeaUpdate::PairAck(ack),
            PairUpdate::Message(message) => NmeaUpdate::Pair(message),
            PairUpdate::NmeaOutputRate { sentence, rate } => {
                NmeaUpdate::NmeaOutputRate { sentence, rate }
            }
        });
        let Some(result) = self.parser.parse_from_byte(byte) else {
            return Ok(pair_update);
        };
        let result = match result {
            Ok(result) => result,
            Err("Source is not supported!" | "Unsupported sentence type.") => {
                return Ok(pair_update.or(raw));
            }
            Err(error) => return Err(error),
        };

        match result {
            ParseResult::RMC(Some(rmc)) => {
                self.update_rmc(&rmc);
                Ok(Some(NmeaUpdate::Sentence(ParseResult::RMC(Some(rmc)))))
            }
            ParseResult::RMC(None) => {
                self.state.fix = None;
                self.state.utc = None;
                Ok(Some(NmeaUpdate::Sentence(ParseResult::RMC(None))))
            }
            ParseResult::GGA(Some(gga)) => {
                self.update_gga(&gga);
                Ok(Some(NmeaUpdate::Sentence(ParseResult::GGA(Some(gga)))))
            }
            ParseResult::GGA(None) => {
                self.state.fix = None;
                Ok(Some(NmeaUpdate::Sentence(ParseResult::GGA(None))))
            }
            ParseResult::GSA(Some(gsa)) => {
                self.update_gsa(&gsa);
                Ok(Some(NmeaUpdate::Sentence(ParseResult::GSA(Some(gsa)))))
            }
            ParseResult::GSA(None) => {
                self.state.signal = GnssSignal::default();
                Ok(Some(NmeaUpdate::Sentence(ParseResult::GSA(None))))
            }
            ParseResult::GSV(Some(gsv)) => {
                self.update_gsv(&gsv);
                Ok(Some(NmeaUpdate::Sentence(ParseResult::GSV(Some(gsv)))))
            }
            ParseResult::GSV(None) => Ok(Some(NmeaUpdate::Sentence(ParseResult::GSV(None)))),
            result => Ok(Some(NmeaUpdate::Sentence(result))),
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
        if gsa.fix_type == FixType::NoFix {
            self.state.signal.satellites_used = SatelliteCount::default();
            self.state.signal.pdop = None;
            self.state.signal.hdop = None;
            self.state.signal.vdop = None;
        } else {
            self.state.signal.satellites_used =
                SatelliteCount::new(gsa.get_fix_satellites_prn().len().min(u8::MAX as usize) as u8);
            self.state.signal.pdop = Some(dop_milli(gsa.pdop));
            self.state.signal.hdop = Some(dop_milli(gsa.hdop));
            self.state.signal.vdop = Some(dop_milli(gsa.vdop));
        }
    }

    fn update_gsv(&mut self, gsv: &GSV) {
        let source = match gsv.source {
            Source::GPS => 0,
            Source::GLONASS => 1,
            Source::Gallileo => 2,
            Source::Beidou => 3,
            Source::GNSS => 4,
        };
        let source_state = &mut self.gsv[source];
        if gsv.message_number == 1 {
            *source_state = GsvState {
                valid: true,
                ..GsvState::EMPTY
            };
        }
        source_state.valid = true;
        source_state.satellites_in_view = gsv.sat_in_view;
        for satellite in gsv.get_in_view_satellites() {
            if let Some(snr) = satellite.snr {
                source_state.satellites_with_signal =
                    source_state.satellites_with_signal.saturating_add(1);
                if source_state
                    .strongest_snr
                    .is_none_or(|current| snr > current)
                {
                    source_state.strongest_snr = Some(snr);
                }
            }
        }

        let specific_sources = &self.gsv[..4];
        let sources = if specific_sources.iter().any(|source| source.valid) {
            specific_sources
        } else {
            &self.gsv[4..]
        };
        let mut satellites_in_view = 0u8;
        let mut satellites_with_signal = 0u8;
        let mut strongest_snr = None;
        for source in sources {
            satellites_in_view = satellites_in_view.saturating_add(source.satellites_in_view);
            satellites_with_signal =
                satellites_with_signal.saturating_add(source.satellites_with_signal);
            if let Some(snr) = source.strongest_snr
                && strongest_snr.is_none_or(|current| snr > current)
            {
                strongest_snr = Some(snr);
            }
        }
        self.state.signal.satellites_in_view = SatelliteCount::new(satellites_in_view);
        self.state.signal.satellites_with_signal = SatelliteCount::new(satellites_with_signal);
        self.state.signal.strongest_snr = strongest_snr.map(SnrDb::new);
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
                self.set_navigation_mode(NavigationMode::Normal).await?;
                self.set_fix_interval(FixIntervalMs::ONE_SECOND).await?;
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

    /// Sets the receiver's position-fix interval.
    pub async fn set_fix_interval(
        &mut self,
        interval: FixIntervalMs,
    ) -> Result<(), GnssError<I::Error>> {
        let mut builder = PairCommandBuilder::new(50).map_err(GnssError::PairCommand)?;
        builder
            .field_u32(interval.milliseconds() as u32)
            .map_err(GnssError::PairCommand)?;
        self.send_pair_builder(builder).await
    }

    /// Requests the receiver's configured position-fix interval.
    pub async fn query_fix_interval(&mut self) -> Result<(), GnssError<I::Error>> {
        self.send_pair_builder(PairCommandBuilder::new(51).map_err(GnssError::PairCommand)?)
            .await
    }

    /// Sets the minimum signal-to-noise ratio for a satellite to be used.
    pub async fn set_minimum_snr(
        &mut self,
        threshold: MinimumSnrDb,
    ) -> Result<(), GnssError<I::Error>> {
        let mut builder = PairCommandBuilder::new(58).map_err(GnssError::PairCommand)?;
        builder
            .field_u32(threshold.decibels() as u32)
            .map_err(GnssError::PairCommand)?;
        self.send_pair_builder(builder).await
    }

    /// Requests the receiver's configured minimum signal-to-noise ratio.
    pub async fn query_minimum_snr(&mut self) -> Result<(), GnssError<I::Error>> {
        self.send_pair_builder(PairCommandBuilder::new(59).map_err(GnssError::PairCommand)?)
            .await
    }

    /// Selects the satellite constellations used during acquisition.
    pub async fn set_gnss_search_mode(
        &mut self,
        mode: GnssSearchMode,
    ) -> Result<(), GnssError<I::Error>> {
        let mut builder = PairCommandBuilder::new(66).map_err(GnssError::PairCommand)?;
        for constellation in [
            mode.gps(),
            mode.glonass(),
            mode.galileo(),
            mode.bds(),
            mode.qzss(),
        ] {
            builder
                .field_bool(constellation)
                .map_err(GnssError::PairCommand)?;
        }
        builder.field_u32(0).map_err(GnssError::PairCommand)?;
        self.send_pair_builder(builder).await
    }

    /// Requests the receiver's configured constellation search mode.
    pub async fn query_gnss_search_mode(&mut self) -> Result<(), GnssError<I::Error>> {
        self.send_pair_builder(PairCommandBuilder::new(67).map_err(GnssError::PairCommand)?)
            .await
    }

    /// Sets the static-navigation speed threshold.
    pub async fn set_static_navigation_threshold(
        &mut self,
        threshold: StaticNavigationThreshold,
    ) -> Result<(), GnssError<I::Error>> {
        let mut builder = PairCommandBuilder::new(70).map_err(GnssError::PairCommand)?;
        builder
            .field_u32(threshold.decimetres_per_second() as u32)
            .map_err(GnssError::PairCommand)?;
        self.send_pair_builder(builder).await
    }

    /// Requests the receiver's static-navigation speed threshold.
    pub async fn query_static_navigation_threshold(&mut self) -> Result<(), GnssError<I::Error>> {
        self.send_pair_builder(PairCommandBuilder::new(71).map_err(GnssError::PairCommand)?)
            .await
    }

    /// Sets the minimum satellite elevation used during navigation.
    pub async fn set_elevation_mask(
        &mut self,
        mask: ElevationMaskDegrees,
    ) -> Result<(), GnssError<I::Error>> {
        let mut builder = PairCommandBuilder::new(72).map_err(GnssError::PairCommand)?;
        builder
            .field_i32(mask.degrees() as i32)
            .map_err(GnssError::PairCommand)?;
        self.send_pair_builder(builder).await
    }

    /// Requests the receiver's minimum satellite elevation.
    pub async fn query_elevation_mask(&mut self) -> Result<(), GnssError<I::Error>> {
        self.send_pair_builder(PairCommandBuilder::new(73).map_err(GnssError::PairCommand)?)
            .await
    }

    /// Enables or disables active interference cancellation.
    pub async fn set_aic_mode(&mut self, mode: AicMode) -> Result<(), GnssError<I::Error>> {
        let mut builder = PairCommandBuilder::new(74).map_err(GnssError::PairCommand)?;
        builder
            .field_u32(mode.wire_value() as u32)
            .map_err(GnssError::PairCommand)?;
        self.send_pair_builder(builder).await
    }

    /// Requests the receiver's active interference cancellation state.
    pub async fn query_aic_mode(&mut self) -> Result<(), GnssError<I::Error>> {
        self.send_pair_builder(PairCommandBuilder::new(75).map_err(GnssError::PairCommand)?)
            .await
    }

    /// Sets the receiver's navigation model.
    pub async fn set_navigation_mode(
        &mut self,
        mode: NavigationMode,
    ) -> Result<(), GnssError<I::Error>> {
        let mut builder = PairCommandBuilder::new(80).map_err(GnssError::PairCommand)?;
        builder
            .field_u32(mode.wire_value() as u32)
            .map_err(GnssError::PairCommand)?;
        self.send_pair_builder(builder).await
    }

    /// Requests the receiver's navigation model.
    pub async fn query_navigation_mode(&mut self) -> Result<(), GnssError<I::Error>> {
        self.send_pair_builder(PairCommandBuilder::new(81).map_err(GnssError::PairCommand)?)
            .await
    }

    /// Sets the receiver's binary debug-log output mode.
    pub async fn set_debug_log_output(
        &mut self,
        output: DebugLogOutput,
    ) -> Result<(), GnssError<I::Error>> {
        let mut builder = PairCommandBuilder::new(86).map_err(GnssError::PairCommand)?;
        builder
            .field_u32(output.wire_value() as u32)
            .map_err(GnssError::PairCommand)?;
        self.send_pair_builder(builder).await
    }

    /// Requests the receiver's binary debug-log output mode.
    pub async fn query_debug_log_output(&mut self) -> Result<(), GnssError<I::Error>> {
        self.send_pair_builder(PairCommandBuilder::new(87).map_err(GnssError::PairCommand)?)
            .await
    }

    /// Sets the output rate for one standard NMEA sentence type.
    pub async fn set_nmea_output_rate(
        &mut self,
        sentence: NmeaSentence,
        rate: NmeaOutputRate,
    ) -> Result<(), GnssError<I::Error>> {
        let command = pair_set_nmea_output_rate(sentence, rate).map_err(GnssError::PairCommand)?;
        self.send_pair_command(&command).await
    }

    /// Restores all standard NMEA output rates to the receiver defaults.
    pub async fn reset_nmea_output_rates(&mut self) -> Result<(), GnssError<I::Error>> {
        let command = pair_set_all_nmea_output_rates().map_err(GnssError::PairCommand)?;
        self.send_pair_command(&command).await
    }

    /// Requests the configured output rate for one sentence type.
    ///
    /// The result arrives asynchronously in the receiver's NMEA stream as
    /// [`NmeaUpdate::NmeaOutputRate`].
    pub async fn query_nmea_output_rate(
        &mut self,
        sentence: NmeaSentence,
    ) -> Result<(), GnssError<I::Error>> {
        let command = pair_get_nmea_output_rate(Some(sentence)).map_err(GnssError::PairCommand)?;
        self.send_pair_command(&command).await
    }

    /// Requests the configured output rates for every sentence type.
    ///
    /// The results arrive asynchronously in the receiver's NMEA stream as
    /// [`NmeaUpdate::NmeaOutputRate`] events.
    pub async fn query_all_nmea_output_rates(&mut self) -> Result<(), GnssError<I::Error>> {
        let command = pair_get_nmea_output_rate(None).map_err(GnssError::PairCommand)?;
        self.send_pair_command(&command).await
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

        self.read_after_config(
            CONFIG_READ_DATA,
            &mut buffer[..available],
            GnssOperation::ReadData,
        )
        .await?;
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

        self.read_after_config(
            CONFIG_READ_DATA,
            &mut buffer[..read_len],
            GnssOperation::ReadData,
        )
        .await?;
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

        self.write_after_config(CONFIG_WRITE_NMEA, data, GnssOperation::WriteData)
            .await
    }

    /// Sends a checked, checksummed proprietary PAIR command.
    pub async fn send_pair_command(
        &mut self,
        command: &PairCommand,
    ) -> Result<(), GnssError<I::Error>> {
        self.write_nmea(command.as_bytes()).await
    }

    async fn send_pair_builder(
        &mut self,
        builder: PairCommandBuilder,
    ) -> Result<(), GnssError<I::Error>> {
        let command = builder.finish().map_err(GnssError::PairCommand)?;
        self.send_pair_command(&command).await
    }

    async fn read_nmea_length(&mut self) -> Result<usize, GnssError<I::Error>> {
        self.read_buffer_length(CONFIG_READ_NMEA_LENGTH).await
    }

    async fn read_buffer_length(&mut self, command: u32) -> Result<usize, GnssError<I::Error>> {
        let mut length = [0; NMEA_LENGTH_BYTES];
        self.read_after_config(command, &mut length, GnssOperation::ReadLength)
            .await?;
        Ok(u32::from_le_bytes(length) as usize)
    }

    async fn read_after_config(
        &mut self,
        command: u32,
        data: &mut [u8],
        operation: GnssOperation,
    ) -> Result<(), GnssError<I::Error>> {
        for attempt in 0..MAX_I2C_RETRIES {
            self.write_config(command, data.len() as u32).await?;
            self.command_delay().await;
            match self.i2c.read(DATA_ADDRESS, data).await {
                Ok(()) => return Ok(()),
                Err(error) if attempt + 1 == MAX_I2C_RETRIES => {
                    return Err(GnssError::I2c {
                        operation,
                        error,
                    });
                }
                Err(_) => self.command_delay().await,
            }
        }
        unreachable!()
    }

    async fn write_after_config(
        &mut self,
        command: u32,
        data: &[u8],
        operation: GnssOperation,
    ) -> Result<(), GnssError<I::Error>> {
        for attempt in 0..MAX_I2C_RETRIES {
            self.write_config(command, data.len() as u32).await?;
            self.command_delay().await;
            match self.i2c.write(WRITE_DATA_ADDRESS, data).await {
                Ok(()) => return Ok(()),
                Err(error) if attempt + 1 == MAX_I2C_RETRIES => {
                    return Err(GnssError::I2c { operation, error });
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

fn pair_set_nmea_output_rate(
    sentence: NmeaSentence,
    rate: NmeaOutputRate,
) -> Result<PairCommand, PairCommandError> {
    let mut builder = PairCommandBuilder::new(62)?;
    builder.field_u32(sentence.command_id() as u32)?;
    builder.field_u32(rate.0 as u32)?;
    builder.finish()
}

fn pair_set_all_nmea_output_rates() -> Result<PairCommand, PairCommandError> {
    let mut builder = PairCommandBuilder::new(62)?;
    builder.field_i32(-1)?;
    builder.finish()
}

fn pair_get_nmea_output_rate(
    sentence: Option<NmeaSentence>,
) -> Result<PairCommand, PairCommandError> {
    let mut builder = PairCommandBuilder::new(63)?;
    if let Some(sentence) = sentence {
        builder.field_u32(sentence.command_id() as u32)?;
    } else {
        builder.field_i32(-1)?;
    }
    builder.finish()
}

fn hex_digit_to_ascii(value: u8) -> u8 {
    match value {
        0..=9 => b'0' + value,
        10..=15 => b'A' + value - 10,
        _ => unreachable!(),
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

        assert!(matches!(update, Some(NmeaUpdate::Sentence(_))));
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
    fn parser_aggregates_gsv_across_constellations() {
        let mut parser = NmeaParser::new();
        for sentence in [
            "$GPGSV,1,1,04,06,67,286,,04,66,087,,03,31,082,29,31,15,035,24,1*66\r\n",
            "$GLGSV,1,1,00,1*78\r\n",
            "$GAGSV,1,1,00,7*73\r\n",
            "$GBGSV,1,1,00,1*76\r\n",
        ] {
            for byte in sentence.as_bytes() {
                parser.push(*byte).unwrap();
            }
        }

        let signal = parser.state().signal;
        assert_eq!(signal.satellites_in_view.get(), 4);
        assert_eq!(signal.satellites_with_signal.get(), 2);
        assert_eq!(signal.strongest_snr.map(|value| value.get()), Some(29));

        for byte in b"$GPGSV,1,1,00,1*64\r\n" {
            parser.push(*byte).unwrap();
        }
        assert_eq!(parser.state().signal.satellites_in_view.get(), 0);
        assert_eq!(parser.state().signal.satellites_with_signal.get(), 0);
        assert_eq!(parser.state().signal.strongest_snr, None);
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
