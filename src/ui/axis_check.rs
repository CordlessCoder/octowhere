//! A guided capture of the sensors in known orientations, for working out how each sensor's axes
//! sit against the screen's.

use super::gesture::Micros;

/// How the board is held: what points up, then which way the reference edge or the glass faces.
pub const POSES: [(&str, &str); 12] = [
    ("SCREEN UP", "TOP EDGE NORTH"),
    ("SCREEN UP", "TOP EDGE EAST"),
    ("SCREEN UP", "TOP EDGE SOUTH"),
    ("SCREEN UP", "TOP EDGE WEST"),
    ("SCREEN DOWN", "TOP EDGE NORTH"),
    ("SCREEN DOWN", "TOP EDGE EAST"),
    ("TOP EDGE UP", "GLASS FACES NORTH"),
    ("TOP EDGE UP", "GLASS FACES EAST"),
    ("BOTTOM EDGE UP", "GLASS FACES NORTH"),
    ("RIGHT EDGE UP", "GLASS FACES NORTH"),
    ("LEFT EDGE UP", "GLASS FACES NORTH"),
    ("RIGHT EDGE UP", "GLASS FACES EAST"),
];

/// How long a capture averages over.
pub const CAPTURE_TIME: Micros = 1_500_000;
/// Largest change on any accelerometer axis, in m/s², that still counts as holding still.
pub const STILL_ACCEL: f32 = 0.6;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Status {
    #[default]
    Ready,
    /// Capturing; this many tenths of a second remain.
    Holding(u8),
    Logged(u16),
    /// The board moved during the capture, which was discarded.
    Moved,
}

/// What the screen draws.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct AxisCheckView {
    pub pose: u8,
    /// Bit `n` is set once pose `n` has been logged.
    pub logged: u16,
    pub status: Status,
}

/// One pose's averaged raw readings, in each sensor's own axes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Record {
    pub pose: usize,
    pub magnetic_microtesla: [f32; 3],
    pub accel_ms2: [f32; 3],
    pub samples: u16,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Capture {
    start: Micros,
    magnetic: [f32; 3],
    accel: [f32; 3],
    accel_min: [f32; 3],
    accel_max: [f32; 3],
    samples: u16,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AxisCheck {
    pose: usize,
    logged: u16,
    status: Status,
    capture: Option<Capture>,
}

impl AxisCheck {
    #[must_use]
    pub fn view(&self) -> AxisCheckView {
        AxisCheckView {
            pose: self.pose as u8,
            logged: self.logged,
            status: self.status,
        }
    }

    /// Moves to the next or previous pose, dropping any capture in progress.
    pub fn step(&mut self, forward: bool) {
        let count = POSES.len();
        self.pose = if forward {
            (self.pose + 1) % count
        } else {
            (self.pose + count - 1) % count
        };
        self.capture = None;
        self.status = Status::Ready;
    }

    pub fn start(&mut self, now: Micros) {
        self.capture = Some(Capture {
            start: now,
            magnetic: [0.0; 3],
            accel: [0.0; 3],
            accel_min: [f32::INFINITY; 3],
            accel_max: [f32::NEG_INFINITY; 3],
            samples: 0,
        });
        self.status = Status::Holding((CAPTURE_TIME / 100_000) as u8);
    }

    /// Adds one sample to a capture in progress, and returns the record when it completes.
    pub fn sample(
        &mut self,
        magnetic_microtesla: [f32; 3],
        accel_ms2: [f32; 3],
        now: Micros,
    ) -> Option<Record> {
        let capture = self.capture.as_mut()?;
        for axis in 0..3 {
            capture.magnetic[axis] += magnetic_microtesla[axis];
            capture.accel[axis] += accel_ms2[axis];
            capture.accel_min[axis] = capture.accel_min[axis].min(accel_ms2[axis]);
            capture.accel_max[axis] = capture.accel_max[axis].max(accel_ms2[axis]);
        }
        capture.samples += 1;
        if (0..3).any(|axis| capture.accel_max[axis] - capture.accel_min[axis] > STILL_ACCEL) {
            self.capture = None;
            self.status = Status::Moved;
            return None;
        }
        let elapsed = now.saturating_sub(capture.start);
        if elapsed < CAPTURE_TIME {
            self.status = Status::Holding((CAPTURE_TIME.saturating_sub(elapsed) / 100_000) as u8);
            return None;
        }
        let capture = self.capture.take()?;
        let samples = capture.samples;
        let mean = |sum: [f32; 3]| sum.map(|value| value / f32::from(samples));
        self.logged |= 1 << self.pose;
        self.status = Status::Logged(samples);
        Some(Record {
            pose: self.pose,
            magnetic_microtesla: mean(capture.magnetic),
            accel_ms2: mean(capture.accel),
            samples,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LEVEL: [f32; 3] = [0.0, 0.0, -9.8];

    #[test]
    fn a_still_capture_logs_the_mean() {
        let mut check = AxisCheck::default();
        check.start(0);
        assert_eq!(check.view().status, Status::Holding(15));
        assert_eq!(check.sample([10.0, 0.0, 0.0], LEVEL, 500_000), None);
        assert_eq!(check.view().status, Status::Holding(10));
        let record = check.sample([20.0, 0.0, 0.0], LEVEL, 1_500_000).unwrap();
        assert_eq!(record.pose, 0);
        assert_eq!(record.magnetic_microtesla, [15.0, 0.0, 0.0]);
        assert_eq!(record.samples, 2);
        assert_eq!(check.view().logged, 1);
        assert_eq!(check.view().status, Status::Logged(2));
    }

    #[test]
    fn moving_discards_the_capture() {
        let mut check = AxisCheck::default();
        check.start(0);
        check.sample([0.0; 3], LEVEL, 100_000);
        assert_eq!(check.sample([0.0; 3], [2.0, 0.0, -9.6], 200_000), None);
        assert_eq!(check.view().status, Status::Moved);
        assert_eq!(check.sample([0.0; 3], LEVEL, 2_000_000), None);
        assert_eq!(check.view().logged, 0);
    }

    #[test]
    fn stepping_wraps_and_drops_a_capture() {
        let mut check = AxisCheck::default();
        check.start(0);
        check.step(false);
        assert_eq!(check.view().pose, 11);
        assert_eq!(check.view().status, Status::Ready);
        assert_eq!(check.sample([0.0; 3], LEVEL, 2_000_000), None);
        check.step(true);
        assert_eq!(check.view().pose, 0);
    }
}
