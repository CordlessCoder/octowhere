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
/// Once calibrated, how far it may stray and still join the fit. Tighter than
/// [`DISTURBANCE_FRACTION`], so a field weakened by something nearby is neither flagged nor
/// learned.
pub const FIT_ADMIT_FRACTION: f32 = 0.15;

/// A sample joins the sphere fit only this far, in µT, from the last one that did, so a board held
/// still does not outweigh every other direction.
pub const FIT_SPACING_UT: f32 = 3.0;
/// Samples the sphere fit needs before it replaces the midpoint of each axis's range.
pub const FIT_MIN_SAMPLES: u32 = 12;
/// Once calibrated, each spaced sample weighs the ones before it down by `1 - 1 / FIT_MEMORY`, so
/// the fit follows an offset that drifts.
pub const FIT_MEMORY: f64 = 300.0;
/// The forgetting never takes the fit below this many samples' worth of what the completed
/// calibration knew, held at the current estimate. Without it, a board kept level forgets the
/// offset along the axis it no longer turns through.
pub const FIT_FLOOR: f64 = 20.0;

/// A candidate fit, started by a sample the fit does not admit, replaces the calibration once it
/// covers as much as a calibration does and its samples lie within this RMS distance, in µT, of
/// its sphere.
/// A magnet passing by does not form a sphere; a changed offset of the board's own turns with the
/// board and does.
pub const CANDIDATE_RESIDUAL_UT: f32 = 2.0;
/// And its radius is within this fraction of the calibration's, as Earth's field has not changed.
pub const CANDIDATE_RADIUS_FRACTION: f32 = 0.15;
/// A candidate is dropped after this many spaced samples in a row that the fit admitted, or once it
/// covers enough but is not a sphere.
pub const CANDIDATE_QUIET_SAMPLES: u32 = 40;

