//! Messages between members: text to one member or to the whole group, acknowledgements, and
//! what a removal sends. Every node holds every message for the horizon and passes on what a
//! neighbour lacks (`context/LORA-PROTOCOL.md`, "Messages").

use bytemuck::Zeroable;
use hkdf::Hkdf;
use sha2::{Digest, Sha256};
use x25519_dalek::{PublicKey, StaticSecret};

use crate::IDS;
use crate::members::{MISMATCHES, PUBLIC_LEN};
use crate::seal::{self, Inauthentic, Key, SIV_LEN};

/// How long every node holds a message, from its timestamp.
pub const HORIZON_S: u32 = 24 * 60 * 60;
/// A message stamped further ahead of a node's clock than this is refused.
pub const AHEAD_S: u32 = 60 * 60;
/// The most messages a store holds.
pub const CAPACITY: usize = 256;
pub const TEXT_MAX: usize = 160;
/// A body's most bytes: its kind, the longest text, and a private message's seal.
pub const BODY_MAX: usize = 1 + TEXT_MAX + SIV_LEN;
/// A message record's body before the message's own: origin, destination, sequence number,
/// previous, timestamp.
pub const FIXED_LEN: usize = 1 + 1 + 4 + 4 + 4;
/// The most a summary says of one origin's gaps.
const HOLES_MAX: usize = 8;
/// The sequence numbers a node reserves in flash at a time.
pub const BLOCK: u32 = 64;

const TO_GROUP: u8 = 0x80;
/// A key message, which nodes keep past the horizon while they keep the old key.
const KEY_MESSAGE: u8 = 0x40;
const ID_MASK: u8 = 0x1f;

/// What a body holds, in its first byte; sealed with the rest in a private message.
pub mod kind {
    pub const TEXT: u8 = 1;
    /// The sequence number of a private message the sender delivered.
    pub const ACK: u8 = 2;
    /// A new group key: see `rekey`.
    pub const KEY: u8 = 3;
    /// The destination was removed from the group.
    pub const REMOVED: u8 = 4;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum To {
    Group,
    /// One member, privately.
    Member(u8),
}

/// A message as every node holds it. A private message's body stays sealed for its
/// destination.
#[derive(Clone, Copy, PartialEq, Eq, Zeroable)]
#[repr(C)]
pub struct Message {
    /// Never 0, which marks an empty place in a store.
    pub seq: u32,
    /// The origin's sequence number before this one, 0 for none known.
    pub prev: u32,
    /// Timebase seconds the origin sent it at.
    pub stamp: u32,
    pub origin: u8,
    dest: u8,
    len: u8,
    body: [u8; BODY_MAX],
}

impl core::fmt::Debug for Message {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Message")
            .field("origin", &self.origin)
            .field("to", &self.to())
            .field("seq", &self.seq)
            .field("prev", &self.prev)
            .field("stamp", &self.stamp)
            .field("body", &self.body())
            .finish()
    }
}

#[cfg(feature = "defmt")]
impl defmt::Format for Message {
    fn format(&self, f: defmt::Formatter) {
        defmt::write!(
            f,
            "{}/{} to {} len={}",
            self.origin,
            self.seq,
            self.to(),
            self.len
        );
    }
}

/// What a node knows of a message by: its origin and sequence number.
pub type Name = (u8, u32);

impl Message {
    /// A message to the whole group, `body` its kind and what it holds. `None` when the body is
    /// too long or `seq` is 0.
    #[must_use]
    pub fn to_group(origin: u8, seq: u32, prev: u32, stamp: u32, body: &[u8]) -> Option<Self> {
        Self::new(origin, TO_GROUP, seq, prev, stamp, body)
    }

