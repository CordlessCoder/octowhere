//! Orientation from the gyro, corrected toward gravity and magnetic north: a Mahony
//! complementary filter.
//!
//! Vectors are in the screen's frame, as in [`super::compass`]. The orientation is the rotation
//! from that frame to east, north and up.

use super::compass::{Attitude, Vec3};

/// Standard gravity, which a still accelerometer reads.
const GRAVITY: f32 = 9.81;
/// An accelerometer reading further than this fraction from `GRAVITY` includes the board's own
/// acceleration, and is not used to correct the tilt.
const GRAVITY_TOLERANCE: f32 = 0.15;
/// Proportional gain on the correction: its inverse is roughly the time, in seconds, a tilt or
/// heading error takes to decay.
const PROPORTIONAL_GAIN: f32 = 1.0;
/// Weight of the heading correction against the tilt's. Lower, as the level field is weak and
/// noisier than gravity.
const HEADING_GAIN: f32 = 0.5;
/// Integral gain, which learns what the rest measurement leaves of the gyro's offset.
const INTEGRAL_GAIN: f32 = 0.05;
/// The accelerometer may wander this far on any axis, in m/s², while the board counts as still.
const STILL_ACCEL: f32 = 0.08;
/// The raw field may wander this far on any axis, in µT, while the board counts as still. A steady
/// turn leaves the accelerometer alone but not the field.
const STILL_FIELD: f32 = 1.5;
/// Stillness this long, in seconds, measures the gyro's offset.
const STILL_TIME: f32 = 0.5;

type Quaternion = [f32; 4];

fn multiply(a: Quaternion, b: Quaternion) -> Quaternion {
    [
        a[0] * b[0] - a[1] * b[1] - a[2] * b[2] - a[3] * b[3],
        a[0] * b[1] + a[1] * b[0] + a[2] * b[3] - a[3] * b[2],
        a[0] * b[2] - a[1] * b[3] + a[2] * b[0] + a[3] * b[1],
        a[0] * b[3] + a[1] * b[2] - a[2] * b[1] + a[3] * b[0],
    ]
}

fn conjugate(q: Quaternion) -> Quaternion {
    [q[0], -q[1], -q[2], -q[3]]
}

/// `v` turned by `q`: from the screen frame to the world when `q` is the orientation.
fn rotate(q: Quaternion, v: Vec3) -> Vec3 {
    let turned = multiply(multiply(q, [0.0, v[0], v[1], v[2]]), conjugate(q));
    [turned[1], turned[2], turned[3]]
}

/// `v` turned by the inverse of `q`: from the world to the screen frame.
fn unrotate(q: Quaternion, v: Vec3) -> Vec3 {
    rotate(conjugate(q), v)
}

fn dot(a: Vec3, b: Vec3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: Vec3, b: Vec3) -> Vec3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn length(v: Vec3) -> f32 {
    libm::sqrtf(dot(v, v))
}

fn normalize(v: Vec3) -> Option<Vec3> {
    let length = length(v);
    (length > 1e-6).then(|| v.map(|c| c / length))
}

fn scale(v: Vec3, factor: f32) -> Vec3 {
    v.map(|c| c * factor)
}