/// A hard-iron offset: the field of the board itself, which turns with the board. Earth's field
/// then lies on a sphere around it, and the offset is that sphere's centre, fitted by least
/// squares. Coverage, for calibration progress, is the range each axis has reached.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HardIron {
    min: Vec3,
    max: Vec3,
    last: Option<Vec3>,
    samples: u32,
    /// The first fitted sample. The fit works relative to it, which keeps it well conditioned
    /// however far the offset is from zero; `p` and `c` below are relative to it.
    origin: Vec3,
    /// Normal equations of `|p|² = 2p·c + k` over the fitted samples: the upper triangle of
    /// `AᵀA` row by row, then `Aᵀb`, with rows `a = [2x, 2y, 2z, 1]` and `b = |p|²`. `f64`, as
    /// the fourth-power sums lose too much in `f32` within a minute.
    normal: [f64; 10],
    rhs: [f64; 4],
    /// `Σb²` and the samples' total weight, for the residual.
    b_squared: f64,
    weight: f64,
    /// Set by [`track`](Self::track): the mean `AᵀA` per sample of the completed calibration,
    /// which the forgetting keeps [`FIT_FLOOR`] samples of.
    floor: Option<[f64; 10]>,
    /// The latest well-posed solution `[c, k]`, kept when a later one is not.
    solution: Option<[f64; 4]>,
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
            origin: [0.0; 3],
            normal: [0.0; 10],
            rhs: [0.0; 4],
            b_squared: 0.0,
            weight: 0.0,
            floor: None,
            solution: None,
        }
    }

    /// Adds `field` to the fit, unless it is too close to the last sample that joined. Returns
    /// whether it joined.
    pub fn update(&mut self, field: Vec3) -> bool {
        for ((min, max), value) in self.min.iter_mut().zip(&mut self.max).zip(field) {
            *min = min.min(value);
            *max = max.max(value);
        }
        if let Some(last) = self.last {
            let step: Vec3 = core::array::from_fn(|axis| field[axis] - last[axis]);
            if length(step) < FIT_SPACING_UT {
                return false;
            }
        }
        if self.last.is_none() {
            self.origin = field;
        }
        self.last = Some(field);
        if let (Some(floor), Some(solution)) = (self.floor, self.solution) {
            self.forget(&floor, solution);
        }
        let [x, y, z]: [f64; 3] =
            core::array::from_fn(|axis| f64::from(field[axis] - self.origin[axis]));
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
        self.b_squared += b * b;
        self.weight += 1.0;
        self.samples += 1;
        if self.samples >= FIT_MIN_SAMPLES
            && let Some(solution) = self.solve()
        {
            self.solution = Some(solution);
        }
        true
    }

    /// Weighs the fit down by one sample's forgetting, and adds that share of the floor back as
    /// a prior centred on `solution`.
    fn forget(&mut self, floor: &[f64; 10], solution: [f64; 4]) {
        let keep = 1.0 - 1.0 / FIT_MEMORY;
        let prior = (1.0 - keep) * FIT_FLOOR;
        let floor_matrix = unpack(floor);
        for (normal, floor) in self.normal.iter_mut().zip(floor) {
            *normal = keep * *normal + prior * floor;
        }
        for (rhs, floor_row) in self.rhs.iter_mut().zip(floor_matrix) {
            let pull: f64 = floor_row.iter().zip(solution).map(|(f, s)| f * s).sum();
            *rhs = keep * *rhs + prior * pull;
        }
        // The prior is not a sample, so it stays out of the residual.
        self.b_squared *= keep;
        self.weight *= keep;
    }

    /// Starts forgetting old samples, keeping [`FIT_FLOOR`] samples' worth of what the fit knows
    /// now. Does nothing before there is a fit.
    pub fn track(&mut self) {
        if self.floor.is_none() && self.solution.is_some() && self.weight > 0.0 {
            self.floor = Some(self.normal.map(|value| value / self.weight));
        }
    }

    #[must_use]
    pub fn is_tracking(&self) -> bool {
        self.floor.is_some()
    }

    /// Solves the normal equations by Gaussian elimination with partial pivoting. `None` when
    /// they are singular, as they are while every sample lies in one plane, or the sphere has no
    /// real radius.
    #[expect(
        clippy::needless_range_loop,
        reason = "row and column indices read as the algebra"
    )]
    fn solve(&self) -> Option<[f64; 4]> {
        let mut m = [[0.0f64; 5]; 4];
        for (row, (normal, rhs)) in m.iter_mut().zip(unpack(&self.normal).iter().zip(self.rhs)) {
            row[..4].copy_from_slice(normal);
            row[4] = rhs;
        }
        let scale = m[0][0].max(m[1][1]).max(m[2][2]);
        for column in 0..4 {
            let pivot =
                (column..4).max_by(|&a, &b| m[a][column].abs().total_cmp(&m[b][column].abs()))?;
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
        (radius_squared(solution) > 0.0).then_some(solution)
    }

    fn fit(&self) -> Option<(Vec3, f32)> {
        self.solution.map(|solution| {
            (
                core::array::from_fn(|axis| solution[axis] as f32 + self.origin[axis]),
                libm::sqrt(radius_squared(solution)) as f32,
            )
        })
    }

    /// The fitted centre, or before there is a fit, the midpoint of each axis's range.
    #[must_use]
    pub fn offset(&self) -> Vec3 {
        if let Some((centre, _)) = self.fit() {
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
        if let Some((_, radius)) = self.fit() {
            return radius;
        }
        (0..3)
            .map(|axis| (self.max[axis] - self.min[axis]).max(0.0) / 2.0)
            .sum::<f32>()
            / 3.0
    }

    /// The RMS distance, in µT, of the fitted samples from the sphere. Meaningful only before
    /// [`track`](Self::track), as the floor does not count as samples.
    #[must_use]
    pub fn residual(&self) -> Option<f32> {
        let solution = self.solution?;
        let normal = unpack(&self.normal);
        let fitted: f64 = (0..4)
            .map(|i| {
                solution[i]
                    * (normal[i]
                        .iter()
                        .zip(solution)
                        .map(|(n, s)| n * s)
                        .sum::<f64>()
                        - 2.0 * self.rhs[i])
            })
            .sum();
        // Σ(b - a·x)², where each term is (|p - c|² - r²)², about (2r·distance)².
        let squares = (self.b_squared + fitted).max(0.0) / self.weight;
        Some((libm::sqrt(squares) / (2.0 * libm::sqrt(radius_squared(solution)))) as f32)
    }

    /// Whether `field`, already corrected by [`offset`](Self::offset), is too strong or too weak
    /// to be the field the calibration saw. A magnet or steel nearby does that, and turns the
    /// heading with it.
    #[must_use]
    pub fn disturbed(&self, field: Vec3) -> bool {
        self.strays(field, DISTURBANCE_FRACTION)
    }

    fn strays(&self, field: Vec3, fraction: f32) -> bool {
        let radius = self.radius();
        (length(field) - radius).abs() > fraction * radius
    }

    /// The least-covered axis's range as a fraction of [`CALIBRATION_SPREAD_UT`], up to 1.
    #[must_use]
    pub fn progress(&self) -> f32 {
        (0..3)
            .map(|axis| (self.max[axis] - self.min[axis]).max(0.0) / CALIBRATION_SPREAD_UT)
            .fold(1.0_f32, f32::min)
    }
}

#[expect(
    clippy::needless_range_loop,
    reason = "row and column indices read as the algebra"
)]
fn unpack(packed: &[f64; 10]) -> [[f64; 4]; 4] {
    let mut matrix = [[0.0; 4]; 4];
    let mut index = 0;
    for i in 0..4 {
        for j in i..4 {
            matrix[i][j] = packed[index];
            matrix[j][i] = packed[index];
            index += 1;
        }
    }
    matrix
}

