//! The settle every sliding surface shares: a cubic ease-out over a fixed time, so a page,
//! the panel and the grid all land on a known frame.

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

#[cfg(test)]
mod tests {
    use super::*;

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
