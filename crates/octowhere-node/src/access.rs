//! When a node sends, by contention (see "Medium access" in the protocol). What it holds says
//! when its own packet is due. Once due, a packet waits a random backoff of whole [`STEP_US`]
//! and goes if the channel is clear then, or backs off again.

use octowhere_mesh::packet::MAX_PACKET;
use octowhere_mesh::schedule::{FLOOR_US, REST_TIMES, ROUND_US, airtime_us};

/// One step of a backoff: long enough for a node to detect another's preamble and for its own
/// transmission to start, so that two nodes a step apart do not both send.
pub const STEP_US: i64 = 10_000;
/// A packet once due waits up to this many steps, so that the nodes that heard one packet do not
/// all answer it at once.
pub const STEPS: u32 = 32;
/// Bench: the steps a packet holding records waits up to, in place of [`STEPS`].
pub static BENCH_RECORD_STEPS: core::sync::atomic::AtomicU32 =
    core::sync::atomic::AtomicU32::new(STEPS);
/// A node that found the channel busy waits up to this many steps, past the longest packet.
pub const BUSY_STEPS: u32 = 64;
/// A node's floor and its news come up to this much early, drawn at each of its packets, so that
/// nodes that started together drift apart rather than contend at every floor.
pub const SPREAD_US: i64 = 10_000_000;
/// A summary or a request, which asks for what a node lacks, waits up to this long once due,
/// drawn once. Nodes that cannot hear each other often answer one packet together, and the
/// backoff keeps a node apart only from those it hears.
pub const REPAIR_SPREAD_US: i64 = 5_000_000;
/// The airtime band O allows a node in an hour: its 10% duty cycle, which EN 300 220-2 V3.3.1
/// (clause 4.4.3) measures over an hour there.
pub const HOUR_AIRTIME_US: i64 = 360_000_000;
/// The ledger sums a node's airtime in slices this long.
const SLICE_US: i64 = 300_000_000;
/// The slices the ledger keeps: a time's own and the twelve before it, which hold the hour
/// before it and up to a slice more.
const SLICES: i64 = 13;

/// What a node holds to send, which says when its own packet is due.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Holding {
    /// Records or messages, which go at once.
    pub records: bool,
    /// A summary or a request, which go at the time drawn for them.
    pub repair: bool,
    /// Positions or a word worth sending before the floor, which wait a round after the last
    /// packet.
    pub news: bool,
    /// The local time a packet is due in a sweep round under way.
    pub sweep: Option<i64>,
}

/// What a node has sent, on its local timer.
#[derive(Clone, Copy, Debug, Default)]
pub struct Access {
    /// When its last own packet started, and the spread drawn for it.
    own: Option<(i64, i64)>,
    /// Until when it stays silent: after its last transmission, or a channel found busy.
    quiet_until: i64,
    /// When the repair it holds is due, once drawn.
    repair_at: Option<i64>,
    ledger: Ledger,
}

/// The airtime a node sent in each of the last [`SLICES`] slices of its local timer.
#[derive(Clone, Copy, Debug, Default)]
struct Ledger {
    /// Slice `k`'s airtime at `k % SLICES`.
    sums: [i64; SLICES as usize],
    /// The newest slice written.
    newest: i64,
}

impl Ledger {
    fn at(slice: i64) -> usize {
        slice.rem_euclid(SLICES) as usize
    }

    fn add(&mut self, at: i64, airtime: i64) {
        let slice = at.div_euclid(SLICE_US);
        for stale in (self.newest + 1).max(slice - SLICES + 1)..=slice {
            self.sums[Self::at(stale)] = 0;
        }
        self.newest = self.newest.max(slice);
        self.sums[Self::at(slice)] += airtime;
    }

    /// The earliest local time from `now` at which `airtime` more keeps every hour's airtime
    /// within [`HOUR_AIRTIME_US`].
    fn allows(&self, now: i64, airtime: i64) -> i64 {
        let slice = now.div_euclid(SLICE_US).max(self.newest);
        let mut oldest = (slice - SLICES + 1).max(self.newest - SLICES + 1);
        let mut held: i64 = (oldest..=self.newest).map(|k| self.sums[Self::at(k)]).sum();
        let first = oldest;
        while held + airtime > HOUR_AIRTIME_US && oldest <= self.newest {
            held -= self.sums[Self::at(oldest)];
            oldest += 1;
        }
        match oldest == first {
            true => now,
            // Slice `oldest - 1` leaves the ledger's hour once a slice is `SLICES` past it.
            false => now.max((oldest - 1 + SLICES) * SLICE_US),
        }
    }
}

impl Access {
    /// When the node's own packet is due at local time `now`, holding `holding`: its first at
    /// once. `spread` is a draw below [`REPAIR_SPREAD_US`], taken if a repair is new.
    pub fn own_due(&mut self, holding: Holding, now: i64, spread: impl FnOnce() -> i64) -> i64 {
        let repair = match holding.repair {
            true => Some(*self.repair_at.get_or_insert_with(|| now + spread())),
            false => {
                self.repair_at = None;
                None
            }
        };
        let due = match self.own {
            None => now,
            Some(_) if holding.records => now,
            Some((at, spread)) => {
                let last = at - spread;
                let mut due = last + FLOOR_US;
                if holding.news {
                    due = due.min(last + ROUND_US);
                }
                due
            }
        };
        let due = [holding.sweep, repair]
            .into_iter()
            .flatten()
            .fold(due, i64::min);
        self.after_quiet(due)
    }

    /// When a transmission due at local time `at` may go: after the rest, and once the hour
    /// has room for the longest packet.
    #[must_use]
    pub fn after_quiet(&self, at: i64) -> i64 {
        let at = at.max(self.quiet_until);
        self.ledger.allows(at, airtime_us(MAX_PACKET))
    }

