//! When the screen rests: the timeout, the dim, and the always-on face or the panel off.
//! Section 3 of the round 3 spec is its design.

use super::gesture::Micros;

/// How long the screen stays lit without use.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Timeout {
    Seconds15,
    Seconds30,
    #[default]
    Minute1,
    Minutes5,
    Never,
}

impl Timeout {
    pub const ALL: [Self; 5] = [
        Self::Seconds15,
        Self::Seconds30,
        Self::Minute1,
        Self::Minutes5,
        Self::Never,
    ];

    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Seconds15 => "15 S",
            Self::Seconds30 => "30 S",
            Self::Minute1 => "1 MIN",
            Self::Minutes5 => "5 MIN",
            Self::Never => "NEVER",
        }
    }

    /// What the settings store keeps: the timeout in seconds, 0 for never.
    #[must_use]
    pub fn seconds(self) -> u16 {
        self.duration()
            .map_or(0, |duration| (duration / 1_000_000) as u16)
    }

    #[must_use]
    pub fn from_seconds(seconds: u16) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|timeout| timeout.seconds() == seconds)
    }

    #[must_use]
    pub fn duration(self) -> Option<Micros> {
        match self {
            Self::Seconds15 => Some(15_000_000),
            Self::Seconds30 => Some(30_000_000),
            Self::Minute1 => Some(60_000_000),
            Self::Minutes5 => Some(300_000_000),
            Self::Never => None,
        }
    }
}

/// Where the screen is on its way to rest.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Rest {
    #[default]
    Awake,
    /// Dimmed since then, showing what it showed.
    Dimmed {
        since: Micros,
    },
    /// Fading to dark since then, on the way to off.
    Darkening {
        since: Micros,
    },
    AlwaysOn,
    Off,
}

/// How long the dim lasts before the always-on face or the panel off, counted from its start.
pub const DIM_HOLD: Micros = 5_000_000;
/// How long the level takes to fade down to the dim, from the dim to dark before the panel
/// goes off, and back up after a wake.
pub const DIM_FADE: Micros = 500_000;
pub const OFF_FADE: Micros = 300_000;
pub const WAKE_FADE: Micros = 250_000;
/// How long the panel takes to leave sleep: 120 ms after its sleep-out command, then its
/// display-on. A wake from off starts the page's entry and the fade up after it, so neither
/// runs on a dark panel.
pub const PANEL_WAKE: Micros = 140_000;
/// A heading change of more than this, in tenths of a degree since the timer last restarted,
/// restarts it while the compass shows.
pub const HEADING_RESTART: u16 = 100;

/// 30 % of the set level, but not below 3 % of full, nor above the set level.
#[must_use]
pub fn dim_level(set: u8) -> u8 {
    const FLOOR: u16 = 8;
    let dimmed = (u16::from(set) * 30 + 50) / 100;
    dimmed.max(FLOOR).min(u16::from(set)) as u8
}

/// The display's level for `percent` of full.
#[must_use]
pub fn level_of(percent: u8) -> u8 {
    ((255 * u16::from(percent) + 50) / 100) as u8
}

/// Whether the screen rests on the always-on face, and how bright it is there.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum AlwaysOn {
    #[default]
    Off,
    /// The level the timeout dims to, so it follows the brightness.
    Dim,
    /// A fixed percentage of full, in [`AlwaysOn::PERCENTS`].
    Percent(u8),
}

impl AlwaysOn {
    pub const PERCENTS: core::ops::RangeInclusive<u8> = 5..=50;
    /// Off, the dim level, then each percentage.
    pub const CHOICES: usize = 2 + (*Self::PERCENTS.end() - *Self::PERCENTS.start() + 1) as usize;

    #[must_use]
    pub fn choice(index: usize) -> Self {
        match index {
            0 => Self::Off,
            1 => Self::Dim,
            _ => Self::Percent(*Self::PERCENTS.start() + (index - 2) as u8),
        }
    }

    #[must_use]
    pub fn index(self) -> usize {
        match self {
            Self::Off => 0,
            Self::Dim => 1,
            Self::Percent(percent) => 2 + usize::from(percent - *Self::PERCENTS.start()),
        }
    }

    #[must_use]
    pub fn is_on(self) -> bool {
        self != Self::Off
    }

    /// The face's level when the screen's is `set`.
    #[must_use]
    pub fn level(self, set: u8) -> u8 {
        match self {
            Self::Off | Self::Dim => dim_level(set),
            Self::Percent(percent) => level_of(percent),
        }
    }

    /// The stored byte: 0 off, 1 the dim level, or the percentage. Settings saved as on or
    /// off, before there was a level, read as the dim level or off.
    #[must_use]
    pub fn to_byte(self) -> u8 {
        match self {
            Self::Off => 0,
            Self::Dim => 1,
            Self::Percent(percent) => percent,
        }
    }

    #[must_use]
    pub fn from_byte(byte: u8) -> Option<Self> {
        match byte {
            0 => Some(Self::Off),
            1 => Some(Self::Dim),
            percent if Self::PERCENTS.contains(&percent) => Some(Self::Percent(percent)),
            _ => None,
        }
    }
}

/// A change of level spread over a time.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Fade {
    pub from: u8,
    pub to: u8,
    pub start: Micros,
    pub duration: Micros,
}

impl Fade {
    #[must_use]
    pub fn level(&self, now: Micros) -> u8 {
        let (from, to) = (i64::from(self.from), i64::from(self.to));
        let t = now.saturating_sub(self.start).min(self.duration) as i64;
        (from + (to - from) * t / self.duration as i64) as u8
    }

    #[must_use]
    pub fn done(&self, now: Micros) -> bool {
        now >= self.start + self.duration
    }
}

/// How far apart two headings are, in tenths of a degree, the short way round.
#[must_use]
pub fn heading_apart(a: u16, b: u16) -> u16 {
    let d = a.abs_diff(b) % 3600;
    d.min(3600 - d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_dim_is_30_percent_with_a_floor() {
        assert_eq!(dim_level(255), 77);
        assert_eq!(dim_level(26), 8);
        assert_eq!(dim_level(5), 5);
    }

    #[test]
    fn a_fade_runs_from_one_level_to_the_other() {
        let fade = Fade {
            from: 26,
            to: 200,
            start: 1_000,
            duration: 100_000,
        };
        assert_eq!(fade.level(0), 26);
        assert_eq!(fade.level(51_000), 113);
        assert_eq!(fade.level(101_000), 200);
        assert_eq!(fade.level(500_000), 200);
        assert!(!fade.done(100_999) && fade.done(101_000));
    }

    #[test]
    fn a_timeout_survives_the_store() {
        for timeout in Timeout::ALL {
            assert_eq!(Timeout::from_seconds(timeout.seconds()), Some(timeout));
        }
        assert_eq!(Timeout::from_seconds(7), None);
    }

    #[test]
    fn headings_compare_the_short_way() {
        assert_eq!(heading_apart(3590, 50), 60);
        assert_eq!(heading_apart(100, 300), 200);
    }
}
