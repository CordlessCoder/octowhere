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
pub use data::{Database, Zone, ZoneId};
#[cfg(feature = "boundaries")]
pub use data::{Locate, Progress};
pub use rule::{Offset, Rule};

const ZONES: &[u8] = include_bytes!("../data/zones.bin");

/// The zones built into the binary.
#[cfg(feature = "boundaries")]
pub static DATABASE: Database = Database::new(ZONES);

/// The zones built into the binary, without their boundaries.
#[cfg(not(feature = "boundaries"))]
pub static DATABASE: Database = Database::new(&TABLES);

/// `zones.bin` up to its boundaries, copied out at compile time so that the boundaries, which
/// come last, are not built in.
#[cfg(not(feature = "boundaries"))]
static TABLES: [u8; data::tables_len(ZONES)] = data::prefix(ZONES);

/// The local date and time in `zone` at `unix`, in seconds since 1970-01-01 UTC.
#[must_use]
pub fn local(unix: i64, zone: &Zone) -> (DateTime, Offset<'static>) {
    let offset = zone.at(unix);
    (
        DateTime::from_unix(unix + i64::from(offset.utc_offset)),
        offset,
    )
}
