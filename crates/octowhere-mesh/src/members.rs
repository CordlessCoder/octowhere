//! The group a node belongs to: its key, the node's own id, and each member's public key, hardware
//! address and name. Pairing fills it, and member records spread changes to it through the mesh.

use core::cell::Cell;

use sha2::{Digest, Sha256};

use crate::IDS;
use crate::seal::Key;

pub const NAME_LEN: usize = 16;
pub const MAC_LEN: usize = 6;
pub const PUBLIC_LEN: usize = 32;
/// A member record's body up to its name: id, public key, join time, change time, hardware
/// address.
pub const RECORD_FIXED_LEN: usize = 1 + PUBLIC_LEN + 4 + 4 + MAC_LEN;
pub const RECORD_MAX_LEN: usize = RECORD_FIXED_LEN + NAME_LEN;

const SLOTS: usize = IDS as usize;

/// Up to [`NAME_LEN`] printable ASCII characters, case kept.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Name {
    len: u8,
    bytes: [u8; NAME_LEN],
}

impl Name {
    /// `None` for an empty, overlong or non-printable name.
    #[must_use]
    pub fn new(text: &[u8]) -> Option<Self> {
        if text.is_empty()
            || text.len() > NAME_LEN
            || !text.iter().all(|&c| (0x20..0x7f).contains(&c))
        {
            return None;
        }
        let mut bytes = [0; NAME_LEN];
        bytes[..text.len()].copy_from_slice(text);
        Some(Self {
            len: text.len() as u8,
            bytes,
        })
    }

    /// The name a device has until its user sets one: `OW-` and the last four hex digits of its
    /// hardware address.
    #[must_use]
    pub fn from_mac(mac: &[u8; MAC_LEN]) -> Self {
        const HEX: &[u8; 16] = b"0123456789ABCDEF";
        let mut text = *b"OW-0000";
        for (i, nibble) in [mac[4] >> 4, mac[4] & 15, mac[5] >> 4, mac[5] & 15]
            .into_iter()
            .enumerate()
        {
            text[3 + i] = HEX[usize::from(nibble)];
        }
        Self::new(&text).expect("a printable name")
    }

    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes[..usize::from(self.len)]
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        core::str::from_utf8(self.as_bytes()).expect("printable ASCII")
    }
}

#[cfg(feature = "defmt")]
impl defmt::Format for Name {
    fn format(&self, f: defmt::Formatter) {
        defmt::write!(f, "{=str}", self.as_str());
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct Member {
    pub public: [u8; PUBLIC_LEN],
    /// UTC seconds it joined, or rejoined after losing its keys.
    pub joined: u32,
    /// UTC seconds its record last changed: its joining or a later rename. The newer record of a
    /// member wins a merge.
    pub changed: u32,
    pub mac: [u8; MAC_LEN],
    pub name: Name,
}

impl Member {
    /// Writes the member record's body for `id` and returns its length.
    pub fn encode(&self, id: u8, out: &mut [u8; RECORD_MAX_LEN]) -> usize {
        out[0] = id;
        out[1..33].copy_from_slice(&self.public);
        out[33..37].copy_from_slice(&self.joined.to_be_bytes());
        out[37..41].copy_from_slice(&self.changed.to_be_bytes());
        out[41..47].copy_from_slice(&self.mac);
        let name = self.name.as_bytes();
        out[RECORD_FIXED_LEN..RECORD_FIXED_LEN + name.len()].copy_from_slice(name);
        RECORD_FIXED_LEN + name.len()
    }

    /// Reads a member record's body.
    #[must_use]
    pub fn decode(body: &[u8]) -> Option<(u8, Self)> {
        let fixed = body.get(..RECORD_FIXED_LEN)?;
        let id = fixed[0];
        if id >= IDS {
            return None;
        }
        Some((
            id,
            Self {
                public: fixed[1..33].try_into().ok()?,
                joined: u32::from_be_bytes(fixed[33..37].try_into().ok()?),
                changed: u32::from_be_bytes(fixed[37..41].try_into().ok()?),
                mac: fixed[41..47].try_into().ok()?,
                name: Name::new(&body[RECORD_FIXED_LEN..])?,
            },
        ))
    }

    /// The bytes its member record takes in a packet, its type and length included.
    #[must_use]
    pub fn record_len(&self) -> usize {
        2 + RECORD_FIXED_LEN + self.name.as_bytes().len()
    }

    /// Of two devices holding one id, the one with the lower rank keeps it.
    fn rank(&self) -> [u8; 32] {
        Sha256::digest(self.public).into()
    }

    fn same_device(&self, other: &Self) -> bool {
        self.public == other.public || self.mac == other.mac
    }
}

/// What a member record heard from another node changed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Merged {
    Unchanged,
    /// The record at its id changed, and `vacated` is the id the member moved from, now empty.
    Changed {
        vacated: Option<u8>,
    },
    /// Another device holds this node's id and keeps it, so this node took the lowest free id.
    Renumbered {
        from: u8,
        to: u8,
    },
}

#[derive(Clone)]
pub struct Group {
    key: Key,
    own: u8,
    members: [Option<Member>; SLOTS],
    /// The ids whose records changed here, or were asked for, and have not been sent since, as
    /// a set.
    unsent: u32,
    /// The members' digest, until they change.
    digest: Cell<Option<u32>>,
}

impl Group {
    /// A new group with `me` as its first member, id 0.
    #[must_use]
    pub fn found(key: Key, me: Member) -> Self {
        let mut members = [None; SLOTS];
        members[0] = Some(me);
        Self {
            key,
            own: 0,
            members,
            unsent: 0,
            digest: Cell::new(None),
        }
    }

