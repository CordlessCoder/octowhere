//! The timebase a node places its slots on: GPS time from its own fix, or another node's clock,
//! taken from when that node's packets arrive. Times are microseconds: `local` on the node's own
//! timer, the timebase's since 1970.

use crate::packet::{Header, Source, Timebase};
use crate::schedule::{GUARD_US, ROUND_US, SWEEP_EVERY, is_sweep_round, round_at};
use crate::table::NEIGHBOUR_ROUNDS;

/// A node's first sweep, and one after it lost its timebase's root, listen this long, which spans
/// every node's floor round.
pub const SWEEP_US: i64 = 3 * ROUND_US;
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
    /// When a first or lost sweep ends.
    sweep_until: Option<i64>,
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
            upstream: now,
        }
    }

    /// Moves the node to id `own`. A clock it was the root of stays its own.
    pub fn renumber(&mut self, own: u8) {
        if let Some((_, timebase)) = &mut self.clock
            && timebase.source == Source::Node(self.own)
            && timebase.hops == 0
        {
            timebase.source = Source::Node(own);
        }
        self.own = own;
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

    /// Ends and starts first and lost sweeps and ages the GPS time. `rtc` is the RTC's UTC at
    /// `now`, which a node that heard nobody starts its own clock from. Call it before reading
    /// the clock.
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
        if self.is_lost(now) {
            self.sweep_until = Some(now + SWEEP_US);
        }
    }

    /// Takes a packet whose transmission started at local time `start`. A sender starts at its
    /// slot's start, `slot` on its timebase, which its header names to the group's schedule.
    pub fn arrival(&mut self, header: &Header, slot: i64, start: i64, now: i64) -> Arrival {
        let offset = start - slot;
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

    /// Sweeps for three rounds from local time `now`, as a notice asks.
    pub fn sweep(&mut self, now: i64) {
        self.sweep_to(now + SWEEP_US);
    }

    /// Sweeps until local time `until`, or later if a sweep under way runs longer. Taking up a
    /// timebase or a fix ends it, as it ends a first sweep, so a caller that must listen on
    /// asks again.
    pub fn sweep_to(&mut self, until: i64) {
        self.sweep_until = Some(self.sweep_until.map_or(until, |held| held.max(until)));
    }

    /// The sweep round under way in the timebase at local time `now`, if one is. It opens a guard
    /// early, as a slot's window does, and closes as early.
    fn sweep_round(&self, now: i64) -> Option<i64> {
        let (time, _) = self.at(now)?;
        let round = round_at(time + GUARD_US);
        is_sweep_round(round).then_some(round)
    }

    /// Whether the node listens throughout at local time `now`: in a first or lost sweep, or in
    /// a sweep round. A sweep ends at its time, though the node ticks only once a round.
    #[must_use]
    pub fn is_sweeping(&self, now: i64) -> bool {
        self.sweep_until.is_some_and(|until| now < until) || self.sweep_round(now).is_some()
    }

    /// The local time the sweep under way at `now` ends.
    #[must_use]
    pub fn sweep_ends(&self, now: i64) -> Option<i64> {
        match self.sweep_until {
            Some(until) if now < until => Some(until),
            _ => {
                let round = self.sweep_round(now)?;
                self.local((round + 1) * ROUND_US - GUARD_US)
            }
        }
    }

    /// The local time the next sweep round after `now` opens, with a timebase.
    #[must_use]
    pub fn next_sweep(&self, now: i64) -> Option<i64> {
        let (time, _) = self.at(now)?;
        let round = round_at(time + GUARD_US) + 1;
        let next = round + (SWEEP_EVERY - round.rem_euclid(SWEEP_EVERY)) % SWEEP_EVERY;
        self.local(next * ROUND_US - GUARD_US)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schedule::{SLOT_US, base_of};

    /// A slot's start in an order that does not shuffle, which the clock does not need.
    fn slot_start(round: i64, id: u8) -> i64 {
        round * ROUND_US + i64::from(id) * SLOT_US
    }

    const SECOND: i64 = 1_000_000;
    /// A UTC time in 2026, and a round there.
    const UTC: i64 = 1_790_000_000 * SECOND;

    fn header(sender: u8, source: Source, hops: u8, start: i64) -> Header {
        Header {
            sender,
            timebase: Timebase { source, hops },
            base: base_of(start),
            phase: 0,
            notice: false,
        }
    }

    /// A node whose timer reads `skew` more than the timebase, and the local time of a slot.
    fn sent(clock: &mut Clock, from: u8, source: Source, hops: u8, skew: i64, now: i64) -> Arrival {
        let round = crate::schedule::round_at(now - skew) + 1;
        let start = slot_start(round, from);
        clock.arrival(&header(from, source, hops, start), start, start + skew, now)
    }

    #[test]
    fn a_node_that_hears_nobody_starts_its_own_clock_from_the_rtc() {
        // The round after UTC's, which is a sweep round.
        let rtc = UTC + ROUND_US;
        let mut clock = Clock::new(24, 0);
        clock.tick(SWEEP_US - 1, Some(rtc));
        assert!(clock.is_sweeping(SWEEP_US - 1));
        assert_eq!(clock.at(0), None);
        clock.tick(SWEEP_US, Some(rtc));
        assert!(!clock.is_sweeping(SWEEP_US));
        let (time, timebase) = clock.at(SWEEP_US).unwrap();
        assert_eq!(time, rtc);
        assert_eq!(timebase.source, Source::Node(24));
    }

    #[test]
    fn a_sweep_runs_to_the_later_end_and_a_timebase_taken_up_ends_it() {
        let mut clock = Clock::new(1, 0);
        clock.tick(SWEEP_US, Some(UTC + ROUND_US));
        let now = SWEEP_US + SECOND;
        clock.sweep_to(now + 2 * ROUND_US);
        clock.sweep_to(now + ROUND_US);
        assert_eq!(clock.sweep_ends(now), Some(now + 2 * ROUND_US));
        sent(&mut clock, 0, Source::Node(0), 0, 7, now);
        assert!(!clock.is_sweeping(now + ROUND_US + 2 * SECOND));
    }

    #[test]
    fn a_notice_starts_a_three_round_sweep() {
        let mut clock = Clock::new(1, 0);
        clock.tick(SWEEP_US, Some(UTC + ROUND_US));
        let now = SWEEP_US + SECOND;
        assert!(!clock.is_sweeping(now));
        clock.sweep(now);
        assert!(clock.is_sweeping(now + SWEEP_US - 1));
        assert_eq!(clock.sweep_ends(now), Some(now + SWEEP_US));
        clock.tick(now + SWEEP_US, None);
        assert!(!clock.is_sweeping(now + SWEEP_US));
    }

    #[test]
    fn a_node_sweeps_its_timebases_sweep_rounds() {
        // A timebase whose round at local time 0 follows a sweep round.
        let start = (round_at(UTC) / SWEEP_EVERY * SWEEP_EVERY + 1) * ROUND_US;
        let mut clock = Clock::new(1, 0);
        clock.gps(-start, 0);
        clock.tick(0, None);
        assert!(!clock.is_sweeping(0), "a fix ends the first sweep");
        let opens = (SWEEP_EVERY - 1) * ROUND_US - GUARD_US;
        assert_eq!(clock.next_sweep(0), Some(opens));
        assert!(!clock.is_sweeping(opens - 1));
        assert!(clock.is_sweeping(opens));
        assert_eq!(clock.sweep_ends(opens), Some(opens + ROUND_US));
        assert!(clock.is_sweeping(opens + ROUND_US - 1));
        assert!(
            !clock.is_sweeping(opens + ROUND_US),
            "for one round, whenever the node ticks"
        );
        assert_eq!(
            clock.next_sweep(opens),
            Some(opens + SWEEP_EVERY * ROUND_US)
        );
    }

    #[test]
    fn a_node_takes_a_heard_clock_and_keeps_to_the_lowest_root() {
        let mut clock = Clock::new(28, 0);
        let skew = 5 * SECOND;
        let arrival = sent(&mut clock, 24, Source::Node(24), 0, skew, SECOND);
        assert_eq!(arrival.taken, Taken::Adopted);
        assert!(
            !clock.is_sweeping(SECOND),
            "a timebase ends the first sweep"
        );
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
    fn a_root_that_moves_id_keeps_its_clock() {
        let mut clock = Clock::new(24, 0);
        clock.tick(SWEEP_US, Some(UTC));
        clock.renumber(5);
        assert_eq!(
            clock.at(SWEEP_US).unwrap(),
            (
                UTC,
                Timebase {
                    source: Source::Node(5),
                    hops: 0
                }
            )
        );
    }

    #[test]
    fn a_node_that_loses_its_upstream_sweeps_then_becomes_a_root() {
        let mut clock = Clock::new(28, 0);
        sent(&mut clock, 24, Source::Node(24), 0, 0, SECOND);
        clock.tick(SECOND + LOST_US, None);
        assert!(!clock.is_sweeping(SECOND + LOST_US));
        clock.tick(SECOND + LOST_US + 1, None);
        assert!(clock.is_sweeping(SECOND + LOST_US + 1));
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
