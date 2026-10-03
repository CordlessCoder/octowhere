//! Removing a member: a new group key, sent to each remaining member in a key message and taken
//! up at a switch round, and the old key kept until every remaining member has been heard on the
//! new one (`context/LORA-PROTOCOL.md`, "Removing a member").

use bytemuck::Zeroable;
use sha2::{Digest, Sha256};

use crate::IDS;
use crate::members::{Group, fingerprint};
use crate::messages::{Message, To, kind};
use crate::schedule::ROUND_US;
use crate::seal::Key;

/// A key message's plaintext: its kind, the key, its generation, the switch round, and the id
/// and fingerprint of the member removed.
pub const KEY_LEN: usize = 1 + 32 + 2 + 4 + 1 + 8;
/// The key messages a remover sends in each packet, which sets how far off the switch is.
pub const KEYS_PER_PACKET: u32 = 2;
/// Rounds past the remover's own key messages for them to cross the hops.
pub const HOP_ROUNDS: u32 = 3;
/// The older keys kept for members not yet heard on a newer one.
pub const OLD_KEYS: usize = 4;
/// The rounds a device that learns of a removal has to decline it, however late it learns.
pub const DECLINE_ROUNDS: u32 = 3;
/// A round's length in seconds.
const ROUND_S: u32 = (ROUND_US / 1_000_000) as u32;
/// The declined keys remembered, so a key sent again does not ask again.
pub const DECLINED: usize = 4;
/// How long after its switch a removal can still be declined: the key before it is kept that
/// long.
pub const UNDO_ROUNDS: u32 = 24 * 3_600 / ROUND_S;

/// A new group key, as a key message carries it.
#[derive(Clone, PartialEq, Eq)]
pub struct NewKey {
    pub key: Key,
    pub generation: u16,
    /// The round the group switches at, on its timebase.
    pub switch: u32,
    /// The id of the member removed, and its fingerprint.
    pub removed: u8,
    pub fingerprint: [u8; 8],
}

impl core::fmt::Debug for NewKey {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("NewKey")
            .field("generation", &self.generation)
            .field("switch", &self.switch)
            .field("removed", &self.removed)
            .finish_non_exhaustive()
    }
}

impl NewKey {
    #[must_use]
    pub fn encode(&self) -> [u8; KEY_LEN] {
        let mut out = [0; KEY_LEN];
        out[0] = kind::KEY;
        out[1..33].copy_from_slice(self.key.bytes());
        out[33..35].copy_from_slice(&self.generation.to_be_bytes());
        out[35..39].copy_from_slice(&self.switch.to_be_bytes());
        out[39] = self.removed;
        out[40..48].copy_from_slice(&self.fingerprint);
        out
    }

    /// Reads a key message's plaintext, its kind included.
    #[must_use]
    pub fn decode(plain: &[u8]) -> Option<Self> {
        let plain = plain.get(..KEY_LEN)?;
        if plain[0] != kind::KEY || plain[39] >= IDS {
            return None;
        }
        Some(Self {
            key: Key::new(plain[1..33].try_into().ok()?),
            generation: u16::from_be_bytes(plain[33..35].try_into().ok()?),
            switch: u32::from_be_bytes(plain[35..39].try_into().ok()?),
            removed: plain[39],
            fingerprint: plain[40..48].try_into().ok()?,
        })
    }

    /// Of two keys of one generation, every node keeps the one with the lower hash.
    fn rank(&self) -> [u8; 32] {
        rank(&self.key)
    }
}

fn rank(key: &Key) -> [u8; 32] {
    Sha256::digest(key.bytes()).into()
}

/// Whether generation `a` is later than `b`, wrapping: up to half the range ahead is later.
fn is_newer(a: u16, b: u16) -> bool {
    a != b && a.wrapping_sub(b) <= u16::MAX / 2
}

/// How many rounds from now a removal's switch is, for a remover sending `messages` key and
/// removal messages.
#[must_use]
pub const fn switch_rounds(messages: u32) -> u32 {
    messages.div_ceil(KEYS_PER_PACKET) + HOP_ROUNDS + 1
}

/// The furthest a key message's switch can be past the round it arrives in: as far as a removal
/// from the largest group needs, and a round for clocks that disagree.
pub const SWITCH_AHEAD: u32 = switch_rounds(IDS as u32) + 1;

/// A removal learned and not yet switched to.
#[derive(Clone, Debug)]
pub struct Pending {
    pub new: NewKey,
    pub remover: u8,
    /// The round this device switches at: the key's, or later, so that a removal learned late
    /// still leaves its user [`DECLINE_ROUNDS`] to decline it.
    pub switch: u32,
}

/// A key the group switched away from, kept for the members not yet heard on a newer one.
#[derive(Clone)]
pub struct Old {
    pub key: Key,
    pub generation: u16,
    /// The members not yet heard on a newer key, as a set.
    pub waiting: u32,
}

/// The last switch: which member it removed, so that a rival key that wins after it can give
/// that member its place back.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Last {
    pub generation: u16,
    pub removed: u8,
    pub fingerprint: [u8; 8],
}

/// The key a removal's switch left, kept for [`UNDO_ROUNDS`] so that this device's user can
/// still decline the removal and go back to it.
#[derive(Clone)]
pub struct Undo {
    pub key: Key,
    pub generation: u16,
    /// The generation the removal switched to. A rival of it has the same.
    pub new_generation: u16,
    /// The group's switch round: a message stamped from its start was made under a newer key.
    pub switched: u32,
    /// The last round the removal can be declined in.
    pub until: u32,
    /// The ids whose slots changed since this device's switch, as a set.
    pub changed: u32,
    /// The id of the member removed.
    pub removed: u8,
    /// Whether another member made the removal. This device's own is kept only so that a rival
    /// winning over it can still be declined.
    pub theirs: bool,
}

/// What declining a removal after its switch did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct Reverted {
    /// From when, in timebase seconds, messages were made under the key left; the caller
    /// forgets them.
    pub since: u32,
    /// The ids whose records were forgotten, as a set, the member removed among them.
    pub forgotten: u32,
}

/// What a key message did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Learned {
    /// Nothing: it is held already, declined, stale, too far ahead, or loses to a rival.
    Ignored,
    /// A removal to show this device's user, who can decline it until this device switches.
    Pending,
}