    /// Notes a transmission of `len` bytes that started at local time `at`: the node's own
    /// packet, with `spread` below [`SPREAD_US`] drawn for it, when `own` holds one. The node
    /// rests [`REST_TIMES`] airtimes after it.
    pub fn sent(&mut self, at: i64, len: usize, own: Option<i64>) {
        self.note(at, len, own, REST_TIMES);
    }

    /// Notes the node's own packet as [`Access::sent`] does, one that carried its own key
    /// messages, after which it does not rest: the switch waits on every key message leaving
    /// the remover, and [`HOUR_AIRTIME_US`] bounds the airtime it spends.
    pub fn sent_keys(&mut self, at: i64, len: usize, spread: i64) {
        self.note(at, len, Some(spread), 0);
    }

    fn note(&mut self, at: i64, len: usize, own: Option<i64>, rest: i64) {
        if let Some(spread) = own {
            self.own = Some((at, spread));
            self.repair_at = None;
        }
        self.quiet_until = at + (1 + rest) * airtime_us(len);
        self.ledger.add(at, airtime_us(len));
    }

    /// Notes the channel found busy at local time `now`, with a backoff of `wait` from there.
    pub fn busy(&mut self, now: i64, wait: i64) {
        self.quiet_until = self.quiet_until.max(now + wait);
    }

    /// Whether the node's own packet went out at or after local time `since`.
    #[must_use]
    pub fn sent_since(&self, since: i64) -> bool {
        self.own.is_some_and(|(at, _)| at >= since)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: i64 = 1_000 * 1_000_000;

    #[test]
    fn the_first_packet_and_records_go_at_once() {
        let mut access = Access::default();
        assert_eq!(access.own_due(Holding::default(), NOW, || 0), NOW);
        access.sent(NOW, 40, Some(0));
        let later = NOW + 10 * 1_000_000;
        let records = Holding {
            records: true,
            ..Holding::default()
        };
        assert_eq!(access.own_due(records, later, || 0), later);
    }

    #[test]
    fn news_waits_a_round_and_the_floor_comes_without_it() {
        let mut access = Access::default();
        access.sent(NOW, 40, Some(0));
        let news = Holding {
            news: true,
            ..Holding::default()
        };
        assert_eq!(access.own_due(news, NOW + 1, || 0), NOW + ROUND_US);
        assert_eq!(
            access.own_due(Holding::default(), NOW + 1, || 0),
            NOW + FLOOR_US
        );
        let sweep = Holding {
            sweep: Some(NOW + 5_000_000),
            ..news
        };
        assert_eq!(access.own_due(sweep, NOW + 1, || 0), NOW + 5_000_000);
    }

    #[test]
    fn a_repair_waits_the_time_drawn_for_it_once() {
        let mut access = Access::default();
        access.sent(NOW, 40, Some(0));
        let repair = Holding {
            repair: true,
            ..Holding::default()
        };
        assert_eq!(
            access.own_due(repair, NOW + 1, || 3_000_000),
            NOW + 3_000_001
        );
        assert_eq!(
            access.own_due(repair, NOW + 2, || 0),
            NOW + 3_000_001,
            "drawn once"
        );
        access.sent(NOW + 3_000_001, 40, Some(0));
        assert_eq!(
            access.own_due(repair, NOW + 4_000_000, || 0),
            NOW + 4_000_000
        );
    }

    #[test]
    fn a_spread_brings_the_floor_early() {
        let mut access = Access::default();
        access.sent(NOW, 40, Some(3_000_000));
        let floor = access.own_due(Holding::default(), NOW + 1, || 0);
        assert_eq!(floor, NOW + FLOOR_US - 3_000_000);
        assert!(access.sent_since(NOW));
    }

    #[test]
    fn a_node_rests_nine_airtimes_after_sending() {
        let mut access = Access::default();
        access.sent(NOW, 255, None);
        let records = Holding {
            records: true,
            ..Holding::default()
        };
        let quiet = NOW + 10 * airtime_us(255);
        assert_eq!(access.own_due(records, NOW + 1, || 0), quiet);
        assert_eq!(access.own_due(Holding::default(), NOW + 1, || 0), quiet);
        assert!(
            !access.sent_since(NOW),
            "an old key's packet is not its own"
        );
        access.busy(NOW + 1, 1);
        assert_eq!(
            access.after_quiet(NOW),
            quiet,
            "a busy channel keeps the rest"
        );
        access.busy(quiet, 30_000);
        assert_eq!(access.after_quiet(NOW), quiet + 30_000);
    }

    #[test]
    fn a_node_does_not_rest_after_its_own_key_messages() {
        let mut access = Access::default();
        access.sent_keys(NOW, 255, 0);
        let records = Holding {
            records: true,
            ..Holding::default()
        };
        assert_eq!(
            access.own_due(records, NOW + 1, || 0),
            NOW + airtime_us(255)
        );
    }

    #[test]
    fn the_hour_holds_a_node_to_its_airtime() {
        let mut access = Access::default();
        let longest = airtime_us(MAX_PACKET);
        let mut at = NOW;
        while access.after_quiet(at) == at {
            access.sent_keys(at, MAX_PACKET, 0);
            at += longest;
        }
        let sent = (at - NOW) / longest;
        assert_eq!(sent, HOUR_AIRTIME_US / longest, "packets in the hour");
        let open = access.after_quiet(at);
        assert!(open >= NOW + 3_600_000_000, "not before an hour has passed");
        assert!(
            open <= (NOW / SLICE_US + 1) * SLICE_US + 3_600_000_000,
            "nor more than a slice after"
        );
    }
}
