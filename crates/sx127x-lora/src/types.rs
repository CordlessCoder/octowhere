#[cfg(feature = "defmt")]
use defmt::error;

use crate::constants::PAYLOAD_SIZE;
use crate::registers::{PREAMBLE_LENGTH_DEFAULT, SYNC_WORD_DEFAULT};
use crate::types::PowerRamp::*;
use crate::validate;
use crate::validate::{RX_TIMEOUT_SYMBOLS_MAX, RX_TIMEOUT_SYMBOLS_MIN};
use crate::{calculate, registers};
use sx127x_common::bits::get_bits;
use sx127x_common::error::Sx127xError;
use sx127x_common::error::Sx127xError::InvalidInput;
use sx127x_common::{DEFAULT_FREQUENCY_HZ, Hz, Sx127xVariant};

#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Bandwidth {
    Bw7_8kHz = 0x0,
    Bw10_4kHz = 0x1,
    Bw15_6kHz = 0x2,
    Bw20_8kHz = 0x3,
    Bw31_25kHz = 0x4,
    Bw41_7kHz = 0x5,
    Bw62_5kHz = 0x6,
    #[default]
    Bw125kHz = 0x7,
    Bw250kHz = 0x8,
    Bw500kHz = 0x9,
}
impl From<u8> for Bandwidth {
    fn from(value: u8) -> Self {
        match value {
            0x0 => Bandwidth::Bw7_8kHz,
            0x1 => Bandwidth::Bw10_4kHz,
            0x2 => Bandwidth::Bw15_6kHz,
            0x3 => Bandwidth::Bw20_8kHz,
            0x4 => Bandwidth::Bw31_25kHz,
            0x5 => Bandwidth::Bw41_7kHz,
            0x6 => Bandwidth::Bw62_5kHz,
            0x8 => Bandwidth::Bw250kHz,
            0x9 => Bandwidth::Bw500kHz,
            _ => Bandwidth::Bw125kHz,
        }
    }
}
impl Bandwidth {
    pub(crate) fn hz(&self) -> u32 {
        match self {
            Bandwidth::Bw7_8kHz => 7_800,
            Bandwidth::Bw10_4kHz => 10_400,
            Bandwidth::Bw15_6kHz => 15_600,
            Bandwidth::Bw20_8kHz => 20_800,
            Bandwidth::Bw31_25kHz => 31_250,
            Bandwidth::Bw41_7kHz => 41_700,
            Bandwidth::Bw62_5kHz => 62_500,
            Bandwidth::Bw125kHz => 125_000,
            Bandwidth::Bw250kHz => 250_000,
            _ => 500_000,
        }
    }

    pub(crate) fn khz(&self) -> f32 {
        self.hz() as f32 / 1000.0
    }

    pub(crate) fn optimized_rx_response(&self) -> OptimizedRxResponse {
        match self {
            Bandwidth::Bw7_8kHz => {
                OptimizedRxResponse::new(false, Some(0x00), Some(0x48), Some(7_810))
            }
            Bandwidth::Bw10_4kHz => {
                OptimizedRxResponse::new(false, Some(0x00), Some(0x44), Some(10_420))
            }
            Bandwidth::Bw15_6kHz => {
                OptimizedRxResponse::new(false, Some(0x00), Some(0x44), Some(15_620))
            }
            Bandwidth::Bw20_8kHz => {
                OptimizedRxResponse::new(false, Some(0x00), Some(0x44), Some(20_830))
            }
            Bandwidth::Bw31_25kHz => {
                OptimizedRxResponse::new(false, Some(0x00), Some(0x44), Some(31_250))
            }
            Bandwidth::Bw41_7kHz => {
                OptimizedRxResponse::new(false, Some(0x00), Some(0x44), Some(41_670))
            }
            Bandwidth::Bw62_5kHz => OptimizedRxResponse::new(false, Some(0x00), Some(0x40), None),
            Bandwidth::Bw125kHz => OptimizedRxResponse::new(false, Some(0x00), Some(0x40), None),
            Bandwidth::Bw250kHz => OptimizedRxResponse::new(false, Some(0x00), Some(0x40), None),
            Bandwidth::Bw500kHz => OptimizedRxResponse::new(true, None, None, None),
        }
    }
}