    /// A group as stored or as pairing delivered it. `None` unless it holds this node's record.
    #[must_use]
    pub fn new(key: Key, own: u8, members: [Option<Member>; SLOTS]) -> Option<Self> {
        members.get(usize::from(own))?.as_ref()?;
        Some(Self {
            key,
            own,
            members,
            unsent: 0,
            digest: Cell::new(None),
        })
    }

    #[must_use]
    pub fn key(&self) -> &Key {
        &self.key
    }

    #[must_use]
    pub fn own(&self) -> u8 {
        self.own
    }

    #[must_use]
    pub fn me(&self) -> &Member {
        self.members[usize::from(self.own)]
            .as_ref()
            .expect("a group holds its own node's record")
    }

    #[must_use]
    pub fn member(&self, id: u8) -> Option<&Member> {
        self.members.get(usize::from(id))?.as_ref()
    }

    pub fn members(&self) -> impl Iterator<Item = (u8, &Member)> {
        (0..IDS).filter_map(|id| Some((id, self.member(id)?)))
    }

    /// The ids members hold, as a set.
    #[must_use]
    pub fn ids(&self) -> u32 {
        self.members().fold(0, |set, (id, _)| set | 1 << id)
    }

    #[must_use]
    pub fn count(&self) -> usize {
        self.members.iter().flatten().count()
    }

    #[must_use]
    pub fn is_full(&self) -> bool {
        self.count() == SLOTS
    }

    #[must_use]
    pub fn lowest_free(&self) -> Option<u8> {
        (0..IDS).find(|&id| self.members[usize::from(id)].is_none())
    }

    #[must_use]
    pub fn by_mac(&self, mac: &[u8; MAC_LEN]) -> Option<u8> {
        self.members()
            .find(|(_, m)| m.mac == *mac)
            .map(|(id, _)| id)
    }

    /// The id a device joining with `mac` gets: its old one if it was a member, otherwise the
    /// lowest free.
    #[must_use]
    pub fn id_for(&self, mac: &[u8; MAC_LEN]) -> Option<u8> {
        self.by_mac(mac).or_else(|| self.lowest_free())
    }

    /// 32 bits of SHA-256 over each member's id, public key and change time, in id order. Two
    /// nodes whose digests agree hold the same records, as far as a merge goes.
    #[must_use]
    pub fn digest(&self) -> u32 {
        if let Some(digest) = self.digest.get() {
            return digest;
        }
        let mut hash = Sha256::new();
        for (id, member) in self.members() {
            hash.update([id]);
            hash.update(member.public);
            hash.update(member.changed.to_be_bytes());
        }
        let hash: [u8; 32] = hash.finalize().into();
        let digest = u32::from_be_bytes([hash[0], hash[1], hash[2], hash[3]]);
        self.digest.set(Some(digest));
        digest
    }

    /// Puts a member pairing enrolled at `id`.
    pub fn enrol(&mut self, id: u8, member: Member) {
        self.members[usize::from(id)] = Some(member);
        self.unsent |= 1 << id;
        self.digest.set(None);
    }

    /// Renames this node, at UTC `now`.
    pub fn rename(&mut self, name: Name, now: u32) {
        let me = self.members[usize::from(self.own)]
            .as_mut()
            .expect("a group holds its own node's record");
        me.name = name;
        me.changed = now.max(me.changed + 1);
        self.unsent |= 1 << self.own;
        self.digest.set(None);
    }

