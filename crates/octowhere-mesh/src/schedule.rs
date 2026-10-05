//! Rounds and slots, in a timebase's microseconds since 1970, and the airtime of a packet.
//! A group's ids send in an order of their own each round, which only its members can work
//! out (see "Shuffled slots" in the protocol).

use core::cell::Cell;

use aes::{
    Aes256,
    cipher::{Array, BlockCipherEncrypt, KeyInit},
};
use hkdf::Hkdf;
use sha2::Sha256;

use crate::{IDS, Ids, seal::Key};

/// A round: every id's slot once.
pub const ROUND_US: i64 = 45_000_000;
/// A round in whole seconds.
pub const ROUND_S: u32 = (ROUND_US / 1_000_000) as u32;
/// The time from one slot's start to the next's.
pub const SLOT_US: i64 = ROUND_US / IDS as i64;
/// A node transmits in every round of this many whose index matches its id's, whether or not it
/// has anything new.
pub const FLOOR_EVERY: i64 = 3;
/// How far either side of a slot's start a listener opens its window, for the senders' and its own
/// clock error.
pub const GUARD_US: i64 = 250_000;
/// Every round whose index is a multiple of this is a sweep round: every node listens throughout
/// it and sends in its slot. Not a whole number of floors, so a sweep round falls on each of the
/// floor's three rounds in turn.
pub const SWEEP_EVERY: i64 = 13;

/// The round holding `t`.
#[must_use]
pub fn round_at(t: i64) -> i64 {
    t.div_euclid(ROUND_US)
}

/// The round holding `t` as removals store and send rounds.
#[must_use]
pub fn stored_round_at(t: i64) -> u32 {
    round_at(t) as u32
}

/// The whole second holding `t`, as a header's base, a record and a message are stamped.
#[must_use]
pub fn second_at(t: i64) -> u32 {
    t.div_euclid(1_000_000) as u32
}

/// The second the round holding `t` starts at.
#[must_use]
pub fn round_start_s(t: i64) -> u32 {
    second_at(round_at(t) * ROUND_US)
}

/// Whether `id` transmits in `round` with nothing new to say.
#[must_use]
pub fn is_floor(round: i64, id: u8) -> bool {
    round.rem_euclid(FLOOR_EVERY) == i64::from(id) % FLOOR_EVERY
}

/// Whether every node sweeps `round`.
#[must_use]
pub fn is_sweep_round(round: i64) -> bool {
    round.rem_euclid(SWEEP_EVERY) == 0
}

/// Each id's place in a round, from 0 for the first slot.
pub type Places = [u8; IDS as usize];

/// A group's slots: the place each id sends in, round by round. Each round sorts the ids by
/// AES-256 of the round's number and the id, under a key HKDF-SHA256 derives from the group key.
/// Every member on a timebase shares its round numbers, and only members hold the key.
pub struct Schedule {
    /// The group key it is derived from.
    key: Key,
    cipher: Aes256,
    /// The places of the last two rounds asked about, as each id's place by round.
    places: Cell<[Option<(i64, Places)>; 2]>,
}

impl Schedule {
    #[must_use]
    pub fn new(key: &Key) -> Self {
        let mut derived = [0; 32];
        Hkdf::<Sha256>::new(None, key.bytes())
            .expand(b"octowhere slots", &mut derived)
            .expect("32 bytes is within HKDF-SHA256's output");
        Self {
            key: key.clone(),
            cipher: Aes256::new(&derived.into()),
            places: Cell::new([None; 2]),
        }
    }

    /// Whether this is the schedule of the group with `key`.
    #[must_use]
    pub fn is_for(&self, key: &Key) -> bool {
        self.key == *key
    }

    #[must_use]
    pub fn places(&self, round: i64) -> Places {
        let mut cached = self.places.get();
        if let Some((_, places)) = cached.iter().flatten().find(|(at, _)| *at == round) {
            return *places;
        }
        // Sorted by the block's first 64 bits, and by id where they tie.
        let mut ranks = [(0u64, 0u8); IDS as usize];
        for (id, rank) in ranks.iter_mut().enumerate() {
            let mut block = [0; 16];
            block[..8].copy_from_slice(&round.to_be_bytes());
            block[8] = id as u8;
            let mut block = Array::from(block);
            self.cipher.encrypt_block(&mut block);
            let block: [u8; 16] = block.into();
            let mut first = [0; 8];
            first.copy_from_slice(&block[..8]);
            *rank = (u64::from_be_bytes(first), id as u8);
        }
        ranks.sort_unstable();
        let mut places: Places = [0; IDS as usize];
        for (place, &(_, id)) in ranks.iter().enumerate() {
            places[usize::from(id)] = place as u8;
        }
        cached.rotate_right(1);
        cached[0] = Some((round, places));
        self.places.set(cached);
        places
    }

    /// When `id`'s slot in `round` starts.
    #[must_use]
    pub fn slot_start(&self, round: i64, id: u8) -> i64 {
        round * ROUND_US + i64::from(self.places(round)[usize::from(id)]) * SLOT_US
    }

