//! The timebase a node places its slots on: GPS time from its own fix, or another node's clock,
//! taken from when that node's packets arrive. Times are microseconds: `local` on the node's own
//! timer, the timebase's since 1970.

use crate::packet::{Header, Source, Timebase};
use crate::schedule::{ROUND_US, named_slot};
use crate::table::NEIGHBOUR_ROUNDS;

/// A sweep listens this long, which spans every node's floor round.
pub const SWEEP_US: i64 = 3 * ROUND_US;
/// A node not timing from its own fix sweeps this often, to find a better timebase.
pub const SWEEP_EVERY_US: i64 = 10 * 60 * 1_000_000;
/// Hearing nothing closer to its timebase's root for this long makes a node sweep, and a sweep
/// that hears nothing closer makes it its own root.
pub const LOST_US: i64 = NEIGHBOUR_ROUNDS * ROUND_US;
/// The node's own GPS time counts as GPS this long after a fix last refined it.
pub const GPS_STALE_US: i64 = 30 * 60 * 1_000_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Taken {
    /// The packet's timebase ranks above the node's, which moved to it.
    Adopted,
    /// The packet was closer to the node's timebase's root, and set its clock.
    Refined,
    Ignored,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct Arrival {
    pub taken: Taken,
    /// How late the packet started against the node's clock before it was taken, when both are
    /// on the same source.
    pub late_us: Option<i64>,
}

pub struct Clock {
    own: u8,
    /// Local time minus timebase time, and the timebase.
    clock: Option<(i64, Timebase)>,
    /// From the node's own fix: local time minus UTC, and the local time a fix last refined it.
    gps: Option<(i64, i64)>,
    sweep_until: Option<i64>,
    next_sweep: i64,
    /// When a packet closer to the timebase's root was last taken.
    upstream: i64,
}

impl Clock {
    /// A node starting at local time `now`, which sweeps first.
    #[must_use]
    pub fn new(own: u8, now: i64) -> Self {
        Self {
            own,
            clock: None,
            gps: None,
            sweep_until: Some(now + SWEEP_US),
            next_sweep: now + SWEEP_US + SWEEP_EVERY_US,
            upstream: now,
        }
    }

    /// Takes the node's GPS time: local time minus UTC, and the local time a fix last refined it.
    pub fn gps(&mut self, offset: i64, refined: i64) {
        self.gps = Some((offset, refined));
    }

    fn live_gps(&self, now: i64) -> Option<i64> {
        self.gps
            .filter(|&(_, refined)| now - refined <= GPS_STALE_US)
            .map(|(offset, _)| offset)
    }

    fn root(&self) -> Timebase {
        Timebase {
            source: Source::Node(self.own),
            hops: 0,
        }
    }

    fn is_lost(&self, now: i64) -> bool {
        self.clock.is_some_and(|(_, timebase)| timebase.hops > 0) && now - self.upstream > LOST_US
    }

    /// Ends and starts sweeps and ages the GPS time. `rtc` is the RTC's UTC at `now`, which a node
    /// that heard nobody starts its own clock from. Call it before reading the clock.
    pub fn tick(&mut self, now: i64, rtc: Option<i64>) {
        if let Some(offset) = self.live_gps(now) {
            self.clock = Some((
                offset,
                Timebase {
                    source: Source::Gps,
                    hops: 0,
                },
            ));
            self.sweep_until = None;
            return;
        }
        if let Some((offset, timebase)) = self.clock
            && timebase.source == Source::Gps
            && timebase.hops == 0
        {
            self.clock = Some((offset, self.root()));
        }
        if let Some(until) = self.sweep_until {
            if now >= until {
                self.sweep_until = None;
                match self.clock {
                    None => self.clock = Some((now - rtc.unwrap_or(now), self.root())),
                    Some((offset, _)) if self.is_lost(now) => {
                        self.clock = Some((offset, self.root()));
                    }
                    Some(_) => {}
                }
            }
            return;
        }
        if now >= self.next_sweep || self.is_lost(now) {
            self.sweep_until = Some(now + SWEEP_US);
            self.next_sweep = now + SWEEP_US + SWEEP_EVERY_US;
        }
    }

    /// Takes a packet whose transmission started at local time `start`. A sender starts at its
    /// slot's start, which its header names.
    pub fn arrival(&mut self, header: &Header, start: i64, now: i64) -> Arrival {
        let offset = start - named_slot(header.base, header.sender);
        let late_us = self
            .clock
            .filter(|(_, timebase)| timebase.source == header.timebase.source)
            .map(|(mine, _)| offset - mine);
        let theirs = header.timebase;
        let taken = if self.live_gps(now).is_some() {
            Taken::Ignored
        } else {
            match self.clock {
                None => Taken::Adopted,
                Some((_, mine)) if theirs.source.outranks(mine.source) => Taken::Adopted,
                Some((_, mine)) if theirs.source == mine.source && theirs.hops < mine.hops => {
                    Taken::Refined
                }
                Some(_) => Taken::Ignored,
            }
        };
        if taken != Taken::Ignored {
            // A clock rooted at this node's id is its own, kept by others while it restarted.
            let timebase = if theirs.source == Source::Node(self.own) {
                self.root()
            } else {
                theirs.next_hop()
            };
            self.clock = Some((offset, timebase));
            self.upstream = now;
        }
        if taken == Taken::Adopted {
            self.sweep_until = None;
        }
        Arrival { taken, late_us }
    }

    /// The timebase's time at local time `local`, and the timebase.
    #[must_use]
    pub fn at(&self, local: i64) -> Option<(i64, Timebase)> {
        self.clock
            .map(|(offset, timebase)| (local - offset, timebase))
    }

    /// The local time the timebase reaches `time`.
    #[must_use]
    pub fn local(&self, time: i64) -> Option<i64> {
        self.clock.map(|(offset, _)| time + offset)
    }

    #[must_use]
    pub fn is_sweeping(&self) -> bool {
        self.sweep_until.is_some()
    }

    /// When the current sweep ends.
    #[must_use]
    pub fn sweep_ends(&self) -> Option<i64> {
        self.sweep_until
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schedule::{base_of, slot_start};

    const SECOND: i64 = 1_000_000;
    /// A UTC time in 2026, and a round there.
    const UTC: i64 = 1_790_000_000 * SECOND;

    fn header(sender: u8, source: Source, hops: u8, start: i64) -> Header {
        Header {
            sender,
            timebase: Timebase { source, hops },
            base: base_of(start),
            phase: 0,
        }
    }

    /// A node whose timer reads `skew` more than the timebase, and the local time of a slot.
    fn sent(clock: &mut Clock, from: u8, source: Source, hops: u8, skew: i64, now: i64) -> Arrival {
        let round = crate::schedule::round_at(now - skew) + 1;
        let start = slot_start(round, from);
        clock.arrival(&header(from, source, hops, start), start + skew, now)
    }

    #[test]
    fn a_node_that_hears_nobody_starts_its_own_clock_from_the_rtc() {
        let mut clock = Clock::new(24, 0);
        clock.tick(SWEEP_US - 1, Some(UTC));
        assert!(clock.is_sweeping());
        assert_eq!(clock.at(0), None);
        clock.tick(SWEEP_US, Some(UTC));
        assert!(!clock.is_sweeping());
        let (time, timebase) = clock.at(SWEEP_US).unwrap();
        assert_eq!(time, UTC);
        assert_eq!(timebase.source, Source::Node(24));
        clock.tick(SWEEP_US + SWEEP_EVERY_US, None);
        assert!(clock.is_sweeping(), "it sweeps again for a better timebase");
    }

    #[test]
    fn a_node_takes_a_heard_clock_and_keeps_to_the_lowest_root() {
        let mut clock = Clock::new(28, 0);
        let skew = 5 * SECOND;
        let arrival = sent(&mut clock, 24, Source::Node(24), 0, skew, SECOND);
        assert_eq!(arrival.taken, Taken::Adopted);
        assert!(!clock.is_sweeping(), "a timebase ends the first sweep");
        assert_eq!(clock.local(UTC), Some(UTC + skew));
        assert_eq!(
            clock.at(0).unwrap().1,
            Timebase {
                source: Source::Node(24),
                hops: 1
            }
        );

        assert_eq!(
            sent(&mut clock, 30, Source::Node(30), 0, 0, 2 * SECOND).taken,
            Taken::Ignored
        );
        let arrival = sent(&mut clock, 3, Source::Node(3), 0, 7 * SECOND, 3 * SECOND);
        assert_eq!(arrival.taken, Taken::Adopted);
        assert_eq!(clock.local(UTC), Some(UTC + 7 * SECOND));
        assert_eq!(
            sent(&mut clock, 9, Source::Gps, 4, 0, 4 * SECOND).taken,
            Taken::Adopted
        );
    }

    #[test]
    fn a_clock_is_refined_only_from_closer_to_its_root() {
        let mut clock = Clock::new(28, 0);
        sent(&mut clock, 9, Source::Gps, 2, 0, SECOND);
        let arrival = sent(&mut clock, 10, Source::Gps, 3, 1_000, 2 * SECOND);
        assert_eq!(
            arrival,
            Arrival {
                taken: Taken::Ignored,
                late_us: Some(1_000)
            }
        );
        let arrival = sent(&mut clock, 11, Source::Gps, 1, 500, 3 * SECOND);
        assert_eq!(
            arrival,
            Arrival {
                taken: Taken::Refined,
                late_us: Some(500)
            }
        );
        assert_eq!(clock.at(0).unwrap().1.hops, 2);
    }

    #[test]
    fn a_node_with_a_fix_keeps_to_it_until_it_goes_stale() {
        let mut clock = Clock::new(1, 0);
        clock.gps(-UTC, 0);
        clock.tick(0, None);
        assert!(!clock.is_sweeping());
        assert_eq!(
            clock.at(0),
            Some((
                UTC,
                Timebase {
                    source: Source::Gps,
                    hops: 0
                }
            ))
        );
        assert_eq!(
            sent(&mut clock, 0, Source::Node(0), 0, 0, SECOND).taken,
            Taken::Ignored
        );

        clock.tick(GPS_STALE_US + 1, None);
        assert_eq!(
            clock.at(0).unwrap(),
            (
                UTC,
                Timebase {
                    source: Source::Node(1),
                    hops: 0
                }
            )
        );
        assert_eq!(
            sent(&mut clock, 0, Source::Node(0), 0, 0, GPS_STALE_US + 2).taken,
            Taken::Adopted
        );
    }

    #[test]
    fn a_node_takes_back_its_own_clock_as_its_root() {
        let mut clock = Clock::new(24, 0);
        let skew = 3 * SECOND;
        let arrival = sent(&mut clock, 28, Source::Node(24), 1, skew, SECOND);
        assert_eq!(arrival.taken, Taken::Adopted);
        assert_eq!(
            clock.at(0).unwrap().1,
            Timebase {
                source: Source::Node(24),
                hops: 0
            }
        );
        assert_eq!(clock.local(UTC), Some(UTC + skew));
    }

    #[test]
    fn a_node_that_loses_its_upstream_sweeps_then_becomes_a_root() {
        let mut clock = Clock::new(28, 0);
        sent(&mut clock, 24, Source::Node(24), 0, 0, SECOND);
        clock.tick(SECOND + LOST_US, None);
        assert!(!clock.is_sweeping());
        clock.tick(SECOND + LOST_US + 1, None);
        assert!(clock.is_sweeping());
        clock.tick(SECOND + LOST_US + 1 + SWEEP_US, None);
        assert_eq!(
            clock.at(0).unwrap().1,
            Timebase {
                source: Source::Node(28),
                hops: 0
            }
        );
    }
}
