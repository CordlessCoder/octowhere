//! What a node knows of every id's position, which ids it has heard, and what its next packet
//! carries.

use crate::IDS;
use crate::packet::{Entry, MAX_DELTA};
use crate::schedule::is_floor;

/// An id heard within this many rounds is a neighbour.
pub const NEIGHBOUR_ROUNDS: i64 = 7;
/// How far ahead of the node's own clock an entry may be stamped.
pub const AHEAD_S: u32 = 3_600;
/// An entry this much newer than the one last sent for its id is worth sending again, as is one
/// that moved [`MOVED_M`]: a floor period, so a still node's entry stays current.
pub const NEWER_S: u32 = 135;
/// A position this far from the one last sent for its id is worth sending again; above GPS noise.
pub const MOVED_M: f32 = 25.0;

const SLOTS: usize = IDS as usize;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Merge {
    /// The first entry for its id.
    New,
    Newer,
    /// No newer than the entry held.
    Stale,
    /// Stamped more than [`AHEAD_S`] ahead of the node's clock.
    Ahead,
    /// Too old to send on.
    Expired,
    /// About this node, which knows its own position better.
    Own,
}

pub struct Table {
    own: u8,
    entries: [Option<Entry>; SLOTS],
    /// What this node last sent for each id.
    sent: [Option<Entry>; SLOTS],
    /// The round each id was last heard in.
    heard: [Option<i64>; SLOTS],
    /// An id was heard for the first time since the last packet.
    appeared: bool,
    /// Where the rotation through the rest of the table resumes.
    rotation: u8,
}

impl Table {
    #[must_use]
    pub fn new(own: u8) -> Self {
        Self {
            own,
            entries: [None; SLOTS],
            sent: [None; SLOTS],
            heard: [None; SLOTS],
            appeared: false,
            rotation: 0,
        }
    }

    #[must_use]
    pub fn own_id(&self) -> u8 {
        self.own
    }

    #[must_use]
    pub fn entry(&self, id: u8) -> Option<&Entry> {
        self.entries.get(usize::from(id))?.as_ref()
    }

    pub fn entries(&self) -> impl Iterator<Item = &Entry> {
        self.entries.iter().flatten()
    }

    /// Takes this node's own position from its latest fix.
    pub fn set_own(&mut self, entry: Entry) {
        debug_assert_eq!(entry.id, self.own);
        self.entries[usize::from(self.own)] = Some(entry);
    }

    /// Merges an entry heard from another node. `now` is the node's clock in seconds, or `None`
    /// without one, which skips the checks against it.
    pub fn merge(&mut self, entry: Entry, now: Option<u32>) -> Merge {
        if entry.id == self.own {
            return Merge::Own;
        }
        if let Some(now) = now {
            if entry.stamp > now.saturating_add(AHEAD_S) {
                return Merge::Ahead;
            }
            if now.saturating_sub(entry.stamp) > MAX_DELTA {
                return Merge::Expired;
            }
        }
        let slot = &mut self.entries[usize::from(entry.id)];
        match slot {
            Some(held) if held.stamp >= entry.stamp => Merge::Stale,
            Some(_) => {
                *slot = Some(entry);
                Merge::Newer
            }
            None => {
                *slot = Some(entry);
                Merge::New
            }
        }
    }

    /// Drops the entries too old to send on.
    pub fn expire(&mut self, now: u32) {
        for slot in &mut self.entries {
            if slot.is_some_and(|entry| now.saturating_sub(entry.stamp) > MAX_DELTA) {
                *slot = None;
            }
        }
    }

    /// Records a packet from `id` in `round`.
    pub fn heard(&mut self, id: u8, round: i64) {
        let last = &mut self.heard[usize::from(id)];
        if last.is_none() {
            self.appeared = true;
        }
        *last = Some(round);
    }

    /// The ids heard within the last [`NEIGHBOUR_ROUNDS`] before `round`, as a set.
    #[must_use]
    pub fn neighbours(&self, round: i64) -> u32 {
        (0..IDS)
            .filter(|&id| {
                self.heard[usize::from(id)].is_some_and(|last| round - last < NEIGHBOUR_ROUNDS)
            })
            .fold(0, |set, id| set | 1 << id)
    }

