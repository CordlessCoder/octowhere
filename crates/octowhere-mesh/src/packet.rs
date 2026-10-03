//! The plaintext a packet seals: the header, then records, each a type byte, a length byte and a
//! body. `seal` puts the synthetic IV in front of it.

use crate::bits::{BitReader, BitWriter, Full};
use crate::members::{GONE_LEN, Gone, Member, RECORD_MAX_LEN, Slot};
use crate::messages::{BODY_MAX, FIXED_LEN, Message, Store};
use crate::seal::SIV_LEN;

pub const VERSION: u8 = 1;
/// The radio's largest payload.
pub const MAX_PACKET: usize = 255;
pub const HEADER_LEN: usize = 8;
/// The most plaintext a packet holds.
pub const MAX_PLAIN: usize = MAX_PACKET - SIV_LEN;

pub mod record {
    pub const POSITIONS: u8 = 1;
    pub const NEIGHBOURS: u8 = 2;
    pub const MEMBER: u8 = 3;
    pub const MESSAGE: u8 = 4;
    /// A digest of the sender's member table.
    pub const MEMBERS: u8 = 6;
    /// The ids whose member records the sender asks for.
    pub const REQUEST: u8 = 7;
    /// A digest of the messages the sender holds.
    pub const MESSAGES: u8 = 8;
    /// The messages the sender holds from each origin.
    pub const SUMMARY: u8 = 9;
    /// A member that left or was removed.
    pub const GONE: u8 = 10;
}

/// Where a node's clock comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Source {
    Gps,
    /// A clock the node with this id started, with no fix anywhere upstream.
    Node(u8),
}

