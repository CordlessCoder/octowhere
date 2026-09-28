const MICRO_PER_G: i64 = 9_806_650;
const MICRO_RAD_PER_DEGREE: i64 = 17_453;

#[must_use]
pub const fn accel_micro_ms2(raw: i16, lsb_per_g: i32) -> i32 {
    round_div(raw as i64 * MICRO_PER_G, lsb_per_g as i64) as i32
}

#[must_use]
pub const fn gyro_micro_rad_s(raw: i16, lsb_per_dps: i32) -> i32 {
    round_div(raw as i64 * MICRO_RAD_PER_DEGREE, lsb_per_dps as i64) as i32
}

const fn round_div(value: i64, divisor: i64) -> i64 {
    if value >= 0 {
        (value + divisor / 2) / divisor
    } else {
        -((-value + divisor / 2) / divisor)
    }
}

#[cfg(test)]
mod tests {
    use super::{accel_micro_ms2, gyro_micro_rad_s};

    #[test]
    fn acceleration_uses_si_units_and_preserves_sign() {
        assert_eq!(accel_micro_ms2(16_384, 16_384), 9_806_650);
        assert_eq!(accel_micro_ms2(-16_384, 16_384), -9_806_650);
    }

    #[test]
    fn gyro_converts_degrees_per_second_to_radians_per_second() {
        assert_eq!(gyro_micro_rad_s(64, 64), 17_453);
        assert_eq!(gyro_micro_rad_s(-64, 64), -17_453);
    }
}