    /// Whether an entry is worth sending ahead of the rotation.
    fn is_fresh(&self, entry: &Entry) -> bool {
        match &self.sent[usize::from(entry.id)] {
            None => true,
            Some(sent) => {
                moved(sent, entry)
                    || (entry.id != self.own && entry.stamp >= sent.stamp.saturating_add(NEWER_S))
            }
        }
    }

    /// Whether this node transmits in its slot in `round`.
    #[must_use]
    pub fn wants_to_send(&self, round: i64) -> bool {
        is_floor(round, self.own)
            || self.appeared
            || self.entries().any(|entry| self.is_fresh(entry))
    }

    /// Picks up to `out.len()` entries for a packet with base timestamp `base`, in the protocol's
    /// order: this node's own, those worth sending ahead of the rotation newest first, then the
    /// rest in rotation. Returns how many it picked.
    pub fn digest(&self, base: u32, out: &mut [Entry]) -> usize {
        let mut picked = 0u32;
        let mut n = 0;
        let mut take = |entry: &Entry, picked: &mut u32, n: &mut usize| {
            if *n < out.len() && *picked & 1 << entry.id == 0 && entry.fits_below(base) {
                out[*n] = *entry;
                *n += 1;
                *picked |= 1 << entry.id;
            }
        };
        if let Some(own) = self.entry(self.own) {
            take(own, &mut picked, &mut n);
        }
        let mut fresh = [(0u32, 0u8); SLOTS];
        let mut count = 0;
        for entry in self.entries().filter(|entry| self.is_fresh(entry)) {
            fresh[count] = (entry.stamp, entry.id);
            count += 1;
        }
        fresh[..count].sort_unstable_by(|a, b| b.cmp(a));
        for &(_, id) in &fresh[..count] {
            if let Some(entry) = self.entry(id) {
                take(entry, &mut picked, &mut n);
            }
        }
        for step in 0..IDS {
            if let Some(entry) = self.entry((self.rotation + step) % IDS) {
                take(entry, &mut picked, &mut n);
            }
        }
        n
    }

    /// Records that a packet carried `entries`, as [`Table::digest`] picked them.
    pub fn sent(&mut self, entries: &[Entry]) {
        for entry in entries {
            self.sent[usize::from(entry.id)] = Some(*entry);
        }
        if let Some(last) = entries.iter().rev().find(|entry| entry.id != self.own) {
            self.rotation = (last.id + 1) % IDS;
        }
        self.appeared = false;
    }
}

