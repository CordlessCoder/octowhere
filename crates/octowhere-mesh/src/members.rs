//! The group a node belongs to: its key, the node's own id, and each member's public key, hardware
//! address and name. Pairing fills it, and member records spread changes to it through the mesh.

use core::cell::Cell;

use sha2::{Digest, Sha256};

use crate::seal::Key;
use crate::{AHEAD_S, IDS};

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

/// A member that left or was removed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct Gone {
    pub public: [u8; PUBLIC_LEN],
    /// UTC seconds it went. It wins a merge against an older record of the same device, and
    /// loses to a newer one, as a device paired again has.
    pub changed: u32,
}

/// A gone record's body: id, public key, change time.
pub const GONE_LEN: usize = 1 + PUBLIC_LEN + 4;

impl Gone {
    pub fn encode(&self, id: u8, out: &mut [u8; GONE_LEN]) {
        out[0] = id;
        out[1..33].copy_from_slice(&self.public);
        out[33..37].copy_from_slice(&self.changed.to_be_bytes());
    }

    #[must_use]
    pub fn decode(body: &[u8]) -> Option<(u8, Self)> {
        let body = body.get(..GONE_LEN)?;
        let id = body[0];
        if id >= IDS {
            return None;
        }
        Some((
            id,
            Self {
                public: body[1..33].try_into().ok()?,
                changed: u32::from_be_bytes(body[33..37].try_into().ok()?),
            },
        ))
    }

    /// Which of two gone records holds an id, so that every node keeps the same one.
    fn outranks(&self, other: &Self) -> bool {
        (self.changed, other.public) > (other.changed, self.public)
    }
}

/// The first eight bytes of SHA-256 over a public key, which name a device in a key message.
#[must_use]
pub fn fingerprint(public: &[u8; PUBLIC_LEN]) -> [u8; 8] {
    let hash: [u8; 32] = Sha256::digest(public).into();
    hash[..8].try_into().expect("eight bytes")
}

/// What an id holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Slot {
    Member(Member),
    /// The member that held it went. The id is free.
    Gone(Gone),
}

impl Slot {
    /// The bytes its record takes in a packet, its type and length included.
    #[must_use]
    pub fn record_len(&self) -> usize {
        match self {
            Self::Member(member) => member.record_len(),
            Self::Gone(_) => 2 + GONE_LEN,
        }
    }
}

/// What a member or gone record heard from another node changed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Merged {
    Unchanged,
    /// The slot at its id changed, and `vacated` is another id that changed with it: the one the
    /// member moved from, or one a gone record of the same device held, now empty.
    Changed {
        vacated: Option<u8>,
    },
    /// Another device holds this node's id and keeps it, so this node took the lowest free id.
    Renumbered {
        from: u8,
        to: u8,
    },
    /// A member went, and the slot at `at` holds its gone record.
    Went {
        at: u8,
    },
}

fn is_ahead(changed: u32, now: u32) -> bool {
    now != 0 && changed > now.saturating_add(AHEAD_S)
}

/// The most gone records kept once new members have taken their ids.
pub const FORMER: usize = 8;
const _: () = assert!(FORMER <= 8, "`former_unsent` is a `u8` set");

#[derive(Clone)]
pub struct Group {
    key: Key,
    /// How many times the key has changed since the group was founded. A new key's message
    /// names its generation, which tells a later key from a rival.
    generation: u16,
    own: u8,
    slots: [Option<Slot>; SLOTS],
    /// Gone records whose ids new members took, oldest first, with those ids.
    former: [Option<(u8, Gone)>; FORMER],
    /// The ids whose slots changed here, or were asked for, and have not been sent since, as
    /// a set.
    unsent: u32,
    /// The places in `former` whose records are to be sent, as a set.
    former_unsent: u8,
    /// The slots' digest, until they change.
    digest: Cell<Option<u32>>,
    /// The ids whose slots changed since [`Group::take_changed`] last took them, as a set.
    changed: u32,
}

impl Group {
    /// A new group with `me` as its first member, id 0.
    #[must_use]
    pub fn found(key: Key, me: Member) -> Self {
        let mut slots = [None; SLOTS];
        slots[0] = Some(Slot::Member(me));
        Self::restore(key, 0, 0, slots).expect("it holds this node's record")
    }