    /// The round and start of `id`'s first slot starting at or after `t`.
    #[must_use]
    pub fn next_slot(&self, t: i64, id: u8) -> (i64, i64) {
        let round = round_at(t);
        let start = self.slot_start(round, id);
        if start >= t {
            (round, start)
        } else {
            (round + 1, self.slot_start(round + 1, id))
        }
    }

    /// The id and start of the first slot starting at or after `t` of an id in `ids`, or `None`
    /// for an empty set.
    #[must_use]
    pub fn next_slot_in(&self, t: i64, ids: Ids) -> Option<(u8, i64)> {
        let round = round_at(t);
        [round, round + 1].into_iter().find_map(|round| {
            ids.iter()
                .map(|id| (id, self.slot_start(round, id)))
                .filter(|&(_, start)| start >= t)
                .min_by_key(|&(_, start)| start)
        })
    }

    /// The start of the slot a packet's sender and base timestamp name. A round's last slot
    /// starts 43.6 s in, so the base second always falls in the slot's own round.
    #[must_use]
    pub fn named_slot(&self, base: u32, sender: u8) -> i64 {
        self.slot_start(round_at(i64::from(base) * 1_000_000), sender)
    }
}

/// A packet's time on air, for spreading factor 7 at 125 kHz, coding rate 4/5, an explicit header,
/// CRC on and an 8-symbol preamble (Semtech AN1200.13).
#[must_use]
pub fn airtime_us(len: usize) -> i64 {
    const SYMBOL_US: i64 = 1_024;
    const SF: i64 = 7;
    let bits = 8 * len as i64 - 4 * SF + 28 + 16;
    let payload_symbols = 8 + (bits + 4 * SF - 1).div_euclid(4 * SF).max(0) * 5;
    // The preamble's 8 symbols and the 4.25 the modem adds.
    49 * SYMBOL_US / 4 + payload_symbols * SYMBOL_US
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slots_divide_a_round_evenly_and_a_day_holds_whole_rounds() {
        assert_eq!(SLOT_US, 1_406_250);
        assert_eq!(86_400_000_000 % ROUND_US, 0);
    }

    fn schedule(fill: u8) -> Schedule {
        Schedule::new(&Key::new([fill; 32]))
    }

    #[test]
    fn every_round_puts_each_id_in_a_slot_of_its_own() {
        let schedule = schedule(7);
        for round in [0, 1, 2, 37_000_000, -5] {
            let mut places = schedule.places(round);
            places.sort_unstable();
            assert_eq!(places, core::array::from_fn(|i| i as u8), "{round}");
        }
    }

    #[test]
    fn the_order_changes_every_round_and_is_the_same_for_every_member() {
        let (one, other) = (schedule(7), schedule(7));
        for round in 0..20 {
            assert_eq!(one.places(round), other.places(round));
            assert_ne!(one.places(round), one.places(round + 1));
        }
    }

    /// Two groups in one place share about one slot pair in 32 a round, rather than every slot
    /// their ids share.
    #[test]
    fn two_groups_share_slots_by_chance() {
        let (ours, theirs) = (schedule(7), schedule(8));
        let rounds = 3_200;
        let shared = (0..rounds)
            .filter(|&round| ours.places(round)[0] == theirs.places(round)[0])
            .count();
        assert!((60..140).contains(&shared), "{shared} of {rounds}");
    }

    #[test]
    fn the_next_slot_is_this_one_until_it_starts() {
        let schedule = schedule(7);
        let start = schedule.slot_start(100, 5);
        assert_eq!(schedule.next_slot(start, 5), (100, start));
        assert_eq!(schedule.next_slot(start - 1, 5), (100, start));
        assert_eq!(
            schedule.next_slot(start + 1, 5),
            (101, schedule.slot_start(101, 5))
        );
    }

    #[test]
    fn the_next_slot_in_a_set_is_the_soonest_of_its_ids() {
        let schedule = schedule(7);
        let ids = Ids::of(3).with(9).with(20);
        let t = 100 * ROUND_US + ROUND_US / 2;
        let (id, start) = schedule.next_slot_in(t, ids).unwrap();
        assert!(start >= t && ids.contains(id));
        // No id in the set has a slot between `t` and the one found.
        for other in [3, 9, 20] {
            let (_, next) = schedule.next_slot(t, other);
            assert!(next >= start, "{other}");
        }
        assert_eq!(schedule.next_slot_in(t, Ids::EMPTY), None);
        let (_, only) = schedule.next_slot(t, 5);
        assert_eq!(schedule.next_slot_in(t, Ids::of(5)), Some((5, only)));
    }

    #[test]
    fn a_base_timestamp_names_its_slot() {
        let schedule = schedule(7);
        for id in 0..IDS {
            for round in [0, 1, 37_000_000] {
                let start = schedule.slot_start(round, id);
                assert_eq!(schedule.named_slot(second_at(start), id), start);
            }
        }
    }

    #[test]
    fn every_id_has_a_floor_round_in_three() {
        for id in 0..IDS {
            assert_eq!((0..3).filter(|&round| is_floor(round, id)).count(), 1);
        }
    }

    #[test]
    fn airtime_matches_the_protocol_table() {
        assert_eq!(airtime_us(255), 399_616);
        assert_eq!(airtime_us(42) / 1000, 87);
        assert_eq!(airtime_us(105) / 1000, 179);
    }
}
