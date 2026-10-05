//! Magnetic declination from the World Magnetic Model 2025, so that a magnetic heading can be
//! turned into a true one. WMM2025 is the US NOAA and UK BGS model, in the public domain
//! (`WMM.COF` of 2024-11-13). It holds from 2025.0 to 2030.0; outside that, and near the
//! poles, where the heading means little, there is no declination.

/// The model's epoch and the years it holds for.
const EPOCH: f32 = 2025.0;
const VALID: core::ops::Range<f32> = 2025.0..2030.0;
/// Its reference radius, and WGS-84's semi-major axis and flattening, in km.
const REFERENCE: f32 = 6371.2;
const WGS84_A: f32 = 6378.137;
const WGS84_F: f32 = 1.0 / 298.257_23;
/// The degree the model runs to.
const DEGREE: usize = 12;
/// How close to a pole, in degrees of latitude, the declination is left undefined.
const POLE: f32 = 89.9;

/// Each Gauss coefficient's degree, order, value and yearly change, in nT and nT a year.
const COEFFICIENTS: [(u8, u8, f32, f32, f32, f32); 90] = [
    (1, 0, -29351.8, 0.0, 12.0, 0.0),
    (1, 1, -1410.8, 4545.4, 9.7, -21.5),
    (2, 0, -2556.6, 0.0, -11.6, 0.0),
    (2, 1, 2951.1, -3133.6, -5.2, -27.7),
    (2, 2, 1649.3, -815.1, -8.0, -12.1),
    (3, 0, 1361.0, 0.0, -1.3, 0.0),
    (3, 1, -2404.1, -56.6, -4.2, 4.0),
    (3, 2, 1243.8, 237.5, 0.4, -0.3),
    (3, 3, 453.6, -549.5, -15.6, -4.1),
    (4, 0, 895.0, 0.0, -1.6, 0.0),
    (4, 1, 799.5, 278.6, -2.4, -1.1),
    (4, 2, 55.7, -133.9, -6.0, 4.1),
    (4, 3, -281.1, 212.0, 5.6, 1.6),
    (4, 4, 12.1, -375.6, -7.0, -4.4),
    (5, 0, -233.2, 0.0, 0.6, 0.0),
    (5, 1, 368.9, 45.4, 1.4, -0.5),
    (5, 2, 187.2, 220.2, 0.0, 2.2),
    (5, 3, -138.7, -122.9, 0.6, 0.4),
    (5, 4, -142.0, 43.0, 2.2, 1.7),
    (5, 5, 20.9, 106.1, 0.9, 1.9),
    (6, 0, 64.4, 0.0, -0.2, 0.0),
    (6, 1, 63.8, -18.4, -0.4, 0.3),
    (6, 2, 76.9, 16.8, 0.9, -1.6),
    (6, 3, -115.7, 48.8, 1.2, -0.4),
    (6, 4, -40.9, -59.8, -0.9, 0.9),
    (6, 5, 14.9, 10.9, 0.3, 0.7),
    (6, 6, -60.7, 72.7, 0.9, 0.9),
    (7, 0, 79.5, 0.0, -0.0, 0.0),
    (7, 1, -77.0, -48.9, -0.1, 0.6),
    (7, 2, -8.8, -14.4, -0.1, 0.5),
    (7, 3, 59.3, -1.0, 0.5, -0.8),
    (7, 4, 15.8, 23.4, -0.1, 0.0),
    (7, 5, 2.5, -7.4, -0.8, -1.0),
    (7, 6, -11.1, -25.1, -0.8, 0.6),
    (7, 7, 14.2, -2.3, 0.8, -0.2),
    (8, 0, 23.2, 0.0, -0.1, 0.0),
    (8, 1, 10.8, 7.1, 0.2, -0.2),
    (8, 2, -17.5, -12.6, 0.0, 0.5),
    (8, 3, 2.0, 11.4, 0.5, -0.4),
    (8, 4, -21.7, -9.7, -0.1, 0.4),
    (8, 5, 16.9, 12.7, 0.3, -0.5),
    (8, 6, 15.0, 0.7, 0.2, -0.6),
    (8, 7, -16.8, -5.2, -0.0, 0.3),
    (8, 8, 0.9, 3.9, 0.2, 0.2),
    (9, 0, 4.6, 0.0, -0.0, 0.0),
    (9, 1, 7.8, -24.8, -0.1, -0.3),
    (9, 2, 3.0, 12.2, 0.1, 0.3),
    (9, 3, -0.2, 8.3, 0.3, -0.3),
    (9, 4, -2.5, -3.3, -0.3, 0.3),
    (9, 5, -13.1, -5.2, 0.0, 0.2),
    (9, 6, 2.4, 7.2, 0.3, -0.1),
    (9, 7, 8.6, -0.6, -0.1, -0.2),
    (9, 8, -8.7, 0.8, 0.1, 0.4),
    (9, 9, -12.9, 10.0, -0.1, 0.1),
    (10, 0, -1.3, 0.0, 0.1, 0.0),
    (10, 1, -6.4, 3.3, 0.0, 0.0),
    (10, 2, 0.2, 0.0, 0.1, -0.0),
    (10, 3, 2.0, 2.4, 0.1, -0.2),
    (10, 4, -1.0, 5.3, -0.0, 0.1),
    (10, 5, -0.6, -9.1, -0.3, -0.1),
    (10, 6, -0.9, 0.4, 0.0, 0.1),
    (10, 7, 1.5, -4.2, -0.1, 0.0),
    (10, 8, 0.9, -3.8, -0.1, -0.1),
    (10, 9, -2.7, 0.9, -0.0, 0.2),
    (10, 10, -3.9, -9.1, -0.0, -0.0),
    (11, 0, 2.9, 0.0, 0.0, 0.0),
    (11, 1, -1.5, 0.0, -0.0, -0.0),
    (11, 2, -2.5, 2.9, 0.0, 0.1),
    (11, 3, 2.4, -0.6, 0.0, -0.0),
    (11, 4, -0.6, 0.2, 0.0, 0.1),
    (11, 5, -0.1, 0.5, -0.1, -0.0),
    (11, 6, -0.6, -0.3, 0.0, -0.0),
    (11, 7, -0.1, -1.2, -0.0, 0.1),
    (11, 8, 1.1, -1.7, -0.1, -0.0),
    (11, 9, -1.0, -2.9, -0.1, 0.0),
    (11, 10, -0.2, -1.8, -0.1, 0.0),
    (11, 11, 2.6, -2.3, -0.1, 0.0),
    (12, 0, -2.0, 0.0, 0.0, 0.0),
    (12, 1, -0.2, -1.3, 0.0, -0.0),
    (12, 2, 0.3, 0.7, -0.0, 0.0),
    (12, 3, 1.2, 1.0, -0.0, -0.1),
    (12, 4, -1.3, -1.4, -0.0, 0.1),
    (12, 5, 0.6, -0.0, -0.0, -0.0),
    (12, 6, 0.6, 0.6, 0.1, -0.0),
    (12, 7, 0.5, -0.1, -0.0, -0.0),
    (12, 8, -0.1, 0.8, 0.0, 0.0),
    (12, 9, -0.4, 0.1, 0.0, -0.0),
    (12, 10, -0.2, -1.0, -0.1, -0.0),
    (12, 11, -1.3, 0.1, -0.0, 0.0),
    (12, 12, -0.7, 0.2, -0.1, -0.1),
];