    /// A group of members with its first key. `None` unless it holds this node's record.
    #[cfg(test)]
    #[must_use]
    pub(crate) fn new(key: Key, own: u8, members: [Option<Member>; SLOTS]) -> Option<Self> {
        Self::restore(key, 0, own, members.map(|member| member.map(Slot::Member)))
    }

    /// A group as stored or as pairing delivered it. `None` unless it holds this node's record.
    #[must_use]
    pub fn restore(
        key: Key,
        generation: u16,
        own: u8,
        slots: [Option<Slot>; SLOTS],
    ) -> Option<Self> {
        let Some(Slot::Member(_)) = slots.get(usize::from(own))? else {
            return None;
        };
        Some(Self {
            key,
            generation,
            own,
            slots,
            former: [None; FORMER],
            unsent: 0,
            former_unsent: 0,
            digest: Cell::new(None),
            changed: 0,
        })
    }

    #[must_use]
    pub fn key(&self) -> &Key {
        &self.key
    }

    #[must_use]
    pub fn generation(&self) -> u16 {
        self.generation
    }

    /// Takes up a new key, which the group switched to.
    pub(crate) fn rekey(&mut self, key: Key, generation: u16) {
        self.key = key;
        self.generation = generation;
    }

    #[must_use]
    pub fn own(&self) -> u8 {
        self.own
    }

    #[must_use]
    pub fn me(&self) -> &Member {
        self.member(self.own)
            .expect("a group holds its own node's record")
    }

    #[must_use]
    pub fn slot(&self, id: u8) -> Option<&Slot> {
        self.slots.get(usize::from(id))?.as_ref()
    }

    #[must_use]
    pub fn member(&self, id: u8) -> Option<&Member> {
        match self.slot(id)? {
            Slot::Member(member) => Some(member),
            Slot::Gone(_) => None,
        }
    }

    #[must_use]
    pub fn gone(&self, id: u8) -> Option<&Gone> {
        match self.slot(id)? {
            Slot::Gone(gone) => Some(gone),
            Slot::Member(_) => None,
        }
    }

    pub fn members(&self) -> impl Iterator<Item = (u8, &Member)> {
        (0..IDS).filter_map(|id| Some((id, self.member(id)?)))
    }

    /// The ids members hold, as a set.
    #[must_use]
    pub fn ids(&self) -> u32 {
        self.members().fold(0, |set, (id, _)| set | 1 << id)
    }

    /// The ids with a member or a gone record, as a set.
    fn held(&self) -> u32 {
        (0..IDS)
            .filter(|&id| self.slot(id).is_some())
            .fold(0, |set, id| set | 1 << id)
    }

    #[must_use]
    pub fn count(&self) -> usize {
        self.members().count()
    }

    #[must_use]
    pub fn is_full(&self) -> bool {
        self.count() == SLOTS
    }

    /// The lowest id no member holds; one a gone member held is free.
    #[must_use]
    pub fn lowest_free(&self) -> Option<u8> {
        (0..IDS).find(|&id| self.member(id).is_none())
    }

    #[must_use]
    pub fn by_mac(&self, mac: &[u8; MAC_LEN]) -> Option<u8> {
        self.members()
            .find(|(_, m)| m.mac == *mac)
            .map(|(id, _)| id)
    }

    /// Whether `print` names a member, or a gone record held, as a key message's member removed
    /// must. A member may have left by the time the key reaches a node.
    #[must_use]
    pub fn names(&self, print: &[u8; 8]) -> bool {
        self.by_fingerprint(print).is_some()
            || (0..IDS)
                .filter_map(|id| self.gone(id))
                .chain(self.former.iter().flatten().map(|(_, gone)| gone))
                .any(|gone| fingerprint(&gone.public) == *print)
    }

    /// The id with `fingerprint`'s member, which a key message names.
    #[must_use]
    pub fn by_fingerprint(&self, fingerprint: &[u8; 8]) -> Option<u8> {
        self.members()
            .find(|(_, m)| self::fingerprint(&m.public) == *fingerprint)
            .map(|(id, _)| id)
    }

    /// The id a device joining with `mac` gets: its old one if it was a member, otherwise the
    /// lowest free.
    #[must_use]
    pub fn id_for(&self, mac: &[u8; MAC_LEN]) -> Option<u8> {
        self.by_mac(mac).or_else(|| self.lowest_free())
    }

