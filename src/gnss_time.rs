//! UTC on the local timer, from when the GNSS receiver's sentences arrive.
//!
//! Without a PPS line, the firmware learns that a second began only when the burst of NMEA for
//! that second's fix is seen. Everything between the second and the sighting makes the sighting
//! late, never early, so the earliest of recent sightings sits closest to the second itself. The
//! receiver's own latency is left in, and is the same on every board.

/// How many recent fixes the earliest sighting is taken over. Long enough for the poll grid's
/// dither to cycle twice; short enough that the local timer's drift across it stays well under a
/// millisecond.
pub const WINDOW: usize = 16;

/// A sighting this far from the estimate belongs to a different second, or the receiver's time
/// jumped, and starts the window again.
const JUMP_MICROS: i64 = 500_000;

/// The local timer minus UTC, in microseconds, over the last [`WINDOW`] fixes.
#[derive(Clone, Debug, Default)]
pub struct SecondEstimator {
    offsets: [i64; WINDOW],
    len: usize,
    next: usize,
}

impl SecondEstimator {
    pub const fn new() -> Self {
        Self {
            offsets: [0; WINDOW],
            len: 0,
            next: 0,
        }
    }

    /// Records that the burst for the fix at `fix_utc_micros` was first seen at `seen_micros` on
    /// the local timer. Returns the estimate after it.
    pub fn add(&mut self, seen_micros: u64, fix_utc_micros: i64) -> i64 {
        let offset = seen_micros as i64 - fix_utc_micros;
        if self
            .offset()
            .is_some_and(|estimate| (offset - estimate).abs() > JUMP_MICROS)
        {
            self.len = 0;
            self.next = 0;
        }
        self.offsets[self.next] = offset;
        self.next = (self.next + 1) % WINDOW;
        self.len = (self.len + 1).min(WINDOW);
        self.offset().unwrap_or(offset)
    }

    /// The local timer minus UTC: the earliest recent sighting's.
    pub fn offset(&self) -> Option<i64> {
        self.offsets[..self.len].iter().copied().min()
    }

    /// How many fixes the estimate is taken over.
    pub fn samples(&self) -> usize {
        self.len
    }

    /// UTC in microseconds at `now_micros` on the local timer.
    pub fn utc_micros(&self, now_micros: u64) -> Option<i64> {
        self.offset().map(|offset| now_micros as i64 - offset)
    }

    /// When the next burst is due on the local timer, the first after `after_micros`, given that
    /// the receiver fixes every `period_micros`.
    pub fn next_burst(&self, after_micros: u64, period_micros: u64) -> Option<u64> {
        let offset = self.offset()?;
        let period = period_micros as i64;
        let since = after_micros as i64 - offset;
        let next = (since.div_euclid(period) + 1) * period + offset;
        u64::try_from(next).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECOND: i64 = 1_000_000;
    /// A fix time in 2026 and a boot a little before it.
    const EPOCH: i64 = 1_790_000_000 * SECOND;
    const BOOT: i64 = EPOCH - 100 * SECOND;

    fn seen(second: i64, late: i64) -> u64 {
        (EPOCH + second * SECOND + late - BOOT) as u64
    }

    #[test]
    fn keeps_the_earliest_sighting() {
        let mut estimator = SecondEstimator::new();
        for (second, late) in [(0, 9_000), (1, 3_000), (2, 12_000), (3, 5_000)] {
            estimator.add(seen(second, late), EPOCH + second * SECOND);
        }
        assert_eq!(estimator.offset(), Some(-BOOT + 3_000));
        assert_eq!(
            estimator.utc_micros(seen(10, 250_000)),
            Some(EPOCH + 10 * SECOND + 247_000)
        );
    }

    #[test]
    fn forgets_a_sighting_older_than_the_window() {
        let mut estimator = SecondEstimator::new();
        estimator.add(seen(0, 1_000), EPOCH);
        for second in 1..=WINDOW as i64 {
            estimator.add(seen(second, 6_000), EPOCH + second * SECOND);
        }
        assert_eq!(estimator.samples(), WINDOW);
        assert_eq!(estimator.offset(), Some(-BOOT + 6_000));
    }

    #[test]
    fn a_jump_starts_the_window_again() {
        let mut estimator = SecondEstimator::new();
        estimator.add(seen(0, 4_000), EPOCH);
        estimator.add(seen(1, 2_000), EPOCH + SECOND);
        // The receiver's clock stepped a second: the same sighting now names the fix before.
        estimator.add(seen(2, 5_000), EPOCH + SECOND);
        assert_eq!(estimator.samples(), 1);
        assert_eq!(estimator.offset(), Some(-BOOT + SECOND + 5_000));
    }

    #[test]
    fn predicts_the_next_burst() {
        let mut estimator = SecondEstimator::new();
        assert_eq!(estimator.next_burst(0, SECOND as u64), None);
        estimator.add(seen(0, 7_000), EPOCH);
        let period = SECOND as u64;
        assert_eq!(
            estimator.next_burst(seen(0, 7_000), period),
            Some(seen(1, 7_000))
        );
        assert_eq!(
            estimator.next_burst(seen(3, 500_000), period),
            Some(seen(4, 7_000))
        );
        assert_eq!(
            estimator.next_burst(seen(4, 6_999), period),
            Some(seen(4, 7_000))
        );
    }
}