// -------------------------------------------------------------------------------------------------
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum CodingRate {
    #[default]
    Cr4_5 = 0x1,
    Cr4_6 = 0x2,
    Cr4_7 = 0x3,
    Cr4_8 = 0x4,
}
impl From<u8> for CodingRate {
    fn from(value: u8) -> Self {
        match value {
            0x2 => CodingRate::Cr4_6,
            0x3 => CodingRate::Cr4_7,
            0x4 => CodingRate::Cr4_8,
            _ => CodingRate::Cr4_5,
        }
    }
}
impl From<CodingRate> for f32 {
    fn from(value: CodingRate) -> Self {
        4f32 / (match value {
            CodingRate::Cr4_5 => 5f32,
            CodingRate::Cr4_6 => 6f32,
            CodingRate::Cr4_7 => 7f32,
            CodingRate::Cr4_8 => 8f32,
        })
    }
}

// -------------------------------------------------------------------------------------------------
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum DeviceMode {
    SLEEP = 0x0,
    #[default]
    STDBY = 0x1,
    FSTX = 0x2,
    TX = 0x3,
    FSRX = 0x4,
    RXCONTINUOUS = 0x5,
    RXSINGLE = 0x6,
    CAD = 0x7,
}
impl From<u8> for DeviceMode {
    fn from(value: u8) -> Self {
        match value {
            0x0 => DeviceMode::SLEEP,
            0x2 => DeviceMode::FSTX,
            0x3 => DeviceMode::TX,
            0x4 => DeviceMode::FSRX,
            0x5 => DeviceMode::RXCONTINUOUS,
            0x6 => DeviceMode::RXSINGLE,
            0x7 => DeviceMode::CAD,
            _ => DeviceMode::STDBY,
        }
    }
}

// -------------------------------------------------------------------------------------------------
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum HeaderMode {
    #[default]
    Explicit = 0x0,
    Implicit = 0x1,
}
impl From<u8> for HeaderMode {
    fn from(value: u8) -> Self {
        match value {
            0x0 => HeaderMode::Explicit,
            _ => HeaderMode::Implicit,
        }
    }
}

// IRQs --------------------------------------------------------------------------------------------
pub trait IRQ: private::Sealed {
    const MASK: u8;
}

pub struct CadDetected {}
impl IRQ for CadDetected {
    const MASK: u8 = registers::IRQ_FLAGS_CAD_DETECTED_MASK;
}

pub struct CadDone {}
impl IRQ for CadDone {
    const MASK: u8 = registers::IRQ_FLAGS_CAD_DONE_MASK;
}

pub struct FhssChangeChannel {}
impl IRQ for FhssChangeChannel {
    const MASK: u8 = registers::IRQ_FLAGS_FHSS_CHANGE_CHANNEL_MASK;
}

pub struct PayloadCrcError {}
impl IRQ for PayloadCrcError {
    const MASK: u8 = registers::IRQ_FLAGS_PAYLOAD_CRC_ERROR_MASK;
}

pub struct RxDone {}
impl IRQ for RxDone {
    const MASK: u8 = registers::IRQ_FLAGS_RX_DONE_MASK;
}

pub struct RxTimeout {}
impl IRQ for RxTimeout {
    const MASK: u8 = registers::IRQ_FLAGS_RX_TIMEOUT_MASK;
}

pub struct TxDone {}
impl IRQ for TxDone {
    const MASK: u8 = registers::IRQ_FLAGS_TX_DONE_MASK;
}

pub struct ValidHeader {}
impl IRQ for ValidHeader {
    const MASK: u8 = registers::IRQ_FLAGS_VALID_HEADER_MASK;
}

// Dio0 --------------------------------------------------------------------------------------------
pub trait Dio0Signal: private::Sealed {
    const VALUE: u8;
}

impl Dio0Signal for RxDone {
    const VALUE: u8 = 0x0;
}
impl Dio0Signal for TxDone {
    const VALUE: u8 = 0x1;
}
impl Dio0Signal for CadDone {
    const VALUE: u8 = 0x2;
}
// Dio1 --------------------------------------------------------------------------------------------
pub trait Dio1Signal: private::Sealed {
    const VALUE: u8;
}
impl Dio1Signal for CadDetected {
    const VALUE: u8 = 0x2;
}
impl Dio1Signal for FhssChangeChannel {
    const VALUE: u8 = 0x1;
}
impl Dio1Signal for RxTimeout {
    const VALUE: u8 = 0x0;
}

// Dio2 --------------------------------------------------------------------------------------------
pub trait Dio2Signal: private::Sealed {
    const VALUE: u8;
}
impl Dio2Signal for FhssChangeChannel {
    const VALUE: u8 = 0x0;
}