    /// Merges a member record heard from another node, at UTC `now`.
    pub fn merge(&mut self, id: u8, record: Member, now: u32) -> Merged {
        if id >= IDS || record.same_device(self.me()) {
            // Nobody knows this node's record better than it does.
            return Merged::Unchanged;
        }
        let elsewhere = (0..IDS).find(|&other| {
            other != id
                && self
                    .member(other)
                    .is_some_and(|held| held.same_device(&record))
        });
        if let Some(other) = elsewhere {
            let held = self.member(other).expect("found above");
            if held.changed >= record.changed {
                return Merged::Unchanged;
            }
        }
        let changed = Merged::Changed { vacated: elsewhere };
        let outcome = match self.member(id) {
            None => changed,
            Some(held) if held.same_device(&record) => {
                if held.changed >= record.changed {
                    return Merged::Unchanged;
                }
                changed
            }
            // Two devices were given this id in separate places.
            Some(held) if record.rank() >= held.rank() => return Merged::Unchanged,
            Some(_) if id == self.own => {
                let mut me = *self.me();
                // Nowhere to move to: keep the id, as though this node had won.
                let Some(to) = self.lowest_free() else {
                    return Merged::Unchanged;
                };
                me.changed = now.max(me.changed + 1);
                self.members[usize::from(to)] = Some(me);
                self.own = to;
                self.unsent |= 1 << to;
                self.digest.set(None);
                Merged::Renumbered { from: id, to }
            }
            // The device that held it moves itself once it hears this record.
            Some(_) => changed,
        };
        if let Some(other) = elsewhere {
            self.members[usize::from(other)] = None;
        }
        self.members[usize::from(id)] = Some(record);
        self.digest.set(None);
        if matches!(outcome, Merged::Changed { .. }) {
            self.unsent |= 1 << id;
        }
        outcome
    }

    /// Whether a record changed here and has not been sent since.
    #[must_use]
    pub fn has_unsent(&self) -> bool {
        self.unsent != 0
    }

    /// Counts the record at `id` as sent, when a packet that reached this node's neighbours
    /// carried it as this node holds it.
    pub fn covered(&mut self, id: u8, record: &Member) {
        if self.member(id) == Some(record) {
            self.unsent &= !(1 << id);
        }
    }

    /// Marks the records held of the ids in the set `ids` to be sent, as another node asked.
    pub fn ask(&mut self, ids: u32) {
        self.unsent |= ids & self.ids();
    }

    /// The ids whose records are to be sent, as a set.
    #[must_use]
    pub fn unsent(&self) -> u32 {
        self.unsent & self.ids()
    }

    /// Counts the record at `id` as sent, once a packet carries it.
    pub fn sent(&mut self, id: u8) {
        self.unsent &= !(1 << id);
    }
}

/// A neighbour whose members digest differs from this node's in this many of its packets
/// running is asked for every record it holds.
pub const MISMATCHES: u8 = 2;

/// The member records a node asks its neighbours for: a sender's own when the node holds none
/// for it, and all of a neighbour's once their tables keep differing.
#[derive(Clone, Debug, Default)]
pub struct Requests {
    /// The ids the next packet asks for, as a set.
    pending: u32,
    /// For each id, how many of its packets running carried a digest unlike this node's.
    mismatched: [u8; SLOTS],
}

impl Requests {
    /// Takes a packet from `sender`, with the members digest it carried and the ids it asked
    /// for. A request is answered only where the two tables differ.
    pub fn heard(
        &mut self,
        group: &mut Group,
        sender: u8,
        digest: Option<u32>,
        asked: Option<u32>,
    ) {
        let ours = group.digest();
        if group.member(sender).is_none() {
            self.pending |= 1 << sender;
        }
        if let Some(mismatched) = self.mismatched.get_mut(usize::from(sender)) {
            match digest {
                Some(theirs) if theirs != ours => {
                    *mismatched += 1;
                    if *mismatched >= MISMATCHES {
                        self.pending = u32::MAX;
                        *mismatched = 0;
                    }
                }
                Some(_) => *mismatched = 0,
                None => {}
            }
        }
        if let Some(ids) = asked
            && digest != Some(ours)
        {
            group.ask(ids);
        }
    }

    /// The ids to ask for, as a set.
    #[must_use]
    pub fn pending(&self) -> u32 {
        self.pending
    }

