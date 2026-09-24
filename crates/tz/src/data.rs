//! The zone table and boundaries, as `tools/tz-data.py` writes them.
//!
//! Little-endian throughout. A 48-byte header: `OWTZ`, the format version (u16), the zone and rule
//! counts (u16 each), two bytes of padding, the offsets of the zone table, the change table, the
//! string pool and the boundaries (u32 each), the 1e-7 degree units in one boundary grid step
//! (u32), and the tzdata and boundary release names (8 bytes each, NUL padded).
//!
//! The rule table follows the header: per rule, its string's offset in the pool (u16) and length
//! (u8). Each zone takes 32 bytes: its name's offset and length (u16, u8), its rule's index (u8),
//! its bounding box as west, south, east and north in grid steps (i32 each), its boundary's
//! offset and length (u32 each), and the index and count of its changes (u16 each). A zone with no
//! boundary has an empty box.
//!
//! A few zones change their clocks on dates no rule describes, and list those changes ahead of
//! the rule. Each takes 16 bytes: its Unix time (i64), the UTC offset after it (i32), whether that
//! is daylight saving time (u8), and its abbreviation's length (u8) and offset in the pool (u16).
//! The first is the state at the start of the data's range rather than a change. The rule governs
//! from the last one on.
//!
//! A boundary is a varint ring count, then per ring a varint vertex count and each vertex as
//! zigzag varint longitude and latitude steps from the vertex before, starting from 0, 0 and
//! running on across rings. Rings close back to their first vertex. A point is in the zone when it
//! is inside an odd number of its rings.

use crate::rule::{Offset, Rule};

const HEADER: usize = 48;
const RULE_RECORD: usize = 3;
const ZONE_RECORD: usize = 32;
const CHANGE_RECORD: usize = 16;

/// A zone table and its boundaries.
#[derive(Clone, Copy)]
pub struct Database {
    data: &'static [u8],
}

/// A zone's place in its [`Database`]. It changes when the data is rebuilt, so store the zone's
/// name instead.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct ZoneId(u16);

impl ZoneId {
    #[must_use]
    pub const fn index(self) -> usize {
        self.0 as usize
    }
}

/// One zone of a [`Database`].
#[derive(Clone, Copy, Debug)]
pub struct Zone {
    pub id: ZoneId,
    /// The IANA name, such as `Europe/Dublin`.
    pub name: &'static str,
    rule: &'static str,
    changes: &'static [u8],
    pool: &'static [u8],
    /// West, south, east and north, in grid steps.
    bounds: [i32; 4],
    boundary: &'static [u8],
}

impl Zone {
    /// The zone's current rule.
    #[must_use]
    pub fn rule(&self) -> Rule<'static> {
        Rule::parse(self.rule).expect("the generator writes only rules that parse")
    }

    /// What the clocks read at `unix`, in seconds since 1970-01-01 UTC.
    #[must_use]
    pub fn at(&self, unix: i64) -> Offset<'static> {
        let count = self.changes.len() / CHANGE_RECORD;
        let time = |index: usize| {
            let at = index * CHANGE_RECORD;
            i64::from_le_bytes(self.changes[at..at + 8].try_into().expect("eight bytes"))
        };
        if count == 0 || unix >= time(count - 1) {
            return self.rule().at(unix);
        }
        // The last listed state at or before `unix`, or the first for a time before them all.
        let (mut low, mut high) = (0, count - 1);
        while high - low > 1 {
            let middle = (low + high) / 2;
            if time(middle) <= unix {
                low = middle;
            } else {
                high = middle;
            }
        }
        let index = low;
        let at = index * CHANGE_RECORD;
        let record = &self.changes[at..at + CHANGE_RECORD];
        let offset = usize::from(u16::from_le_bytes([record[14], record[15]]));
        let abbreviation = &self.pool[offset..offset + usize::from(record[13])];
        Offset {
            utc_offset: i32::from_le_bytes(record[8..12].try_into().expect("four bytes")),
            dst: record[12] != 0,
            abbreviation: core::str::from_utf8(abbreviation).expect("the generator writes ASCII"),
        }
    }

    /// The rule as its POSIX `TZ` string.
    #[must_use]
    pub fn rule_text(&self) -> &'static str {
        self.rule
    }
}

