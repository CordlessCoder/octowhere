#![no_std]

pub mod bits;
pub mod calculate;
pub mod error;
pub mod registers;
pub mod spi;

pub type Hz = u32;

pub const CHIP_VERSION: u8 = Sx1276::CHIP_VERSION;
pub const DEFAULT_FREQUENCY_HZ: u32 = Sx1276::DEFAULT_FREQUENCY_HZ;
pub const FSTEP: f32 = (FXOSC_HZ as f32) / (2u32.pow(19) as f32);
pub const FXOSC_HZ: u32 = 32_000_000;
pub const LF_MAX_HZ: u32 = 525_000_000;
pub const HF_MIN_HZ: u32 = 779_000_000;

#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Modem {
    Fsk = 0x0,
    LoRa = 0x1,
}

pub trait Sx127xVariant {
    const CHIP_VERSION: u8;
    const DEFAULT_FREQUENCY_HZ: u32;
    const PA_DAC: u8;
    const LOW_DATA_RATE_OPTIMIZE_REGISTER: u8;
    const LOW_DATA_RATE_OPTIMIZE_MASK: u8;
    const LOW_DATA_RATE_OPTIMIZE_OFFSET: u8;
    const AGC_AUTO_REGISTER: u8;
    const AGC_AUTO_MASK: u8;
    const AGC_AUTO_OFFSET: u8;
    const BANDWIDTH_MASK: u8;
    const BANDWIDTH_OFFSET: u8;
    const CODING_RATE_MASK: u8;
    const CODING_RATE_OFFSET: u8;
    const HEADER_MODE_MASK: u8;
    const HEADER_MODE_OFFSET: u8;
    const CRC_REGISTER: u8;
    const CRC_MASK: u8;
    const CRC_OFFSET: u8;
    const HIGH_BANDWIDTH_OPTIMIZATION: bool;
    const CLEAR_DETECT_OPTIMIZE_AUTOMATIC_IF: bool;
    const IF_FREQUENCY_OPTIMIZATION: bool;
    const INVERT_IQ_2_SUPPORTED: bool;

    fn low_frequency_mode(frequency: u32) -> bool;
    fn rssi_constant(frequency: u32) -> i16;
    fn pa_dac(high_power: bool) -> u8;
    fn bandwidth_register_value(value: u8) -> Option<u8>;
    fn bandwidth_from_register(value: u8) -> u8;
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Sx1272;

impl Sx127xVariant for Sx1272 {
    const CHIP_VERSION: u8 = 0x22;
    const DEFAULT_FREQUENCY_HZ: u32 = 868_000_000;
    const PA_DAC: u8 = 0x5a;
    const LOW_DATA_RATE_OPTIMIZE_REGISTER: u8 = 0x1d;
    const LOW_DATA_RATE_OPTIMIZE_MASK: u8 = 0x01;
    const LOW_DATA_RATE_OPTIMIZE_OFFSET: u8 = 0;
    const AGC_AUTO_REGISTER: u8 = 0x1e;
    const AGC_AUTO_MASK: u8 = 0x04;
    const AGC_AUTO_OFFSET: u8 = 2;
    const BANDWIDTH_MASK: u8 = 0xc0;
    const BANDWIDTH_OFFSET: u8 = 6;
    const CODING_RATE_MASK: u8 = 0x38;
    const CODING_RATE_OFFSET: u8 = 3;
    const HEADER_MODE_MASK: u8 = 0x04;
    const HEADER_MODE_OFFSET: u8 = 2;
    const CRC_REGISTER: u8 = 0x1d;
    const CRC_MASK: u8 = 0x02;
    const CRC_OFFSET: u8 = 1;
    const HIGH_BANDWIDTH_OPTIMIZATION: bool = false;
    const CLEAR_DETECT_OPTIMIZE_AUTOMATIC_IF: bool = true;
    const IF_FREQUENCY_OPTIMIZATION: bool = false;
    const INVERT_IQ_2_SUPPORTED: bool = false;

    fn low_frequency_mode(_frequency: u32) -> bool {
        false
    }
    fn rssi_constant(_frequency: u32) -> i16 {
        -139
    }
    fn pa_dac(high_power: bool) -> u8 {
        if high_power { 0x87 } else { 0x84 }
    }
    fn bandwidth_register_value(value: u8) -> Option<u8> {
        match value {
            0x7 => Some(0x0),
            0x8 => Some(0x1),
            0x9 => Some(0x2),
            _ => None,
        }
    }
    fn bandwidth_from_register(value: u8) -> u8 {
        match value {
            0x0 => 0x7,
            0x1 => 0x8,
            0x2 => 0x9,
            _ => 0x7,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Sx1276;

impl Sx127xVariant for Sx1276 {
    const CHIP_VERSION: u8 = 0x12;
    const DEFAULT_FREQUENCY_HZ: u32 = 434_000_000;
    const PA_DAC: u8 = 0x4d;
    const LOW_DATA_RATE_OPTIMIZE_REGISTER: u8 = 0x26;
    const LOW_DATA_RATE_OPTIMIZE_MASK: u8 = 0x08;
    const LOW_DATA_RATE_OPTIMIZE_OFFSET: u8 = 3;
    const AGC_AUTO_REGISTER: u8 = 0x26;
    const AGC_AUTO_MASK: u8 = 0x04;
    const AGC_AUTO_OFFSET: u8 = 2;
    const BANDWIDTH_MASK: u8 = 0xf0;
    const BANDWIDTH_OFFSET: u8 = 4;
    const CODING_RATE_MASK: u8 = 0x0e;
    const CODING_RATE_OFFSET: u8 = 1;
    const HEADER_MODE_MASK: u8 = 0x01;
    const HEADER_MODE_OFFSET: u8 = 0;
    const CRC_REGISTER: u8 = 0x1e;
    const CRC_MASK: u8 = 0x04;
    const CRC_OFFSET: u8 = 2;
    const HIGH_BANDWIDTH_OPTIMIZATION: bool = true;
    const CLEAR_DETECT_OPTIMIZE_AUTOMATIC_IF: bool = false;
    const IF_FREQUENCY_OPTIMIZATION: bool = true;
    const INVERT_IQ_2_SUPPORTED: bool = true;

    fn low_frequency_mode(frequency: u32) -> bool {
        frequency < HF_MIN_HZ
    }
    fn rssi_constant(frequency: u32) -> i16 {
        if frequency < HF_MIN_HZ { -164 } else { -157 }
    }
    fn pa_dac(high_power: bool) -> u8 {
        if high_power { 0x07 } else { 0x04 }
    }
    fn bandwidth_register_value(value: u8) -> Option<u8> {
        (value <= 0x9).then_some(value)
    }
    fn bandwidth_from_register(value: u8) -> u8 {
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sx1272_variant_registers() {
        assert_eq!(Sx1272::CHIP_VERSION, 0x22);
        assert_eq!(Sx1272::PA_DAC, 0x5a);
        assert_eq!(Sx1272::LOW_DATA_RATE_OPTIMIZE_REGISTER, 0x1d);
        assert_eq!(Sx1272::AGC_AUTO_REGISTER, 0x1e);
        assert_eq!(Sx1272::bandwidth_register_value(0x7), Some(0));
        assert_eq!(Sx1272::bandwidth_register_value(0x6), None);
    }

    #[test]
    fn sx1276_variant_registers() {
        assert_eq!(Sx1276::CHIP_VERSION, 0x12);
        assert_eq!(Sx1276::PA_DAC, 0x4d);
        assert!(Sx1276::low_frequency_mode(434_000_000));
        assert!(!Sx1276::low_frequency_mode(868_000_000));
    }
}