    /// Takes the ids in `asked` as asked for, once a packet carried them.
    pub fn sent(&mut self, asked: u32) {
        self.pending &= !asked;
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) fn member(n: u8, changed: u32) -> Member {
        Member {
            public: [n; 32],
            joined: 1_790_000_000,
            changed,
            mac: [0x10, 0, 0, 0, 0, n],
            name: Name::from_mac(&[0, 0, 0, 0, 0, n]),
        }
    }

    fn group(own: u8, ids: &[(u8, u8)]) -> Group {
        let mut members = [None; SLOTS];
        for &(id, n) in ids {
            members[usize::from(id)] = Some(member(n, 100));
        }
        Group::new(Key::new([5; 32]), own, members).unwrap()
    }

    #[test]
    fn a_record_reads_back_as_it_was_written() {
        let mut m = member(7, 1_790_000_123);
        m.name = Name::new(b"Ana's watch ~16!").unwrap();
        let mut body = [0; RECORD_MAX_LEN];
        let len = m.encode(9, &mut body);
        assert_eq!(len, RECORD_MAX_LEN);
        assert_eq!(Member::decode(&body[..len]), Some((9, m)));
        assert_eq!(
            Member::decode(&body[..RECORD_FIXED_LEN]),
            None,
            "a name is never empty"
        );
        body[0] = 32;
        assert_eq!(Member::decode(&body[..len]), None);
    }

    #[test]
    fn names_are_printable_ascii_up_to_sixteen() {
        assert_eq!(
            Name::from_mac(&[0, 0, 0, 0, 0x1c, 0x1c]).as_str(),
            "OW-1C1C"
        );
        assert!(Name::new(b"").is_none());
        assert!(Name::new(b"seventeen chars!!").is_none());
        assert!(Name::new(b"tab\there").is_none());
        assert!(Name::new("café".as_bytes()).is_none());
        assert_eq!(Name::new(b" ~").unwrap().as_str(), " ~");
    }

    #[test]
    fn ids_go_to_the_lowest_free_or_back_to_a_returning_device() {
        let g = group(0, &[(0, 1), (1, 2), (3, 4)]);
        assert_eq!(g.id_for(&member(9, 0).mac), Some(2));
        assert_eq!(g.id_for(&member(4, 0).mac), Some(3));
        assert!(!g.is_full());
    }

    #[test]
    fn a_newer_record_of_a_member_replaces_its_older_one() {
        let mut g = group(0, &[(0, 1), (1, 2)]);
        let mut renamed = member(2, 200);
        renamed.name = Name::new(b"Bo").unwrap();
        assert_eq!(g.merge(1, renamed, 0), Merged::Changed { vacated: None });
        assert_eq!(g.member(1).unwrap().name.as_str(), "Bo");
        assert_eq!(g.merge(1, member(2, 150), 0), Merged::Unchanged);
        assert_eq!(
            g.merge(5, member(6, 1), 0),
            Merged::Changed { vacated: None }
        );
    }

    #[test]
    fn a_record_about_this_node_is_never_taken() {
        let mut g = group(0, &[(0, 1)]);
        assert_eq!(g.merge(0, member(1, 999), 0), Merged::Unchanged);
        assert_eq!(g.merge(4, member(1, 999), 0), Merged::Unchanged);
        assert_eq!(g.me().changed, 100);
    }

    #[test]
    fn a_member_that_moved_is_dropped_from_its_old_id() {
        let mut g = group(0, &[(0, 1), (1, 2)]);
        assert_eq!(
            g.merge(4, member(2, 101), 0),
            Merged::Changed { vacated: Some(1) }
        );
        assert!(g.member(1).is_none());
        assert_eq!(
            g.merge(1, member(2, 100), 0),
            Merged::Unchanged,
            "the stale record"
        );
        assert!(g.member(1).is_none());
    }

    #[test]
    fn of_two_devices_given_one_id_the_lower_hash_keeps_it() {
        let (low, high) = {
            let (a, b) = (member(2, 100), member(3, 100));
            if a.rank() < b.rank() { (2, 3) } else { (3, 2) }
        };
        let mut g = group(0, &[(0, 1), (1, high)]);
        assert_eq!(
            g.merge(1, member(low, 100), 0),
            Merged::Changed { vacated: None }
        );
        assert_eq!(g.member(1).unwrap().public, [low; 32]);
        assert_eq!(g.merge(1, member(high, 500), 0), Merged::Unchanged);
    }