    /// A private message to `dest`, `plain` its kind and what it holds, sealed under the key
    /// the two members share. A key message is marked for keeping.
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn private(
        origin: u8,
        dest: u8,
        key_message: bool,
        seq: u32,
        prev: u32,
        stamp: u32,
        plain: &[u8],
        key: &Key,
    ) -> Option<Self> {
        if dest >= IDS || plain.len() + SIV_LEN > BODY_MAX {
            return None;
        }
        let dest = dest | if key_message { KEY_MESSAGE } else { 0 };
        let mut message = Self::new(origin, dest, seq, prev, stamp, &[])?;
        message.body[SIV_LEN..SIV_LEN + plain.len()].copy_from_slice(plain);
        let associated = message.associated();
        message.len = seal::seal_bound(key, &associated, &mut message.body, plain.len()) as u8;
        Some(message)
    }

    fn new(origin: u8, dest: u8, seq: u32, prev: u32, stamp: u32, body: &[u8]) -> Option<Self> {
        if origin >= IDS || seq == 0 || body.len() > BODY_MAX {
            return None;
        }
        let mut message = Self::zeroed();
        message.seq = seq;
        message.prev = prev;
        message.stamp = stamp;
        message.origin = origin;
        message.dest = dest;
        message.len = body.len() as u8;
        message.body[..body.len()].copy_from_slice(body);
        Some(message)
    }

    #[must_use]
    pub fn to(&self) -> To {
        if self.dest & TO_GROUP != 0 {
            To::Group
        } else {
            To::Member(self.dest & ID_MASK)
        }
    }

    /// Whether it carries a new group key, which nodes keep while they keep the old key.
    #[must_use]
    pub fn is_key(&self) -> bool {
        self.dest & KEY_MESSAGE != 0
    }

    #[must_use]
    pub fn name(&self) -> Name {
        (self.origin, self.seq)
    }

    /// The body as carried: a group message's kind and contents, or a private one's seal.
    #[must_use]
    pub fn body(&self) -> &[u8] {
        &self.body[..usize::from(self.len)]
    }

    /// Opens a private message under the key its two members share, returning its kind and
    /// contents in `out`.
    pub fn open<'a>(
        &self,
        key: &Key,
        out: &'a mut [u8; BODY_MAX],
    ) -> Result<&'a [u8], Inauthentic> {
        let len = usize::from(self.len);
        out[..len].copy_from_slice(self.body());
        seal::open_bound(key, &self.associated(), &mut out[..len])
    }

    /// What a private message's seal binds it to: its origin, destination and sequence number.
    fn associated(&self) -> [u8; 6] {
        let seq = self.seq.to_be_bytes();
        [self.origin, self.dest, seq[0], seq[1], seq[2], seq[3]]
    }

    /// The bytes its record takes in a packet, its type and length included.
    #[must_use]
    pub fn record_len(&self) -> usize {
        2 + FIXED_LEN + usize::from(self.len)
    }

    /// Writes the record's body and returns its length.
    pub fn encode(&self, out: &mut [u8]) -> usize {
        out[0] = self.origin;
        out[1] = self.dest;
        out[2..6].copy_from_slice(&self.seq.to_be_bytes());
        out[6..10].copy_from_slice(&self.prev.to_be_bytes());
        out[10..14].copy_from_slice(&self.stamp.to_be_bytes());
        out[FIXED_LEN..FIXED_LEN + usize::from(self.len)].copy_from_slice(self.body());
        FIXED_LEN + usize::from(self.len)
    }

    #[must_use]
    pub fn decode(record: &[u8]) -> Option<Self> {
        let fixed = record.get(..FIXED_LEN)?;
        let word =
            |at: usize| u32::from_be_bytes(fixed[at..at + 4].try_into().expect("four bytes"));
        Self::new(
            fixed[0],
            fixed[1] & (TO_GROUP | KEY_MESSAGE | ID_MASK),
            word(2),
            word(6),
            word(10),
            &record[FIXED_LEN..],
        )
    }

    /// The order a store keeps and sends messages in: oldest first, then by origin and number.
    fn order(&self) -> (u32, u8, u32) {
        (self.stamp, self.origin, self.seq)
    }

    fn is_past(&self, now: u32) -> bool {
        self.stamp.saturating_add(HORIZON_S) <= now
    }
}