// Dio3 --------------------------------------------------------------------------------------------
pub trait Dio3Signal: private::Sealed {
    const VALUE: u8;
}
impl Dio3Signal for CadDone {
    const VALUE: u8 = 0x0;
}
impl Dio3Signal for PayloadCrcError {
    const VALUE: u8 = 0x2;
}
impl Dio3Signal for ValidHeader {
    const VALUE: u8 = 0x1;
}

// Dio4 --------------------------------------------------------------------------------------------
pub trait Dio4Signal: private::Sealed {
    const VALUE: u8;
}
pub struct PllLock {}
impl Dio4Signal for CadDetected {
    const VALUE: u8 = 0x0;
}
impl Dio4Signal for PllLock {
    const VALUE: u8 = 0x1;
}

// Dio5 --------------------------------------------------------------------------------------------
pub trait Dio5Signal: private::Sealed {
    const VALUE: u8;
}
pub struct ClkOut {}
pub struct ModeReady {}
impl Dio5Signal for ClkOut {
    const VALUE: u8 = 0x1;
}
impl Dio5Signal for ModeReady {
    const VALUE: u8 = 0x0;
}

// -------------------------------------------------------------------------------------------------

mod private {
    use crate::types::{
        CadDetected, CadDone, ClkOut, FhssChangeChannel, ModeReady, PayloadCrcError, PllLock,
        RxDone, RxTimeout, TxDone, ValidHeader,
    };

    pub trait Sealed {}
    impl Sealed for CadDetected {}
    impl Sealed for CadDone {}
    impl Sealed for ClkOut {}
    impl Sealed for FhssChangeChannel {}
    impl Sealed for ModeReady {}
    impl Sealed for PayloadCrcError {}
    impl Sealed for PllLock {}
    impl Sealed for RxDone {}
    impl Sealed for RxTimeout {}
    impl Sealed for TxDone {}
    impl Sealed for ValidHeader {}
}

// -------------------------------------------------------------------------------------------------
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum LNAGain {
    Auto,
    #[default]
    G1 = 0x1,
    G2 = 0x2,
    G3 = 0x3,
    G4 = 0x4,
    G5 = 0x5,
    G6 = 0x6,
}
impl From<u8> for LNAGain {
    fn from(value: u8) -> Self {
        match value {
            0x2 => LNAGain::G2,
            0x3 => LNAGain::G3,
            0x4 => LNAGain::G4,
            0x5 => LNAGain::G5,
            0x6 => LNAGain::G6,
            _ => LNAGain::G1,
        }
    }
}

#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Default)]
pub struct LNA {
    pub boost_hf: bool,
    pub gain: LNAGain,
}
impl LNA {
    pub fn new(boost_hf: bool, gain: LNAGain) -> Self {
        Self { boost_hf, gain }
    }
}
impl From<u8> for LNA {
    fn from(value: u8) -> Self {
        Self {
            boost_hf: get_bits(
                value,
                registers::LNA_BOOST_HF_MASK,
                registers::LNA_BOOST_HF_OFFSET,
            ) == 1,
            gain: LNAGain::from(get_bits(
                value,
                registers::LNA_GAIN_MASK,
                registers::LNA_GAIN_OFFSET,
            )),
        }
    }
}

// -------------------------------------------------------------------------------------------------
pub(crate) struct OptimizedRxResponse {
    pub(crate) automatic_if: bool,
    pub(crate) if_freq_1: Option<u8>,
    pub(crate) if_freq_2: Option<u8>,
    pub(crate) frequency_offset: Option<i32>,
}
impl OptimizedRxResponse {
    fn new(
        automatic_if: bool,
        if_freq_1: Option<u8>,
        if_freq_2: Option<u8>,
        frequency_offset: Option<i32>,
    ) -> Self {
        Self {
            automatic_if,
            if_freq_1,
            if_freq_2,
            frequency_offset,
        }
    }
}

