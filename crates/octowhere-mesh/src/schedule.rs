//! Rounds and slots, in a timebase's microseconds since 1970, and the airtime of a packet.

use crate::IDS;

/// A round: every id's slot once.
pub const ROUND_US: i64 = 45_000_000;
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

/// When `id`'s slot in `round` starts.
#[must_use]
pub fn slot_start(round: i64, id: u8) -> i64 {
    round * ROUND_US + i64::from(id) * SLOT_US
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

/// The round and start of `id`'s first slot starting at or after `t`.
#[must_use]
pub fn next_slot(t: i64, id: u8) -> (i64, i64) {
    let round = round_at(t);
    let start = slot_start(round, id);
    if start >= t {
        (round, start)
    } else {
        (round + 1, slot_start(round + 1, id))
    }
}

/// The id and start of the first slot of any id starting at or after `t`.
#[must_use]
pub fn next_any_slot(t: i64) -> (u8, i64) {
    let round = round_at(t);
    let index = (t - round * ROUND_US + SLOT_US - 1).div_euclid(SLOT_US);
    if index >= i64::from(IDS) {
        (0, slot_start(round + 1, 0))
    } else {
        (index as u8, slot_start(round, index as u8))
    }
}

/// The id and start of the first slot starting at or after `t` of an id in the set `ids`, or
/// `None` for an empty set.
#[must_use]
pub fn next_slot_in(t: i64, ids: u32) -> Option<(u8, i64)> {
    let mut slot = next_any_slot(t);
    for _ in 0..IDS {
        if ids & 1 << slot.0 != 0 {
            return Some(slot);
        }
        slot = next_any_slot(slot.1 + 1);
    }
    None
}

/// A packet's base timestamp: the whole second its slot starts in.
#[must_use]
pub fn base_of(start: i64) -> u32 {
    start.div_euclid(1_000_000) as u32
}

/// The start of the slot a packet's sender and base timestamp name. A round's last slot starts
/// 43.6 s in, so the base second always falls in the slot's own round.
#[must_use]
pub fn named_slot(base: u32, sender: u8) -> i64 {
    slot_start(round_at(i64::from(base) * 1_000_000), sender)
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

    #[test]
    fn the_next_slot_is_this_one_until_it_starts() {
        let start = slot_start(100, 5);
        assert_eq!(next_slot(start, 5), (100, start));
        assert_eq!(next_slot(start - 1, 5), (100, start));
        assert_eq!(next_slot(start + 1, 5), (101, slot_start(101, 5)));
        assert_eq!(next_any_slot(start + 1), (6, slot_start(100, 6)));
        assert_eq!(
            next_any_slot(slot_start(100, 31) + 1),
            (0, slot_start(101, 0))
        );
    }

    #[test]
    fn the_next_slot_in_a_set_skips_the_ids_outside_it() {
        let start = slot_start(100, 5);
        let ids = 1 << 3 | 1 << 9;
        assert_eq!(next_slot_in(start, ids), Some((9, slot_start(100, 9))));
        assert_eq!(
            next_slot_in(slot_start(100, 9) + 1, ids),
            Some((3, slot_start(101, 3)))
        );
        assert_eq!(next_slot_in(start, 1 << 5), Some((5, start)));
        assert_eq!(
            next_slot_in(start + 1, 1 << 5),
            Some((5, slot_start(101, 5)))
        );
        assert_eq!(next_slot_in(start, 0), None);
    }

    #[test]
    fn a_base_timestamp_names_its_slot() {
        for id in 0..IDS {
            for round in [0, 1, 37_000_000] {
                let start = slot_start(round, id);
                assert_eq!(named_slot(base_of(start), id), start);
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
