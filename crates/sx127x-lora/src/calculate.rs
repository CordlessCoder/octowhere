use sx127x_common::{Hz, Sx127xVariant};

pub(crate) fn data_rate(symbol_rate: f32, spreading_factor: f32, coding_rate: f32) -> u16 {
    (symbol_rate * spreading_factor * coding_rate) as u16
}

pub(crate) fn fei_hz<V: Sx127xVariant>(fei: i32, bandwidth_khz: f32) -> f64 {
    let hz = fei as f64 * 2u32.pow(24) as f64 / 32_000_000.0;
    if V::FEI_BANDWIDTH_SCALING {
        hz * (bandwidth_khz as f64 / 500.0)
    } else {
        hz
    }
}

pub(crate) fn fei_ppm(hz: f64, frf: u32) -> f64 {
    hz * (10u32.pow(6) / frf) as f64
}

pub(crate) fn ocp_trim(imax: u8) -> u8 {
    if imax < 45 {
        0
    } else if imax <= 120 {
        (imax - 45) / 5
    } else if imax <= 240 {
        (((imax as u16) + 30) / 10) as u8
    } else {
        27
    }
}

pub(crate) fn rssi_constant<V: Sx127xVariant>(frequency: Hz) -> i16 {
    V::rssi_constant(frequency)
}

pub(crate) fn rssi_dbm<V: Sx127xVariant>(frequency: Hz, rssi: i16) -> i16 {
    rssi_constant::<V>(frequency) + rssi
}

pub(crate) fn last_packet_rssi_dbm_with_raw_snr<V: Sx127xVariant>(
    frequency: Hz,
    last_packet_rssi: i16,
    last_packet_snr_raw: i8,
    rssi: i16,
) -> i16 {
    if last_packet_snr_raw >= 0 {
        rssi_dbm::<V>(frequency, rssi * 16 / 15)
    } else {
        let snr_db = -((-(last_packet_snr_raw as i16) + 3) / 4);
        rssi_dbm::<V>(frequency, last_packet_rssi) + snr_db
    }
}

/// Calculates the symbol period (Ts) in milliseconds.
///
/// See: datasheet section 4.1.1.7
pub(crate) fn symbol_period(symbol_rate: f32) -> f32 {
    (1f32 / symbol_rate) * 1000f32
}

/// Calculates the symbol rate (Rs)
///
/// See: datasheet section 4.1.1.5
pub(crate) fn symbol_rate(bandwidth: u32, spreading_factor: u32) -> f32 {
    bandwidth as f32 / 2u32.pow(spreading_factor) as f32
}

#[cfg(test)]
mod tests {
    use super::*;
    use sx127x_common::{Sx1272, Sx1276};

    #[test]
    fn data_rate_ok() {
        let res = data_rate(1953f32, 6f32, 0.8f32);
        assert_eq!(res, 9374u16);
    }

    #[test]
    fn fei_new_neg_fei_hz_ok() {
        let res = fei_hz::<Sx1276>(-2i32, 16f32);
        assert!((res - -0.033554432).abs() < 1e-9);
    }

    #[test]
    fn fei_new_pos_fei_hz_ok() {
        let res = fei_hz::<Sx1276>(8i32, 16f32);
        assert!((res - 0.134217728).abs() < 1e-9);
    }

    #[test]
    fn fei_new_neg_fei_ppm_ok() {
        let fei_hz = fei_hz::<Sx1276>(-4i32, 16f32);
        let fei_ppm = fei_ppm(fei_hz, 32u32);
        assert!((fei_ppm - -2097.152).abs() < 1e-3);
    }

    #[test]
    fn fei_new_pos_fei_ppm_ok() {
        let fei_hz = fei_hz::<Sx1276>(8i32, 16f32);
        let fei_ppm = fei_ppm(fei_hz, 32u32);
        assert!((fei_ppm - 4194.304).abs() < 1e-3);
    }

    #[test]
    fn fei_hz_handles_full_signed_register_range() {
        let res = fei_hz::<Sx1276>(0x7ffff, 500.0);
        assert!(res.is_finite());
        assert!(res > 0.0);
    }

    #[test]
    fn sx1272_fei_does_not_scale_with_bandwidth() {
        assert!((fei_hz::<Sx1272>(8, 16.0) - 4.194304).abs() < 1e-9);
        assert!((fei_hz::<Sx1272>(-2, 16.0) + 1.048576).abs() < 1e-9);
    }

    #[test]
    fn ocp_trim_below_datasheet_range_is_safe() {
        assert_eq!(ocp_trim(1), 0);
        assert_eq!(ocp_trim(44), 0);
    }

    #[test]
    fn last_packet_rssi_dbm_snr_negative() {
        assert_eq!(
            last_packet_rssi_dbm_with_raw_snr::<Sx1276>(778_999_999, 46, -8, 42),
            -120
        );
    }

    #[test]
    fn last_packet_rssi_dbm_snr_positive() {
        assert_eq!(
            last_packet_rssi_dbm_with_raw_snr::<Sx1276>(779_000_000, 46, 40, 42),
            -113
        );
    }

    #[test]
    fn last_packet_rssi_dbm_rounds_negative_quarter_db_snr_down() {
        assert_eq!(
            last_packet_rssi_dbm_with_raw_snr::<Sx1272>(868_000_000, 46, -2, 42),
            -94
        );
    }

    #[test]
    fn ocp_trim_imax_0() {
        let res = ocp_trim(0);
        assert_eq!(res, 0);
    }

    #[test]
    fn ocp_trim_imax_45() {
        let res = ocp_trim(45);
        assert_eq!(res, 0);
    }

    #[test]
    fn ocp_trim_imax_50() {
        let res = ocp_trim(50);
        assert_eq!(res, 1);
    }

    #[test]
    fn ocp_trim_imax_120() {
        let res = ocp_trim(120);
        assert_eq!(res, 15);
    }

    #[test]
    fn ocp_trim_imax_125() {
        let res = ocp_trim(125);
        assert_eq!(res, 15);
    }

    #[test]
    fn ocp_trim_imax_130() {
        let res = ocp_trim(130);
        assert_eq!(res, 16);
    }

    #[test]
    fn ocp_trim_imax_230() {
        let res = ocp_trim(230);
        assert_eq!(res, 26);
    }

    #[test]
    fn ocp_trim_imax_240() {
        let res = ocp_trim(240);
        assert_eq!(res, 27);
    }

    #[test]
    fn rssi_constant_hf() {
        assert_eq!(rssi_constant::<Sx1276>(779_000_000), -157);
    }

    #[test]
    fn rssi_constant_lf() {
        assert_eq!(rssi_constant::<Sx1276>(778_999_999), -164);
    }

    #[test]
    fn rssi_dbm_hf() {
        assert_eq!(rssi_dbm::<Sx1276>(779_000_000, 42), -115);
    }

    #[test]
    fn rssi_dbm_lf() {
        assert_eq!(rssi_dbm::<Sx1276>(778_999_999, 42), -122);
    }

    #[test]
    fn sx1272_rssi_dbm() {
        assert_eq!(rssi_dbm::<Sx1272>(868_000_000, 42), -97);
    }

    #[test]
    fn symbol_period_ok() {
        assert!((symbol_period(976.562) - 1.024).abs() < 1e-3);
    }

    #[test]
    fn symbol_rate_ok() {
        let bandwidth = 125_000u32;
        let spreading_factor = 7u32;
        let symbol_rate = symbol_rate(bandwidth, spreading_factor);
        assert!((symbol_rate - 976.562).abs() < 1e-3);
    }
}