// -------------------------------------------------------------------------------------------------
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum RxStatus {
    SignalDetected,
    SignalSynchronized,
    RxOnGoing,
    HeaderInfoValid,
    #[default]
    ModemClear,
}
impl From<u8> for RxStatus {
    fn from(value: u8) -> Self {
        match value {
            registers::MODEM_STAT_MODEM_STATUS_SIGNAL_DETECTED => RxStatus::SignalDetected,
            registers::MODEM_STAT_MODEM_STATUS_SIGNAL_SYNCHRONIZED => RxStatus::SignalSynchronized,
            registers::MODEM_STAT_MODEM_STATUS_RX_ONGOING_MASK => RxStatus::RxOnGoing,
            registers::MODEM_STAT_MODEM_STATUS_HEADER_INFO_VALID_MASK => RxStatus::HeaderInfoValid,
            _ => RxStatus::ModemClear,
        }
    }
}

// -------------------------------------------------------------------------------------------------
pub struct RxPacket {
    pub coding_rate: CodingRate,
    pub length: usize,
    pub payload: [u8; PAYLOAD_SIZE],
    pub rssi: i16,
    pub snr: i16,
    /// Signed `PacketSnr` register value in quarter-dB units.
    pub snr_raw: i8,
}
impl RxPacket {
    pub(crate) fn new(
        coding_rate: CodingRate,
        length: usize,
        payload: [u8; PAYLOAD_SIZE],
        rssi: i16,
        snr: i16,
        snr_raw: i8,
    ) -> Self {
        Self {
            coding_rate,
            length,
            payload,
            rssi,
            snr,
            snr_raw,
        }
    }

    pub fn payload(&self) -> &[u8] {
        &self.payload[..self.length]
    }
}

// -------------------------------------------------------------------------------------------------
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct Sx127xLoraConfig {
    /// Will automatically optimize for bandwidth, data rate and spurious rx response.
    pub auto_optimize: bool,
    pub bandwidth: Bandwidth,
    pub coding_rate: CodingRate,
    pub frequency: Hz,
    pub header_mode: HeaderMode,
    pub invert_iq: bool,
    pub preamble_length: PreambleLength,
    pub spreading_factor: SpreadingFactor,
    pub sync_word: u8,
    /// Whether or not to use the cyclic redundancy check (CRC) generation and verification on rx/tx payloads.
    pub use_crc: bool,
}
impl Sx127xLoraConfig {
    pub fn for_variant<V: Sx127xVariant>() -> Self {
        Self {
            frequency: V::DEFAULT_FREQUENCY_HZ,
            ..Self::default()
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new(
        auto_optimize: bool,
        bandwidth: Bandwidth,
        coding_rate: CodingRate,
        frequency: Hz,
        header_mode: HeaderMode,
        invert_iq: bool,
        preamble_length: PreambleLength,
        spreading_factor: SpreadingFactor,
        sync_word: u8,
        use_crc: bool,
    ) -> Result<Self, Sx127xError<()>> {
        if !validate::header_mode_sf(header_mode, spreading_factor) {
            #[cfg(feature = "defmt")]
            error!("SF6 requires implicit header mode");
            return Err(Sx127xError::SF6RequiresImplicitHeaderMode);
        }
        Ok(Self {
            auto_optimize,
            bandwidth,
            coding_rate,
            frequency,
            header_mode,
            invert_iq,
            preamble_length,
            spreading_factor,
            sync_word,
            use_crc,
        })
    }

    pub(crate) fn is_valid(&self) -> bool {
        validate::header_mode_sf(self.header_mode, self.spreading_factor)
    }
}
impl Default for Sx127xLoraConfig {
    fn default() -> Self {
        Self {
            auto_optimize: false,
            bandwidth: Bandwidth::default(),
            coding_rate: CodingRate::default(),
            frequency: DEFAULT_FREQUENCY_HZ,
            header_mode: HeaderMode::default(),
            invert_iq: false,
            preamble_length: PreambleLength::default(),
            spreading_factor: SpreadingFactor::default(),
            sync_word: SYNC_WORD_DEFAULT,
            use_crc: false,
        }
    }
}

// -------------------------------------------------------------------------------------------------
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OCP {
    pub on: bool,
    pub imax: u8,
}
impl OCP {
    pub fn new(on: bool, imax: u8) -> Self {
        Self { on, imax }
    }

    pub fn trim(&self) -> u8 {
        calculate::ocp_trim(self.imax)
    }
}
impl Default for OCP {
    fn default() -> Self {
        Self {
            on: true,
            imax: 100,
        }
    }
}

// -------------------------------------------------------------------------------------------------
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum PowerRamp {
    Ms3_4 = 0x0,
    Ms2 = 0x1,
    Ms1 = 0x2,
    Us500 = 0x3,
    Us250 = 0x4,
    Us125 = 0x5,
    Us100 = 0x6,
    Us62 = 0x7,
    Us50 = 0x8,
    #[default]
    Us40 = 0x9,
    Us31 = 0xa,
    Us25 = 0xb,
    Us20 = 0xc,
    Us15 = 0xd,
    Us12 = 0xe,
    Us10 = 0xf,
}
impl From<u8> for PowerRamp {
    fn from(value: u8) -> Self {
        match value {
            0x0 => Ms3_4,
            0x1 => Ms2,
            0x2 => Ms1,
            0x3 => Us500,
            0x4 => Us250,
            0x5 => Us125,
            0x6 => Us100,
            0x7 => Us62,
            0x8 => Us50,
            0xa => Us31,
            0xb => Us25,
            0xc => Us20,
            0xd => Us15,
            0xe => Us12,
            0xf => Us10,
            _ => Us40,
        }
    }
}

#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TxConfig {
    pub(crate) high_power: bool,
    pub(crate) ocp: OCP,
    pub(crate) power: u8,
    pub(crate) ramp: PowerRamp,
    pub(crate) use_rfo: bool,
}
impl TxConfig {
    pub fn new(
        ocp: OCP,
        mut power: u8,
        ramp: PowerRamp,
        use_rfo: bool,
    ) -> Result<Self, Sx127xError<()>> {
        let high_power = !use_rfo && power > 17;
        if use_rfo {
            if !validate::rfo_power(power) {
                return Err(InvalidInput);
            }
        } else {
            if !validate::boost_power(power) {
                return Err(InvalidInput);
            }
            power -= 2;
            if power > 17 {
                power -= 3
            }
        }
        Ok(Self {
            high_power,
            ocp,
            power,
            ramp,
            use_rfo,
        })
    }
}

// -------------------------------------------------------------------------------------------------
/// Frequency Error Indication (FEI)
pub struct FEI {
    pub hz: f64,
    pub ppm: f64,
}

impl FEI {
    pub fn new(bandwidth: Bandwidth, fei: i32, frequency: u32) -> Self {
        Self::new_for_variant::<sx127x_common::Sx1276>(bandwidth, fei, frequency)
    }