impl Database {
    /// Wraps data `tools/tz-data.py` wrote.
    ///
    /// # Panics
    ///
    /// If `data` does not start with the header of this format version.
    #[must_use]
    pub const fn new(data: &'static [u8]) -> Self {
        assert!(data.len() >= HEADER);
        assert!(data[0] == b'O' && data[1] == b'W' && data[2] == b'T' && data[3] == b'Z');
        assert!(data[4] == 1 && data[5] == 0, "an unknown format version");
        Self { data }
    }

    const fn u16_at(&self, at: usize) -> u16 {
        u16::from_le_bytes([self.data[at], self.data[at + 1]])
    }

    const fn u32_at(&self, at: usize) -> u32 {
        u32::from_le_bytes([
            self.data[at],
            self.data[at + 1],
            self.data[at + 2],
            self.data[at + 3],
        ])
    }

    fn i32_at(&self, at: usize) -> i32 {
        self.u32_at(at) as i32
    }

    #[must_use]
    pub const fn len(&self) -> usize {
        self.u16_at(6) as usize
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn rule_count(&self) -> usize {
        self.u16_at(8) as usize
    }

    fn pool(&self) -> &'static [u8] {
        let data: &'static [u8] = self.data;
        &data[self.u32_at(20) as usize..self.u32_at(24) as usize]
    }

    fn string(&self, offset: usize, length: usize) -> &'static str {
        core::str::from_utf8(&self.pool()[offset..offset + length])
            .expect("the generator writes ASCII")
    }

    fn release(&self, at: usize) -> &'static str {
        let data: &'static [u8] = self.data;
        let field = &data[at..at + 8];
        let end = field
            .iter()
            .position(|&byte| byte == 0)
            .unwrap_or(field.len());
        core::str::from_utf8(&field[..end]).expect("the generator writes ASCII")
    }

    /// The IANA time zone database release the rules come from, such as `2026d`.
    #[must_use]
    pub fn tzdata_release(&self) -> &'static str {
        self.release(32)
    }

    /// The timezone-boundary-builder release the boundaries come from.
    #[must_use]
    pub fn boundary_release(&self) -> &'static str {
        self.release(40)
    }

    fn e7_per_step(&self) -> i64 {
        i64::from(self.u32_at(28))
    }

    /// The zone at `index`, in the table's order.
    ///
    /// # Panics
    ///
    /// If `index` is not below [`len`](Self::len).
    #[must_use]
    pub fn zone(&self, id: ZoneId) -> Zone {
        assert!(id.index() < self.len());
        let at = self.u32_at(12) as usize + id.index() * ZONE_RECORD;
        let rule = usize::from(self.data[at + 3]);
        assert!(rule < self.rule_count());
        let rule_at = HEADER + rule * RULE_RECORD;
        let boundary_at = self.u32_at(24) as usize + self.u32_at(at + 20) as usize;
        let changes_at =
            self.u32_at(16) as usize + usize::from(self.u16_at(at + 28)) * CHANGE_RECORD;
        let changes_end = changes_at + usize::from(self.u16_at(at + 30)) * CHANGE_RECORD;
        let data: &'static [u8] = self.data;
        Zone {
            id,
            name: self.string(usize::from(self.u16_at(at)), usize::from(self.data[at + 2])),
            rule: self.string(
                usize::from(self.u16_at(rule_at)),
                usize::from(self.data[rule_at + 2]),
            ),
            changes: &data[changes_at..changes_end],
            pool: self.pool(),
            bounds: [
                self.i32_at(at + 4),
                self.i32_at(at + 8),
                self.i32_at(at + 12),
                self.i32_at(at + 16),
            ],
            boundary: &data[boundary_at..boundary_at + self.u32_at(at + 24) as usize],
        }
    }

    /// Every zone, in the table's order.
    pub fn zones(&self) -> impl Iterator<Item = Zone> + '_ {
        (0..self.len()).map(|index| self.zone(ZoneId(index as u16)))
    }

    /// The zone named `name`.
    #[must_use]
    pub fn find(&self, name: &str) -> Option<Zone> {
        self.zones().find(|zone| zone.name == name)
    }

    /// Starts looking for the zone containing a position, given in 1e-7 degrees as the GNSS
    /// receiver reports it.
    #[must_use]
    pub fn locate(&self, latitude_e7: i32, longitude_e7: i32) -> Locate {
        Locate {
            database: *self,
            latitude: i64::from(latitude_e7),
            longitude: i64::from(longitude_e7),
            next: 0,
        }
    }

    /// The zone containing a position, found all at once.
    #[must_use]
    pub fn zone_at(&self, latitude_e7: i32, longitude_e7: i32) -> Option<Zone> {
        let mut locate = self.locate(latitude_e7, longitude_e7);
        loop {
            match locate.step() {
                Progress::Searching => {}
                Progress::Found(zone) => return Some(zone),
                Progress::Nowhere => return None,
            }
        }
    }
}

