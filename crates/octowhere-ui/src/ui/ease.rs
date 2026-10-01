//! The settle every sliding surface shares: a cubic ease-out over a fixed time, so a page,
//! the panel and the grid all land on a known frame. Also the flight: the curve the Marathon
//! logo animation's end bars travel on, measured a frame at a time at 30 fps, which the charging
//! gauge's ends and its solid wipe follow.

use super::gesture::Micros;

/// How long a settle takes, whatever the distance.
pub const SETTLE: Micros = 160_000;

/// Easing from one position to another, since a time.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ease {
    from: f32,
    to: i32,
    start: Micros,
}

impl Ease {
    #[must_use]
    pub const fn new(from: f32, to: i32, start: Micros) -> Self {
        Self { from, to, start }
    }

    #[must_use]
    pub const fn to(&self) -> i32 {
        self.to
    }

    /// The position at `now`, and whether it has arrived.
    #[must_use]
    pub fn at(&self, now: Micros) -> (f32, bool) {
        let t = (now.saturating_sub(self.start) as f32 / SETTLE as f32).min(1.0);
        let eased = 1.0 - (1.0 - t) * (1.0 - t) * (1.0 - t);
        (self.from + (self.to as f32 - self.from) * eased, t >= 1.0)
    }
}

/// How far along its way a flight is, a frame at a time: a slow start, one long jump, and a
/// long tail.
pub const FLIGHT: [f32; 14] = [
    0.0, 0.025, 0.066, 0.139, 0.279, 0.672, 0.82, 0.885, 0.926, 0.951, 0.975, 0.984, 0.992, 1.0,
];

/// `FLIGHT` at `u`, from 0 to 1 over its frames, straight between them.
#[must_use]
pub fn flight(u: f32) -> f32 {
    let at = u.clamp(0.0, 1.0) * (FLIGHT.len() - 1) as f32;
    let i = (libm::floorf(at) as usize).min(FLIGHT.len() - 2);
    FLIGHT[i] + (FLIGHT[i + 1] - FLIGHT[i]) * (at - i as f32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_flight_runs_from_0_to_1_without_turning_back() {
        assert_eq!((flight(0.0), flight(1.0)), (0.0, 1.0));
        assert!(FLIGHT.windows(2).all(|pair| pair[1] > pair[0]));
        for (i, &at) in FLIGHT.iter().enumerate() {
            let u = i as f32 / (FLIGHT.len() - 1) as f32;
            assert!((flight(u) - at).abs() < 1e-6);
        }
    }

    #[test]
    fn it_arrives_after_the_settle_whatever_the_distance() {
        for from in [-466.0, -10.0, 300.0] {
            let ease = Ease::new(from, 0, 1_000);
            assert_eq!(ease.at(1_000), (from, false));
            assert!(!ease.at(1_000 + SETTLE - 1).1);
            assert_eq!(ease.at(1_000 + SETTLE), (0.0, true));
        }
    }

    #[test]
    fn it_covers_most_of_the_way_early() {
        let (halfway, _) = Ease::new(0.0, 100, 0).at(SETTLE / 2);
        assert!((halfway - 87.5).abs() < 0.01);
    }
}