    pub fn new_for_variant<V: Sx127xVariant>(
        bandwidth: Bandwidth,
        fei: i32,
        frequency: u32,
    ) -> Self {
        let hz = calculate::fei_hz::<V>(fei, bandwidth.khz());
        Self {
            hz,
            ppm: calculate::fei_ppm(hz, frequency),
        }
    }
}

// -------------------------------------------------------------------------------------------------
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PreambleLength(pub(crate) u16);

impl PreambleLength {
    pub fn new(length: u16) -> Result<Self, Sx127xError<()>> {
        if !validate::preamble_length(length) {
            return Err(InvalidInput);
        }
        Ok(PreambleLength(length))
    }
}

impl Default for PreambleLength {
    fn default() -> Self {
        Self(PREAMBLE_LENGTH_DEFAULT)
    }
}

// -------------------------------------------------------------------------------------------------
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum SpreadingFactor {
    /// Only implicit header mode is possible with Sf6.
    Sf6 = 0x6,
    #[default]
    Sf7 = 0x7,
    Sf8 = 0x8,
    Sf9 = 0x9,
    Sf10 = 0xa,
    Sf11 = 0xb,
    Sf12 = 0xc,
}
impl From<u8> for SpreadingFactor {
    fn from(value: u8) -> Self {
        match value {
            0x6 => SpreadingFactor::Sf6,
            0x8 => SpreadingFactor::Sf8,
            0x9 => SpreadingFactor::Sf9,
            0xa => SpreadingFactor::Sf10,
            0xb => SpreadingFactor::Sf11,
            0xc => SpreadingFactor::Sf12,
            _ => SpreadingFactor::Sf7,
        }
    }
}

// -------------------------------------------------------------------------------------------------
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct TimeoutSymbols(pub(crate) u16);

impl TimeoutSymbols {
    pub fn new(timeout: u16) -> Result<Self, Sx127xError<()>> {
        if !validate::rx_timeout_symbols(timeout) {
            return Err(InvalidInput);
        }
        Ok(TimeoutSymbols(timeout))
    }

    pub fn max() -> Self {
        TimeoutSymbols(RX_TIMEOUT_SYMBOLS_MAX)
    }

    pub fn min() -> Self {
        TimeoutSymbols(RX_TIMEOUT_SYMBOLS_MIN)
    }
}