/// A device's removals: the one under way, the older keys kept, and those declined.
#[derive(Clone, Default)]
pub struct Rekey {
    pending: Option<Pending>,
    /// Newest first.
    old: [Option<Old>; OLD_KEYS],
    declined: [Option<[u8; 8]>; DECLINED],
    /// The removal this device made last, by its key's generation and the id removed, which a
    /// rival key of that generation undoes.
    removing: Option<(u16, u8)>,
    last: Option<Last>,
    undo: Option<Undo>,
}

impl Rekey {
    #[must_use]
    pub fn pending(&self) -> Option<&Pending> {
        self.pending.as_ref()
    }

    pub fn old(&self) -> impl Iterator<Item = &Old> {
        self.old.iter().flatten()
    }

    pub fn declined(&self) -> impl Iterator<Item = &[u8; 8]> {
        self.declined.iter().flatten()
    }

    /// The last round the removal last switched to can be declined in.
    #[must_use]
    pub fn undo_until(&self) -> Option<u32> {
        self.undo
            .as_ref()
            .filter(|undo| undo.theirs)
            .map(|undo| undo.until)
    }

    /// Notes the ids, a set, whose slots changed, which declining the last removal forgets.
    /// Returns whether that changed what is kept.
    pub fn changed(&mut self, ids: u32) -> bool {
        match &mut self.undo {
            Some(undo) if undo.changed | ids != undo.changed => {
                undo.changed |= ids;
                true
            }
            _ => false,
        }
    }

    /// Whether some member has not been heard on the newest key since a switch.
    #[must_use]
    pub fn is_waiting(&self) -> bool {
        self.old().any(|old| old.waiting != 0)
    }

    /// Starts removing the member `id` with the fresh key `key`, in round `round`. Returns the
    /// new key, which goes to every remaining member, or `None` when `id` is not another
    /// member or a removal is under way.
    pub fn start(&mut self, group: &Group, id: u8, key: Key, round: u32) -> Option<NewKey> {
        if self.pending.is_some() || id == group.own() {
            return None;
        }
        let removed = group.member(id)?;
        let remaining = group.ids() & !(1 << id) & !(1 << group.own());
        let new = NewKey {
            key,
            generation: group.generation().wrapping_add(1),
            switch: round + switch_rounds(remaining.count_ones() + 1),
            removed: id,
            fingerprint: fingerprint(&removed.public),
        };
        self.pending = Some(Pending {
            new: new.clone(),
            remover: group.own(),
            switch: new.switch,
        });
        self.removing = Some((new.generation, id));
        Some(new)
    }

    /// Takes a key message from `remover`, opened, in round `round`. A key more than one
    /// generation ahead is taken only while `lagging`, when this node has heard no member on its
    /// key for a while: otherwise a member could skip past every other removal.
    pub fn learned(
        &mut self,
        group: &Group,
        remover: u8,
        new: NewKey,
        round: u32,
        lagging: bool,
    ) -> Learned {
        if remover == group.own()
            || group.member(remover).is_none()
            || new.fingerprint == fingerprint(&group.me().public)
            || !group.names(&new.fingerprint)
            || new.switch > round.saturating_add(SWITCH_AHEAD)
            || self
                .declined()
                .any(|declined| *declined == fingerprint(new.key.bytes()))
        {
            return Learned::Ignored;
        }
        let current = group.generation();
        match new.generation.wrapping_sub(current) {
            // A rival of the key the group switched to: the lower hash wins.
            0 if new.key == *group.key() || new.rank() >= rank(group.key()) => {
                return Learned::Ignored;
            }
            0 | 1 => {}
            // Wrapping: a generation up to half the range ahead is newer.
            ahead if lagging && ahead <= u16::MAX / 2 => {}
            _ => return Learned::Ignored,
        }
        if let Some(pending) = &self.pending
            && pending.new.generation == new.generation
            && (pending.new.key == new.key || pending.new.rank() <= new.rank())
        {
            return Learned::Ignored;
        }
        let switch = new.switch.max(round.saturating_add(DECLINE_ROUNDS));
        self.pending = Some(Pending {
            new,
            remover,
            switch,
        });
        Learned::Pending
    }

    /// Declines the removal under way, which another member asked for. Returns whether there
    /// was one to decline.
    pub fn decline(&mut self, group: &Group) -> bool {
        match &self.pending {
            Some(pending) if pending.remover != group.own() => {
                let key = pending.new.key.clone();
                self.decline_key(&key);
                self.pending = None;
                true
            }
            _ => false,
        }
    }

    fn decline_key(&mut self, key: &Key) {
        self.declined.rotate_right(1);
        self.declined[0] = Some(fingerprint(key.bytes()));
    }

    /// Whether the removal under way switches by round `round`.
    #[must_use]
    pub fn is_due(&self, round: u32) -> bool {
        self.pending
            .as_ref()
            .is_some_and(|pending| round >= pending.switch)
    }

    /// Declines the removal of the member `removed` that this device last switched for, in
    /// round `round`, up to [`UNDO_ROUNDS`] after its switch: `group` goes back to the key before
    /// it, and the member has its id back. Its record, and those of the ids that changed since,
    /// come again from the nodes that never switched. Returns `None` with no such removal to
    /// decline, or while another removal is under way.
    pub fn undo(&mut self, group: &mut Group, round: u32, removed: u8) -> Option<Reverted> {
        if self.pending.is_some() {
            return None;
        }
        let undo = self
            .undo
            .take_if(|undo| undo.theirs && undo.removed == removed && round <= undo.until)?;
        // Every key switched to since, rivals included, is not to be taken again.
        self.decline_key(group.key());
        for at in 0..OLD_KEYS {
            if let Some(old) = self.old[at]
                .take_if(|old| old.key == undo.key || is_newer(old.generation, undo.generation))
                && old.key != undo.key
            {
                self.decline_key(&old.key);
            }
        }
        self.compact();
        let changed = undo.changed | group.take_changed();
        let forgotten = group.forget_changed(changed);
        group.take_changed();
        group.rekey(undo.key, undo.generation);
        self.last = None;
        self.removing = None;
        Some(Reverted {
            since: undo.switched.saturating_mul(ROUND_S),
            forgotten,
        })
    }

    /// Drops the key kept to decline the last removal once round `round` is past its day.
    /// Returns whether it did.
    pub fn expire(&mut self, round: u32) -> bool {
        self.undo.take_if(|undo| round > undo.until).is_some()
    }