/// How far east of true north magnetic north lies, in degrees, at `latitude` and `longitude` in
/// degrees, `altitude_km` above the WGS-84 ellipsoid, in decimal `year`.
#[must_use]
pub fn declination(latitude: f32, longitude: f32, altitude_km: f32, year: f32) -> Option<f32> {
    if !VALID.contains(&year) || latitude.abs() > POLE {
        return None;
    }
    let t = year - EPOCH;
    let (phi, lambda) = (latitude.to_radians(), longitude.to_radians());
    // The point in geocentric spherical coordinates.
    let e2 = WGS84_F * (2.0 - WGS84_F);
    let (sin_phi, cos_phi) = libm::sincosf(phi);
    let rc = WGS84_A / libm::sqrtf(1.0 - e2 * sin_phi * sin_phi);
    let p = (rc + altitude_km) * cos_phi;
    let z = (rc * (1.0 - e2) + altitude_km) * sin_phi;
    let r = libm::hypotf(p, z);
    let geocentric = libm::asinf(z / r);
    // Colatitude's cosine and sine.
    let (x, s) = libm::sincosf(geocentric);

    // Schmidt semi-normalised Legendre functions and their derivatives by colatitude.
    let mut pnm = [[0.0f32; DEGREE + 1]; DEGREE + 1];
    let mut dpnm = [[0.0f32; DEGREE + 1]; DEGREE + 1];
    pnm[0][0] = 1.0;
    for n in 1..=DEGREE {
        let nf = n as f32;
        if n == 1 {
            pnm[1][1] = s;
            dpnm[1][1] = x;
        } else {
            let k = libm::sqrtf((2.0 * nf - 1.0) / (2.0 * nf));
            pnm[n][n] = k * s * pnm[n - 1][n - 1];
            dpnm[n][n] = k * (s * dpnm[n - 1][n - 1] + x * pnm[n - 1][n - 1]);
        }
        for m in 0..n {
            let mf = m as f32;
            let back = if n >= 2 {
                libm::sqrtf((nf - 1.0) * (nf - 1.0) - mf * mf)
            } else {
                0.0
            };
            let (p2, dp2) = if n >= 2 {
                (pnm[n - 2][m], dpnm[n - 2][m])
            } else {
                (0.0, 0.0)
            };
            let norm = libm::sqrtf(nf * nf - mf * mf);
            pnm[n][m] = ((2.0 * nf - 1.0) * x * pnm[n - 1][m] - back * p2) / norm;
            dpnm[n][m] =
                ((2.0 * nf - 1.0) * (x * dpnm[n - 1][m] - s * pnm[n - 1][m]) - back * dp2) / norm;
        }
    }

    let ratio = REFERENCE / r;
    let (mut north, mut east, mut down) = (0.0f32, 0.0f32, 0.0f32);
    for &(n, m, g, h, g_dot, h_dot) in &COEFFICIENTS {
        let (n, m) = (usize::from(n), usize::from(m));
        let g = g + t * g_dot;
        let h = h + t * h_dot;
        let scale = libm::powf(ratio, n as f32 + 2.0);
        let (sin_m, cos_m) = libm::sincosf(m as f32 * lambda);
        let along = g * cos_m + h * sin_m;
        north += scale * along * dpnm[n][m];
        east += scale * m as f32 * (g * sin_m - h * cos_m) * pnm[n][m];
        down -= scale * (n as f32 + 1.0) * along * pnm[n][m];
    }
    east /= s;
    // From the geocentric frame to the geodetic one; east is the same in both.
    let psi = geocentric - phi;
    let north = north * libm::cosf(psi) - down * libm::sinf(psi);
    Some(libm::atan2f(east, north).to_degrees())
}