/// Whether `text` can be a message's text: 1 to [`TEXT_MAX`] printable ASCII characters.
#[must_use]
pub fn is_text(text: &[u8]) -> bool {
    (1..=TEXT_MAX).contains(&text.len()) && text.iter().all(|&c| (0x20..0x7f).contains(&c))
}

/// The key two members' private messages are sealed under: 256 bits of HKDF-SHA256 over their
/// X25519 shared secret, bound to both public keys. `None` for a key whose shared secret is not
/// contributory.
#[must_use]
pub fn pairwise(me: &StaticSecret, theirs: &[u8; PUBLIC_LEN]) -> Option<Key> {
    let shared = me.diffie_hellman(&PublicKey::from(*theirs));
    if !shared.was_contributory() {
        return None;
    }
    let mine = PublicKey::from(me).to_bytes();
    let (low, high) = if mine <= *theirs {
        (mine, *theirs)
    } else {
        (*theirs, mine)
    };
    let mut key = [0; 32];
    Hkdf::<Sha256>::new(None, shared.as_bytes())
        .expand_multi_info(&[b"octowhere private", &low, &high], &mut key)
        .expect("32 bytes is a valid length");
    Some(Key::new(key))
}

/// The pairwise keys worked out so far, by id, each with the public key it was worked out for.
pub struct Pairwise {
    keys: [Option<([u8; PUBLIC_LEN], Key)>; IDS as usize],
}

impl Default for Pairwise {
    fn default() -> Self {
        Self {
            keys: [const { None }; IDS as usize],
        }
    }
}

impl Pairwise {
    /// The key shared with the member `id`, whose public key is `theirs`.
    pub fn key(&mut self, me: &StaticSecret, id: u8, theirs: &[u8; PUBLIC_LEN]) -> Option<&Key> {
        let held = self.keys.get_mut(usize::from(id))?;
        if held.as_ref().is_none_or(|(public, _)| public != theirs) {
            *held = Some((*theirs, pairwise(me, theirs)?));
        }
        held.as_ref().map(|(_, key)| key)
    }
}

/// What [`Store::insert`] did with a message.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Insert {
    New,
    Known,
    /// Past the horizon.
    Old,
    /// Stamped too far ahead of this node's clock.
    Ahead,
    /// The store is full of newer messages.
    Full,
}

/// Every message a node holds, with those it is to send. All zeroes is an empty store, so it
/// can be made in place on any heap.
#[derive(Zeroable)]
#[repr(C)]
pub struct Store {
    messages: [Message; CAPACITY],
    /// The places holding a message to be sent, as bits.
    unsent: [u32; CAPACITY / 32],
    /// The exclusive or of each message's hash.
    digest: u32,
}

/// 32 bits of SHA-256 over a message's origin and sequence number.
fn hash((origin, seq): Name) -> u32 {
    let seq = seq.to_be_bytes();
    let hash: [u8; 32] = Sha256::digest([origin, seq[0], seq[1], seq[2], seq[3]]).into();
    u32::from_be_bytes([hash[0], hash[1], hash[2], hash[3]])
}

impl Store {
    fn place(&self, (origin, seq): Name) -> Option<usize> {
        self.messages
            .iter()
            .position(|held| held.seq == seq && held.origin == origin)
    }

    #[must_use]
    pub fn get(&self, name: Name) -> Option<&Message> {
        self.place(name).map(|at| &self.messages[at])
    }

    pub fn iter(&self) -> impl Iterator<Item = &Message> {
        self.messages.iter().filter(|held| held.seq != 0)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.iter().count()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.digest == 0 && self.iter().next().is_none()
    }

    /// 32 bits that two nodes holding the same messages agree on; 0 for none.
    #[must_use]
    pub fn digest(&self) -> u32 {
        self.digest
    }