    /// Switches `group` to the pending key: the member removed goes, as of the start of the
    /// key's switch round, the same on every node, and the old key is kept for every other
    /// member until it is heard on the new one.
    pub fn switch(&mut self, group: &mut Group) -> Option<Switched> {
        let pending = self.pending.take()?;
        let new = pending.new;
        let ours = pending.remover == group.own();
        let rival = new.generation == group.generation();
        // What changed before the switch is no business of declining it.
        group.take_changed();
        let again = self.last.filter(|last| {
            rival && last.generation == new.generation && last.fingerprint == new.fingerprint
        });
        let mut restored = None;
        if rival && again.is_none() {
            // A rival that won after the switch: the removal the losing key made is undone, and
            // its member's record comes back from the nodes that never took that key.
            if let Some(last) = self.last.filter(|last| last.generation == new.generation)
                && group.forget_gone(last.removed, &last.fingerprint)
            {
                restored = Some(last.removed);
            }
        }
        // This device's removal lost to a rival of its generation, before the switch or after,
        // that removes another member.
        let undone = match self.removing {
            Some((generation, id))
                if !ours && generation == new.generation && id != new.removed =>
            {
                1 << id
            }
            _ => 0,
        };
        let at = new.switch.saturating_mul(ROUND_S);
        // A rival removing the member the losing key removed leaves it gone.
        let removed = match again {
            Some(last) => Some(last.removed),
            None => group.remove(&new.fingerprint, at),
        };
        let changed = group.take_changed();
        let of_undo = |undo: &Undo| rival && undo.new_generation == new.generation;
        self.undo = match (self.undo.take(), removed) {
            // A rival keeps the key before both, and the day the first switch gave.
            (Some(undo), Some(removed)) if of_undo(&undo) => Some(Undo {
                switched: undo.switched.min(new.switch),
                changed: undo.changed | changed,
                removed,
                theirs: !ours,
                ..undo
            }),
            // A rival that removes nobody gave the member back: there is nothing to decline.
            (Some(undo), None) if of_undo(&undo) => None,
            // A key that removes nobody leaves the last removal as it was to decline.
            (Some(undo), None) => Some(Undo {
                changed: undo.changed | changed,
                ..undo
            }),
            (_, Some(removed)) => Some(Undo {
                key: group.key().clone(),
                generation: group.generation(),
                new_generation: new.generation,
                switched: new.switch,
                until: pending.switch.saturating_add(UNDO_ROUNDS),
                changed,
                removed,
                theirs: !ours,
            }),
            (None, None) => None,
        };
        let waiting = group.ids() & !(1 << group.own());
        if waiting != 0 {
            self.old.rotate_right(1);
            self.old[0] = Some(Old {
                key: group.key().clone(),
                generation: group.generation(),
                waiting,
            });
        }
        group.rekey(new.key, new.generation);
        self.removing = ours.then_some((new.generation, new.removed));
        self.last = removed.map(|removed| Last {
            generation: new.generation,
            removed,
            fingerprint: new.fingerprint,
        });
        Some(Switched {
            removed,
            remover: pending.remover,
            restored,
            undone,
        })
    }

    /// Takes a packet from `sender` under the newest key: it needs no older one. Returns
    /// whether that changed what is kept.
    pub fn heard(&mut self, sender: u8) -> bool {
        let mut changed = false;
        for old in self.old.iter_mut() {
            if let Some(held) = old
                && held.waiting & 1 << sender != 0
            {
                changed = true;
                held.waiting &= !(1 << sender);
                if held.waiting == 0 {
                    *old = None;
                }
            }
        }
        if changed {
            self.compact();
        }
        changed
    }

    /// Keeps the old keys newest first, with the gaps after.
    fn compact(&mut self) {
        let mut kept = 0;
        for at in 0..OLD_KEYS {
            if let Some(old) = self.old[at].take() {
                self.old[kept] = Some(old);
                kept += 1;
            }
        }
    }

    /// Whether `sender` is a member still waited for under the old key with generation
    /// `generation`.
    #[must_use]
    pub fn is_waiting_for(&self, generation: u16, sender: u8) -> bool {
        self.old()
            .any(|old| old.generation == generation && old.waiting & 1 << sender != 0)
    }

    /// The id this device removed last, if a rival key could still undo it.
    #[must_use]
    pub fn removing(&self) -> Option<u8> {
        self.removing.map(|(_, id)| id)
    }

    /// Forgets every removal, as leaving the group does.
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    /// Drops a member that went from the members waited for. Returns whether that changed
    /// what is kept.
    pub fn went(&mut self, id: u8) -> bool {
        self.heard(id)
    }
}

/// The most bytes [`Rekey::encode`] writes.
pub const STORED_MAX: usize = 1
    + 1
    + 4
    + KEY_LEN
    + 1
    + OLD_KEYS * (32 + 2 + 4)
    + 1
    + DECLINED * 8
    + 4
    + 1
    + 2
    + 1
    + 8
    + 1
    + 32
    + 2
    + 2
    + 4
    + 4
    + 4
    + 1
    + 1;

impl Rekey {
    /// Writes what a restart has to keep: the removal under way, the old keys with the members
    /// each waits for, the keys declined, this device's removals, the last switch, and the key
    /// kept to decline it.
    pub fn encode(&self, out: &mut [u8; STORED_MAX]) -> usize {
        let mut len = 0;
        let mut put = |bytes: &[u8]| {
            out[len..len + bytes.len()].copy_from_slice(bytes);
            len += bytes.len();
        };
        match &self.pending {
            Some(pending) => {
                put(&[1, pending.remover]);
                put(&pending.switch.to_be_bytes());
                put(&pending.new.encode());
            }
            None => put(&[0]),
        }
        put(&[self.old().count() as u8]);
        for old in self.old() {
            put(old.key.bytes());
            put(&old.generation.to_be_bytes());
            put(&old.waiting.to_be_bytes());
        }
        put(&[self.declined().count() as u8]);
        for declined in self.declined() {
            put(declined);
        }
        match self.removing {
            Some((generation, id)) => {
                put(&[1]);
                put(&generation.to_be_bytes());
                put(&[id]);
            }
            None => put(&[0, 0, 0, 0]),
        }
        match &self.last {
            Some(last) => {
                put(&[1]);
                put(&last.generation.to_be_bytes());
                put(&[last.removed]);
                put(&last.fingerprint);
            }
            None => put(&[0]),
        }
        match &self.undo {
            Some(undo) => {
                put(&[1]);
                put(undo.key.bytes());
                put(&undo.generation.to_be_bytes());
                put(&undo.new_generation.to_be_bytes());
                put(&undo.switched.to_be_bytes());
                put(&undo.until.to_be_bytes());
                put(&undo.changed.to_be_bytes());
                put(&[undo.removed, u8::from(undo.theirs)]);
            }
            None => put(&[0]),
        }
        len
    }

