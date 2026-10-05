//! How the group screens write addresses, codes, countdowns and ages, and when each next
//! changes.

use core::fmt::Write as _;

use heapless::String;

use super::view::{At, Mac};
use crate::ui::gesture::Micros;

const SECOND: i64 = 1_000_000;

/// A hardware address as twelve hex digits.
#[must_use]
pub fn mac(mac: &Mac) -> String<12> {
    let mut text = String::new();
    for byte in mac {
        _ = write!(text, "{byte:02X}");
    }
    text
}

/// A six-digit code as two groups of three, leading zeros kept.
#[must_use]
pub fn code(code: u32) -> String<7> {
    let mut text = String::new();
    _ = write!(text, "{:03} {:03}", code / 1000 % 1000, code % 1000);
    text
}

/// The time left before `deadline`, in whole seconds rounded up, as minutes and seconds, and
/// when that next changes.
#[must_use]
pub fn countdown(deadline: At, now: Micros) -> (String<5>, Option<Micros>) {
    let left = (deadline - now as i64).max(0);
    let seconds = (left + SECOND - 1) / SECOND;
    let mut text = String::new();
    _ = write!(text, "{:02}:{:02}", (seconds / 60).min(99), seconds % 60);
    let next = (seconds > 0).then(|| (deadline - (seconds - 1) * SECOND).max(0) as Micros);
    (text, next)
}

/// How long ago `since` was, as `08S`, `05M 20S`, `40M`, `02H 15M` or `03D`, and when that next
/// changes.
#[must_use]
pub fn age(since: At, now: Micros) -> (String<8>, Micros) {
    let elapsed = (now as i64 - since).max(0);
    let seconds = elapsed / SECOND;
    let (minutes, hours, days) = (seconds / 60, seconds / 3600, seconds / 86_400);
    let mut text = String::new();
    // The unit the text counts in, which it next changes after.
    let unit = if seconds < 60 {
        _ = write!(text, "{seconds:02}S");
        1
    } else if seconds < 600 {
        _ = write!(text, "{minutes:02}M {:02}S", seconds % 60);
        1
    } else if seconds < 3600 {
        _ = write!(text, "{minutes:02}M");
        60
    } else if days < 1 {
        _ = write!(text, "{hours:02}H {:02}M", minutes % 60);
        60
    } else {
        _ = write!(text, "{:02}D", days.min(999));
        86_400
    };
    let next = since + (seconds / unit + 1) * unit * SECOND;
    (text, next.max(0) as Micros)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_code_keeps_its_leading_zeros() {
        assert_eq!(code(4_731), "004 731");
        assert_eq!(code(482_731), "482 731");
        assert_eq!(code(0), "000 000");
    }

    #[test]
    fn an_address_is_twelve_hex_digits() {
        assert_eq!(mac(&[0x48, 0xa1, 0xb2, 0xc3, 0x8c, 0x91]), "48A1B2C38C91");
    }

    #[test]
    fn a_countdown_rounds_up_and_says_when_it_ticks() {
        let deadline = 108 * SECOND;
        assert_eq!(
            countdown(deadline, 0),
            ("01:48".try_into().unwrap(), Some(1_000_000))
        );
        assert_eq!(countdown(deadline, 500_000).0, "01:48");
        assert_eq!(countdown(deadline, 1_000_000).0, "01:47");
        assert_eq!(
            countdown(deadline, 108_000_000),
            ("00:00".try_into().unwrap(), None)
        );
        assert_eq!(countdown(deadline, 200_000_000).0, "00:00");
    }

    #[test]
    fn an_age_counts_in_the_unit_it_shows() {
        let at = |seconds: i64| (seconds * SECOND) as Micros;
        assert_eq!(age(0, at(8)), ("08S".try_into().unwrap(), at(9)));
        assert_eq!(age(0, at(320)), ("05M 20S".try_into().unwrap(), at(321)));
        assert_eq!(age(0, at(2_430)), ("40M".try_into().unwrap(), at(2_460)));
        assert_eq!(
            age(0, at(8_100)),
            ("02H 15M".try_into().unwrap(), at(8_160))
        );
        assert_eq!(age(0, at(90_000)), ("01D".try_into().unwrap(), at(172_800)));
        // Before the clock started.
        assert_eq!(age(-30 * SECOND, at(0)).0, "30S");
    }
}