    /// Takes a message heard or made here, at `now`, the timebase second its round started at.
    /// A new one is to be sent.
    pub fn insert(&mut self, message: Message, now: u32) -> Insert {
        if message.is_past(now) {
            return Insert::Old;
        }
        if message.stamp > now.saturating_add(AHEAD_S) {
            return Insert::Ahead;
        }
        if self.place(message.name()).is_some() {
            return Insert::Known;
        }
        let at = match self.messages.iter().position(|held| held.seq == 0) {
            Some(free) => free,
            None => {
                let (oldest, held) = self
                    .messages
                    .iter()
                    .enumerate()
                    .min_by_key(|(_, held)| held.order())
                    .expect("a full store holds messages");
                if message.order() <= held.order() {
                    return Insert::Full;
                }
                self.remove(oldest);
                oldest
            }
        };
        self.messages[at] = message;
        self.digest ^= hash(message.name());
        self.unsent[at / 32] |= 1 << (at % 32);
        Insert::New
    }

    fn remove(&mut self, at: usize) {
        self.digest ^= hash(self.messages[at].name());
        self.messages[at] = Message::zeroed();
        self.unsent[at / 32] &= !(1 << (at % 32));
    }

    /// Drops the messages past the horizon at `now`, the timebase second a round started at,
    /// and passes each to `keep` first, which a removal's key messages need.
    pub fn expire(&mut self, now: u32, mut keep: impl FnMut(&Message)) {
        for at in 0..CAPACITY {
            let held = &self.messages[at];
            if held.seq != 0 && held.is_past(now) {
                keep(held);
                self.remove(at);
            }
        }
    }

    /// Marks a message held to be sent.
    pub fn mark(&mut self, name: Name) {
        if let Some(at) = self.place(name) {
            self.unsent[at / 32] |= 1 << (at % 32);
        }
    }

    /// Counts a message as sent: a packet carried it, or one that reached this node's
    /// neighbours did.
    pub fn sent(&mut self, name: Name) {
        if let Some(at) = self.place(name) {
            self.unsent[at / 32] &= !(1 << (at % 32));
        }
    }

    #[must_use]
    pub fn has_unsent(&self) -> bool {
        self.unsent.iter().any(|&word| word != 0)
    }

    /// The oldest message to be sent that comes after `after` in sending order.
    #[must_use]
    pub fn next_unsent(&self, after: Option<&Message>) -> Option<&Message> {
        let after = after.map(Message::order);
        (0..CAPACITY)
            .filter(|&at| self.unsent[at / 32] & 1 << (at % 32) != 0)
            .map(|at| &self.messages[at])
            .filter(|held| after.is_none_or(|after| held.order() > after))
            .min_by_key(|held| held.order())
    }

    /// The smallest sequence number from `origin` above `above`, with its message.
    fn next_from(&self, origin: u8, above: u32) -> Option<&Message> {
        self.iter()
            .filter(|held| held.origin == origin && held.seq > above)
            .min_by_key(|held| held.seq)
    }

    /// Writes a summary of what the store holds into `out`, as much as fits, and returns its
    /// length: the origins it covers as a set, then for each covered origin with messages, the
    /// origin, its oldest and newest sequence numbers, and the gaps between them, each as the
    /// number held before it and the last missing. A covered origin with no entry has nothing
    /// here.
    pub fn summary(&self, out: &mut [u8]) -> usize {
        if out.len() < 4 {
            return 0;
        }
        let mut covered = 0u32;
        let mut len = 4;
        for origin in 0..IDS {
            let Some(first) = self.next_from(origin, 0) else {
                covered |= 1 << origin;
                continue;
            };
            let start = len;
            if len + 10 > out.len() {
                break;
            }
            out[len] = origin;
            out[len + 1..len + 5].copy_from_slice(&first.seq.to_be_bytes());
            len += 10;
            let (mut newest, mut holes) = (first.seq, 0);
            while let Some(next) = self.next_from(origin, newest) {
                if next.prev > newest && holes < HOLES_MAX && len + 8 <= out.len() {
                    out[len..len + 4].copy_from_slice(&newest.to_be_bytes());
                    out[len + 4..len + 8].copy_from_slice(&next.prev.to_be_bytes());
                    len += 8;
                    holes += 1;
                }
                newest = next.seq;
            }
            out[start + 5..start + 9].copy_from_slice(&newest.to_be_bytes());
            out[start + 9] = holes as u8;
            covered |= 1 << origin;
        }
        out[..4].copy_from_slice(&covered.to_le_bytes());
        len
    }

