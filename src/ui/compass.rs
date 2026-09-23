//! Heading and tilt from the magnetometer and accelerometer.
//!
//! Vectors are in the screen's frame: x toward the right edge, y toward the top edge, z out of the
//! glass. The accelerometer reads the reaction to gravity, so at rest it points up.

pub type Vec3 = [f32; 3];

/// A signed axis permutation into the screen frame: screen axis `i` is `sign * sensor[axis]`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AxisMap(pub [(usize, f32); 3]);

impl AxisMap {
    #[must_use]
    pub fn apply(self, v: Vec3) -> Vec3 {
        core::array::from_fn(|i| {
            let (axis, sign) = self.0[i];
            sign * v[axis]
        })
    }
}

/// Spread each axis must cover before the hard-iron offset is trusted. Earth's field is about
/// 50 µT, so turning the board through every orientation spreads each axis by about 100 µT.
pub const CALIBRATION_SPREAD_UT: f32 = 40.0;

/// How far the corrected field's strength may stray from the calibration's before the reading
/// counts as disturbed.
pub const DISTURBANCE_FRACTION: f32 = 0.35;

/// A sample joins the sphere fit only this far, in µT, from the last one that did, so a board held
/// still does not outweigh every other direction.
pub const FIT_SPACING_UT: f32 = 3.0;
/// Samples the sphere fit needs before it replaces the midpoint of each axis's range.
pub const FIT_MIN_SAMPLES: u32 = 12;

/// A hard-iron offset: the field of the board itself, which turns with the board. Earth's field
/// then lies on a sphere around it, and the offset is that sphere's centre, fitted by least
/// squares. Coverage, for calibration progress, is the range each axis has reached.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HardIron {
    min: Vec3,
    max: Vec3,
    last: Option<Vec3>,
    samples: u32,
    /// Normal equations of `|p|² = 2p·c + k` over the fitted samples: the upper triangle of
    /// `AᵀA` row by row, then `Aᵀb`, with rows `a = [2x, 2y, 2z, 1]` and `b = |p|²`. `f64`, as
    /// the fourth-power sums lose too much in `f32` within a minute.
    normal: [f64; 10],
    rhs: [f64; 4],
    /// The fitted centre and radius, once there are enough samples and the fit is well posed.
    fit: Option<(Vec3, f32)>,
}

impl Default for HardIron {
    fn default() -> Self {
        Self::new()
    }
}