/// Whether two positions are more than [`MOVED_M`] apart, on a flat earth, which is close enough
/// at that distance.
fn moved(a: &Entry, b: &Entry) -> bool {
    const METRES_PER_E7: f32 = 0.011_131_95;
    let dlat = (i64::from(a.latitude) - i64::from(b.latitude)) as f32 * METRES_PER_E7;
    let dlon = (i64::from(a.longitude) - i64::from(b.longitude) + 1_800_000_000)
        .rem_euclid(3_600_000_000)
        - 1_800_000_000;
    let latitude = (a.latitude as f32 * 1e-7).to_radians();
    let dlon = dlon as f32 * METRES_PER_E7 * libm::cosf(latitude);
    dlat * dlat + dlon * dlon > MOVED_M * MOVED_M
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::packet::{Hdop, Quality};

    fn entry(id: u8, stamp: u32, latitude: i32) -> Entry {
        Entry {
            id,
            latitude,
            longitude: -62_603_000,
            stamp,
            quality: Quality::Autonomous,
            hdop: Hdop::from_milli(900),
        }
    }

    const DUBLIN: i32 = 533_498_000;
    /// About 33 m north of Dublin's latitude.
    const NORTH: i32 = DUBLIN + 3_000;

    #[test]
    fn the_newer_entry_wins_and_the_clock_gates_the_rest() {
        let mut table = Table::new(0);
        let now = 1_000_000;
        assert_eq!(
            table.merge(entry(3, now - 10, DUBLIN), Some(now)),
            Merge::New
        );
        assert_eq!(
            table.merge(entry(3, now - 20, DUBLIN), Some(now)),
            Merge::Stale
        );
        assert_eq!(
            table.merge(entry(3, now - 5, DUBLIN), Some(now)),
            Merge::Newer
        );
        assert_eq!(
            table.merge(entry(4, now + AHEAD_S + 1, DUBLIN), Some(now)),
            Merge::Ahead
        );
        assert_eq!(
            table.merge(entry(4, now - MAX_DELTA - 1, DUBLIN), Some(now)),
            Merge::Expired
        );
        assert_eq!(
            table.merge(entry(4, now + AHEAD_S + 1, DUBLIN), None),
            Merge::New
        );
        assert_eq!(table.merge(entry(0, now, DUBLIN), Some(now)), Merge::Own);
        table.expire(now + MAX_DELTA);
        assert!(table.entry(3).is_none());
    }

    #[test]
    fn neighbours_are_ids_heard_in_the_last_seven_rounds() {
        let mut table = Table::new(0);
        table.heard(5, 100);
        table.heard(9, 106);
        assert_eq!(table.neighbours(106), 1 << 5 | 1 << 9);
        assert_eq!(table.neighbours(107), 1 << 9);
    }

    #[test]
    fn a_node_sends_on_its_floor_and_when_it_has_news() {
        let mut table = Table::new(1);
        let (floor, quiet) = (1, 2);
        assert!(table.wants_to_send(floor));
        assert!(!table.wants_to_send(quiet));

        table.set_own(entry(1, 1_000, DUBLIN));
        assert!(table.wants_to_send(quiet), "its first position");
        table.sent(&[entry(1, 1_000, DUBLIN)]);
        table.set_own(entry(1, 1_100, DUBLIN + 100));
        assert!(!table.wants_to_send(quiet), "a metre is noise");
        table.set_own(entry(1, 1_200, NORTH));
        assert!(table.wants_to_send(quiet), "33 m is a move");
        table.sent(&[entry(1, 1_200, NORTH)]);

        table.heard(7, quiet);
        assert!(table.wants_to_send(quiet), "a node appeared");
        table.sent(&[]);
        assert!(!table.wants_to_send(quiet));

        table.merge(entry(7, 1_200, DUBLIN), None);
        assert!(table.wants_to_send(quiet), "an entry not yet relayed");
        table.sent(&[entry(7, 1_200, DUBLIN)]);
        table.merge(entry(7, 1_300, DUBLIN), None);
        assert!(!table.wants_to_send(quiet), "100 s newer and still");
        table.merge(entry(7, 1_200 + NEWER_S, DUBLIN), None);
        assert!(table.wants_to_send(quiet));
    }

    #[test]
    fn a_digest_leads_with_its_own_then_news_then_rotates() {
        let mut table = Table::new(2);
        table.set_own(entry(2, 5_000, DUBLIN));
        for id in [4, 6, 8] {
            table.merge(entry(id, 4_000 + u32::from(id), DUBLIN), None);
        }
        table.sent(&[
            entry(4, 4_004, DUBLIN),
            entry(6, 4_006, DUBLIN),
            entry(8, 4_008, DUBLIN),
        ]);
        table.merge(entry(6, 4_900, NORTH), None);
        table.merge(entry(10, 4_800, DUBLIN), None);

        let mut out = [entry(0, 0, 0); 3];
        let n = table.digest(5_000, &mut out);
        let ids: [u8; 3] = core::array::from_fn(|i| out[i].id);
        assert_eq!((n, ids), (3, [2, 6, 10]), "own, then the news newest first");

        table.sent(&out[..n]);
        let n = table.digest(5_000, &mut out);
        let ids: [u8; 3] = core::array::from_fn(|i| out[i].id);
        assert_eq!(
            (n, ids),
            (3, [2, 4, 6]),
            "then the rotation, after the last id sent"
        );
    }

    #[test]
    fn a_digest_leaves_out_what_does_not_fit_below_its_base() {
        let mut table = Table::new(2);
        table.merge(entry(4, 6_000, DUBLIN), None);
        table.merge(entry(5, 5_000 - MAX_DELTA - 1, DUBLIN), None);
        let mut out = [entry(0, 0, 0); 4];
        assert_eq!(table.digest(5_000, &mut out), 0);
    }
}
