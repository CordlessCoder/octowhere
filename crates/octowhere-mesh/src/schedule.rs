//! Rounds, in a timebase's microseconds since 1970, and the airtime of a packet. A round is
//! the unit the floor, sweep rounds, a removal's switch and the message horizon count in.

/// A UTC day holds exactly 1920 of them.
pub const ROUND_US: i64 = 45_000_000;
/// A round in whole seconds.
pub const ROUND_S: u32 = (ROUND_US / 1_000_000) as u32;
/// The longest a node goes without sending, whether or not it has anything new: a little under
/// three rounds, so that a node listening for three rounds hears every node in reach.
pub const FLOOR_US: i64 = 3 * ROUND_US - 5_000_000;
/// How far a packet's start may sit from where the node's clock puts it and still refine that
/// clock at once, for the sender's and the node's own clock error.
pub const GUARD_US: i64 = 250_000;
/// After each transmission a node is silent for this many times its airtime, which keeps it
/// under band O's 10% at any moment.
pub const REST_TIMES: i64 = 9;
/// Every round whose index is a multiple of this is a sweep round: members on old keys are sent
/// their key messages in it, and a node's word that it is on the group's key goes out in it for
/// a day.
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

/// Whether `round` is a sweep round.
#[must_use]
pub fn is_sweep_round(round: i64) -> bool {
    round.rem_euclid(SWEEP_EVERY) == 0
}

/// A packet's time on air, for spreading factor 7 at 125 kHz, coding rate 4/5, an explicit header,
/// CRC on and an 8-symbol preamble (Semtech AN1200.13).
#[must_use]
pub const fn airtime_us(len: usize) -> i64 {
    const SYMBOL_US: i64 = 1_024;
    const SF: i64 = 7;
    let bits = 8 * len as i64 - 4 * SF + 28 + 16;
    let blocks = (bits + 4 * SF - 1).div_euclid(4 * SF);
    let payload_symbols = 8 + if blocks > 0 { blocks } else { 0 } * 5;
    // The preamble's 8 symbols and the 4.25 the modem adds.
    49 * SYMBOL_US / 4 + payload_symbols * SYMBOL_US
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_day_holds_whole_rounds() {
        assert_eq!(86_400_000_000 % ROUND_US, 0);
    }

    #[test]
    fn airtime_matches_the_protocol_table() {
        assert_eq!(airtime_us(255), 399_616);
        for (len, ms) in [
            (36, 77),
            (48, 98),
            (57, 108),
            (111, 190),
            (184, 297),
            (248, 389),
        ] {
            assert_eq!((airtime_us(len) + 500) / 1000, ms, "{len} bytes");
        }
    }
}