impl HardIron {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            min: [f32::INFINITY; 3],
            max: [f32::NEG_INFINITY; 3],
            last: None,
            samples: 0,
            normal: [0.0; 10],
            rhs: [0.0; 4],
            fit: None,
        }
    }

    pub fn update(&mut self, field: Vec3) {
        for ((min, max), value) in self.min.iter_mut().zip(&mut self.max).zip(field) {
            *min = min.min(value);
            *max = max.max(value);
        }
        if let Some(last) = self.last {
            let step: Vec3 = core::array::from_fn(|axis| field[axis] - last[axis]);
            if length(step) < FIT_SPACING_UT {
                return;
            }
        }
        self.last = Some(field);
        let [x, y, z] = field.map(f64::from);
        let row = [2.0 * x, 2.0 * y, 2.0 * z, 1.0];
        let b = x * x + y * y + z * z;
        let mut index = 0;
        for i in 0..4 {
            for j in i..4 {
                self.normal[index] += row[i] * row[j];
                index += 1;
            }
            self.rhs[i] += row[i] * b;
        }
        self.samples += 1;
        if self.samples >= FIT_MIN_SAMPLES {
            self.fit = self.solve();
        }
    }

    /// Solves the normal equations by Gaussian elimination with partial pivoting. `None` when
    /// they are singular, as they are while every sample lies in one plane.
    #[expect(clippy::needless_range_loop, reason = "row and column indices read as the algebra")]
    fn solve(&self) -> Option<(Vec3, f32)> {
        let mut m = [[0.0f64; 5]; 4];
        let mut index = 0;
        for i in 0..4 {
            for j in i..4 {
                m[i][j] = self.normal[index];
                m[j][i] = self.normal[index];
                index += 1;
            }
            m[i][4] = self.rhs[i];
        }
        let scale = m[0][0].max(m[1][1]).max(m[2][2]);
        for column in 0..4 {
            let pivot = (column..4)
                .max_by(|&a, &b| m[a][column].abs().total_cmp(&m[b][column].abs()))?;
            if m[pivot][column].abs() <= scale * 1e-9 {
                return None;
            }
            m.swap(column, pivot);
            for row in 0..4 {
                if row != column {
                    let factor = m[row][column] / m[column][column];
                    for k in column..5 {
                        m[row][k] -= factor * m[column][k];
                    }
                }
            }
        }
        let solution: [f64; 4] = core::array::from_fn(|i| m[i][4] / m[i][i]);
        let centre = [solution[0], solution[1], solution[2]];
        let radius_squared = solution[3] + centre.iter().map(|c| c * c).sum::<f64>();
        (radius_squared > 0.0).then(|| (centre.map(|c| c as f32), libm::sqrt(radius_squared) as f32))
    }

    /// The fitted centre, or before there is a fit, the midpoint of each axis's range.
    #[must_use]
    pub fn offset(&self) -> Vec3 {
        if let Some((centre, _)) = self.fit {
            return centre;
        }
        core::array::from_fn(|axis| {
            if self.min[axis] <= self.max[axis] {
                (self.min[axis] + self.max[axis]) / 2.0
            } else {
                0.0
            }
        })
    }

    /// The strength of the field the calibration saw: the fitted radius, or before there is a fit,
    /// the mean of the half-ranges.
    #[must_use]
    pub fn radius(&self) -> f32 {
        if let Some((_, radius)) = self.fit {
            return radius;
        }
        (0..3)
            .map(|axis| (self.max[axis] - self.min[axis]).max(0.0) / 2.0)
            .sum::<f32>()
            / 3.0
    }

    /// Whether `field`, already corrected by [`offset`](Self::offset), is too strong or too weak
    /// to be the field the calibration saw. A magnet or steel nearby does that, and turns the
    /// heading with it.
    #[must_use]
    pub fn disturbed(&self, field: Vec3) -> bool {
        let radius = self.radius();
        (length(field) - radius).abs() > DISTURBANCE_FRACTION * radius
    }

    /// The least-covered axis's range as a fraction of [`CALIBRATION_SPREAD_UT`], up to 1.
    #[must_use]
    pub fn progress(&self) -> f32 {
        (0..3)
            .map(|axis| (self.max[axis] - self.min[axis]).max(0.0) / CALIBRATION_SPREAD_UT)
            .fold(1.0_f32, f32::min)
    }
}

fn length(v: Vec3) -> f32 {
    libm::sqrtf(v[0] * v[0] + v[1] * v[1] + v[2] * v[2])
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Attitude {
    /// Degrees clockwise from magnetic north to the top edge, in `0.0..360.0`. `None` when the
    /// top edge points too nearly straight up or down to have one.
    pub heading_deg: Option<f32>,
    /// How far the top edge is raised above level.
    pub pitch_deg: f32,
    /// How far the right edge is raised above level.
    pub roll_deg: f32,
}

/// Tilt-compensated heading and tilt. `None` when either vector is too short to have a
/// direction, or the field lies along gravity.
#[must_use]
pub fn attitude(accel: Vec3, field: Vec3) -> Option<Attitude> {
    let down = normalize([-accel[0], -accel[1], -accel[2]])?;
    let east = normalize(cross(down, field))?;
    let north = cross(east, down);
    // The top edge's components along east and north; their length is the cosine of the pitch.
    let (east_y, north_y) = (east[1], north[1]);
    let heading_deg = (libm::sqrtf(east_y * east_y + north_y * north_y) > 0.2).then(|| {
        let degrees = libm::atan2f(east_y, north_y).to_degrees();
        if degrees < 0.0 { degrees + 360.0 } else { degrees }
    });
    Some(Attitude {
        heading_deg,
        pitch_deg: libm::asinf((-down[1]).clamp(-1.0, 1.0)).to_degrees(),
        roll_deg: libm::asinf((-down[0]).clamp(-1.0, 1.0)).to_degrees(),
    })
}

fn cross(a: Vec3, b: Vec3) -> Vec3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn normalize(v: Vec3) -> Option<Vec3> {
    let length = length(v);
    (length > 1e-3).then(|| [v[0] / length, v[1] / length, v[2] / length])
}

/// What the compass screen draws, in whole units so the UI state keeps `Eq`.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CompassView {
    /// Both sensors produced a sample.
    pub live: bool,
    /// Calibration progress, 0 to 100. The heading is withheld below 100.
    pub calibration_percent: u8,
    /// Tenths of a degree clockwise from magnetic north.
    pub heading_decidegrees: Option<u16>,
    pub pitch_deg: i8,
    pub roll_deg: i8,
    /// The field's strength does not match the calibration's, so the heading is unreliable.
    pub disturbed: bool,
}