    #[must_use]
    pub fn decode(bytes: &[u8]) -> Option<Self> {
        let mut rest = bytes;
        let mut take = |n: usize| -> Option<&[u8]> {
            let (taken, after) = rest.split_at_checked(n)?;
            rest = after;
            Some(taken)
        };
        let mut rekey = Self::default();
        if take(1)?[0] == 1 {
            let remover = take(1)?[0];
            let switch = u32::from_be_bytes(take(4)?.try_into().ok()?);
            rekey.pending = Some(Pending {
                new: NewKey::decode(take(KEY_LEN)?)?,
                remover,
                switch,
            });
        }
        let count = usize::from(take(1)?[0]);
        if count > OLD_KEYS {
            return None;
        }
        for at in 0..count {
            let key = Key::new(take(32)?.try_into().ok()?);
            let generation = u16::from_be_bytes(take(2)?.try_into().ok()?);
            let waiting = u32::from_be_bytes(take(4)?.try_into().ok()?);
            rekey.old[at] = Some(Old {
                key,
                generation,
                waiting,
            });
        }
        let count = usize::from(take(1)?[0]);
        if count > DECLINED {
            return None;
        }
        for at in 0..count {
            rekey.declined[at] = Some(take(8)?.try_into().ok()?);
        }
        let removing = take(4)?;
        if removing[0] == 1 {
            rekey.removing = Some((u16::from_be_bytes([removing[1], removing[2]]), removing[3]));
        }
        if take(1)?[0] == 1 {
            rekey.last = Some(Last {
                generation: u16::from_be_bytes(take(2)?.try_into().ok()?),
                removed: take(1)?[0],
                fingerprint: take(8)?.try_into().ok()?,
            });
        }
        // Missing from what was stored before declining after a switch was possible.
        if take(1).is_some_and(|flag| flag[0] == 1) {
            rekey.undo = Some(Undo {
                key: Key::new(take(32)?.try_into().ok()?),
                generation: u16::from_be_bytes(take(2)?.try_into().ok()?),
                new_generation: u16::from_be_bytes(take(2)?.try_into().ok()?),
                switched: u32::from_be_bytes(take(4)?.try_into().ok()?),
                until: u32::from_be_bytes(take(4)?.try_into().ok()?),
                changed: u32::from_be_bytes(take(4)?.try_into().ok()?),
                removed: take(1)?[0],
                theirs: take(1)?[0] == 1,
            });
        }
        rest.is_empty().then_some(rekey)
    }
}

/// The newest key message to each member, kept past the message horizon to catch up a member
/// that missed a switch. All zeroes is none kept.
#[derive(Zeroable)]
#[repr(C)]
pub struct Kept {
    messages: [Message; IDS as usize],
}

impl Kept {
    /// Keeps `message` if it is a key message newer than the one kept for its destination.
    pub fn keep(&mut self, message: &Message) {
        if let To::Member(dest) = message.to()
            && message.is_key()
        {
            let held = &mut self.messages[usize::from(dest)];
            if held.seq == 0 || held.stamp <= message.stamp {
                *held = *message;
            }
        }
    }

    #[must_use]
    pub fn get(&self, id: u8) -> Option<&Message> {
        self.messages
            .get(usize::from(id))
            .filter(|held| held.seq != 0)
    }

    pub fn clear(&mut self) {
        *self = Self::zeroed();
    }
}