fn add(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

/// The orientation whose world east, north and up are these screen-frame vectors, which must
/// be orthonormal and right-handed.
fn from_axes(east: Vec3, north: Vec3, up: Vec3) -> Quaternion {
    // Rows of the screen-to-world matrix are the world axes in the screen frame.
    let m = [east, north, up];
    let trace = m[0][0] + m[1][1] + m[2][2];
    let q = if trace > 0.0 {
        let s = libm::sqrtf(trace + 1.0) * 2.0;
        [
            s / 4.0,
            (m[2][1] - m[1][2]) / s,
            (m[0][2] - m[2][0]) / s,
            (m[1][0] - m[0][1]) / s,
        ]
    } else if m[0][0] > m[1][1] && m[0][0] > m[2][2] {
        let s = libm::sqrtf(1.0 + m[0][0] - m[1][1] - m[2][2]) * 2.0;
        [
            (m[2][1] - m[1][2]) / s,
            s / 4.0,
            (m[0][1] + m[1][0]) / s,
            (m[0][2] + m[2][0]) / s,
        ]
    } else if m[1][1] > m[2][2] {
        let s = libm::sqrtf(1.0 + m[1][1] - m[0][0] - m[2][2]) * 2.0;
        [
            (m[0][2] - m[2][0]) / s,
            (m[0][1] + m[1][0]) / s,
            s / 4.0,
            (m[1][2] + m[2][1]) / s,
        ]
    } else {
        let s = libm::sqrtf(1.0 + m[2][2] - m[0][0] - m[1][1]) * 2.0;
        [
            (m[1][0] - m[0][1]) / s,
            (m[0][2] + m[2][0]) / s,
            (m[1][2] + m[2][1]) / s,
            s / 4.0,
        ]
    };
    normalize_quaternion(q)
}

fn normalize_quaternion(q: Quaternion) -> Quaternion {
    let length = libm::sqrtf(q.iter().map(|c| c * c).sum());
    q.map(|c| c / length)
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Stillness {
    reference: Vec3,
    reference_field: Option<Vec3>,
    gyro_sum: Vec3,
    elapsed: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fusion {
    orientation: Option<Quaternion>,
    /// Subtracted from every gyro reading.
    gyro_offset: Vec3,
    /// What the integral term has added back on top of `gyro_offset`.
    integral: Vec3,
    stillness: Option<Stillness>,
}

impl Default for Fusion {
    fn default() -> Self {
        Self::new()
    }
}

impl Fusion {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            orientation: None,
            gyro_offset: [0.0; 3],
            integral: [0.0; 3],
            stillness: None,
        }
    }

    /// The gyro offset in use, in rad/s: the rest measurement less what the integral learned.
    #[must_use]
    pub fn gyro_offset(&self) -> Vec3 {
        [
            self.gyro_offset[0] - self.integral[0],
            self.gyro_offset[1] - self.integral[1],
            self.gyro_offset[2] - self.integral[2],
        ]
    }

    /// Sets the orientation straight from gravity and, when given, the field. Without the field
    /// the heading starts at the top edge's direction, as though it faced north.
    pub fn reset(&mut self, accel: Vec3, field: Option<Vec3>) {
        let Some(up) = normalize(accel) else {
            return;
        };
        let hint = field
            .and_then(|field| {
                // North is where the field points, level: east is across it.
                normalize(cross(cross(up, field), up))
            })
            .or_else(|| normalize(add([0.0, 1.0, 0.0], scale(up, -up[1]))))
            .or_else(|| normalize(add([1.0, 0.0, 0.0], scale(up, -up[0]))));
        let Some(north) = hint else {
            return;
        };
        let east = cross(north, up);
        self.orientation = Some(from_axes(east, north, up));
    }

    /// Advances by `dt` seconds. `gyro` is in rad/s and `accel` in m/s². `raw_field` is the
    /// magnetometer as read, for telling a still board from a steadily turning one; `field` is the
    /// calibrated reading, or `None` while it cannot be trusted. The first call with an
    /// accelerometer reading only sets the orientation.
    pub fn update(
        &mut self,
        gyro: Vec3,
        accel: Option<Vec3>,
        raw_field: Option<Vec3>,
        field: Option<Vec3>,
        dt: f32,
    ) {
        if let Some(accel) = accel {
            self.measure_offset(gyro, accel, raw_field, dt);
        }
        let Some(mut q) = self.orientation else {
            if let Some(accel) = accel {
                self.reset(accel, field);
            }
            return;
        };

        let up = unrotate(q, [0.0, 0.0, 1.0]);
        let mut error = [0.0; 3];
        if let Some(accel) = accel
            && (length(accel) - GRAVITY).abs() <= GRAVITY_TOLERANCE * GRAVITY
            && let Some(measured) = normalize(accel)
        {
            error = add(error, cross(measured, up));
        }
        // The field's level part, against where north should be. Normalising the level part
        // alone keeps the pull at the sine of the heading error, however steep the dip.
        if let Some(field) = field
            && let Some(level) = normalize(add(field, scale(up, -dot(field, up))))
        {
            let north = unrotate(q, [0.0, 1.0, 0.0]);
            error = add(error, scale(up, HEADING_GAIN * dot(cross(level, north), up)));
        }
        if accel.is_some() {
            self.integral = add(self.integral, scale(error, INTEGRAL_GAIN * dt));
        }

        let rate = add(
            add(
                [
                    gyro[0] - self.gyro_offset[0],
                    gyro[1] - self.gyro_offset[1],
                    gyro[2] - self.gyro_offset[2],
                ],
                self.integral,
            ),
            scale(error, PROPORTIONAL_GAIN),
        );
        let step = multiply(q, [0.0, rate[0], rate[1], rate[2]]);
        for (component, change) in q.iter_mut().zip(step) {
            *component += 0.5 * change * dt;
        }
        self.orientation = Some(normalize_quaternion(q));
    }

    fn measure_offset(&mut self, gyro: Vec3, accel: Vec3, raw_field: Option<Vec3>, dt: f32) {
        let still = self.stillness.is_some_and(|stillness| {
            let steady = |reference: Vec3, now: Vec3, tolerance: f32| {
                (0..3).all(|axis| (now[axis] - reference[axis]).abs() <= tolerance)
            };
            steady(stillness.reference, accel, STILL_ACCEL)
                && match (stillness.reference_field, raw_field) {
                    (Some(reference), Some(now)) => steady(reference, now, STILL_FIELD),
                    _ => true,
                }
        });
        if !still {
            self.stillness = Some(Stillness {
                reference: accel,
                reference_field: raw_field,
                gyro_sum: [0.0; 3],
                elapsed: 0.0,
            });
            return;
        }
        let Some(stillness) = &mut self.stillness else {
            return;
        };
        stillness.gyro_sum = add(stillness.gyro_sum, scale(gyro, dt));
        stillness.elapsed += dt;
        if stillness.elapsed >= STILL_TIME {
            self.gyro_offset = scale(stillness.gyro_sum, 1.0 / stillness.elapsed);
            self.integral = [0.0; 3];
            stillness.gyro_sum = [0.0; 3];
            stillness.elapsed = 0.0;
        }
    }

    /// Heading and tilt as [`super::compass::attitude`] defines them.
    #[must_use]
    pub fn attitude(&self) -> Option<Attitude> {
        let q = self.orientation?;
        let right = rotate(q, [1.0, 0.0, 0.0]);
        let top = rotate(q, [0.0, 1.0, 0.0]);
        let heading_deg = (libm::sqrtf(top[0] * top[0] + top[1] * top[1]) > 0.2).then(|| {
            let degrees = libm::atan2f(top[0], top[1]).to_degrees();
            if degrees < 0.0 { degrees + 360.0 } else { degrees }
        });
        Some(Attitude {
            heading_deg,
            pitch_deg: libm::asinf(top[2].clamp(-1.0, 1.0)).to_degrees(),
            roll_deg: libm::asinf(right[2].clamp(-1.0, 1.0)).to_degrees(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = 0.02;
    const LEVEL: Vec3 = [0.0, 0.0, GRAVITY];

    /// The field with the top edge toward `heading` degrees, level: 18 µT north, 45 µT down.
    fn field_at(heading: f32) -> Vec3 {
        let (sin, cos) = libm::sincosf(heading.to_radians());
        // North, in the screen frame, is `heading` anticlockwise of the top edge.
        [-18.0 * sin, 18.0 * cos, -45.0]
    }

    fn heading(fusion: &Fusion) -> f32 {
        fusion.attitude().unwrap().heading_deg.unwrap()
    }

    fn close(a: f32, b: f32, tolerance: f32) -> bool {
        let difference = (a - b).rem_euclid(360.0);
        difference.min(360.0 - difference) < tolerance
    }

    #[test]
    fn it_starts_from_gravity_and_the_field() {
        for start in [0.0, 90.0, 200.0] {
            let mut fusion = Fusion::new();
            fusion.update([0.0; 3], Some(LEVEL), Some(field_at(start)), Some(field_at(start)), DT);
            assert!(close(heading(&fusion), start, 0.01), "{start}");
        }
    }

    #[test]
    fn the_gyro_turns_the_heading_between_corrections() {
        let mut fusion = Fusion::new();
        fusion.update([0.0; 3], Some(LEVEL), Some(field_at(0.0)), Some(field_at(0.0)), DT);
        // Turning clockwise seen from above is a negative rate about the screen's z, out of the
        // glass, when the screen faces up.
        let rate = -core::f32::consts::FRAC_PI_2;
        for _ in 0..50 {
            fusion.update([0.0, 0.0, rate], None, None, None, DT);
        }
        assert!(close(heading(&fusion), 90.0, 0.5), "{}", heading(&fusion));
    }

    #[test]
    fn a_still_board_measures_the_gyro_offset() {
        let offset = [0.04, 0.32, 0.01];
        let mut fusion = Fusion::new();
        // The first half second drifts on the unmeasured offset; the heading then settles with a
        // time constant of two seconds.
        for _ in 0..400 {
            fusion.update(offset, Some(LEVEL), Some(field_at(30.0)), Some(field_at(30.0)), DT);
        }
        let learned = fusion.gyro_offset();
        for axis in 0..3 {
            assert!((learned[axis] - offset[axis]).abs() < 1e-3, "{learned:?}");
        }
        assert!(close(heading(&fusion), 30.0, 0.5), "{}", heading(&fusion));
        let attitude = fusion.attitude().unwrap();
        assert!(attitude.pitch_deg.abs() < 0.5 && attitude.roll_deg.abs() < 0.5);
    }

    #[test]
    fn a_steady_turn_is_not_taken_for_the_offset() {
        let mut fusion = Fusion::new();
        let rate = -0.3;
        for step in 0..100 {
            let heading = (-rate * step as f32 * DT).to_degrees();
            fusion.update([0.0, 0.0, rate], Some(LEVEL), Some(field_at(heading)), None, DT);
        }
        assert_eq!(fusion.gyro_offset(), [0.0; 3]);
    }

    #[test]
    fn the_field_pulls_a_wrong_heading_back() {
        let mut fusion = Fusion::new();
        fusion.update([0.0; 3], Some(LEVEL), Some(field_at(40.0)), Some(field_at(40.0)), DT);
        for _ in 0..500 {
            fusion.update([0.0; 3], Some(LEVEL), Some(field_at(0.0)), Some(field_at(0.0)), DT);
        }
        assert!(close(heading(&fusion), 0.0, 1.0), "{}", heading(&fusion));
    }

    #[test]
    fn a_shove_does_not_tilt_it() {
        let mut fusion = Fusion::new();
        fusion.update([0.0; 3], Some(LEVEL), Some(field_at(0.0)), Some(field_at(0.0)), DT);
        for _ in 0..25 {
            fusion.update([0.0; 3], Some([6.0, 0.0, GRAVITY]), Some(field_at(0.0)), Some(field_at(0.0)), DT);
        }
        let attitude = fusion.attitude().unwrap();
        assert!(attitude.roll_deg.abs() < 0.5, "{attitude:?}");
    }

    #[test]
    fn the_field_does_not_tilt_it() {
        let mut fusion = Fusion::new();
        fusion.update([0.0; 3], Some(LEVEL), Some(field_at(0.0)), Some(field_at(0.0)), DT);
        for _ in 0..500 {
            // A field that would read as tilted, with no accelerometer to hold the tilt.
            fusion.update([0.0; 3], None, Some([10.0, 18.0, -40.0]), Some([10.0, 18.0, -40.0]), DT);
        }
        let attitude = fusion.attitude().unwrap();
        assert!(attitude.pitch_deg.abs() < 0.01 && attitude.roll_deg.abs() < 0.01, "{attitude:?}");
    }

    #[test]
    fn tilt_follows_gravity() {
        let (sin, cos) = libm::sincosf(30f32.to_radians());
        let mut fusion = Fusion::new();
        fusion.update([0.0; 3], Some([0.0, sin * GRAVITY, cos * GRAVITY]), None, None, DT);
        assert!((fusion.attitude().unwrap().pitch_deg - 30.0).abs() < 0.01);
    }
}