/// The decimal year at UTC second `unix`.
#[must_use]
pub fn decimal_year(unix: i64) -> f32 {
    let days = unix.div_euclid(86_400);
    let seconds = unix.rem_euclid(86_400);
    // Civil date from days since 1970-01-01, after Howard Hinnant's algorithm.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
    let start = {
        let y = year - 1;
        let era = y.div_euclid(400);
        let yoe = y - era * 400;
        era * 146_097 + yoe * 365 + yoe / 4 - yoe / 100 + 306 - 719_468
    };
    let length = if leap { 366.0 } else { 365.0 };
    year as f32 + ((days - start) as f32 + seconds as f32 / 86_400.0) / length
}

#[cfg(test)]
mod tests {
    use super::*;

    /// NOAA's test values for WMM2025: year, altitude in km, latitude, longitude, declination.
    const TESTS: [(f32, f32, f32, f32, f32); 12] = [
        (2025.0, 28.0, 89.0, -121.0, -99.77),
        (2025.0, 48.0, 80.0, -96.0, -29.91),
        (2025.0, 54.0, 82.0, 87.0, 54.89),
        (2025.0, 65.0, 43.0, 93.0, 0.50),
        (2025.0, 51.0, -33.0, 109.0, -5.49),
        (2025.0, 39.0, -59.0, -8.0, -15.75),
        (2025.0, 3.0, -50.0, -103.0, 27.96),
        (2025.0, 94.0, -29.0, -110.0, 15.74),
        (2025.0, 66.0, 14.0, 143.0, -0.19),
        (2025.0, 18.0, 0.0, 21.0, 1.29),
        (2025.5, 6.0, -36.0, -137.0, 20.28),
        (2025.5, 63.0, 26.0, 81.0, 0.51),
    ];

    #[test]
    fn the_model_matches_noaas_test_values() {
        for (year, altitude, latitude, longitude, expected) in TESTS {
            let found = declination(latitude, longitude, altitude, year).expect("in range");
            assert!(
                (found - expected).abs() < 0.03,
                "{latitude} {longitude} at {year}: {found} for {expected}"
            );
        }
    }

    #[test]
    fn dublin_lies_a_little_west() {
        let found = declination(53.35, -6.26, 0.0, 2026.75).unwrap();
        assert!((-2.5..-0.5).contains(&found), "{found}");
    }

    #[test]
    fn there_is_none_outside_the_model_or_at_a_pole() {
        assert_eq!(declination(53.35, -6.26, 0.0, 2024.9), None);
        assert_eq!(declination(53.35, -6.26, 0.0, 2030.0), None);
        assert_eq!(declination(89.95, 0.0, 0.0, 2026.0), None);
    }

    #[test]
    fn a_decimal_year_counts_the_days_into_it() {
        // 2026-01-01T00:00:00Z and 2026-07-02T12:00:00Z.
        assert!((decimal_year(1_767_225_600) - 2026.0).abs() < 1e-3);
        assert!((decimal_year(1_782_993_600) - 2026.5).abs() < 1e-3);
    }
}