/// What a switch did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct Switched {
    pub removed: Option<u8>,
    pub remover: u8,
    /// The member the losing key of a rival removed, given its place back.
    pub restored: Option<u8>,
    /// The members this device removed under the losing key, as a set: it removes them again
    /// under the new key once their records are back.
    pub undone: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::members::tests::member;
    use crate::members::{Gone, Slot, fingerprint};

    fn group(own: u8, ids: &[(u8, u8)]) -> Group {
        let mut slots = [None; IDS as usize];
        for &(id, n) in ids {
            slots[usize::from(id)] = Some(Slot::Member(member(n, 100)));
        }
        Group::restore(Key::new([5; 32]), 3, own, slots).unwrap()
    }

    /// Holds the device `of` as gone from id `id`: a key that names it removes nobody.
    fn left(g: &mut Group, id: u8, of: u8) {
        g.merge_gone(
            id,
            Gone {
                public: [of; 32],
                changed: 50,
            },
            0,
        );
    }

    fn new(n: u8, generation: u16, switch: u32, removed: u8, of: u8) -> NewKey {
        NewKey {
            key: Key::new([n; 32]),
            generation,
            switch,
            removed,
            fingerprint: fingerprint(&[of; 32]),
        }
    }

    #[test]
    fn a_key_message_reads_back() {
        let key = new(7, 9, 1_000, 4, 2);
        assert_eq!(NewKey::decode(&key.encode()), Some(key.clone()));
        let mut wrong = key.encode();
        wrong[0] = kind::TEXT;
        assert_eq!(NewKey::decode(&wrong), None);
        assert_eq!(NewKey::decode(&key.encode()[..KEY_LEN - 1]), None);
    }

    #[test]
    fn the_switch_is_as_far_off_as_the_removers_messages_take() {
        // Six key messages and the removal notice, two a packet, then hops.
        assert_eq!(switch_rounds(7), 4 + HOP_ROUNDS + 1);
        let g = group(0, &[(0, 1), (1, 2), (2, 3), (3, 4)]);
        let mut rekey = Rekey::default();
        let started = rekey.start(&g, 2, Key::new([9; 32]), 1_000).unwrap();
        assert_eq!(started.generation, 4);
        assert_eq!(started.removed, 2);
        assert_eq!(started.fingerprint, fingerprint(&[3; 32]));
        assert_eq!(started.switch, 1_000 + switch_rounds(3));
        assert!(
            rekey.start(&g, 3, Key::new([8; 32]), 1_000).is_none(),
            "one at a time"
        );
        assert!(
            Rekey::default()
                .start(&g, 0, Key::new([9; 32]), 1_000)
                .is_none()
        );
        assert!(
            Rekey::default()
                .start(&g, 9, Key::new([9; 32]), 1_000)
                .is_none()
        );
    }

    #[test]
    fn a_removal_switches_the_group_and_keeps_the_old_key_for_the_rest() {
        let mut g = group(0, &[(0, 1), (1, 2), (2, 3), (3, 4)]);
        let mut rekey = Rekey::default();
        assert_eq!(
            rekey.learned(&g, 1, new(9, 4, 1_010, 2, 3), 1_000, false),
            Learned::Pending
        );
        assert!(!rekey.is_due(1_009));
        assert!(rekey.is_due(1_010));
        let switched = rekey.switch(&mut g).unwrap();
        assert_eq!(switched.removed, Some(2));
        assert_eq!(switched.remover, 1);
        assert_eq!(g.generation(), 4);
        assert!(*g.key() == Key::new([9; 32]));
        assert_eq!(
            g.gone(2).unwrap().changed,
            1_010 * ROUND_S,
            "gone as of the switch round's start, the same on every node"
        );
        let old = rekey.old().next().unwrap();
        assert!(old.key == Key::new([5; 32]));
        assert_eq!(old.waiting, 1 << 1 | 1 << 3);
        assert!(rekey.is_waiting_for(3, 3));
        rekey.heard(1);
        assert!(rekey.is_waiting());
        rekey.heard(3);
        assert!(
            !rekey.is_waiting(),
            "dropped once everyone is heard on the new key"
        );
        assert_eq!(rekey.old().count(), 0);
    }

    #[test]
    fn a_switch_with_no_member_left_keeps_no_old_key() {
        let mut g = group(0, &[(0, 1), (1, 2)]);
        let mut rekey = Rekey::default();
        rekey.start(&g, 1, Key::new([9; 32]), 1_000).unwrap();
        let switched = rekey.switch(&mut g).unwrap();
        assert_eq!(switched.removed, Some(1));
        assert_eq!(rekey.old().count(), 0);
        assert!(!rekey.is_waiting());
    }

    #[test]
    fn a_late_key_message_still_leaves_time_to_decline() {
        let mut g = group(0, &[(0, 1), (1, 2), (2, 3)]);
        let mut rekey = Rekey::default();
        assert_eq!(
            rekey.learned(&g, 1, new(9, 4, 1_010, 2, 3), 1_020, false),
            Learned::Pending
        );
        assert!(!rekey.is_due(1_020 + DECLINE_ROUNDS - 1));
        assert!(rekey.is_due(1_020 + DECLINE_ROUNDS));
        assert!(rekey.switch(&mut g).is_some());
        assert_eq!(g.gone(2).unwrap().changed, 1_010 * ROUND_S);
    }

    #[test]
    fn a_key_skipping_generations_is_taken_only_by_a_lagging_node() {
        let g = group(0, &[(0, 1), (1, 2), (2, 3)]);
        // A switch round long past would otherwise switch at once, past every decline.
        let forced = new(9, 104, 0, 1, 2);
        let mut rekey = Rekey::default();
        assert_eq!(
            rekey.learned(&g, 2, forced.clone(), 1_000, false),
            Learned::Ignored
        );
        assert_eq!(
            rekey.learned(&g, 2, new(9, 5, 1_010, 1, 2), 1_000, false),
            Learned::Ignored
        );
        assert_eq!(rekey.learned(&g, 2, forced, 1_000, true), Learned::Pending);
        assert!(
            !rekey.is_due(1_000 + DECLINE_ROUNDS - 1),
            "even then its user can decline"
        );
    }

    #[test]
    fn a_key_message_from_a_stranger_or_of_an_old_generation_is_ignored() {
        let g = group(0, &[(0, 1), (1, 2), (2, 3)]);
        let mut rekey = Rekey::default();
        assert_eq!(
            rekey.learned(&g, 7, new(9, 4, 1_010, 2, 3), 1_000, false),
            Learned::Ignored
        );
        assert_eq!(
            rekey.learned(&g, 1, new(9, 2, 1_010, 2, 3), 1_000, true),
            Learned::Ignored
        );
        assert_eq!(
            rekey.learned(&g, 1, new(5, 3, 1_010, 2, 3), 1_000, false),
            Learned::Ignored,
            "the current key is no rival of itself"
        );
        assert_eq!(
            rekey.learned(&g, 1, new(9, 4, 1_010, 0, 1), 1_000, false),
            Learned::Ignored,
            "a key message never removes the device it goes to"
        );
        assert!(rekey.pending().is_none());
    }

    #[test]
    fn a_key_naming_nobody_or_switching_too_far_ahead_is_ignored() {
        let mut g = group(0, &[(0, 1), (1, 2), (2, 3)]);
        let mut rekey = Rekey::default();
        assert_eq!(
            rekey.learned(&g, 1, new(9, 4, 1_010, 2, 99), 1_000, false),
            Learned::Ignored,
            "names nobody"
        );
        assert_eq!(
            rekey.learned(&g, 1, new(9, 4, u32::MAX, 2, 3), 1_000, false),
            Learned::Ignored,
            "never due"
        );
        assert_eq!(
            rekey.learned(
                &g,
                1,
                new(9, 4, 1_000 + SWITCH_AHEAD + 1, 2, 3),
                1_000,
                false
            ),
            Learned::Ignored
        );
        assert_eq!(
            rekey.learned(&g, 1, new(9, 4, 1_000 + SWITCH_AHEAD, 2, 3), 1_000, false),
            Learned::Pending
        );
        left(&mut g, 6, 99);
        let mut rekey = Rekey::default();
        assert_eq!(
            rekey.learned(&g, 1, new(9, 4, 1_010, 6, 99), 1_000, false),
            Learned::Pending,
            "a member gone by the time the key arrives"
        );
    }

    #[test]
    fn a_declined_removal_is_not_asked_again() {
        let mut g = group(0, &[(0, 1), (1, 2), (2, 3)]);
        let mut rekey = Rekey::default();
        rekey.learned(&g, 1, new(9, 4, 1_010, 2, 3), 1_000, false);
        assert!(rekey.decline(&g));
        assert!(rekey.pending().is_none());
        assert!(!rekey.is_due(2_000));
        assert!(rekey.switch(&mut g).is_none());
        assert_eq!(
            rekey.learned(&g, 1, new(9, 4, 1_010, 2, 3), 1_100, true),
            Learned::Ignored
        );
        assert_eq!(g.generation(), 3);
    }

    #[test]
    fn the_remover_cannot_decline_its_own_removal() {
        let g = group(0, &[(0, 1), (1, 2), (2, 3)]);
        let mut rekey = Rekey::default();
        rekey.start(&g, 2, Key::new([9; 32]), 1_000).unwrap();
        assert!(!rekey.decline(&g));
        assert!(rekey.pending().is_some());
    }

    /// Two keys of generation 4, the lower ranked first.
    fn rivals() -> (u8, u8) {
        let (a, b) = (Key::new([20; 32]), Key::new([21; 32]));
        if rank(&a) < rank(&b) {
            (20, 21)
        } else {
            (21, 20)
        }
    }

    #[test]
    fn of_two_removals_at_once_the_lower_key_wins_everywhere() {
        let (low, high) = rivals();
        // Before either switch: the lower replaces the higher, and not the other way round.
        let g = group(0, &[(0, 1), (1, 2), (2, 3), (3, 4)]);
        let mut rekey = Rekey::default();
        rekey.learned(&g, 1, new(high, 4, 1_010, 2, 3), 1_000, false);
        assert_eq!(
            rekey.learned(&g, 3, new(low, 4, 1_012, 1, 2), 1_000, false),
            Learned::Pending
        );
        assert_eq!(rekey.pending().unwrap().remover, 3);
        assert_eq!(
            rekey.learned(&g, 1, new(high, 4, 1_010, 2, 3), 1_000, false),
            Learned::Ignored
        );

        // After switching to the higher: the lower still wins, and the member the higher
        // removed has its id back.
        let mut g = group(0, &[(0, 1), (1, 2), (2, 3), (3, 4)]);
        let mut rekey = Rekey::default();
        rekey.learned(&g, 1, new(high, 4, 1_010, 2, 3), 1_000, false);
        rekey.switch(&mut g).unwrap();
        assert!(g.gone(2).is_some());
        assert_eq!(
            rekey.learned(&g, 3, new(low, 4, 1_012, 1, 2), 1_020, false),
            Learned::Pending
        );
        let switched = rekey.switch(&mut g).unwrap();
        assert_eq!(switched.removed, Some(1));
        assert_eq!(switched.restored, Some(2));
        assert!(g.slot(2).is_none(), "its record comes back from the others");
        assert!(*g.key() == Key::new([low; 32]));
        assert_eq!(
            rekey.old().count(),
            2,
            "both older keys wait for their members"
        );
    }

    #[test]
    fn a_remover_whose_removal_lost_removes_again() {
        // Before the switch.
        let mut g = group(0, &[(0, 1), (1, 2), (2, 3), (3, 4)]);
        let mut rekey = Rekey::default();
        let mine = rekey.start(&g, 2, Key::new([30; 32]), 1_000).unwrap();
        let rival = (31..=u8::MAX)
            .map(|n| new(n, 4, 1_012, 3, 4))
            .find(|rival| rival.rank() < mine.rank())
            .unwrap();
        assert_eq!(
            rekey.learned(&g, 1, rival.clone(), 1_001, false),
            Learned::Pending
        );
        let switched = rekey.switch(&mut g).unwrap();
        assert_eq!(switched.removed, Some(3));
        assert_eq!(switched.undone, 1 << 2, "2 is a member again");
        assert_eq!(rekey.removing(), None);

        // After the switch.
        let mut g = group(0, &[(0, 1), (1, 2), (2, 3), (3, 4)]);
        let mut rekey = Rekey::default();
        rekey.start(&g, 2, Key::new([30; 32]), 1_000).unwrap();
        rekey.switch(&mut g).unwrap();
        assert_eq!(rekey.removing(), Some(2));
        assert_eq!(rekey.learned(&g, 1, rival, 1_020, false), Learned::Pending);
        let switched = rekey.switch(&mut g).unwrap();
        assert_eq!(switched.restored, Some(2));
        assert_eq!(switched.undone, 1 << 2);

        // A later removal by another member undoes nothing.
        let mut g = group(0, &[(0, 1), (1, 2), (2, 3), (3, 4)]);
        let mut rekey = Rekey::default();
        rekey.start(&g, 2, Key::new([30; 32]), 1_000).unwrap();
        rekey.switch(&mut g).unwrap();
        rekey.learned(&g, 1, new(40, 5, 1_030, 3, 4), 1_020, false);
        assert_eq!(rekey.switch(&mut g).unwrap().undone, 0);
    }

    #[test]
    fn a_removal_can_be_declined_for_a_day_after_its_switch() {
        let mut g = group(0, &[(0, 1), (1, 2), (2, 3), (3, 4)]);
        let mut rekey = Rekey::default();
        rekey.learned(&g, 1, new(9, 4, 1_010, 2, 3), 1_000, false);
        rekey.switch(&mut g).unwrap();
        let until = 1_010 + UNDO_ROUNDS;
        assert_eq!(rekey.undo_until(), Some(until));
        // A member renamed after the switch, under the new key.
        g.merge(3, member(4, 2_000), 0);
        assert!(rekey.changed(g.take_changed()));
        assert_eq!(
            rekey.undo(&mut g, until, 3),
            None,
            "names the member removed"
        );
        assert_eq!(
            rekey.undo(&mut g, until, 2),
            Some(Reverted {
                since: 1_010 * ROUND_S,
                forgotten: 1 << 2 | 1 << 3,
            })
        );
        assert_eq!(g.generation(), 3);
        assert!(*g.key() == Key::new([5; 32]));
        assert!(g.slot(2).is_none() && g.slot(3).is_none(), "both come back");
        assert_eq!(rekey.old().count(), 0, "the key gone back to is current");
        assert_eq!(
            rekey.learned(&g, 1, new(9, 4, 1_010, 2, 3), until, true),
            Learned::Ignored,
            "the key left is not taken again"
        );
        assert_eq!(rekey.undo(&mut g, until, 2), None);
        assert_eq!(rekey.undo_until(), None);
    }

    #[test]
    fn a_removal_cannot_be_declined_after_its_day() {
        let mut g = group(0, &[(0, 1), (1, 2), (2, 3)]);
        let mut rekey = Rekey::default();
        rekey.learned(&g, 1, new(9, 4, 1_010, 2, 3), 1_020, false);
        rekey.switch(&mut g).unwrap();
        let until = 1_020 + DECLINE_ROUNDS + UNDO_ROUNDS;
        assert_eq!(rekey.undo_until(), Some(until), "a day from its own switch");
        assert_eq!(rekey.undo(&mut g, until + 1, 2), None);
        assert_eq!(g.generation(), 4);
        assert!(!rekey.expire(until));
        assert!(rekey.expire(until + 1));
        assert_eq!(rekey.undo_until(), None);
    }

    #[test]
    fn only_a_removal_another_member_made_can_be_declined_after_it() {
        let mut g = group(0, &[(0, 1), (1, 2), (2, 3)]);
        let mut rekey = Rekey::default();
        rekey.start(&g, 2, Key::new([9; 32]), 1_000).unwrap();
        rekey.switch(&mut g).unwrap();
        assert_eq!(rekey.undo_until(), None);
        assert_eq!(rekey.undo(&mut g, 1_010, 2), None);

        // Nor while the next is under way, and only the last.
        let mut g = group(0, &[(0, 1), (1, 2), (2, 3), (3, 4)]);
        let mut rekey = Rekey::default();
        rekey.learned(&g, 1, new(9, 4, 1_010, 2, 3), 1_000, false);
        rekey.switch(&mut g).unwrap();
        rekey.learned(&g, 1, new(10, 5, 1_030, 3, 4), 1_020, false);
        assert_eq!(rekey.undo(&mut g, 1_020, 2), None);
        rekey.switch(&mut g).unwrap();
        assert_eq!(rekey.undo(&mut g, 1_040, 2), None);
        assert_eq!(
            rekey
                .undo(&mut g, 1_040, 3)
                .map(|reverted| reverted.forgotten),
            Some(1 << 3)
        );
        assert_eq!(g.generation(), 4);
        assert_eq!(g.gone(2).map(|gone| gone.changed), Some(1_010 * ROUND_S));
    }

    #[test]
    fn declining_after_a_rival_won_goes_back_before_both() {
        let (low, high) = rivals();
        let mut g = group(0, &[(0, 1), (1, 2), (2, 3), (3, 4)]);
        let mut rekey = Rekey::default();
        rekey.learned(&g, 1, new(high, 4, 1_010, 2, 3), 1_000, false);
        rekey.switch(&mut g).unwrap();
        rekey.learned(&g, 3, new(low, 4, 1_012, 1, 2), 1_020, false);
        rekey.switch(&mut g).unwrap();
        assert_eq!(
            rekey.undo_until(),
            Some(1_010 + UNDO_ROUNDS),
            "a rival gives no more time"
        );
        let reverted = rekey.undo(&mut g, 1_030, 1).unwrap();
        assert_eq!(reverted.since, 1_010 * ROUND_S, "from the first switch");
        assert_eq!(reverted.forgotten, 1 << 1);
        assert_eq!(g.generation(), 3);
        assert!(*g.key() == Key::new([5; 32]));
        for rival in [high, low] {
            assert_eq!(
                rekey.learned(&g, 3, new(rival, 4, 1_012, 1, 2), 1_030, false),
                Learned::Ignored
            );
        }
    }

    #[test]
    fn a_rival_removing_the_same_member_leaves_it_gone() {
        let (low, high) = rivals();
        let mut g = group(0, &[(0, 1), (1, 2), (2, 3), (3, 4)]);
        let mut rekey = Rekey::default();
        rekey.learned(&g, 1, new(high, 4, 1_010, 2, 3), 1_000, false);
        rekey.switch(&mut g).unwrap();
        rekey.learned(&g, 3, new(low, 4, 1_012, 2, 3), 1_020, false);
        let switched = rekey.switch(&mut g).unwrap();
        assert_eq!(switched.removed, Some(2));
        assert_eq!(switched.restored, None);
        assert!(g.gone(2).is_some());
        assert_eq!(rekey.undo_until(), Some(1_010 + UNDO_ROUNDS));
        assert!(rekey.undo(&mut g, 1_030, 2).is_some());
        assert!(g.slot(2).is_none());
    }

    #[test]
    fn a_rival_from_another_member_over_this_devices_removal_can_be_declined() {
        let mut g = group(0, &[(0, 1), (1, 2), (2, 3), (3, 4)]);
        let mut rekey = Rekey::default();
        let mine = rekey.start(&g, 2, Key::new([30; 32]), 1_000).unwrap();
        rekey.switch(&mut g).unwrap();
        assert_eq!(rekey.undo_until(), None);
        let rival = (31..=u8::MAX)
            .map(|n| new(n, 4, 1_012, 3, 4))
            .find(|rival| rival.rank() < mine.rank())
            .unwrap();
        rekey.learned(&g, 1, rival, 1_020, false);
        rekey.switch(&mut g).unwrap();
        assert_eq!(rekey.undo_until(), Some(mine.switch + UNDO_ROUNDS));
        rekey.undo(&mut g, 1_030, 3).unwrap();
        assert!(*g.key() == Key::new([5; 32]));
        assert_eq!(g.generation(), 3);
        assert_eq!(rekey.removing(), None);
    }

    #[test]
    fn a_key_that_removes_nobody_leaves_the_last_removal_to_decline() {
        let mut g = group(0, &[(0, 1), (1, 2), (2, 3), (3, 4)]);
        left(&mut g, 6, 99);
        let mut rekey = Rekey::default();
        rekey.learned(&g, 1, new(9, 4, 1_010, 2, 3), 1_000, false);
        rekey.switch(&mut g).unwrap();
        rekey.learned(&g, 1, new(10, 5, 1_020, 2, 99), 1_015, false);
        assert_eq!(rekey.switch(&mut g).unwrap().removed, None);
        assert_eq!(rekey.undo_until(), Some(1_010 + UNDO_ROUNDS));
        rekey.undo(&mut g, 1_030, 2).unwrap();
        assert_eq!(g.generation(), 3);
        assert!(*g.key() == Key::new([5; 32]));
        assert_eq!(rekey.old().count(), 0);
        for key in [new(9, 4, 1_010, 2, 3), new(10, 5, 1_020, 2, 99)] {
            assert_eq!(rekey.learned(&g, 1, key, 1_030, true), Learned::Ignored);
        }
    }

    #[test]
    fn a_rival_that_removes_nobody_leaves_nothing_to_decline() {
        let (low, high) = rivals();
        let mut g = group(0, &[(0, 1), (1, 2), (2, 3), (3, 4)]);
        left(&mut g, 6, 99);
        let mut rekey = Rekey::default();
        rekey.learned(&g, 1, new(high, 4, 1_010, 2, 3), 1_000, false);
        rekey.switch(&mut g).unwrap();
        rekey.learned(&g, 3, new(low, 4, 1_012, 2, 99), 1_020, false);
        assert_eq!(rekey.switch(&mut g).unwrap().restored, Some(2));
        assert_eq!(rekey.undo_until(), None);
        assert_eq!(rekey.undo(&mut g, 1_030, 2), None);
        assert_eq!(g.generation(), 4);
    }

    #[test]
    fn declining_a_rival_of_a_key_that_removed_nobody_goes_back_to_that_key() {
        let (low, high) = rivals();
        let mut g = group(0, &[(0, 1), (1, 2), (2, 3), (3, 4), (4, 5)]);
        left(&mut g, 6, 99);
        let mut rekey = Rekey::default();
        rekey.learned(&g, 1, new(9, 4, 1_010, 2, 3), 1_000, false);
        rekey.switch(&mut g).unwrap();
        rekey.learned(&g, 1, new(high, 5, 1_020, 2, 99), 1_015, false);
        assert_eq!(rekey.switch(&mut g).unwrap().removed, None);
        rekey.learned(&g, 3, new(low, 5, 1_022, 4, 5), 1_025, false);
        rekey.switch(&mut g).unwrap();
        assert_eq!(
            rekey.undo(&mut g, 1_030, 2),
            None,
            "4's removal is the last"
        );
        rekey.undo(&mut g, 1_030, 4).unwrap();
        assert_eq!(g.generation(), 5);
        assert!(*g.key() == Key::new([high; 32]));
        assert!(g.gone(2).is_some(), "2's removal stands");
    }

    #[test]
    fn a_rival_removing_the_member_this_device_removed_undoes_nothing() {
        // After the switch.
        let mut g = group(0, &[(0, 1), (1, 2), (2, 3), (3, 4)]);
        let mut rekey = Rekey::default();
        let mine = rekey.start(&g, 2, Key::new([30; 32]), 1_000).unwrap();
        rekey.switch(&mut g).unwrap();
        let rival = (31..=u8::MAX)
            .map(|n| new(n, 4, 1_012, 2, 3))
            .find(|rival| rival.rank() < mine.rank())
            .unwrap();
        rekey.learned(&g, 1, rival.clone(), 1_020, false);
        let switched = rekey.switch(&mut g).unwrap();
        assert_eq!((switched.removed, switched.undone), (Some(2), 0));
        assert!(g.gone(2).is_some());

        // Before it.
        let mut g = group(0, &[(0, 1), (1, 2), (2, 3), (3, 4)]);
        let mut rekey = Rekey::default();
        rekey.start(&g, 2, Key::new([30; 32]), 1_000).unwrap();
        rekey.learned(&g, 1, rival, 1_001, false);
        assert_eq!(rekey.switch(&mut g).unwrap().undone, 0);
    }

    #[test]
    fn declining_keeps_an_older_rival_of_the_key_gone_back_to() {
        let mut g = group(0, &[(0, 1), (1, 2), (2, 3), (3, 4)]);
        let mut rekey = Rekey::default();
        rekey.learned(&g, 1, new(9, 4, 1_010, 2, 3), 1_000, false);
        rekey.old[0] = Some(Old {
            key: Key::new([20; 32]),
            generation: 3,
            waiting: 1 << 3,
        });
        rekey.switch(&mut g).unwrap();
        assert_eq!(rekey.old().count(), 2);
        rekey.undo(&mut g, 1_020, 2).unwrap();
        assert_eq!(rekey.old().count(), 1);
        assert!(
            rekey.old().all(|old| old.key == Key::new([20; 32])),
            "its members still wait on it"
        );
    }

    #[test]
    fn what_a_restart_keeps_reads_back() {
        let mut g = group(0, &[(0, 1), (1, 2), (2, 3), (3, 4)]);
        let mut rekey = Rekey::default();
        let mut empty = [0; STORED_MAX];
        let len = rekey.encode(&mut empty);
        assert!(Rekey::decode(&empty[..len]).is_some_and(|read| read.pending().is_none()));

        rekey.start(&g, 2, Key::new([9; 32]), 1_000).unwrap();
        rekey.switch(&mut g).unwrap();
        rekey.learned(&g, 3, new(10, 5, 1_030, 1, 2), 1_020, false);
        rekey.decline(&g);
        rekey.learned(&g, 3, new(11, 5, 1_031, 1, 2), 1_040, false);
        let mut out = [0; STORED_MAX];
        let len = rekey.encode(&mut out);
        let read = Rekey::decode(&out[..len]).unwrap();
        let mut again = [0; STORED_MAX];
        assert_eq!(read.encode(&mut again), len);
        assert_eq!(again[..len], out[..len]);
        let pending = read.pending().unwrap();
        assert_eq!(pending.new, new(11, 5, 1_031, 1, 2));
        assert_eq!(pending.switch, 1_040 + DECLINE_ROUNDS);
        assert_eq!(read.old().count(), 1);
        assert_eq!(read.declined().count(), 1);
        assert_eq!(read.removing(), Some(2));
        assert!(Rekey::decode(&out[..len - 2]).is_none());

        // The key kept to decline a removal after its switch, and its absence from what was
        // stored before there was one.
        let mut g = group(0, &[(0, 1), (1, 2), (2, 3)]);
        let mut rekey = Rekey::default();
        rekey.learned(&g, 1, new(9, 4, 1_010, 2, 3), 1_000, false);
        rekey.switch(&mut g).unwrap();
        g.merge(1, member(2, 2_000), 0);
        rekey.changed(g.take_changed());
        let len = rekey.encode(&mut out);
        let mut read = Rekey::decode(&out[..len]).unwrap();
        let undo = read.undo.as_ref().unwrap();
        assert!(undo.key == Key::new([5; 32]));
        assert_eq!(
            (
                undo.generation,
                undo.new_generation,
                undo.switched,
                undo.until
            ),
            (3, 4, 1_010, 1_010 + UNDO_ROUNDS)
        );
        assert_eq!(
            (undo.changed, undo.removed, undo.theirs),
            (1 << 1 | 1 << 2, 2, true)
        );
        assert!(read.undo(&mut g, 1_010, 2).is_some());
        assert!(*g.key() == Key::new([5; 32]));
        assert_eq!(g.generation(), 3);
        let mut before = rekey.clone();
        before.undo = None;
        let len = before.encode(&mut out);
        assert!(Rekey::decode(&out[..len - 1]).is_some_and(|read| read.undo_until().is_none()));
    }

    #[test]
    fn the_newest_key_message_to_each_member_is_kept() {
        extern crate std;
        let key = Key::new([1; 32]);
        let mut kept = std::boxed::Box::new(Kept::zeroed());
        let older = Message::private(1, 4, true, 5, 0, 100, &[kind::KEY], &key).unwrap();
        let newer = Message::private(2, 4, true, 9, 0, 200, &[kind::KEY], &key).unwrap();
        let text = Message::private(2, 5, false, 10, 9, 300, &[kind::TEXT], &key).unwrap();
        kept.keep(&newer);
        kept.keep(&older);
        kept.keep(&text);
        assert_eq!(kept.get(4), Some(&newer));
        assert_eq!(kept.get(5), None, "only key messages");
        kept.clear();
        assert_eq!(kept.get(4), None);
    }
}
