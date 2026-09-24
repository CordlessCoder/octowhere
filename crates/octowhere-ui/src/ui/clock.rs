//! Time as the screens receive it: UTC from the real-time clock, and the zone to show it in.

pub use octowhere_tz::{DateTime, Offset, ZoneId};
use octowhere_tz::DATABASE;

/// The real-time clock's last reading.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ClockState {
    /// Seconds since 1970-01-01 UTC, or `None` when the clock could not be read.
    pub utc: Option<i64>,
    /// GNSS has set the clock since the firmware started.
    pub set_from_gnss: bool,
    /// The clock's oscillator stopped since it was last set, so its time is unreliable until GNSS
    /// sets it again.
    pub stopped: bool,
}

/// How the zone the clocks show is chosen.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ZoneMode {
    /// The zone GNSS last placed the device in.
    #[default]
    Automatic,
    /// The zone the user chose.
    Manual,
}

/// The zone the clocks show.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ZoneState {
    pub mode: ZoneMode,
    /// `None` only in automatic mode before any fix has placed the device.
    pub zone: Option<ZoneId>,
}

/// What the clocks read in a zone.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LocalTime {
    pub time: DateTime,
    pub offset: Offset<'static>,
    /// The zone's IANA name, such as `Europe/Dublin`.
    pub zone: &'static str,
}

impl ClockState {
    /// The clock's time in UTC.
    #[must_use]
    pub fn utc_time(&self) -> Option<DateTime> {
        self.utc.map(DateTime::from_unix)
    }

    /// The clock's time in the zone `zones` names, or `None` if either is not known.
    #[must_use]
    pub fn local(&self, zones: ZoneState) -> Option<LocalTime> {
        let zone = DATABASE.zone(zones.zone?);
        let (time, offset) = octowhere_tz::local(self.utc?, &zone);
        Some(LocalTime {
            time,
            offset,
            zone: zone.name,
        })
    }
}