    /// 32 bits of SHA-256 over each slot's id, public key and change time, in id order, with a
    /// gone record marked. Two nodes whose digests agree hold the same records, as far as a
    /// merge goes.
    #[must_use]
    pub fn digest(&self) -> u32 {
        if let Some(digest) = self.digest.get() {
            return digest;
        }
        let mut hash = Sha256::new();
        for id in 0..IDS {
            match self.slot(id) {
                Some(Slot::Member(member)) => {
                    hash.update([id]);
                    hash.update(member.public);
                    hash.update(member.changed.to_be_bytes());
                }
                Some(Slot::Gone(gone)) => {
                    hash.update([id | 0x80]);
                    hash.update(gone.public);
                    hash.update(gone.changed.to_be_bytes());
                }
                None => {}
            }
        }
        let hash: [u8; 32] = hash.finalize().into();
        let digest = u32::from_be_bytes([hash[0], hash[1], hash[2], hash[3]]);
        self.digest.set(Some(digest));
        digest
    }

    fn set(&mut self, id: u8, slot: Option<Slot>) {
        if let (Some(Slot::Gone(gone)), Some(Slot::Member(_))) = (self.slot(id).copied(), slot) {
            self.keep_former(id, gone);
        }
        self.slots[usize::from(id)] = slot;
        self.unsent |= 1 << id;
        self.changed |= 1 << id;
        self.digest.set(None);
    }

    /// The ids whose slots changed since the last call, as a set.
    pub fn take_changed(&mut self) -> u32 {
        core::mem::take(&mut self.changed)
    }

    fn keep_former(&mut self, id: u8, gone: Gone) {
        if self.former[0].is_some() && self.former.iter().all(Option::is_some) {
            self.former.rotate_left(1);
            self.former[FORMER - 1] = None;
            self.former_unsent >>= 1;
        }
        if let Some(free) = self.former.iter().position(Option::is_none) {
            self.former[free] = Some((id, gone));
        }
    }

    /// Where a gone record of the device `public` is held: an id, or a place in `former`.
    fn gone_of(&self, public: &[u8; PUBLIC_LEN]) -> Option<(Held, Gone)> {
        (0..IDS)
            .find_map(|id| {
                self.gone(id)
                    .filter(|gone| gone.public == *public)
                    .map(|gone| (Held::Slot(id), *gone))
            })
            .or_else(|| {
                self.former.iter().enumerate().find_map(|(at, former)| {
                    former
                        .filter(|(_, gone)| gone.public == *public)
                        .map(|(_, gone)| (Held::Former(at), gone))
                })
            })
    }

    /// Puts a member pairing enrolled at `id`, which forgets any gone record of the device.
    pub fn enrol(&mut self, id: u8, member: Member) {
        if let Some((held, _)) = self.gone_of(&member.public) {
            self.forget(held);
        }
        self.set(id, Some(Slot::Member(member)));
    }

    fn forget(&mut self, held: Held) {
        match held {
            Held::Slot(id) => self.set(id, None),
            Held::Former(at) => {
                self.former[at] = None;
                self.former_unsent &= !(1 << at);
            }
        }
    }

    /// Renames this node, at UTC `now`.
    pub fn rename(&mut self, name: Name, now: u32) {
        let mut me = *self.me();
        me.name = name;
        me.changed = now.max(me.changed + 1);
        self.set(self.own, Some(Slot::Member(me)));
    }

    /// The gone record this node sends as it leaves, at UTC `now`. The group itself is about to
    /// be forgotten, so it is left as it is.
    #[must_use]
    pub fn leaving(&self, now: u32) -> Gone {
        let me = self.me();
        Gone {
            public: me.public,
            changed: now.max(me.changed + 1),
        }
    }

    /// Replaces the member `fingerprint` names with a gone record at UTC `at`, as a switch to a
    /// new key that removes it does. Returns its id, or `None` when no member has it.
    pub(crate) fn remove(&mut self, fingerprint: &[u8; 8], at: u32) -> Option<u8> {
        let id = self.by_fingerprint(fingerprint)?;
        if id == self.own {
            return None;
        }
        let member = *self.member(id).expect("found above");
        self.set(
            id,
            Some(Slot::Gone(Gone {
                public: member.public,
                changed: at.max(member.changed + 1),
            })),
        );
        Some(id)
    }