    /// Marks to be sent every message a neighbour's summary shows it lacks.
    pub fn answer(&mut self, summary: &[u8]) {
        let Some(covered) = summary.get(..4) else {
            return;
        };
        let covered = u32::from_le_bytes(covered.try_into().expect("four bytes"));
        let mut entries: [Option<&[u8]>; IDS as usize] = [None; IDS as usize];
        let mut rest = &summary[4..];
        while let Some(&origin) = rest.first() {
            let Some(&holes) = rest.get(9) else { break };
            let len = 10 + 8 * usize::from(holes);
            let Some(entry) = rest.get(..len) else { break };
            if let Some(slot) = entries.get_mut(usize::from(origin)) {
                *slot = Some(entry);
            }
            rest = &rest[len..];
        }
        for at in 0..CAPACITY {
            let held = &self.messages[at];
            if held.seq == 0 || covered & 1 << held.origin == 0 {
                continue;
            }
            if lacks(entries[usize::from(held.origin)], held.seq) {
                self.unsent[at / 32] |= 1 << (at % 32);
            }
        }
    }
}

/// Whether a summary's entry for an origin shows its sender lacks `seq`.
fn lacks(entry: Option<&[u8]>, seq: u32) -> bool {
    let Some(entry) = entry else {
        return true;
    };
    let word = |at: usize| u32::from_be_bytes(entry[at..at + 4].try_into().expect("four bytes"));
    let (oldest, newest) = (word(1), word(5));
    if seq < oldest || seq > newest {
        return true;
    }
    (0..usize::from(entry[9])).any(|hole| {
        let at = 10 + 8 * hole;
        (word(at) + 1..=word(at + 4)).contains(&seq)
    })
}

/// Which neighbours' messages differ from this node's, and whether a summary is to go out.
#[derive(Clone, Debug, Default)]
pub struct Summaries {
    /// For each id, how many of its packets running carried a digest unlike this node's.
    mismatched: [u8; IDS as usize],
    pending: bool,
}

impl Summaries {
    /// Takes a packet from `sender` with the messages digest it carried, 0 for none, and the
    /// summary it carried. A summary is answered only where the two stores differ.
    pub fn heard(&mut self, store: &mut Store, sender: u8, theirs: u32, summary: Option<&[u8]>) {
        let ours = store.digest();
        if let Some(mismatched) = self.mismatched.get_mut(usize::from(sender)) {
            if theirs == ours {
                *mismatched = 0;
            } else {
                *mismatched += 1;
                if *mismatched >= MISMATCHES {
                    self.pending = true;
                    *mismatched = 0;
                }
            }
        }
        if let Some(summary) = summary
            && theirs != ours
        {
            store.answer(summary);
        }
    }

    #[must_use]
    pub fn pending(&self) -> bool {
        self.pending
    }

    /// Takes the summary as sent.
    pub fn sent(&mut self) {
        self.pending = false;
    }
}

/// This device's sequence numbers. Every number below the block end stored last may have
/// been used, so a restart starts above it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sequence {
    next: u32,
    reserved: u32,
    last: u32,
}

impl Sequence {
    /// From the block end stored last, or none.
    #[must_use]
    pub fn new(stored: Option<u32>) -> Self {
        let start = stored.unwrap_or(1).max(1);
        Self {
            next: start,
            reserved: start,
            last: 0,
        }
    }

    /// The block end to store before another number can be taken, when the block is used up.
    #[must_use]
    pub fn to_reserve(&self) -> Option<u32> {
        (self.next >= self.reserved).then(|| self.next.saturating_add(BLOCK))
    }