/// A search for the zone containing a position, a zone's boundary at a time, so that a caller
/// can yield between steps.
pub struct Locate {
    database: Database,
    latitude: i64,
    longitude: i64,
    next: usize,
}

#[derive(Clone, Copy, Debug)]
pub enum Progress {
    Searching,
    Found(Zone),
    /// Only in a sliver the simplified boundaries leave between zones.
    Nowhere,
}

impl Locate {
    /// Tests the next zone whose bounding box holds the position.
    pub fn step(&mut self) -> Progress {
        let scale = self.database.e7_per_step();
        while self.next < self.database.len() {
            let zone = self.database.zone(ZoneId(self.next as u16));
            self.next += 1;
            let [west, south, east, north] = zone.bounds.map(|bound| i64::from(bound) * scale);
            let boxed =
                (west..=east).contains(&self.longitude) && (south..=north).contains(&self.latitude);
            if !boxed {
                continue;
            }
            return if contains(zone.boundary, scale, self.longitude, self.latitude) {
                Progress::Found(zone)
            } else {
                Progress::Searching
            };
        }
        Progress::Nowhere
    }
}

fn varint(data: &mut &[u8]) -> u32 {
    let mut value = 0;
    for shift in (0..32).step_by(7) {
        let (&byte, rest) = data.split_first().expect("the generator ends every varint");
        *data = rest;
        value |= u32::from(byte & 0x7F) << shift;
        if byte & 0x80 == 0 {
            break;
        }
    }
    value
}

fn zigzag(data: &mut &[u8]) -> i64 {
    let value = varint(data);
    i64::from((value >> 1) as i32 ^ -((value & 1) as i32))
}

/// The even-odd test of `(x, y)` against every ring of `boundary`, exact in integers so that it
/// agrees with the generator's own.
fn contains(mut boundary: &[u8], scale: i64, x: i64, y: i64) -> bool {
    let rings = varint(&mut boundary);
    let (mut previous_x, mut previous_y) = (0, 0);
    let mut inside = false;
    for _ in 0..rings {
        let count = varint(&mut boundary);
        let mut next = || {
            previous_x += zigzag(&mut boundary);
            previous_y += zigzag(&mut boundary);
            (previous_x * scale, previous_y * scale)
        };
        let first = next();
        let mut a = first;
        for index in 1..=count {
            let b = if index == count { first } else { next() };
            if (a.1 > y) != (b.1 > y) {
                let d = i128::from(b.1 - a.1);
                let left = i128::from(x - a.0) * d;
                let right = i128::from(y - a.1) * i128::from(b.0 - a.0);
                if (d > 0 && left < right) || (d < 0 && left > right) {
                    inside = !inside;
                }
            }
            a = b;
        }
    }
    inside
}