impl Source {
    /// Whether nodes on `other` should move to this one: GPS ranks above any node's clock, and a
    /// lower root above a higher one.
    #[must_use]
    pub fn outranks(self, other: Self) -> bool {
        match (self, other) {
            (Self::Gps, Self::Node(_)) => true,
            (Self::Node(root), Self::Node(theirs)) => root < theirs,
            (_, Self::Gps) => false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct Timebase {
    pub source: Source,
    /// Receptions from the source: 0 for a node timing from its own fix, and for a root.
    pub hops: u8,
}

impl Timebase {
    pub const MAX_HOPS: u8 = 31;

    /// The timebase of a node that took its clock from a packet on this one.
    #[must_use]
    pub fn next_hop(self) -> Self {
        Self {
            source: self.source,
            hops: (self.hops + 1).min(Self::MAX_HOPS),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct Header {
    pub sender: u8,
    pub timebase: Timebase,
    /// The whole second, on the sender's timebase, that its slot starts in.
    pub base: u32,
    /// Reserved for CAD.
    pub phase: u8,
    /// The packet asks a node on a timebase ranked below the sender's to sweep. It is sent when
    /// that node listens for the sender, off the sender's own slot, so its arrival says nothing
    /// of the sender's timebase.
    pub notice: bool,
}

impl Header {
    fn encode(&self, out: &mut [u8; HEADER_LEN]) {
        let (source, root) = match self.timebase.source {
            Source::Gps => (0, 0),
            Source::Node(root) => (1, root),
        };
        let mut writer = BitWriter::new(out);
        for (value, bits) in [
            (u64::from(VERSION), 4),
            (u64::from(self.sender), 5),
            (source, 1),
            (u64::from(root), 5),
            (u64::from(self.timebase.hops), 5),
            (u64::from(self.notice), 4),
            (u64::from(self.base), 32),
            (u64::from(self.phase), 8),
        ] {
            writer
                .put(value, bits)
                .expect("the fields fill the header exactly");
        }
    }

    fn decode(bytes: &[u8]) -> Result<Self, Malformed> {
        let mut reader = BitReader::new(bytes.get(..HEADER_LEN).ok_or(Malformed::Short)?);
        let mut take = |bits| {
            reader
                .take(bits)
                .expect("the fields fill the header exactly")
        };
        if take(4) != u64::from(VERSION) {
            return Err(Malformed::Version);
        }
        let sender = take(5) as u8;
        let source = take(1);
        let root = take(5) as u8;
        let hops = take(5) as u8;
        let flags = take(4);
        Ok(Self {
            sender,
            timebase: Timebase {
                source: if source == 0 {
                    Source::Gps
                } else {
                    Source::Node(root)
                },
                hops,
            },
            base: take(32) as u32,
            phase: take(8) as u8,
            notice: flags & 1 != 0,
        })
    }
}

/// How a fix was made.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Quality {
    Autonomous,
    /// Corrected, by SBAS, DGPS or RTK.
    Differential,
    /// Not measured: estimated, entered or simulated.
    Estimated,
    Reserved,
}

/// Horizontal dilution of precision, to a log-scale step.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct Hdop(u8);

impl Hdop {
    /// Each step's upper bound in thousandths; the last step is anything above 13.
    const BOUNDS: [u32; 7] = [1_000, 1_500, 2_000, 3_000, 5_000, 8_000, 13_000];

    #[must_use]
    pub fn from_milli(milli: u32) -> Self {
        Self(
            Self::BOUNDS
                .iter()
                .take_while(|&&bound| milli > bound)
                .count() as u8,
        )
    }

    /// The step's upper bound in thousandths, or `None` for the last step.
    #[must_use]
    pub fn at_most_milli(self) -> Option<u32> {
        Self::BOUNDS.get(usize::from(self.0)).copied()
    }
}

/// A node's position as the table holds it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct Entry {
    pub id: u8,
    /// Degrees × 10⁷.
    pub latitude: i32,
    pub longitude: i32,
    /// UTC seconds of the fix, set by the node it describes and never changed by a relay.
    pub stamp: u32,
    pub quality: Quality,
    pub hdop: Hdop,
}

const LATITUDE_BITS: u32 = 25;
const LONGITUDE_BITS: u32 = 26;
const DELTA_BITS: u32 = 12;
const ENTRY_BITS: usize = 5 + 25 + 26 + 12 + 2 + 3;
/// How far below a packet's base timestamp an entry's stamp can be.
pub const MAX_DELTA: u32 = (1 << DELTA_BITS) - 1;

impl Entry {
    /// The entry with its coordinates moved to the centre of the step they encode to.
    #[must_use]
    pub fn quantized(self) -> Self {
        Self {
            latitude: decode_latitude(encode_latitude(self.latitude)),
            longitude: decode_longitude(encode_longitude(self.longitude)),
            ..self
        }
    }

    /// Whether the entry's stamp fits below `base`.
    #[must_use]
    pub fn fits_below(&self, base: u32) -> bool {
        base.checked_sub(self.stamp)
            .is_some_and(|delta| delta <= MAX_DELTA)
    }

    fn encode(&self, base: u32, writer: &mut BitWriter) -> Result<(), Full> {
        let quality = match self.quality {
            Quality::Autonomous => 0,
            Quality::Differential => 1,
            Quality::Estimated => 2,
            Quality::Reserved => 3,
        };
        writer.put(u64::from(self.id), 5)?;
        writer.put(encode_latitude(self.latitude), LATITUDE_BITS)?;
        writer.put(encode_longitude(self.longitude), LONGITUDE_BITS)?;
        writer.put(u64::from(base - self.stamp), DELTA_BITS)?;
        writer.put(quality, 2)?;
        writer.put(u64::from(self.hdop.0), 3)
    }

    fn decode(base: u32, reader: &mut BitReader) -> Option<Self> {
        let id = reader.take(5)? as u8;
        let latitude = decode_latitude(reader.take(LATITUDE_BITS)?);
        let longitude = decode_longitude(reader.take(LONGITUDE_BITS)?);
        let stamp = base.checked_sub(reader.take(DELTA_BITS)? as u32)?;
        let quality = match reader.take(2)? {
            0 => Quality::Autonomous,
            1 => Quality::Differential,
            2 => Quality::Estimated,
            _ => Quality::Reserved,
        };
        let hdop = Hdop(reader.take(3)? as u8);
        Some(Self {
            id,
            latitude,
            longitude,
            stamp,
            quality,
            hdop,
        })
    }
}

const LATITUDE_SPAN: i64 = 1_800_000_000;
const LONGITUDE_SPAN: i64 = 3_600_000_000;

fn encode_latitude(latitude: i32) -> u64 {
    let steps = 1i64 << LATITUDE_BITS;
    ((i64::from(latitude) + LATITUDE_SPAN / 2) * steps / LATITUDE_SPAN).clamp(0, steps - 1) as u64
}

fn decode_latitude(code: u64) -> i32 {
    let steps = 1i64 << LATITUDE_BITS;
    ((2 * code as i64 + 1) * LATITUDE_SPAN / (2 * steps) - LATITUDE_SPAN / 2) as i32
}

fn encode_longitude(longitude: i32) -> u64 {
    let steps = 1i64 << LONGITUDE_BITS;
    ((i64::from(longitude) + LONGITUDE_SPAN / 2) * steps / LONGITUDE_SPAN).rem_euclid(steps) as u64
}

fn decode_longitude(code: u64) -> i32 {
    let steps = 1i64 << LONGITUDE_BITS;
    ((2 * code as i64 + 1) * LONGITUDE_SPAN / (2 * steps) - LONGITUDE_SPAN / 2) as i32
}

/// The bytes a positions record of `entries` takes, its type and length included.
#[must_use]
pub fn positions_len(entries: usize) -> usize {
    2 + (entries * ENTRY_BITS).div_ceil(8)
}

/// The bytes a record of one 32-bit word takes: neighbours, a members digest or a request.
pub const WORD_LEN: usize = 6;
pub const NEIGHBOURS_LEN: usize = WORD_LEN;

/// Writes a packet's plaintext: the header, then records in the order they are added.
pub struct Builder<'a> {
    buf: &'a mut [u8],
    len: usize,
    base: u32,
}

impl<'a> Builder<'a> {
    /// Starts a plaintext in `buf`, which holds at most [`MAX_PLAIN`] of it.
    pub fn new(buf: &'a mut [u8], header: &Header) -> Self {
        let mut bytes = [0; HEADER_LEN];
        header.encode(&mut bytes);
        buf[..HEADER_LEN].copy_from_slice(&bytes);
        Self {
            buf,
            len: HEADER_LEN,
            base: header.base,
        }
    }

    /// The bytes left for records.
    #[must_use]
    pub fn room(&self) -> usize {
        self.buf.len().min(MAX_PLAIN) - self.len
    }

    /// The most entries a positions record can hold in what is left.
    #[must_use]
    pub fn room_for_entries(&self) -> usize {
        let room = self.room();
        if room < 2 {
            return 0;
        }
        ((room - 2) * 8 / ENTRY_BITS).min(255 * 8 / ENTRY_BITS)
    }

    fn word(&mut self, kind: u8, value: u32) -> Result<(), Full> {
        if self.room() < WORD_LEN {
            return Err(Full);
        }
        let at = self.len;
        self.buf[at] = kind;
        self.buf[at + 1] = 4;
        self.buf[at + 2..at + 6].copy_from_slice(&value.to_le_bytes());
        self.len += WORD_LEN;
        Ok(())
    }

    pub fn neighbours(&mut self, heard: u32) -> Result<(), Full> {
        self.word(record::NEIGHBOURS, heard)
    }

    pub fn members_digest(&mut self, digest: u32) -> Result<(), Full> {
        self.word(record::MEMBERS, digest)
    }

    /// Asks for the member records of the ids in the set `ids`.
    pub fn request(&mut self, ids: u32) -> Result<(), Full> {
        self.word(record::REQUEST, ids)
    }

    pub fn messages_digest(&mut self, digest: u32) -> Result<(), Full> {
        self.word(record::MESSAGES, digest)
    }

    /// Writes a summary of what `store` holds, as much of it as fits in the room left less
    /// `spare`.
    pub fn summary(&mut self, store: &Store, spare: usize) -> Result<(), Full> {
        let room = self.room().saturating_sub(spare);
        if room < 2 + 4 {
            return Err(Full);
        }
        let at = self.len;
        let end = at + 2 + (room - 2).min(255);
        let len = store.summary(&mut self.buf[at + 2..end]);
        self.buf[at] = record::SUMMARY;
        self.buf[at + 1] = len as u8;
        self.len += 2 + len;
        Ok(())
    }

    pub fn message(&mut self, message: &Message) -> Result<(), Full> {
        if message.record_len() > self.room() {
            return Err(Full);
        }
        let mut body = [0; FIXED_LEN + BODY_MAX];
        let len = message.encode(&mut body);
        let at = self.len;
        self.buf[at] = record::MESSAGE;
        self.buf[at + 1] = len as u8;
        self.buf[at + 2..at + 2 + len].copy_from_slice(&body[..len]);
        self.len += 2 + len;
        Ok(())
    }

    /// Writes a positions record of `entries`, each of which must fit below the base timestamp.
    pub fn positions(&mut self, entries: &[Entry]) -> Result<(), Full> {
        let len = positions_len(entries.len());
        if entries.is_empty() || len > self.room() || len - 2 > 255 {
            return Err(Full);
        }
        let at = self.len;
        self.buf[at] = record::POSITIONS;
        self.buf[at + 1] = (len - 2) as u8;
        let mut writer = BitWriter::new(&mut self.buf[at + 2..at + len]);
        for entry in entries {
            assert!(
                entry.fits_below(self.base),
                "an entry must fit below the base"
            );
            entry.encode(self.base, &mut writer)?;
        }
        self.len += len;
        Ok(())
    }

    /// Writes a member record for `id`.
    pub fn member(&mut self, id: u8, member: &Member) -> Result<(), Full> {
        let mut body = [0; RECORD_MAX_LEN];
        let len = member.encode(id, &mut body);
        if 2 + len > self.room() {
            return Err(Full);
        }
        let at = self.len;
        self.buf[at] = record::MEMBER;
        self.buf[at + 1] = len as u8;
        self.buf[at + 2..at + 2 + len].copy_from_slice(&body[..len]);
        self.len += 2 + len;
        Ok(())
    }

    /// Writes a gone record for `id`.
    pub fn gone(&mut self, id: u8, gone: &Gone) -> Result<(), Full> {
        if 2 + GONE_LEN > self.room() {
            return Err(Full);
        }
        let mut body = [0; GONE_LEN];
        gone.encode(id, &mut body);
        let at = self.len;
        self.buf[at] = record::GONE;
        self.buf[at + 1] = GONE_LEN as u8;
        self.buf[at + 2..at + 2 + GONE_LEN].copy_from_slice(&body);
        self.len += 2 + GONE_LEN;
        Ok(())
    }

    /// Writes what the slot at `id` holds: a member record or a gone record.
    pub fn slot(&mut self, id: u8, slot: &Slot) -> Result<(), Full> {
        match slot {
            Slot::Member(member) => self.member(id, member),
            Slot::Gone(gone) => self.gone(id, gone),
        }
    }

    /// The plaintext's length.
    #[must_use]
    pub fn finish(self) -> usize {
        self.len
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Malformed {
    Short,
    Version,
    /// A record runs past the end of the packet.
    Record,
}

/// A packet's plaintext, read.
pub struct Plain<'a> {
    pub header: Header,
    records: &'a [u8],
}

impl<'a> Plain<'a> {
    pub fn parse(plain: &'a [u8]) -> Result<Self, Malformed> {
        let header = Header::decode(plain)?;
        let records = &plain[HEADER_LEN..];
        let mut rest = records;
        while !rest.is_empty() {
            let len = *rest.get(1).ok_or(Malformed::Record)? as usize;
            rest = rest.get(2 + len..).ok_or(Malformed::Record)?;
        }
        Ok(Self { header, records })
    }

    #[must_use]
    pub fn records(&self) -> Records<'a> {
        Records {
            rest: self.records,
            base: self.header.base,
        }
    }
}

pub enum Record<'a> {
    Positions(Positions<'a>),
    /// The ids the sender heard in its recent rounds.
    Neighbours(u32),
    /// A digest of the sender's member table.
    Members(u32),
    /// The ids whose member records the sender asks for.
    Request(u32),
    Member(u8, Member),
    Gone(u8, Gone),
    Message(Message),
    /// A digest of the messages the sender holds.
    Messages(u32),
    /// What the sender holds from each origin, as [`Store::summary`] writes it.
    Summary(&'a [u8]),
    /// A record of a type this version does not read, or one too short for its type.
    Other(u8, &'a [u8]),
}

pub struct Records<'a> {
    rest: &'a [u8],
    base: u32,
}

impl<'a> Iterator for Records<'a> {
    type Item = Record<'a>;

    fn next(&mut self) -> Option<Record<'a>> {
        let (&kind, rest) = self.rest.split_first()?;
        let (&len, rest) = rest.split_first()?;
        let (body, rest) = rest.split_at(usize::from(len));
        self.rest = rest;
        Some(match kind {
            record::POSITIONS => Record::Positions(Positions {
                reader: BitReader::new(body),
                left: body.len() * 8 / ENTRY_BITS,
                base: self.base,
            }),
            record::NEIGHBOURS | record::MEMBERS | record::REQUEST | record::MESSAGES
                if body.len() >= 4 =>
            {
                let word = u32::from_le_bytes(body[..4].try_into().expect("four bytes"));
                match kind {
                    record::NEIGHBOURS => Record::Neighbours(word),
                    record::MEMBERS => Record::Members(word),
                    record::MESSAGES => Record::Messages(word),
                    _ => Record::Request(word),
                }
            }
            record::MESSAGE => match Message::decode(body) {
                Some(message) => Record::Message(message),
                None => Record::Other(kind, body),
            },
            record::SUMMARY => Record::Summary(body),
            record::MEMBER => match Member::decode(body) {
                Some((id, member)) => Record::Member(id, member),
                None => Record::Other(kind, body),
            },
            record::GONE => match Gone::decode(body) {
                Some((id, gone)) => Record::Gone(id, gone),
                None => Record::Other(kind, body),
            },
            _ => Record::Other(kind, body),
        })
    }
}

pub struct Positions<'a> {
    reader: BitReader<'a>,
    left: usize,
    base: u32,
}

impl Iterator for Positions<'_> {
    type Item = Entry;

    fn next(&mut self) -> Option<Entry> {
        while self.left > 0 {
            self.left -= 1;
            // An entry whose delta reaches below 1970 cannot be a real one, and is skipped.
            if let Some(entry) = Entry::decode(self.base, &mut self.reader) {
                return Some(entry);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header() -> Header {
        Header {
            sender: 24,
            timebase: Timebase {
                source: Source::Node(3),
                hops: 2,
            },
            base: 1_790_000_000,
            phase: 0,
            notice: false,
        }
    }

    #[test]
    fn a_digest_and_a_request_survive_the_packet() {
        let mut buf = [0; MAX_PLAIN];
        let mut builder = Builder::new(&mut buf, &header());
        builder.members_digest(0xdead_beef).unwrap();
        builder.request(1 << 4 | 1 << 31).unwrap();
        let len = builder.finish();
        let plain = Plain::parse(&buf[..len]).unwrap();
        let mut records = plain.records();
        assert!(matches!(records.next(), Some(Record::Members(0xdead_beef))));
        assert!(matches!(records.next(), Some(Record::Request(0x8000_0010))));
        assert!(records.next().is_none());
    }

    #[test]
    fn a_notice_survives_the_header() {
        let header = Header {
            notice: true,
            ..header()
        };
        let mut bytes = [0; HEADER_LEN];
        header.encode(&mut bytes);
        assert_eq!(Header::decode(&bytes), Ok(header));
        let mut bytes = [0; HEADER_LEN];
        self::header().encode(&mut bytes);
        assert!(!Header::decode(&bytes).unwrap().notice);
    }

    fn entry(id: u8, stamp: u32) -> Entry {
        Entry {
            id,
            latitude: 533_498_000,
            longitude: -62_603_000,
            stamp,
            quality: Quality::Differential,
            hdop: Hdop::from_milli(1_200),
        }
    }

    #[test]
    fn a_packet_reads_back_as_it_was_written() {
        let mut buf = [0; MAX_PLAIN];
        let mut builder = Builder::new(&mut buf, &header());
        builder.neighbours(0x8000_0011).unwrap();
        let entries = [entry(24, 1_790_000_000), entry(3, 1_789_996_000)];
        builder.positions(&entries).unwrap();
        let len = builder.finish();
        assert_eq!(len, HEADER_LEN + NEIGHBOURS_LEN + positions_len(2));
        let plain = Plain::parse(&buf[..len]).unwrap();
        assert_eq!(plain.header, header());
        let mut records = plain.records();
        assert!(matches!(
            records.next(),
            Some(Record::Neighbours(0x8000_0011))
        ));
        let Some(Record::Positions(read)) = records.next() else {
            panic!("no positions record")
        };
        let read: [Entry; 2] = {
            let mut read = read;
            [read.next().unwrap(), read.next().unwrap()]
        };
        assert_eq!(read, entries.map(Entry::quantized));
        assert!(records.next().is_none());
    }

    #[test]
    fn a_member_record_reads_back() {
        let member = crate::members::tests::member(4, 1_790_000_001);
        let mut buf = [0; MAX_PLAIN];
        let mut builder = Builder::new(&mut buf, &header());
        builder.member(12, &member).unwrap();
        let len = builder.finish();
        let plain = Plain::parse(&buf[..len]).unwrap();
        assert!(matches!(
            plain.records().next(),
            Some(Record::Member(12, read)) if read == member
        ));
    }

    #[test]
    fn messages_and_a_summary_read_back() {
        use crate::Zeroable;
        use crate::messages::{Store, kind};
        extern crate std;

        let message = Message::to_group(3, 9, 8, 1_790_000_000, &[kind::TEXT, b'o', b'k']).unwrap();
        let mut store = std::boxed::Box::new(Store::zeroed());
        store.insert(message, 1_790_000_000);
        let mut buf = [0; MAX_PLAIN];
        let mut builder = Builder::new(&mut buf, &header());
        builder.messages_digest(store.digest()).unwrap();
        builder.summary(&store, 0).unwrap();
        builder.message(&message).unwrap();
        let len = builder.finish();
        assert_eq!(
            len,
            HEADER_LEN + WORD_LEN + 2 + 4 + 10 + message.record_len()
        );
        let plain = Plain::parse(&buf[..len]).unwrap();
        let mut records = plain.records();
        assert!(matches!(records.next(), Some(Record::Messages(d)) if d == store.digest()));
        let Some(Record::Summary(summary)) = records.next() else {
            panic!("no summary");
        };
        let mut other = std::boxed::Box::new(Store::zeroed());
        other.insert(message, 1_790_000_000);
        other.sent((3, 9));
        other.answer(summary);
        assert!(!other.has_unsent(), "it holds what the summary says");
        assert!(matches!(records.next(), Some(Record::Message(read)) if read == message));
        assert!(records.next().is_none());
    }

    #[test]
    fn a_gone_record_reads_back() {
        let gone = Gone {
            public: [3; 32],
            changed: 1_790_000_002,
        };
        let mut buf = [0; MAX_PLAIN];
        let mut builder = Builder::new(&mut buf, &header());
        builder.slot(30, &Slot::Gone(gone)).unwrap();
        let len = builder.finish();
        assert_eq!(len, HEADER_LEN + 2 + GONE_LEN);
        let plain = Plain::parse(&buf[..len]).unwrap();
        assert!(matches!(
            plain.records().next(),
            Some(Record::Gone(30, read)) if read == gone
        ));
    }

    #[test]
    fn a_gps_header_round_trips() {
        let mut header = header();
        header.timebase = Timebase {
            source: Source::Gps,
            hops: 0,
        };
        let mut bytes = [0; HEADER_LEN];
        header.encode(&mut bytes);
        assert_eq!(Header::decode(&bytes), Ok(header));
    }

    #[test]
    fn coordinates_keep_to_a_metre() {
        for (latitude, longitude) in [
            (900_000_000, 1_800_000_000),
            (-900_000_000, -1_800_000_000),
            (0, 0),
            (533_498_123, -62_603_456),
            (-337_000_001, 1_512_000_009),
        ] {
            let quantized = entry(0, 0);
            let quantized = Entry {
                latitude,
                longitude,
                ..quantized
            }
            .quantized();
            // 1e-7 degrees is about 1.1 cm.
            assert!((i64::from(quantized.latitude) - i64::from(latitude)).abs() <= 30);
            let lon_error =
                (i64::from(quantized.longitude) - i64::from(longitude)).rem_euclid(LONGITUDE_SPAN);
            assert!(lon_error.min(LONGITUDE_SPAN - lon_error) <= 30);
        }
    }

    #[test]
    fn twenty_four_entries_fill_a_packet() {
        let mut buf = [0; MAX_PLAIN];
        let mut builder = Builder::new(&mut buf, &header());
        builder.neighbours(0).unwrap();
        assert_eq!(builder.room_for_entries(), 24);
        let entries = [entry(1, 1_790_000_000); 24];
        builder.positions(&entries).unwrap();
        assert!(builder.finish() + SIV_LEN <= MAX_PACKET);
    }

    #[test]
    fn unknown_records_are_skipped_and_overruns_refused() {
        let mut buf = [0; MAX_PLAIN];
        let len = Builder::new(&mut buf, &header()).finish();
        buf[len..len + 5].copy_from_slice(&[99, 3, 1, 2, 3]);
        let plain = Plain::parse(&buf[..len + 5]).unwrap();
        assert!(matches!(
            plain.records().next(),
            Some(Record::Other(99, [1, 2, 3]))
        ));
        assert_eq!(Plain::parse(&buf[..len + 4]).err(), Some(Malformed::Record));
        buf[0] ^= 0xf0;
        assert_eq!(Plain::parse(&buf[..len]).err(), Some(Malformed::Version));
    }

    #[test]
    fn hdop_steps_are_log_scale() {
        assert_eq!(Hdop::from_milli(900).at_most_milli(), Some(1_000));
        assert_eq!(Hdop::from_milli(1_000).at_most_milli(), Some(1_000));
        assert_eq!(Hdop::from_milli(2_600).at_most_milli(), Some(3_000));
        assert_eq!(Hdop::from_milli(20_000).at_most_milli(), None);
    }
}