    /// Gives the member that a gone record at `id` names by `fingerprint` its id back, as a
    /// removal undone. Its record comes back from nodes that still hold it. Returns whether
    /// there was such a gone record.
    pub(crate) fn forget_gone(&mut self, id: u8, fingerprint: &[u8; 8]) -> bool {
        if self
            .gone(id)
            .is_some_and(|gone| self::fingerprint(&gone.public) == *fingerprint)
        {
            self.set(id, None);
            return true;
        }
        false
    }

    /// Forgets what the ids `ids`, a set, hold, other than this node's, and every gone record set
    /// apart, as declining a removal after its switch does. Those are never stored, so a restart
    /// forgets them too. The nodes that never switched send the rest again as they hold them.
    /// Returns the ids that held something, as a set.
    pub(crate) fn forget_changed(&mut self, ids: u32) -> u32 {
        let mut forgotten = 0;
        let own = self.own;
        for id in (0..IDS).filter(|&id| ids & 1 << id != 0 && id != own) {
            if self.slot(id).is_some() {
                self.set(id, None);
                forgotten |= 1 << id;
            }
        }
        for at in 0..FORMER {
            if self.former[at].is_some() {
                self.forget(Held::Former(at));
            }
        }
        forgotten
    }

    /// Merges a member record heard from another node, at UTC `now`, 0 when unknown.
    pub fn merge(&mut self, id: u8, record: Member, now: u32) -> Merged {
        if id >= IDS || record.same_device(self.me()) || is_ahead(record.changed, now) {
            // Nobody knows this node's record better than it does.
            return Merged::Unchanged;
        }
        let gone = self.gone_of(&record.public);
        if let Some((held, gone)) = gone
            && gone.changed >= record.changed
        {
            // The sender missed it going: tell it.
            match held {
                Held::Slot(at) => self.unsent |= 1 << at,
                Held::Former(at) => self.former_unsent |= 1 << at,
            }
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
        // The device was paired again since it went.
        let vacated = match gone {
            Some((Held::Slot(at), _)) => {
                self.slots[usize::from(at)] = None;
                self.changed |= 1 << at;
                self.digest.set(None);
                (at != id).then_some(at)
            }
            Some((held, _)) => {
                self.forget(held);
                None
            }
            None => None,
        };
        let changed = Merged::Changed {
            vacated: elsewhere.or(vacated),
        };
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
                self.set(to, Some(Slot::Member(me)));
                self.own = to;
                Merged::Renumbered { from: id, to }
            }
            // The device that held it moves itself once it hears this record.
            Some(_) => changed,
        };
        if let Some(other) = elsewhere {
            self.set(other, None);
        }
        self.set(id, Some(Slot::Member(record)));
        if matches!(outcome, Merged::Renumbered { .. }) {
            // The others need this node's new record; the one heard is no news to them.
            self.unsent &= !(1 << id);
        }
        outcome
    }

    /// Merges a gone record heard from another node, at UTC `now`, 0 when unknown.
    pub fn merge_gone(&mut self, id: u8, gone: Gone, now: u32) -> Merged {
        if id >= IDS || gone.public == self.me().public || is_ahead(gone.changed, now) {
            // A device that leaves knows it; one removed is shown it and leaves by itself.
            return Merged::Unchanged;
        }
        let held = (0..IDS).find(|&at| {
            self.member(at)
                .is_some_and(|member| member.public == gone.public)
        });
        if let Some(at) = held {
            let member = self.member(at).expect("found above");
            if member.changed > gone.changed {
                // Paired again since: the sender needs the newer record.
                self.unsent |= 1 << at;
                return Merged::Unchanged;
            }
            self.set(at, Some(Slot::Gone(gone)));
            return Merged::Went { at };
        }
        match self.gone_of(&gone.public) {
            Some((_, ours)) if ours.changed >= gone.changed => return Merged::Unchanged,
            Some((place, _)) => self.forget(place),
            None => {}
        }
        match self.slot(id).copied() {
            None => {
                self.set(id, Some(Slot::Gone(gone)));
                Merged::Went { at: id }
            }
            Some(Slot::Gone(theirs)) if gone.outranks(&theirs) => {
                self.keep_former(id, theirs);
                self.set(id, Some(Slot::Gone(gone)));
                Merged::Went { at: id }
            }
            // A member holds the id, or a gone record that every node keeps over this one.
            Some(_) => {
                self.keep_former(id, gone);
                Merged::Unchanged
            }
        }
    }

    /// Whether a record changed here and has not been sent since.
    #[must_use]
    pub fn has_unsent(&self) -> bool {
        self.unsent() != 0 || self.former_unsent != 0
    }

    /// Counts the slot at `id` as sent, when a packet that reached this node's neighbours
    /// carried it as this node holds it.
    pub fn covered(&mut self, id: u8, slot: &Slot) {
        if self.slot(id) == Some(slot) {
            self.unsent &= !(1 << id);
        }
    }

    /// Marks the slots held of the ids in the set `ids` to be sent, as another node asked.
    pub(crate) fn ask(&mut self, ids: u32) {
        self.unsent |= ids & self.held();
    }

    /// The ids whose slots are to be sent, as a set.
    #[must_use]
    pub fn unsent(&self) -> u32 {
        self.unsent & self.held()
    }

    /// Counts the slot at `id` as sent, once a packet carries it.
    pub fn sent(&mut self, id: u8) {
        self.unsent &= !(1 << id);
    }

    /// The gone records kept apart from the slots that are to be sent, with their places.
    pub fn former_unsent(&self) -> impl Iterator<Item = (usize, u8, Gone)> + '_ {
        self.former
            .iter()
            .enumerate()
            .filter(|(at, _)| self.former_unsent & 1 << at != 0)
            .filter_map(|(at, former)| former.map(|(id, gone)| (at, id, gone)))
    }

    pub fn former_sent(&mut self, at: usize) {
        self.former_unsent &= !(1 << at);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Held {
    Slot(u8),
    Former(usize),
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
        // A sender held as gone is telling the others it left.
        if group.slot(sender).is_none() {
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
        g.covered(0, &Slot::Member(member(1, 299)));
        assert!(g.has_unsent(), "an older record covers nothing");
        g.covered(0, &Slot::Member(member(1, 300)));
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

    fn gone(n: u8, changed: u32) -> Gone {
        Gone {
            public: [n; 32],
            changed,
        }
    }

    #[test]
    fn a_record_stamped_far_ahead_is_refused() {
        let mut g = group(0, &[(0, 1), (1, 2)]);
        assert_eq!(g.merge_gone(1, gone(2, 10_000), 1_000), Merged::Unchanged);
        assert_eq!(g.merge(1, member(2, 10_000), 1_000), Merged::Unchanged);
        assert_eq!(
            g.merge(1, member(2, 1_000 + AHEAD_S), 1_000),
            Merged::Changed { vacated: None }
        );
        assert_eq!(
            g.merge_gone(1, gone(2, 5_000), 0),
            Merged::Went { at: 1 },
            "no clock"
        );
    }

    #[test]
    fn an_undone_removal_gives_the_member_its_id_back() {
        let mut g = group(0, &[(0, 1), (1, 2)]);
        g.remove(&fingerprint(&[2; 32]), 500);
        assert!(
            !g.forget_gone(1, &fingerprint(&[3; 32])),
            "another device's"
        );
        assert!(g.forget_gone(1, &fingerprint(&[2; 32])));
        assert!(g.slot(1).is_none());
        assert_eq!(
            g.merge(1, member(2, 100), 0),
            Merged::Changed { vacated: None }
        );
    }

    #[test]
    fn a_gone_record_reads_back_as_it_was_written() {
        let mut body = [0; GONE_LEN];
        gone(7, 1_790_000_123).encode(9, &mut body);
        assert_eq!(Gone::decode(&body), Some((9, gone(7, 1_790_000_123))));
        assert_eq!(Gone::decode(&body[..GONE_LEN - 1]), None);
        body[0] = 32;
        assert_eq!(Gone::decode(&body), None);
    }

    #[test]
    fn a_member_that_went_frees_its_id() {
        let mut g = group(0, &[(0, 1), (1, 2), (2, 3)]);
        let before = g.digest();
        assert_eq!(g.merge_gone(1, gone(2, 200), 0), Merged::Went { at: 1 });
        assert!(g.member(1).is_none());
        assert_eq!(g.gone(1), Some(&gone(2, 200)));
        assert_eq!(g.lowest_free(), Some(1));
        assert_eq!(g.count(), 2);
        assert_eq!(g.ids(), 1 | 1 << 2);
        assert_eq!(g.unsent(), 1 << 1, "it passes the news on");
        assert_ne!(g.digest(), before);
        assert_eq!(g.merge_gone(1, gone(2, 200), 0), Merged::Unchanged);
    }

    #[test]
    fn a_gone_record_finds_its_member_at_another_id() {
        let mut g = group(0, &[(0, 1), (4, 2)]);
        assert_eq!(g.merge_gone(1, gone(2, 200), 0), Merged::Went { at: 4 });
        assert!(g.slot(1).is_none());
    }

    #[test]
    fn a_sender_held_as_gone_is_not_asked_for_its_record() {
        let mut g = group(0, &[(0, 1), (1, 2)]);
        g.merge_gone(1, gone(2, 200), 0);
        let mut requests = Requests::default();
        let ours = g.digest();
        requests.heard(&mut g, 1, Some(ours), None);
        assert_eq!(requests.pending(), 0);
    }

    #[test]
    fn a_stale_record_of_a_gone_member_is_refused_and_answered() {
        let mut g = group(0, &[(0, 1), (1, 2)]);
        g.merge_gone(1, gone(2, 200), 0);
        g.sent(1);
        assert_eq!(g.merge(1, member(2, 150), 0), Merged::Unchanged);
        assert!(g.member(1).is_none());
        assert_eq!(g.unsent(), 1 << 1, "the gone record goes back");
    }

    #[test]
    fn a_device_paired_again_wins_over_its_gone_record() {
        let mut g = group(0, &[(0, 1), (1, 2)]);
        g.merge_gone(1, gone(2, 200), 0);
        assert_eq!(
            g.merge(1, member(2, 300), 0),
            Merged::Changed { vacated: None }
        );
        assert_eq!(g.member(1), Some(&member(2, 300)));
        assert_eq!(g.merge_gone(1, gone(2, 250), 0), Merged::Unchanged);
        assert_eq!(g.member(1), Some(&member(2, 300)));

        let mut g = group(0, &[(0, 1), (1, 2)]);
        g.merge_gone(1, gone(2, 200), 0);
        assert_eq!(
            g.merge(3, member(2, 300), 0),
            Merged::Changed { vacated: Some(1) }
        );
        assert!(g.slot(1).is_none());
    }

    #[test]
    fn a_new_member_at_a_gone_id_keeps_the_gone_record_apart() {
        let mut g = group(0, &[(0, 1), (1, 2)]);
        g.merge_gone(1, gone(2, 200), 0);
        g.enrol(1, member(9, 300));
        assert_eq!(g.member(1), Some(&member(9, 300)));
        assert_eq!(g.former_unsent().count(), 0);
        assert_eq!(g.merge(1, member(2, 150), 0), Merged::Unchanged);
        assert_eq!(g.member(1), Some(&member(9, 300)));
        let mut former = g.former_unsent();
        assert_eq!(former.next(), Some((0, 1, gone(2, 200))));
        assert_eq!(former.next(), None);
        drop(former);
        g.former_sent(0);
        assert_eq!(g.former_unsent().count(), 0);
    }

    #[test]
    fn pairing_a_gone_device_again_forgets_its_gone_record() {
        let mut g = group(0, &[(0, 1), (1, 2)]);
        g.merge_gone(1, gone(2, 200), 0);
        g.enrol(3, member(2, 300));
        assert!(g.slot(1).is_none());
        assert_eq!(g.merge(3, member(2, 300), 0), Merged::Unchanged);
    }

    #[test]
    fn a_removal_replaces_the_member_its_fingerprint_names() {
        let mut g = group(0, &[(0, 1), (5, 2)]);
        assert_eq!(g.remove(&fingerprint(&[2; 32]), 500), Some(5));
        assert_eq!(g.gone(5), Some(&gone(2, 500)));
        assert_eq!(g.remove(&fingerprint(&[2; 32]), 600), None, "already gone");
        assert_eq!(g.remove(&fingerprint(&[1; 32]), 600), None, "never itself");
        let mut g = group(0, &[(0, 1), (5, 2)]);
        assert_eq!(
            g.remove(&fingerprint(&[2; 32]), 50),
            Some(5),
            "a gone record is newer than the record it replaces"
        );
        assert_eq!(g.gone(5), Some(&gone(2, 101)));
    }

    #[test]
    fn declining_a_removal_after_its_switch_forgets_what_changed_since() {
        let mut g = group(0, &[(0, 1), (1, 2), (2, 3), (3, 4)]);
        g.remove(&fingerprint(&[3; 32]), 500);
        // Since the switch, a device joined at the id it freed and a member renamed.
        g.enrol(2, member(9, 0));
        g.merge(1, member(2, 550), 0);
        g.rename(Name::from_mac(&[0, 0, 0, 0, 0, 7]), 700);
        let changed = g.take_changed();
        assert_eq!(changed, 1 << 0 | 1 << 1 | 1 << 2);
        assert!(g.former.iter().any(Option::is_some));
        assert_eq!(g.forget_changed(changed), 1 << 1 | 1 << 2);
        assert!(g.slot(1).is_none() && g.slot(2).is_none());
        assert_eq!(g.member(3), Some(&member(4, 100)));
        assert_eq!(g.me().changed, 700, "this node's own record stays");
        assert!(
            g.former.iter().all(Option::is_none),
            "the gone record set apart is forgotten too"
        );
        assert_eq!(
            g.merge(2, member(3, 100), 0),
            Merged::Changed { vacated: None },
            "the member removed comes back"
        );
    }

    #[test]
    fn a_gone_record_about_this_node_is_ignored() {
        let mut g = group(0, &[(0, 1), (1, 2)]);
        assert_eq!(g.merge_gone(0, gone(1, 999), 0), Merged::Unchanged);
        assert_eq!(g.merge_gone(3, gone(1, 999), 0), Merged::Unchanged);
        assert!(g.member(0).is_some());
        assert_eq!(g.leaving(50), gone(1, 101));
    }

    #[test]
    fn a_gone_record_of_a_stranger_takes_only_a_free_id() {
        let mut g = group(0, &[(0, 1), (1, 2)]);
        assert_eq!(g.merge_gone(4, gone(8, 200), 0), Merged::Went { at: 4 });
        assert_eq!(g.merge_gone(1, gone(9, 200), 0), Merged::Unchanged);
        assert_eq!(g.member(1), Some(&member(2, 100)));
        assert_eq!(
            g.merge(1, member(9, 150), 0),
            Merged::Unchanged,
            "kept apart, it still answers"
        );
    }

    #[test]
    fn every_node_keeps_the_same_of_two_gone_records_for_an_id() {
        let (a, b) = (gone(8, 200), gone(9, 300));
        let mut first = group(0, &[(0, 1)]);
        first.merge_gone(4, a, 0);
        first.merge_gone(4, b, 0);
        let mut second = group(0, &[(0, 1)]);
        second.merge_gone(4, b, 0);
        second.merge_gone(4, a, 0);
        assert_eq!(first.gone(4), Some(&b));
        assert_eq!(second.gone(4), Some(&b));
        assert_eq!(first.digest(), second.digest());
    }

    #[test]
    fn the_digest_tells_a_gone_record_from_a_member() {
        let mut a = group(0, &[(0, 1), (1, 2)]);
        let mut b = group(0, &[(0, 1), (1, 2)]);
        a.merge_gone(1, gone(2, 200), 0);
        assert_ne!(a.digest(), b.digest());
        b.merge_gone(1, gone(2, 200), 0);
        assert_eq!(a.digest(), b.digest());
    }

    #[test]
    fn a_stored_group_keeps_its_gone_records_and_generation() {
        let mut slots = [None; SLOTS];
        slots[0] = Some(Slot::Member(member(1, 100)));
        slots[3] = Some(Slot::Gone(gone(2, 200)));
        let g = Group::restore(Key::new([5; 32]), 7, 0, slots).unwrap();
        assert_eq!(g.generation(), 7);
        assert_eq!(g.gone(3), Some(&gone(2, 200)));
        assert_eq!(g.lowest_free(), Some(1));
        slots[0] = Some(Slot::Gone(gone(1, 300)));
        assert!(
            Group::restore(Key::new([5; 32]), 7, 0, slots).is_none(),
            "a group needs this node's record"
        );
    }
}