    /// Takes up the block end [`Sequence::to_reserve`] gave, once it is stored.
    pub fn reserved(&mut self, end: u32) {
        self.reserved = self.reserved.max(end);
    }

    /// The next number and the one before it, 0 for none this run, or `None` until a block is
    /// stored.
    pub fn take(&mut self) -> Option<(u32, u32)> {
        if self.next >= self.reserved {
            return None;
        }
        let (seq, prev) = (self.next, self.last);
        self.next += 1;
        self.last = seq;
        Some((seq, prev))
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::boxed::Box;

    use super::*;

    const NOW: u32 = 1_790_000_000;

    fn store() -> Box<Store> {
        Box::new(Store::zeroed())
    }

    fn text(origin: u8, seq: u32, prev: u32, stamp: u32) -> Message {
        Message::to_group(origin, seq, prev, stamp, &[kind::TEXT, b'h', b'i']).unwrap()
    }

    #[test]
    fn a_message_reads_back_as_it_was_written() {
        let key = Key::new([4; 32]);
        for message in [
            text(3, 7, 6, NOW),
            Message::private(2, 9, true, 70, 0, NOW, &[kind::KEY; 48], &key).unwrap(),
        ] {
            let mut out = [0; FIXED_LEN + BODY_MAX];
            let len = message.encode(&mut out);
            assert_eq!(len + 2, message.record_len());
            assert_eq!(Message::decode(&out[..len]), Some(message));
        }
        let private = Message::private(2, 9, true, 70, 0, NOW, b"\x03secret", &key).unwrap();
        assert_eq!(private.to(), To::Member(9));
        assert!(private.is_key());
        assert!(!text(3, 7, 6, NOW).is_key());
        assert_eq!(text(3, 7, 6, NOW).to(), To::Group);
        assert!(
            Message::to_group(3, 0, 0, NOW, b"x").is_none(),
            "0 is no number"
        );
        assert!(Message::to_group(32, 1, 0, NOW, b"x").is_none());
    }

    #[test]
    fn a_private_message_opens_only_for_its_pair_and_unaltered() {
        let (a, b, c) = (
            StaticSecret::from([1; 32]),
            StaticSecret::from([2; 32]),
            StaticSecret::from([3; 32]),
        );
        let public = |s: &StaticSecret| PublicKey::from(s).to_bytes();
        let ab = pairwise(&a, &public(&b)).unwrap();
        assert!(
            pairwise(&b, &public(&a)).unwrap() == ab,
            "both ends work out the same key"
        );
        let message =
            Message::private(1, 2, false, 5, 4, NOW, b"\x01meet at the car", &ab).unwrap();
        let mut out = [0; BODY_MAX];
        assert_eq!(message.open(&ab, &mut out), Ok(&b"\x01meet at the car"[..]));
        let ac = pairwise(&a, &public(&c)).unwrap();
        assert_eq!(message.open(&ac, &mut out), Err(Inauthentic));
        let mut moved = message;
        moved.seq = 6;
        assert_eq!(
            moved.open(&ab, &mut out),
            Err(Inauthentic),
            "bound to its number"
        );
        let mut readdressed = message;
        readdressed.dest = 3;
        assert_eq!(readdressed.open(&ab, &mut out), Err(Inauthentic));
        assert!(pairwise(&a, &[0; 32]).is_none(), "not contributory");
    }

    #[test]
    fn text_is_printable_ascii_up_to_160() {
        assert!(is_text(b"Meet at the car park ~ 5 min"));
        assert!(is_text(&[b'a'; TEXT_MAX]));
        assert!(!is_text(&[b'a'; TEXT_MAX + 1]));
        assert!(!is_text(b""));
        assert!(!is_text("caf\u{e9}".as_bytes()));
        assert!(!is_text(b"line\nbreak"));
    }

    #[test]
    fn a_store_takes_each_message_once_within_the_horizon() {
        let mut s = store();
        assert_eq!(s.digest(), 0);
        assert_eq!(s.insert(text(1, 1, 0, NOW), NOW), Insert::New);
        assert_eq!(s.insert(text(1, 1, 0, NOW), NOW), Insert::Known);
        assert_eq!(s.insert(text(1, 2, 1, NOW - HORIZON_S), NOW), Insert::Old);
        assert_eq!(
            s.insert(text(1, 3, 2, NOW + AHEAD_S + 1), NOW),
            Insert::Ahead
        );
        assert_eq!(s.len(), 1);
        assert_ne!(s.digest(), 0);
        let mut kept = 0;
        s.expire(NOW + HORIZON_S - 1, |_| kept += 1);
        assert_eq!((s.len(), kept), (1, 0));
        s.expire(NOW + HORIZON_S, |_| kept += 1);
        assert_eq!((s.len(), kept), (0, 1));
        assert_eq!(s.digest(), 0);
    }

    #[test]
    fn stores_holding_the_same_messages_agree_whatever_the_order() {
        let (mut a, mut b) = (store(), store());
        for message in [
            text(1, 1, 0, NOW),
            text(2, 9, 0, NOW + 5),
            text(1, 2, 1, NOW + 9),
        ] {
            a.insert(message, NOW);
        }
        for message in [
            text(1, 2, 1, NOW + 9),
            text(1, 1, 0, NOW),
            text(2, 9, 0, NOW + 5),
        ] {
            b.insert(message, NOW);
        }
        assert_eq!(a.digest(), b.digest());
        b.insert(text(3, 1, 0, NOW), NOW);
        assert_ne!(a.digest(), b.digest());
    }

    #[test]
    fn a_full_store_keeps_the_newest() {
        let mut s = store();
        for seq in 1..=CAPACITY as u32 {
            assert_eq!(s.insert(text(1, seq, seq - 1, NOW + seq), NOW), Insert::New);
        }
        assert_eq!(s.insert(text(2, 1, 0, NOW), NOW), Insert::Full);
        assert_eq!(s.insert(text(2, 1, 0, NOW + 500), NOW), Insert::New);
        assert!(s.get((1, 1)).is_none(), "the oldest went");
        assert_eq!(s.len(), CAPACITY);
    }

    #[test]
    fn messages_go_out_oldest_first_and_once() {
        let mut s = store();
        s.insert(text(2, 1, 0, NOW + 9), NOW);
        s.insert(text(1, 1, 0, NOW), NOW);
        s.insert(text(1, 2, 1, NOW + 5), NOW);
        let first = *s.next_unsent(None).unwrap();
        assert_eq!(first.name(), (1, 1));
        let second = *s.next_unsent(Some(&first)).unwrap();
        assert_eq!(second.name(), (1, 2));
        s.sent(first.name());
        s.sent(second.name());
        assert_eq!(s.next_unsent(None).unwrap().name(), (2, 1));
        s.sent((2, 1));
        assert!(!s.has_unsent());
        s.mark((1, 2));
        assert_eq!(s.next_unsent(None).unwrap().name(), (1, 2));
    }

    /// Fills `a` with `held` from origin 4, numbered and chained as `chain` gives them, and has
    /// a store holding all of `chain` answer its summary.
    fn answered(chain: &[(u32, u32)], held: &[u32]) -> std::vec::Vec<u32> {
        let (mut a, mut b) = (store(), store());
        for &(seq, prev) in chain {
            b.insert(text(4, seq, prev, NOW + seq), NOW);
            if held.contains(&seq) {
                a.insert(text(4, seq, prev, NOW + seq), NOW);
            }
        }
        a.insert(text(7, 1, 0, NOW), NOW);
        b.insert(text(7, 1, 0, NOW), NOW);
        for &seq in chain.iter().map(|(seq, _)| seq) {
            b.sent((4, seq));
        }
        b.sent((7, 1));
        let mut summary = [0; 253];
        let len = a.summary(&mut summary);
        b.answer(&summary[..len]);
        let mut sent = std::vec::Vec::new();
        let mut after = None;
        while let Some(next) = b.next_unsent(after.as_ref()) {
            sent.push(next.seq);
            after = Some(*next);
        }
        sent
    }

    #[test]
    fn a_summary_brings_what_its_sender_lacks() {
        // Numbers skip at a restart: 3 follows nothing known, 70 follows 3.
        let chain = [
            (1, 0),
            (2, 1),
            (3, 2),
            (64, 0),
            (65, 64),
            (66, 65),
            (70, 66),
        ];
        assert_eq!(answered(&chain, &[1, 2, 3, 64, 65, 66, 70]), [0u32; 0]);
        assert_eq!(
            answered(&chain, &[2, 3, 64, 65, 66, 70]),
            [1],
            "before its oldest"
        );
        assert_eq!(
            answered(&chain, &[1, 2, 3, 64]),
            [65, 66, 70],
            "after its newest"
        );
        assert_eq!(
            answered(&chain, &[1, 2, 3, 64, 70]),
            [65, 66],
            "a gap the chain shows"
        );
        assert_eq!(answered(&chain, &[]), [1, 2, 3, 64, 65, 66, 70]);
    }

    #[test]
    fn a_summary_that_runs_out_of_room_covers_only_what_it_describes() {
        let mut a = store();
        for origin in 0..IDS {
            a.insert(text(origin, 1, 0, NOW), NOW);
        }
        let mut summary = [0; 60];
        let len = a.summary(&mut summary);
        let covered = u32::from_le_bytes(summary[..4].try_into().unwrap());
        assert_eq!(covered, 0b11111, "five entries of ten bytes fit");
        assert_eq!(len, 54);
        let mut b = store();
        b.insert(text(30, 1, 0, NOW), NOW);
        b.sent((30, 1));
        b.answer(&summary[..len]);
        assert!(!b.has_unsent(), "nothing is known of origin 30's");
    }

    #[test]
    fn a_neighbour_differing_twice_running_gets_a_summary() {
        let mut s = store();
        s.insert(text(1, 1, 0, NOW), NOW);
        let ours = s.digest();
        let mut summaries = Summaries::default();
        summaries.heard(&mut s, 3, ours ^ 1, None);
        assert!(!summaries.pending());
        summaries.heard(&mut s, 3, ours, None);
        summaries.heard(&mut s, 3, ours ^ 1, None);
        assert!(
            !summaries.pending(),
            "a match between starts the count again"
        );
        summaries.heard(&mut s, 3, ours ^ 1, None);
        assert!(summaries.pending());
        summaries.sent();
        assert!(!summaries.pending());
    }

    #[test]
    fn a_summary_is_answered_only_where_stores_differ() {
        let mut s = store();
        s.insert(text(1, 1, 0, NOW), NOW);
        s.sent((1, 1));
        let ours = s.digest();
        let mut summaries = Summaries::default();
        let empty = [0xff, 0xff, 0xff, 0xff];
        summaries.heard(&mut s, 3, ours, Some(&empty));
        assert!(!s.has_unsent());
        summaries.heard(&mut s, 3, 0, Some(&empty));
        assert!(s.has_unsent());
    }

    #[test]
    fn sequence_numbers_wait_for_their_block_and_skip_it_at_a_restart() {
        let mut sequence = Sequence::new(None);
        assert_eq!(sequence.take(), None);
        assert_eq!(sequence.to_reserve(), Some(1 + BLOCK));
        sequence.reserved(1 + BLOCK);
        assert_eq!(sequence.take(), Some((1, 0)));
        assert_eq!(sequence.take(), Some((2, 1)));
        assert_eq!(sequence.to_reserve(), None);
        for _ in 3..=BLOCK {
            sequence.take().unwrap();
        }
        assert_eq!(sequence.take(), None);
        assert_eq!(sequence.to_reserve(), Some(1 + 2 * BLOCK));

        let mut restarted = Sequence::new(Some(1 + BLOCK));
        assert_eq!(restarted.take(), None);
        restarted.reserved(restarted.to_reserve().unwrap());
        assert_eq!(restarted.take(), Some((1 + BLOCK, 0)));
    }
}