impl CompassView {
    #[must_use]
    pub fn new(attitude: Option<Attitude>, field: Option<Vec3>, calibration: &HardIron) -> Self {
        let calibration_percent = (calibration.progress() * 100.0) as u8;
        let Some(attitude) = attitude else {
            return Self {
                calibration_percent,
                ..Self::default()
            };
        };
        Self {
            live: true,
            calibration_percent,
            heading_decidegrees: attitude
                .heading_deg
                .filter(|_| calibration_percent >= 100)
                .map(|degrees| (libm::roundf(degrees * 10.0) as u16) % 3600),
            pitch_deg: libm::roundf(attitude.pitch_deg) as i8,
            roll_deg: libm::roundf(attitude.roll_deg) as i8,
            disturbed: calibration_percent >= 100
                && field.is_some_and(|field| calibration.disturbed(field)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const G: f32 = 9.81;
    // A northern-hemisphere field: 18 µT north, 45 µT down.
    const LEVEL_NORTH: Vec3 = [0.0, 18.0, -45.0];

    fn heading(accel: Vec3, field: Vec3) -> f32 {
        attitude(accel, field).unwrap().heading_deg.unwrap()
    }

    fn close(a: f32, b: f32) -> bool {
        let difference = (a - b).rem_euclid(360.0);
        difference.min(360.0 - difference) < 0.01
    }

    #[test]
    fn level_heading_follows_the_top_edge() {
        let up = [0.0, 0.0, G];
        assert!(close(heading(up, LEVEL_NORTH), 0.0));
        // Top edge east: north lies toward the left edge.
        assert!(close(heading(up, [-18.0, 0.0, -45.0]), 90.0));
        assert!(close(heading(up, [0.0, -18.0, -45.0]), 180.0));
        assert!(close(heading(up, [18.0, 0.0, -45.0]), 270.0));
    }

    #[test]
    fn tilt_does_not_move_the_heading() {
        // Facing north, top edge raised 30°: turn gravity and the field together about x.
        let (sin, cos) = libm::sincosf(30f32.to_radians());
        let about_x = |v: Vec3| [v[0], cos * v[1] + sin * v[2], -sin * v[1] + cos * v[2]];
        let tilted = attitude(about_x([0.0, 0.0, G]), about_x(LEVEL_NORTH)).unwrap();
        assert!(close(tilted.heading_deg.unwrap(), 0.0));
        assert!((tilted.pitch_deg - 30.0).abs() < 0.01);
        assert!(tilted.roll_deg.abs() < 0.01);
    }

    #[test]
    fn roll_is_the_right_edge_raised() {
        let (sin, cos) = libm::sincosf(20f32.to_radians());
        let tilted = attitude([sin * G, 0.0, cos * G], LEVEL_NORTH).unwrap();
        assert!((tilted.roll_deg - 20.0).abs() < 0.01);
    }

    #[test]
    fn upright_top_edge_has_no_heading() {
        let standing = attitude([0.0, G, 0.0], LEVEL_NORTH).unwrap();
        assert_eq!(standing.heading_deg, None);
        assert!((standing.pitch_deg - 90.0).abs() < 0.01);
    }

    #[test]
    fn hard_iron_offset_is_the_midpoint_until_there_is_a_fit() {
        let mut calibration = HardIron::new();
        assert_eq!(calibration.progress(), 0.0);
        calibration.update([10.0, -20.0, 5.0]);
        calibration.update([90.0, 40.0, 65.0]);
        assert_eq!(calibration.offset(), [50.0, 10.0, 35.0]);
        assert_eq!(calibration.progress(), 1.0);
    }

    /// Points on a sphere of radius 44 about `centre`, over part of it only: elevations from
    /// -20° to 70°, as a board turned mostly face up gives.
    fn partial_sphere(centre: Vec3) -> impl Iterator<Item = Vec3> {
        (0..10).flat_map(move |ring| {
            let elevation = (-20.0 + ring as f32 * 10.0).to_radians();
            (0..24).map(move |step| {
                let azimuth = (step as f32 * 15.0).to_radians();
                let (sin_e, cos_e) = libm::sincosf(elevation);
                let (sin_a, cos_a) = libm::sincosf(azimuth);
                [
                    centre[0] + 44.0 * cos_e * cos_a,
                    centre[1] + 44.0 * cos_e * sin_a,
                    centre[2] + 44.0 * sin_e,
                ]
            })
        })
    }

    #[test]
    fn the_sphere_fit_finds_the_centre_from_part_of_the_sphere() {
        let centre = [-2.0, 59.5, 4.0];
        let mut calibration = HardIron::new();
        partial_sphere(centre).for_each(|sample| calibration.update(sample));
        let offset = calibration.offset();
        for axis in 0..3 {
            assert!((offset[axis] - centre[axis]).abs() < 0.05, "{offset:?}");
        }
        assert!((calibration.radius() - 44.0).abs() < 0.05);
        // The midpoint of each range would put z well off: its lowest point is only -20°.
        let midpoint_z = centre[2] + 44.0 * (libm::sinf(70f32.to_radians()) - libm::sinf(20f32.to_radians())) / 2.0;
        assert!((midpoint_z - centre[2]).abs() > 10.0);
    }

    #[test]
    fn a_board_held_still_adds_one_sample() {
        let mut calibration = HardIron::new();
        for _ in 0..100 {
            calibration.update([1.0, 2.0, 3.0]);
        }
        assert_eq!(calibration.samples, 1);
    }

    #[test]
    fn heading_is_withheld_until_calibrated() {
        let level = attitude([0.0, 0.0, G], LEVEL_NORTH);
        let mut calibration = HardIron::new();
        calibration.update([0.0; 3]);
        calibration.update([20.0; 3]);
        let partial = CompassView::new(level, None, &calibration);
        assert_eq!(partial.calibration_percent, 50);
        assert_eq!(partial.heading_decidegrees, None);
        calibration.update([40.0; 3]);
        assert_eq!(
            CompassView::new(level, None, &calibration).heading_decidegrees,
            Some(0)
        );
    }

    #[test]
    fn a_field_far_from_the_calibrated_strength_is_disturbed() {
        let mut calibration = HardIron::new();
        calibration.update([-50.0; 3]);
        calibration.update([50.0; 3]);
        assert!(!calibration.disturbed([0.0, 30.0, -40.0]));
        assert!(calibration.disturbed([0.0, 120.0, -40.0]));
        assert!(calibration.disturbed([0.0, 10.0, 0.0]));
    }

    #[test]
    fn axis_map_permutes_and_flips() {
        let map = AxisMap([(1, 1.0), (0, -1.0), (2, -1.0)]);
        assert_eq!(map.apply([1.0, 2.0, 3.0]), [2.0, -1.0, -3.0]);
    }
}