    #[test]
    fn this_node_moves_when_another_keeps_its_id() {
        let (low, high) = {
            let (a, b) = (member(2, 100), member(3, 100));
            if a.rank() < b.rank() { (2, 3) } else { (3, 2) }
        };
        let mut g = group(1, &[(0, 1), (1, high), (2, 7)]);
        assert_eq!(
            g.merge(1, member(low, 100), 1_000),
            Merged::Renumbered { from: 1, to: 3 }
        );
        assert_eq!(g.own(), 3);
        assert_eq!(g.me().public, [high; 32]);
        assert_eq!(g.me().changed, 1_000, "its record announces the move");
        assert_eq!(g.member(1).unwrap().public, [low; 32]);

        let mut g = group(1, &[(0, 1), (1, low)]);
        assert_eq!(g.merge(1, member(high, 100), 1_000), Merged::Unchanged);
        assert_eq!(g.own(), 1);
    }

    #[test]
    fn a_changed_or_asked_for_record_is_sent_once() {
        let mut g = group(3, &[(0, 1), (3, 2), (31, 3)]);
        assert!(!g.has_unsent());
        g.rename(Name::new(b"New").unwrap(), 50);
        assert_eq!(
            g.merge(31, member(3, 200), 1_000),
            Merged::Changed { vacated: None }
        );
        assert_eq!(g.unsent(), 1 << 3 | 1 << 31);
        g.sent(3);
        g.sent(31);
        assert!(!g.has_unsent());
        // Only records held are sent, whatever is asked.
        g.ask(u32::MAX);
        assert_eq!(g.unsent(), 1 | 1 << 3 | 1 << 31);
        for id in [0, 3, 31] {
            g.sent(id);
        }

        assert_eq!(
            g.merge(0, member(1, 300), 1_000),
            Merged::Changed { vacated: None }
        );
        g.covered(0, &member(1, 299));
        assert!(g.has_unsent(), "an older record covers nothing");
        g.covered(0, &member(1, 300));
        assert!(!g.has_unsent());
    }

    #[test]
    fn the_digest_follows_what_a_merge_compares() {
        let g = group(3, &[(0, 1), (3, 2), (31, 3)]);
        let mut other = group(0, &[(0, 1), (3, 2), (31, 3)]);
        assert_eq!(g.digest(), other.digest(), "whichever node holds them");
        assert_eq!(
            other.merge(3, member(2, 200), 1_000),
            Merged::Changed { vacated: None }
        );
        assert_ne!(g.digest(), other.digest());
        let mut more = group(3, &[(0, 1), (3, 2), (31, 3)]);
        more.enrol(5, member(9, 100));
        assert_ne!(g.digest(), more.digest());
    }

    #[test]
    fn a_node_asks_for_a_record_it_lacks_and_for_all_once_tables_keep_differing() {
        let mut g = group(0, &[(0, 1), (3, 2)]);
        let mut requests = Requests::default();
        let ours = g.digest();
        requests.heard(&mut g, 3, Some(ours), None);
        assert_eq!(requests.pending(), 0);
        requests.heard(&mut g, 7, Some(ours), None);
        assert_eq!(
            requests.pending(),
            1 << 7,
            "a sender it holds no record for"
        );
        requests.sent(1 << 7);
        requests.heard(&mut g, 3, Some(ours ^ 1), None);
        assert_eq!(requests.pending(), 0, "one packet may only be out of date");
        requests.heard(&mut g, 3, Some(ours), None);
        requests.heard(&mut g, 3, Some(ours ^ 1), None);
        assert_eq!(
            requests.pending(),
            0,
            "a match in between starts the count again"
        );
        requests.heard(&mut g, 3, Some(ours ^ 1), None);
        assert_eq!(requests.pending(), u32::MAX);
    }

    #[test]
    fn a_request_is_answered_only_where_tables_differ() {
        let mut g = group(0, &[(0, 1), (3, 2)]);
        let mut requests = Requests::default();
        let ours = g.digest();
        requests.heard(&mut g, 3, Some(ours), Some(u32::MAX));
        assert!(!g.has_unsent());
        requests.heard(&mut g, 3, Some(ours ^ 1), Some(1 << 3 | 1 << 9));
        assert_eq!(g.unsent(), 1 << 3, "only the records held");
    }

    #[test]
    fn a_rename_is_newer_than_the_record_it_replaces() {
        let mut g = group(0, &[(0, 1)]);
        g.rename(Name::new(b"New").unwrap(), 50);
        assert_eq!(g.me().changed, 101);
        assert_eq!(g.me().joined, 1_790_000_000);
    }
}