fn radius_squared(solution: [f64; 4]) -> f64 {
    solution[3] + solution[..3].iter().map(|c| c * c).sum::<f64>()
}

/// The hard-iron calibration the compass uses, which follows a changing environment. Samples
/// within [`FIT_ADMIT_FRACTION`] go to a fit that slowly forgets. The rest start a candidate fit,
/// which takes over if it forms a sphere of Earth's field, and is dropped otherwise once the
/// disturbance passes.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Calibration {
    main: HardIron,
    candidate: Option<HardIron>,
    /// Spaced candidate samples since the last one the fit did not admit.
    quiet: u32,
}

/// What [`Calibration::update`] did with a sample, for the log.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CalibrationEvent {
    None,
    Calibrated,
    CandidateStarted,
    CandidateDropped,
    CandidateTaken,
}

impl Calibration {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            main: HardIron::new(),
            candidate: None,
            quiet: 0,
        }
    }

    /// Adds a raw sample, in screen axes.
    pub fn update(&mut self, field: Vec3) -> CalibrationEvent {
        if !self.main.is_tracking() {
            self.main.update(field);
            if self.main.progress() >= 1.0 {
                self.main.track();
            }
            return if self.main.is_tracking() {
                CalibrationEvent::Calibrated
            } else {
                CalibrationEvent::None
            };
        }
        let offset = self.main.offset();
        let disturbed = self.main.strays(
            core::array::from_fn(|axis| field[axis] - offset[axis]),
            FIT_ADMIT_FRACTION,
        );
        if !disturbed {
            self.main.update(field);
        }
        let mut event = CalibrationEvent::None;
        let candidate = match (&mut self.candidate, disturbed) {
            (Some(candidate), _) => candidate,
            (None, true) => {
                self.quiet = 0;
                event = CalibrationEvent::CandidateStarted;
                self.candidate.insert(HardIron::new())
            }
            (None, false) => return event,
        };
        if !candidate.update(field) {
            return event;
        }
        self.quiet = if disturbed { 0 } else { self.quiet + 1 };
        // `None` until the candidate has a fit.
        let spherical = candidate
            .residual()
            .map(|residual| residual <= CANDIDATE_RESIDUAL_UT);
        let formed = candidate.progress() >= 1.0;
        if formed
            && spherical == Some(true)
            && (candidate.radius() - self.main.radius()).abs()
                <= CANDIDATE_RADIUS_FRACTION * self.main.radius()
        {
            let mut taken = *candidate;
            taken.track();
            self.main = taken;
            self.candidate = None;
            CalibrationEvent::CandidateTaken
        } else if self.quiet >= CANDIDATE_QUIET_SAMPLES || (formed && spherical == Some(false)) {
            // Not a sphere, though it covers enough: it holds samples from before a change
            // settled, such as a magnet being brought up. The next sample the fit does not admit
            // starts afresh.
            self.candidate = None;
            CalibrationEvent::CandidateDropped
        } else {
            event
        }
    }

    #[must_use]
    pub fn offset(&self) -> Vec3 {
        self.main.offset()
    }

    #[must_use]
    pub fn radius(&self) -> f32 {
        self.main.radius()
    }

    #[must_use]
    pub fn progress(&self) -> f32 {
        self.main.progress()
    }

    #[must_use]
    pub fn disturbed(&self, field: Vec3) -> bool {
        self.main.disturbed(field)
    }

    #[must_use]
    pub fn candidate(&self) -> Option<&HardIron> {
        self.candidate.as_ref()
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
    pub fn new(attitude: Option<Attitude>, field: Option<Vec3>, calibration: &Calibration) -> Self {
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
        partial_sphere(centre).for_each(|sample| {
            calibration.update(sample);
        });
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
        let mut calibration = Calibration::new();
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

    /// Deterministic noise.
    struct Noise(u32);

    impl Noise {
        /// A value in `-amplitude..amplitude`.
        fn next(&mut self, amplitude: f32) -> f32 {
            self.0 = self.0.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            ((self.0 >> 8) as f32 / (1 << 24) as f32 * 2.0 - 1.0) * amplitude
        }

        fn vector(&mut self, amplitude: f32) -> Vec3 {
            core::array::from_fn(|_| self.next(amplitude))
        }
    }

    const EARTH_UT: f32 = 44.0;
    const SITE_OFFSET: Vec3 = [2.1, 59.5, 4.0];

    fn distance(a: Vec3, b: Vec3) -> f32 {
        length(core::array::from_fn(|axis| a[axis] - b[axis]))
    }

    /// The `index`th of `count` directions spread evenly over a sphere, in a spiral from one pole
    /// to the other.
    fn direction(index: usize, count: usize) -> Vec3 {
        let z = 1.0 - 2.0 * (index as f32 + 0.5) / count as f32;
        let ring = libm::sqrtf(1.0 - z * z);
        let (sin, cos) = libm::sincosf(index as f32 * 2.399_963);
        [ring * cos, ring * sin, z]
    }

    /// A board turned through every orientation, with a hard-iron offset of `centre` and
    /// 0.5 µT of noise.
    fn tumble(centre: Vec3, count: usize, noise: &mut Noise) -> impl Iterator<Item = Vec3> + use<> {
        let noise: Vec<Vec3> = (0..count).map(|_| noise.vector(0.5)).collect();
        (0..count).map(move |index| {
            let direction = direction(index, count);
            core::array::from_fn(|axis| {
                centre[axis] + EARTH_UT * direction[axis] + noise[index][axis]
            })
        })
    }

    fn calibrated(noise: &mut Noise) -> Calibration {
        let mut calibration = Calibration::new();
        let events: Vec<_> = tumble(SITE_OFFSET, 200, noise)
            .map(|sample| calibration.update(sample))
            .collect();
        assert!(events.contains(&CalibrationEvent::Calibrated));
        assert!(distance(calibration.offset(), SITE_OFFSET) < 0.5);
        calibration
    }

    #[test]
    fn a_changed_offset_is_taken_from_the_candidate() {
        let mut noise = Noise(1);
        let mut calibration = calibrated(&mut noise);
        let moved = [
            SITE_OFFSET[0] + 15.0,
            SITE_OFFSET[1] - 10.0,
            SITE_OFFSET[2] + 12.0,
        ];
        let events: Vec<_> = tumble(moved, 300, &mut noise)
            .map(|sample| calibration.update(sample))
            .collect();
        assert!(
            events.contains(&CalibrationEvent::CandidateTaken),
            "{events:?}"
        );
        assert!(
            distance(calibration.offset(), moved) < 1.0,
            "{:?}",
            calibration.offset()
        );
        assert!((calibration.radius() - EARTH_UT).abs() < 1.0);
    }

    #[test]
    fn a_magnet_fixed_to_the_board_is_learned_after_it_settles() {
        let mut noise = Noise(6);
        let mut calibration = calibrated(&mut noise);
        let magnet = [300.0, -150.0, 600.0];
        let moved: Vec3 = core::array::from_fn(|axis| SITE_OFFSET[axis] + magnet[axis]);
        // Brought up to the board over 30 samples, then fixed to it.
        let approach: Vec<Vec3> = (1..=30)
            .map(|step| {
                let share = (step as f32 / 30.0).powi(3);
                core::array::from_fn(|axis| SITE_OFFSET[axis] + share * magnet[axis])
            })
            .collect();
        let samples: Vec<_> = approach
            .into_iter()
            .chain(tumble(moved, 300, &mut noise))
            .collect();
        let events: Vec<_> = samples
            .into_iter()
            .map(|sample| calibration.update(sample))
            .collect();
        assert!(
            events.contains(&CalibrationEvent::CandidateTaken),
            "{events:?}"
        );
        assert!(
            distance(calibration.offset(), moved) < 1.0,
            "{:?}",
            calibration.offset()
        );
    }

    #[test]
    fn a_passing_magnet_is_not_learned() {
        let mut noise = Noise(2);
        let mut calibration = calibrated(&mut noise);
        // A field that does not turn with the board, growing to 80 µT and fading again.
        let passing = tumble(SITE_OFFSET, 120, &mut noise)
            .enumerate()
            .map(|(index, sample)| {
                let strength = 80.0 * libm::sinf(index as f32 / 120.0 * core::f32::consts::PI);
                [sample[0] + strength, sample[1] - 0.5 * strength, sample[2]]
            });
        let samples: Vec<_> = passing
            .chain(tumble(SITE_OFFSET, 200, &mut noise))
            .collect();
        let events: Vec<_> = samples
            .into_iter()
            .map(|sample| calibration.update(sample))
            .collect();
        assert!(events.contains(&CalibrationEvent::CandidateStarted));
        assert!(
            !events.contains(&CalibrationEvent::CandidateTaken),
            "{events:?}"
        );
        assert!(calibration.candidate().is_none());
        assert!(
            distance(calibration.offset(), SITE_OFFSET) < 0.5,
            "{:?}",
            calibration.offset()
        );
    }

    #[test]
    fn a_weakened_field_is_not_taken() {
        let mut noise = Noise(5);
        let mut calibration = calibrated(&mut noise);
        // Steel around the board: a clean sphere, but not of Earth's field.
        let events: Vec<_> = tumble(SITE_OFFSET, 300, &mut noise)
            .map(|sample| {
                calibration.update(core::array::from_fn(|axis| {
                    SITE_OFFSET[axis] + 0.6 * (sample[axis] - SITE_OFFSET[axis])
                }))
            })
            .collect();
        assert!(
            !events.contains(&CalibrationEvent::CandidateTaken),
            "{events:?}"
        );
        assert!((calibration.radius() - EARTH_UT).abs() < 0.5);
    }

    #[test]
    fn the_fit_follows_a_slow_drift() {
        let mut noise = Noise(3);
        let mut calibration = calibrated(&mut noise);
        let drift = [4.0, -3.0, 5.0];
        let at = |step: f32| -> Vec3 {
            core::array::from_fn(|axis| SITE_OFFSET[axis] + step * drift[axis])
        };
        for step in 1..=10 {
            for sample in tumble(at(step as f32 / 10.0), 100, &mut noise) {
                assert_eq!(calibration.update(sample), CalibrationEvent::None);
            }
        }
        for sample in tumble(at(1.0), 500, &mut noise) {
            calibration.update(sample);
        }
        assert!(
            distance(calibration.offset(), at(1.0)) < 0.5,
            "{:?}",
            calibration.offset()
        );
    }

    #[test]
    fn a_board_kept_level_keeps_its_offset() {
        let mut noise = Noise(4);
        let mut calibration = calibrated(&mut noise);
        // The site's field, 8.8 µT horizontal and 42.9 µT down, turned level with a few degrees
        // of wobble.
        for step in 0..5_000 {
            let (sin, cos) = libm::sincosf(step as f32 * 0.5);
            let wobble = noise.vector(0.05);
            let earth = [
                8.8 * cos + 42.9 * wobble[0],
                8.8 * sin + 42.9 * wobble[1],
                -42.9,
            ];
            let jitter = noise.vector(0.5);
            calibration.update(core::array::from_fn(|axis| {
                SITE_OFFSET[axis] + earth[axis] + jitter[axis]
            }));
        }
        assert!(
            distance(calibration.offset(), SITE_OFFSET) < 1.0,
            "{:?}",
            calibration.offset()
        );
        assert!(
            (calibration.radius() - 43.8).abs() < 1.0,
            "{}",
            calibration.radius()
        );
    }

    #[test]
    fn axis_map_permutes_and_flips() {
        let map = AxisMap([(1, 1.0), (0, -1.0), (2, -1.0)]);
        assert_eq!(map.apply([1.0, 2.0, 3.0]), [2.0, -1.0, -3.0]);
    }
}
