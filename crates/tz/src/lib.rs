//! Time zones for octowhere: which zone a position lies in, and what the clocks there read.
//!
//! The zones, their current rules and their simplified boundaries are built into the binary from
//! `data/zones.bin`, which `tools/tz-data.py` writes. Only each zone's current rule is kept, so a
//! time before the zone last changed its rules converts by today's.

#![no_std]

pub mod civil;
mod data;
mod references;
mod rule;

pub use civil::DateTime;
pub use data::{Database, Locate, Progress, Zone, ZoneId};
pub use rule::{Offset, Rule};

/// The zones built into the binary.
pub static DATABASE: Database = Database::new(include_bytes!("../data/zones.bin"));

/// The local date and time in `zone` at `unix`, in seconds since 1970-01-01 UTC.
#[must_use]
pub fn local(unix: i64, zone: &Zone) -> (DateTime, Offset<'static>) {
    let offset = zone.at(unix);
    (
        DateTime::from_unix(unix + i64::from(offset.utc_offset)),
        offset,
    )
}
