//! The group a node belongs to: its key, the node's own id, and each member's public key, hardware
//! address and name. Pairing fills it, and member records spread changes to it through the mesh.

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
    /// Where the rotation of member records resumes.
    rotation: u8,
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
            rotation: 0,
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
            rotation: 0,
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

    /// Puts a member pairing enrolled at `id`.
    pub fn enrol(&mut self, id: u8, member: Member) {
        self.members[usize::from(id)] = Some(member);
    }

    /// Renames this node, at UTC `now`.
    pub fn rename(&mut self, name: Name, now: u32) {
        let me = self.members[usize::from(self.own)]
            .as_mut()
            .expect("a group holds its own node's record");
        me.name = name;
        me.changed = now.max(me.changed + 1);
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
                Merged::Renumbered { from: id, to }
            }
            // The device that held it moves itself once it hears this record.
            Some(_) => changed,
        };
        if let Some(other) = elsewhere {
            self.members[usize::from(other)] = None;
        }
        self.members[usize::from(id)] = Some(record);
        outcome
    }

    /// The next member record to send, in rotation through the members, this node included.
    pub fn next_record(&mut self) -> (u8, Member) {
        for step in 0..IDS {
            let id = (self.rotation + step) % IDS;
            if let Some(&member) = self.member(id) {
                self.rotation = (id + 1) % IDS;
                return (id, member);
            }
        }
        unreachable!("a group holds its own node's record")
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
    fn records_rotate_through_every_member() {
        let mut g = group(3, &[(0, 1), (3, 2), (31, 3)]);
        let ids: [u8; 4] = core::array::from_fn(|_| g.next_record().0);
        assert_eq!(ids, [0, 3, 31, 0]);
    }

    #[test]
    fn a_rename_is_newer_than_the_record_it_replaces() {
        let mut g = group(0, &[(0, 1)]);
        g.rename(Name::new(b"New").unwrap(), 50);
        assert_eq!(g.me().changed, 101);
        assert_eq!(g.me().joined, 1_790_000_000);
    }
}
