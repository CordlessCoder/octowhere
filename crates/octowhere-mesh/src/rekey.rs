//! Removing a member: a new group key, sent to each remaining member in a key message and taken
//! up at a switch round, and the old key kept until every remaining member has been heard on the
//! new one (`context/LORA-PROTOCOL.md`, "Removing a member").

use bytemuck::Zeroable;
use sha2::{Digest, Sha256};

use crate::IDS;
use crate::identity::{Identity, SIGNATURE_LEN, verify};
use crate::members::{Group, Member, PUBLIC_LEN, RECORD_MAX_LEN, fingerprint};
use crate::messages::{Message, To, kind};
use crate::schedule::ROUND_US;
use crate::seal::Key;

/// A key message's plaintext: its kind, the key, its generation, the switch round, the id and
/// fingerprint of the member removed, and the fingerprint of the key it follows.
pub const KEY_LEN: usize = 1 + 32 + 2 + 4 + 1 + 8 + 8;
/// The key messages a remover sends in each packet, which sets how far off the switch is. A
/// signed key message takes most of a packet.
pub const KEYS_PER_PACKET: u32 = 1;
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

const ON_KEY_DOMAIN: &[u8] = b"octowhere on key";
/// An [`OnKey`] record's body: id, generation, signature.
pub const ON_KEY_LEN: usize = 1 + 2 + SIGNATURE_LEN;

/// A member's signed word that it is on the key of a generation. It alone ends the wait for the
/// member under older keys: a header's sender id proves nothing, since any member can send
/// under another's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OnKey {
    pub id: u8,
    pub generation: u16,
    pub signature: [u8; SIGNATURE_LEN],
}

impl OnKey {
    /// `me`'s word, at `id`, that it is on `key`, of `generation`.
    #[must_use]
    pub fn new(id: u8, generation: u16, key: &Key, me: &Identity) -> Self {
        Self {
            id,
            generation,
            signature: me.sign(&[ON_KEY_DOMAIN, &[id], &generation.to_be_bytes(), key.bytes()]),
        }
    }

    /// Whether the device with `public` signed it for `key`, the key of its generation.
    #[must_use]
    pub fn verify(&self, public: &[u8; PUBLIC_LEN], key: &Key) -> bool {
        verify(
            public,
            &[
                ON_KEY_DOMAIN,
                &[self.id],
                &self.generation.to_be_bytes(),
                key.bytes(),
            ],
            &self.signature,
        )
    }

    pub fn encode(&self, out: &mut [u8; ON_KEY_LEN]) {
        out[0] = self.id;
        out[1..3].copy_from_slice(&self.generation.to_be_bytes());
        out[3..].copy_from_slice(&self.signature);
    }

    #[must_use]
    pub fn decode(body: &[u8]) -> Option<Self> {
        let body: &[u8; ON_KEY_LEN] = body.get(..ON_KEY_LEN)?.try_into().ok()?;
        Some(Self {
            id: body[0],
            generation: u16::from_be_bytes([body[1], body[2]]),
            signature: body[3..].try_into().ok()?,
        })
    }
}

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
    /// The fingerprint of the key it replaces: a node takes it only on that key.
    pub follows: [u8; 8],
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
        out[48..56].copy_from_slice(&self.follows);
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
            follows: plain[48..56].try_into().ok()?,
        })
    }
}

/// Whether the key `a` its remover made wins over `b`, of the same generation, on every node:
/// the lower remover's id wins, and of one remover's two, the lower hash.
fn beats((a_remover, a): (u8, &Key), (b_remover, b): (u8, &Key)) -> bool {
    let rank = |key: &Key| -> [u8; 32] { Sha256::digest(key.bytes()).into() };
    (a_remover, rank(a)) < (b_remover, rank(b))
}

/// The fingerprint of a group key, which a key message following it names.
#[must_use]
pub fn key_fingerprint(key: &Key) -> [u8; 8] {
    fingerprint(key.bytes())
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
    /// Whether a rival of its generation won over it, rather than a removal after it.
    pub lost: bool,
}

impl Old {
    /// The generation of the key message that catches up a member still on this key: the
    /// rival that won over it, or the removal after it.
    #[must_use]
    pub fn catches_up_with(&self) -> u16 {
        if self.lost {
            self.generation
        } else {
            self.generation.wrapping_add(1)
        }
    }
}

/// The last switch: which member it removed, so that a rival key that wins after it can give
/// that member its place back.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Last {
    pub generation: u16,
    pub removed: u8,
    pub fingerprint: [u8; 8],
    /// Its record before the switch, put back as it was. Missing from what was stored before
    /// it was kept.
    pub record: Option<Member>,
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
    /// Nothing: it is held already, declined, stale, switches too far ahead, or loses to a rival.
    Ignored,
    /// A removal to show this device's user, who can decline it until this device switches.
    Pending,
    /// Not yet: it follows a key this node does not hold, a rival that won elsewhere or a
    /// switch it missed. The caller tries it again once this node has switched.
    Later,
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
    /// The member that removed for each of the last generations switched to, newest first: a
    /// node keeps only that member's key messages of a generation for catch-up.
    removers: [Option<(u16, u8)>; OLD_KEYS],
    /// The fingerprint of the key the group's key followed, which a rival of it follows too.
    /// `None` when this device did not switch to it.
    follows: Option<[u8; 8]>,
    /// The members this device removed under a key a rival won over, to remove again, as a set.
    again: u32,
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

    /// The member the removal last switched to removed, while it can still be declined.
    #[must_use]
    pub fn undo_removed(&self) -> Option<u8> {
        self.undo
            .as_ref()
            .filter(|undo| undo.theirs)
            .map(|undo| undo.removed)
    }

    /// The removal last switched to, while this device can still decline it, and the member it
    /// removed as it was.
    #[must_use]
    pub fn declinable(&self) -> Option<(&Undo, Option<&Last>)> {
        let undo = self.undo.as_ref().filter(|undo| undo.theirs)?;
        let last = self
            .last
            .as_ref()
            .filter(|last| last.generation == undo.new_generation);
        Some((undo, last))
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

    /// The member that removed for `generation`, as this device switched to it.
    #[must_use]
    pub fn remover_of(&self, generation: u16) -> Option<u8> {
        self.removers
            .iter()
            .flatten()
            .find(|(held, _)| *held == generation)
            .map(|&(_, remover)| remover)
    }

    /// The members this device removed under a key a rival won over, as a set: it removes each
    /// again once that member's record is back.
    #[must_use]
    pub fn again(&self) -> u32 {
        self.again
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
            follows: key_fingerprint(group.key()),
        };
        self.pending = Some(Pending {
            new: new.clone(),
            remover: group.own(),
            switch: new.switch,
        });
        self.removing = Some((new.generation, id));
        self.again &= !(1 << id);
        Some(new)
    }

    /// Takes a key message from `remover`, opened, in round `round`. Only the key after the
    /// group's, or a rival of either that wins, is taken: a member that missed several switches
    /// takes them one at a time.
    pub fn learned(&mut self, group: &Group, remover: u8, new: NewKey, round: u32) -> Learned {
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
            // A rival of the key the group switched to, which a device that did not switch to it
            // cannot rank.
            0 => {
                let rival = self.follows == Some(new.follows)
                    && new.key != *group.key()
                    && self
                        .remover_of(current)
                        .is_some_and(|ours| beats((remover, &new.key), (ours, group.key())));
                if !rival {
                    return Learned::Ignored;
                }
            }
            1 if new.follows == key_fingerprint(group.key()) => {}
            // Wrapping: a generation up to half the range ahead is newer.
            ahead if ahead <= u16::MAX / 2 => return Learned::Later,
            _ => return Learned::Ignored,
        }
        if let Some(pending) = &self.pending
            && pending.new.generation == new.generation
            && (pending.new.key == new.key
                || !beats((remover, &new.key), (pending.remover, &pending.new.key)))
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
        self.follows = None;
        // A rival this device lost to, made again, would only race the key gone back to.
        self.again = 0;
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
            // A rival that won after the switch: the removal the losing key made is undone.
            if let Some(last) = self.last.filter(|last| last.generation == new.generation)
                && group.put_back(last.removed, last.record, &last.fingerprint)
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
        let (removed, record) = match again {
            Some(last) => (Some(last.removed), last.record),
            None => {
                let record = group
                    .by_fingerprint(&new.fingerprint)
                    .and_then(|id| group.member(id).copied());
                (group.remove(&new.fingerprint, at), record)
            }
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
        self.wait_only_for(group.ids());
        let waiting = group.ids() & !(1 << group.own());
        if waiting != 0 {
            self.old.rotate_right(1);
            self.old[0] = Some(Old {
                key: group.key().clone(),
                generation: group.generation(),
                waiting,
                lost: rival,
            });
        }
        group.rekey(new.key, new.generation);
        self.follows = Some(new.follows);
        if let Some(at) = self
            .removers
            .iter()
            .position(|held| held.is_some_and(|(generation, _)| generation == new.generation))
        {
            // A rival's switch: its remover is that generation's now.
            self.removers[at] = None;
        }
        self.removers.rotate_right(1);
        self.removers[0] = Some((new.generation, pending.remover));
        self.removing = ours.then_some((new.generation, new.removed));
        self.again |= undone;
        self.last = removed.map(|removed| Last {
            generation: new.generation,
            removed,
            fingerprint: new.fingerprint,
            record,
        });
        Some(Switched {
            removed,
            remover: pending.remover,
            restored,
            undone,
        })
    }

    /// Takes a member's word, checked, that it is on the key of `generation`: it needs no key
    /// older. Returns whether that changed what is kept.
    pub fn on_key(&mut self, id: u8, generation: u16) -> bool {
        let mut changed = false;
        for old in self.old.iter_mut() {
            if let Some(held) = old
                && held.generation < generation
                && held.waiting & 1 << id != 0
            {
                changed = true;
                held.waiting &= !(1 << id);
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

    /// Stops waiting for anyone outside `members`, a set of ids: a member removed or gone is
    /// never heard on a newer key. Returns whether that changed what is kept.
    pub fn wait_only_for(&mut self, members: u32) -> bool {
        let mut changed = false;
        for old in self.old.iter_mut() {
            if let Some(held) = old
                && held.waiting & !members != 0
            {
                changed = true;
                held.waiting &= members;
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

    /// Whether `id` is a member still waited for under a key older than `generation`'s.
    #[must_use]
    pub fn is_waiting_for_under_older(&self, id: u8, generation: u16) -> bool {
        self.old()
            .any(|old| old.generation < generation && old.waiting & 1 << id != 0)
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
        self.wait_only_for(!(1 << id))
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
    + 1
    + 1
    + OLD_KEYS * 3
    + 1
    + 1
    + 8
    + 1
    + RECORD_MAX_LEN
    + 4;

impl Rekey {
    /// Writes what a restart has to keep: the removal under way, the old keys with the members
    /// each waits for and whether a rival won over each, the keys declined, this device's
    /// removals and those it makes again, the last switch with the record it removed, the key
    /// kept to decline it, who removed for the last generations, and what the group's key
    /// followed.
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
        put(&[self.removers.iter().flatten().count() as u8]);
        for (generation, remover) in self.removers.iter().flatten() {
            put(&generation.to_be_bytes());
            put(&[*remover]);
        }
        put(&[self
            .old()
            .enumerate()
            .fold(0, |lost, (at, old)| lost | u8::from(old.lost) << at)]);
        match &self.follows {
            Some(follows) => {
                put(&[1]);
                put(follows);
            }
            None => put(&[0]),
        }
        match self
            .last
            .and_then(|last| Some((last.removed, last.record?)))
        {
            Some((id, record)) => {
                let mut bytes = [0; RECORD_MAX_LEN];
                let record_len = record.encode(id, &mut bytes);
                put(&[record_len as u8]);
                put(&bytes[..record_len]);
            }
            None => put(&[0]),
        }
        put(&self.again.to_be_bytes());
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
                lost: false,
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
                record: None,
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
        // Missing, as the undo was, from what was stored before them.
        if let Some(count) = take(1).map(|count| usize::from(count[0])) {
            if count > OLD_KEYS {
                return None;
            }
            for at in 0..count {
                let generation = u16::from_be_bytes(take(2)?.try_into().ok()?);
                rekey.removers[at] = Some((generation, take(1)?[0]));
            }
        }
        // Missing, as the rest after it, from what was stored before rivals were ranked by
        // their removers.
        if let Some(lost) = take(1).map(|lost| lost[0]) {
            for (at, old) in rekey.old.iter_mut().enumerate() {
                if let Some(old) = old {
                    old.lost = lost & 1 << at != 0;
                }
            }
        }
        if take(1).is_some_and(|flag| flag[0] == 1) {
            rekey.follows = Some(take(8)?.try_into().ok()?);
        }
        if let Some(len) = take(1).map(|len| usize::from(len[0]))
            && len > 0
        {
            let (id, record) = Member::decode(take(len)?)?;
            match &mut rekey.last {
                Some(last) if last.removed == id => last.record = Some(record),
                _ => return None,
            }
        }
        if let Some(again) = take(4) {
            rekey.again = u32::from_be_bytes(again.try_into().ok()?);
        }
        rest.is_empty().then_some(rekey)
    }
}

/// The key messages kept to catch up a member that missed switches, past the message horizon:
/// for each member, the newest of each of the last [`OLD_KEYS`] generations. All zeroes is none
/// kept.
#[derive(Zeroable)]
#[repr(C)]
pub struct Kept {
    messages: [[Message; OLD_KEYS]; IDS as usize],
}

impl Kept {
    /// Keeps `message`, a key message whose signature the caller checked, in place of an older
    /// one of its generation, or of the oldest generation held. Of two members' for one
    /// generation, the one from `remover` stays, when the caller knows who removed for it, and
    /// otherwise the first.
    pub fn keep(&mut self, message: &Message, remover: Option<u8>) {
        let (To::Member(dest), Some(generation)) = (message.to(), message.generation()) else {
            return;
        };
        let Some(held) = self.messages.get_mut(usize::from(dest)) else {
            return;
        };
        let behind = |kept: &Message| {
            kept.generation()
                .map_or(u16::MAX, |kept| generation.wrapping_sub(kept))
        };
        let at = held
            .iter()
            .position(|kept| kept.seq != 0 && kept.generation() == Some(generation))
            .or_else(|| held.iter().position(|kept| kept.seq == 0))
            .or_else(|| {
                // Wrapping: up to half the range behind is older.
                (0..OLD_KEYS)
                    .filter(|&at| (1..=u16::MAX / 2).contains(&behind(&held[at])))
                    .max_by_key(|&at| behind(&held[at]))
            });
        let Some(at) = at else { return };
        let kept = &held[at];
        let replace = if kept.seq == 0 || kept.generation() != Some(generation) {
            true
        } else if kept.origin == message.origin {
            kept.stamp <= message.stamp
        } else {
            remover == Some(message.origin)
        };
        if replace {
            held[at] = *message;
        }
    }

    /// Drops the key messages of `generation` from any member but `remover`, once a switch has
    /// said who removed for it.
    pub fn keep_only(&mut self, generation: u16, remover: u8) {
        for kept in self.messages.iter_mut().flatten() {
            if kept.seq != 0 && kept.generation() == Some(generation) && kept.origin != remover {
                *kept = Message::zeroed();
            }
        }
    }

    /// The key message of `generation` kept for the member `id`.
    #[must_use]
    pub fn get(&self, id: u8, generation: u16) -> Option<&Message> {
        self.messages
            .get(usize::from(id))?
            .iter()
            .find(|kept| kept.seq != 0 && kept.generation() == Some(generation))
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
    use crate::members::tests::{member, signed};
    use crate::members::{Name, Slot, fingerprint};

    fn group(own: u8, ids: &[(u8, u8)]) -> Group {
        let mut slots = [None; IDS as usize];
        for &(id, n) in ids {
            slots[usize::from(id)] = Some(Slot::Member(member(n, 100)));
        }
        Group::restore(Key::new([5; 32]), 3, own, slots).unwrap()
    }

    /// Holds the device `of` as gone from id `id`: a key that names it removes nobody.
    fn left(g: &mut Group, id: u8, of: u8) {
        g.merge_gone(id, crate::members::tests::left(id, of, 50), 0);
    }

    /// A key of generation `generation` removing the device `of` at id `removed`, following the
    /// test group's key.
    fn new(n: u8, generation: u16, switch: u32, removed: u8, of: u8) -> NewKey {
        NewKey {
            key: Key::new([n; 32]),
            generation,
            switch,
            removed,
            fingerprint: fingerprint(&member(of, 0).public),
            follows: key_fingerprint(&Key::new([5; 32])),
        }
    }

    impl NewKey {
        /// The same key, following the key `n` instead.
        fn after(self, n: u8) -> Self {
            Self {
                follows: key_fingerprint(&Key::new([n; 32])),
                ..self
            }
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
        // Six key messages and the removal notice, one a packet, then hops.
        assert_eq!(switch_rounds(7), 7 + HOP_ROUNDS + 1);
        let g = group(0, &[(0, 1), (1, 2), (2, 3), (3, 4)]);
        let mut rekey = Rekey::default();
        let started = rekey.start(&g, 2, Key::new([9; 32]), 1_000).unwrap();
        assert_eq!(started.generation, 4);
        assert_eq!(started.removed, 2);
        assert_eq!(started.fingerprint, fingerprint(&member(3, 0).public));
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
            rekey.learned(&g, 1, new(9, 4, 1_010, 2, 3), 1_000),
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
        rekey.on_key(1, 4);
        assert!(rekey.is_waiting());
        rekey.on_key(3, 4);
        assert!(
            !rekey.is_waiting(),
            "dropped once everyone is heard on the new key"
        );
        assert_eq!(rekey.old().count(), 0);
    }

    #[test]
    fn a_member_removed_later_is_waited_for_on_no_key() {
        let mut g = group(0, &[(0, 1), (1, 2), (2, 3), (3, 4)]);
        let mut rekey = Rekey::default();
        rekey.start(&g, 2, Key::new([9; 32]), 1_000).unwrap();
        rekey.switch(&mut g).unwrap();
        rekey.start(&g, 3, Key::new([8; 32]), 1_100).unwrap();
        rekey.switch(&mut g).unwrap();
        assert_eq!(rekey.old().count(), 2);
        assert!(
            rekey.old().all(|old| old.waiting == 1 << 1),
            "the second removal's member is waited for on neither key"
        );
        rekey.on_key(1, 4);
        assert!(
            rekey.is_waiting_for(4, 1),
            "on the first new key, it still needs the second"
        );
        rekey.on_key(1, 5);
        assert!(!rekey.is_waiting());
    }

    #[test]
    fn a_word_on_a_key_reads_back_and_holds_only_for_its_signer_and_key() {
        let me = crate::members::tests::key(2);
        let key = Key::new([9; 32]);
        let on_key = OnKey::new(1, 4, &key, &me);
        let mut body = [0; ON_KEY_LEN];
        on_key.encode(&mut body);
        assert_eq!(OnKey::decode(&body), Some(on_key));
        assert_eq!(OnKey::decode(&body[..ON_KEY_LEN - 1]), None);
        assert!(on_key.verify(&me.public(), &key));
        assert!(
            !on_key.verify(&me.public(), &Key::new([8; 32])),
            "another key"
        );
        assert!(!on_key.verify(&crate::members::tests::key(3).public(), &key));
        assert!(!OnKey { id: 2, ..on_key }.verify(&me.public(), &key));
        assert!(
            !OnKey {
                generation: 5,
                ..on_key
            }
            .verify(&me.public(), &key)
        );
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
            rekey.learned(&g, 1, new(9, 4, 1_010, 2, 3), 1_020),
            Learned::Pending
        );
        assert!(!rekey.is_due(1_020 + DECLINE_ROUNDS - 1));
        assert!(rekey.is_due(1_020 + DECLINE_ROUNDS));
        assert!(rekey.switch(&mut g).is_some());
        assert_eq!(g.gone(2).unwrap().changed, 1_010 * ROUND_S);
    }

    #[test]
    fn a_key_following_a_key_not_held_waits() {
        let mut g = group(0, &[(0, 1), (1, 2), (2, 3), (3, 4)]);
        let mut rekey = Rekey::default();
        let next = new(9, 5, 1_020, 3, 4).after(8);
        assert_eq!(
            rekey.learned(&g, 2, next.clone(), 1_000),
            Learned::Later,
            "the switch to the key it follows comes first"
        );
        assert_eq!(
            rekey.learned(&g, 2, new(9, 4, 1_010, 3, 4).after(7), 1_000),
            Learned::Later,
            "it follows a key that won elsewhere"
        );
        assert!(rekey.pending().is_none());
        rekey.learned(&g, 2, new(8, 4, 1_010, 1, 2), 1_000);
        rekey.switch(&mut g).unwrap();
        assert_eq!(rekey.learned(&g, 2, next, 1_015), Learned::Pending);
        assert_eq!(
            rekey.learned(&g, 2, new(3, 2, 1_016, 3, 4), 1_015),
            Learned::Ignored,
            "stale"
        );
    }

    #[test]
    fn a_key_message_from_a_stranger_or_of_an_old_generation_is_ignored() {
        let g = group(0, &[(0, 1), (1, 2), (2, 3)]);
        let mut rekey = Rekey::default();
        assert_eq!(
            rekey.learned(&g, 7, new(9, 4, 1_010, 2, 3), 1_000),
            Learned::Ignored
        );
        assert_eq!(
            rekey.learned(&g, 1, new(9, 2, 1_010, 2, 3), 1_000),
            Learned::Ignored
        );
        assert_eq!(
            rekey.learned(&g, 1, new(5, 3, 1_010, 2, 3), 1_000),
            Learned::Ignored,
            "the current key is no rival of itself"
        );
        assert_eq!(
            rekey.learned(&g, 1, new(9, 4, 1_010, 0, 1), 1_000),
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
            rekey.learned(&g, 1, new(9, 4, 1_010, 2, 99), 1_000),
            Learned::Ignored,
            "names nobody"
        );
        assert_eq!(
            rekey.learned(&g, 1, new(9, 4, u32::MAX, 2, 3), 1_000),
            Learned::Ignored,
            "never due"
        );
        assert_eq!(
            rekey.learned(&g, 1, new(9, 4, 1_000 + SWITCH_AHEAD + 1, 2, 3), 1_000,),
            Learned::Ignored
        );
        assert_eq!(
            rekey.learned(&g, 1, new(9, 4, 1_000 + SWITCH_AHEAD, 2, 3), 1_000),
            Learned::Pending
        );
        left(&mut g, 6, 99);
        let mut rekey = Rekey::default();
        assert_eq!(
            rekey.learned(&g, 1, new(9, 4, 1_010, 6, 99), 1_000),
            Learned::Pending,
            "a member gone by the time the key arrives"
        );
    }

    #[test]
    fn a_declined_removal_is_not_asked_again() {
        let mut g = group(0, &[(0, 1), (1, 2), (2, 3)]);
        let mut rekey = Rekey::default();
        rekey.learned(&g, 1, new(9, 4, 1_010, 2, 3), 1_000);
        assert!(rekey.decline(&g));
        assert!(rekey.pending().is_none());
        assert!(!rekey.is_due(2_000));
        assert!(rekey.switch(&mut g).is_none());
        assert_eq!(
            rekey.learned(&g, 1, new(9, 4, 1_010, 2, 3), 1_100),
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

    #[test]
    fn of_two_removals_at_once_the_lower_removers_wins_everywhere() {
        // Member 3 removes 2, and member 1 removes 3.
        let losing = new(21, 4, 1_010, 2, 3);
        let winning = new(20, 4, 1_012, 3, 4);
        // Before either switch: the lower remover's replaces the higher's, and not the other way
        // round.
        let g = group(0, &[(0, 1), (1, 2), (2, 3), (3, 4)]);
        let mut rekey = Rekey::default();
        rekey.learned(&g, 3, losing.clone(), 1_000);
        assert_eq!(
            rekey.learned(&g, 1, winning.clone(), 1_000),
            Learned::Pending
        );
        assert_eq!(rekey.pending().unwrap().remover, 1);
        assert_eq!(
            rekey.learned(&g, 3, losing.clone(), 1_000),
            Learned::Ignored
        );

        // After switching to the higher's: the lower's still wins, and the member the higher's
        // removed has its record back as it was.
        let mut g = group(0, &[(0, 1), (1, 2), (2, 3), (3, 4)]);
        let record = *g.member(2).unwrap();
        let mut rekey = Rekey::default();
        rekey.learned(&g, 3, losing, 1_000);
        rekey.switch(&mut g).unwrap();
        assert!(g.gone(2).is_some());
        assert_eq!(
            rekey.learned(&g, 1, winning.clone(), 1_020),
            Learned::Pending
        );
        let switched = rekey.switch(&mut g).unwrap();
        assert_eq!(switched.removed, Some(3));
        assert_eq!(switched.restored, Some(2));
        assert_eq!(g.member(2), Some(&record));
        assert!(*g.key() == Key::new([20; 32]));
        assert_eq!(
            rekey.old().count(),
            2,
            "both older keys wait for their members"
        );
        let lost = rekey.old().next().unwrap();
        assert!(lost.key == Key::new([21; 32]) && lost.lost);
        assert!(
            rekey.old().all(|old| old.catches_up_with() == 4),
            "a member on either is sent the winner"
        );

        // Of one remover's two keys, the lower hash wins; a key following another is no rival.
        let lower = (0..=u8::MAX)
            .map(|n| new(n, 4, 1_014, 2, 3))
            .find(|key| beats((1, &key.key), (1, &winning.key)))
            .unwrap();
        assert_eq!(
            rekey.learned(&g, 1, lower.clone().after(7), 1_030),
            Learned::Ignored
        );
        assert_eq!(rekey.learned(&g, 1, lower, 1_030), Learned::Pending);
    }

    #[test]
    fn a_remover_whose_removal_lost_removes_again() {
        let members = [(0, 1), (1, 2), (2, 3), (3, 4)];
        // Member 1, a lower id than this device's, removes 0.
        let rival = new(31, 4, 1_012, 0, 1);
        // Before the switch.
        let mut g = group(2, &members);
        let mut rekey = Rekey::default();
        rekey.start(&g, 3, Key::new([30; 32]), 1_000).unwrap();
        assert_eq!(rekey.learned(&g, 1, rival.clone(), 1_001), Learned::Pending);
        let switched = rekey.switch(&mut g).unwrap();
        assert_eq!(switched.removed, Some(0));
        assert_eq!(switched.undone, 1 << 3, "3 is a member again");
        assert_eq!(rekey.removing(), None);

        // After the switch.
        let mut g = group(2, &members);
        let mut rekey = Rekey::default();
        rekey.start(&g, 3, Key::new([30; 32]), 1_000).unwrap();
        rekey.switch(&mut g).unwrap();
        assert_eq!(rekey.removing(), Some(3));
        assert_eq!(rekey.learned(&g, 1, rival, 1_020), Learned::Pending);
        let switched = rekey.switch(&mut g).unwrap();
        assert_eq!(switched.restored, Some(3));
        assert_eq!(switched.undone, 1 << 3);
        assert!(g.member(3).is_some(), "with its record, to remove again");
        let mut out = [0; STORED_MAX];
        let len = rekey.encode(&mut out);
        let mut read = Rekey::decode(&out[..len]).unwrap();
        assert_eq!(read.again(), 1 << 3, "across a restart");
        read.start(&g, 3, Key::new([32; 32]), 1_030).unwrap();
        assert_eq!(read.again(), 0);

        // A later removal by another member undoes nothing.
        let mut g = group(2, &members);
        let mut rekey = Rekey::default();
        rekey.start(&g, 3, Key::new([30; 32]), 1_000).unwrap();
        rekey.switch(&mut g).unwrap();
        rekey.learned(&g, 1, new(40, 5, 1_030, 0, 1).after(30), 1_020);
        assert_eq!(rekey.switch(&mut g).unwrap().undone, 0);

        // A rival from a higher id loses.
        let g = group(2, &members);
        let mut rekey = Rekey::default();
        rekey.start(&g, 1, Key::new([30; 32]), 1_000).unwrap();
        assert_eq!(
            rekey.learned(&g, 3, new(31, 4, 1_012, 0, 1), 1_001),
            Learned::Ignored
        );
    }

    #[test]
    fn a_removal_can_be_declined_for_a_day_after_its_switch() {
        let mut g = group(0, &[(0, 1), (1, 2), (2, 3), (3, 4)]);
        let mut rekey = Rekey::default();
        rekey.learned(&g, 1, new(9, 4, 1_010, 2, 3), 1_000);
        rekey.switch(&mut g).unwrap();
        let until = 1_010 + UNDO_ROUNDS;
        assert_eq!(rekey.undo_until(), Some(until));
        // A member renamed after the switch, under the new key.
        g.merge(3, signed(3, 4, 2_000), 0);
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
            rekey.learned(&g, 1, new(9, 4, 1_010, 2, 3), until),
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
        rekey.learned(&g, 1, new(9, 4, 1_010, 2, 3), 1_020);
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
        rekey.learned(&g, 1, new(9, 4, 1_010, 2, 3), 1_000);
        rekey.switch(&mut g).unwrap();
        rekey.learned(&g, 1, new(10, 5, 1_030, 3, 4).after(9), 1_020);
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
        let (losing, winning) = (new(21, 4, 1_010, 2, 3), new(20, 4, 1_012, 3, 4));
        let mut g = group(0, &[(0, 1), (1, 2), (2, 3), (3, 4)]);
        let mut rekey = Rekey::default();
        rekey.learned(&g, 3, losing.clone(), 1_000);
        rekey.switch(&mut g).unwrap();
        rekey.learned(&g, 1, winning.clone(), 1_020);
        rekey.switch(&mut g).unwrap();
        assert_eq!(
            rekey.undo_until(),
            Some(1_010 + UNDO_ROUNDS),
            "a rival gives no more time"
        );
        let reverted = rekey.undo(&mut g, 1_030, 3).unwrap();
        assert_eq!(reverted.since, 1_010 * ROUND_S, "from the first switch");
        assert_eq!(reverted.forgotten, 1 << 2 | 1 << 3);
        assert_eq!(g.generation(), 3);
        assert!(*g.key() == Key::new([5; 32]));
        for (remover, rival) in [(3, losing), (1, winning)] {
            assert_eq!(rekey.learned(&g, remover, rival, 1_030), Learned::Ignored);
        }
    }

    #[test]
    fn a_rival_removing_the_same_member_leaves_it_gone() {
        let mut g = group(0, &[(0, 1), (1, 2), (2, 3), (3, 4)]);
        let mut rekey = Rekey::default();
        rekey.learned(&g, 3, new(21, 4, 1_010, 2, 3), 1_000);
        rekey.switch(&mut g).unwrap();
        rekey.learned(&g, 1, new(20, 4, 1_012, 2, 3), 1_020);
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
        let mut g = group(2, &[(0, 1), (1, 2), (2, 3), (3, 4)]);
        let mut rekey = Rekey::default();
        let mine = rekey.start(&g, 3, Key::new([30; 32]), 1_000).unwrap();
        rekey.switch(&mut g).unwrap();
        assert_eq!(rekey.undo_until(), None);
        rekey.learned(&g, 1, new(31, 4, 1_012, 0, 1), 1_020);
        rekey.switch(&mut g).unwrap();
        assert_eq!(rekey.undo_until(), Some(mine.switch + UNDO_ROUNDS));
        rekey.undo(&mut g, 1_030, 0).unwrap();
        assert!(*g.key() == Key::new([5; 32]));
        assert_eq!(g.generation(), 3);
        assert_eq!(rekey.removing(), None);
    }

    #[test]
    fn a_key_that_removes_nobody_leaves_the_last_removal_to_decline() {
        let mut g = group(0, &[(0, 1), (1, 2), (2, 3), (3, 4)]);
        left(&mut g, 6, 99);
        let mut rekey = Rekey::default();
        rekey.learned(&g, 1, new(9, 4, 1_010, 2, 3), 1_000);
        rekey.switch(&mut g).unwrap();
        rekey.learned(&g, 1, new(10, 5, 1_020, 2, 99).after(9), 1_015);
        assert_eq!(rekey.switch(&mut g).unwrap().removed, None);
        assert_eq!(rekey.undo_until(), Some(1_010 + UNDO_ROUNDS));
        rekey.undo(&mut g, 1_030, 2).unwrap();
        assert_eq!(g.generation(), 3);
        assert!(*g.key() == Key::new([5; 32]));
        assert_eq!(rekey.old().count(), 0);
        for key in [new(9, 4, 1_010, 2, 3), new(10, 5, 1_020, 2, 99).after(9)] {
            assert_eq!(rekey.learned(&g, 1, key, 1_030), Learned::Ignored);
        }
    }

    #[test]
    fn a_rival_that_removes_nobody_leaves_nothing_to_decline() {
        let mut g = group(0, &[(0, 1), (1, 2), (2, 3), (3, 4)]);
        left(&mut g, 6, 99);
        let mut rekey = Rekey::default();
        rekey.learned(&g, 3, new(21, 4, 1_010, 2, 3), 1_000);
        rekey.switch(&mut g).unwrap();
        rekey.learned(&g, 1, new(20, 4, 1_012, 6, 99), 1_020);
        assert_eq!(rekey.switch(&mut g).unwrap().restored, Some(2));
        assert_eq!(rekey.undo_until(), None);
        assert_eq!(rekey.undo(&mut g, 1_030, 2), None);
        assert_eq!(g.generation(), 4);
    }

    #[test]
    fn declining_a_rival_of_a_key_that_removed_nobody_goes_back_to_that_key() {
        let mut g = group(0, &[(0, 1), (1, 2), (2, 3), (3, 4), (4, 5)]);
        left(&mut g, 6, 99);
        let mut rekey = Rekey::default();
        rekey.learned(&g, 1, new(9, 4, 1_010, 2, 3), 1_000);
        rekey.switch(&mut g).unwrap();
        rekey.learned(&g, 3, new(21, 5, 1_020, 6, 99).after(9), 1_015);
        assert_eq!(rekey.switch(&mut g).unwrap().removed, None);
        rekey.learned(&g, 1, new(20, 5, 1_022, 4, 5).after(9), 1_025);
        rekey.switch(&mut g).unwrap();
        assert_eq!(
            rekey.undo(&mut g, 1_030, 2),
            None,
            "4's removal is the last"
        );
        rekey.undo(&mut g, 1_030, 4).unwrap();
        assert_eq!(g.generation(), 5);
        assert!(*g.key() == Key::new([21; 32]));
        assert!(g.gone(2).is_some(), "2's removal stands");
    }

    #[test]
    fn a_rival_removing_the_member_this_device_removed_undoes_nothing() {
        let rival = new(31, 4, 1_012, 3, 4);
        // After the switch.
        let mut g = group(2, &[(0, 1), (1, 2), (2, 3), (3, 4)]);
        let mut rekey = Rekey::default();
        rekey.start(&g, 3, Key::new([30; 32]), 1_000).unwrap();
        rekey.switch(&mut g).unwrap();
        rekey.learned(&g, 1, rival.clone(), 1_020);
        let switched = rekey.switch(&mut g).unwrap();
        assert_eq!((switched.removed, switched.undone), (Some(3), 0));
        assert!(g.gone(3).is_some());

        // Before it.
        let mut g = group(2, &[(0, 1), (1, 2), (2, 3), (3, 4)]);
        let mut rekey = Rekey::default();
        rekey.start(&g, 3, Key::new([30; 32]), 1_000).unwrap();
        rekey.learned(&g, 1, rival, 1_001);
        assert_eq!(rekey.switch(&mut g).unwrap().undone, 0);
    }

    #[test]
    fn declining_keeps_an_older_rival_of_the_key_gone_back_to() {
        let mut g = group(0, &[(0, 1), (1, 2), (2, 3), (3, 4)]);
        let mut rekey = Rekey::default();
        rekey.learned(&g, 1, new(9, 4, 1_010, 2, 3), 1_000);
        rekey.old[0] = Some(Old {
            key: Key::new([20; 32]),
            generation: 3,
            waiting: 1 << 3,
            lost: true,
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
    fn the_most_a_restart_keeps_fills_its_buffer_exactly() {
        let mut rekey = Rekey {
            pending: Some(Pending {
                new: new(9, 4, 1_010, 2, 3),
                remover: 1,
                switch: 1_010,
            }),
            removing: Some((4, 2)),
            last: Some(Last {
                generation: 4,
                removed: 2,
                fingerprint: [3; 8],
                record: Some(Member {
                    name: Name::new(b"Sixteen letters!").unwrap(),
                    ..member(3, 0)
                }),
            }),
            follows: Some([4; 8]),
            again: u32::MAX,
            undo: Some(Undo {
                key: Key::new([5; 32]),
                generation: 3,
                new_generation: 4,
                switched: 1_010,
                until: 2_930,
                changed: u32::MAX,
                removed: 2,
                theirs: true,
            }),
            ..Rekey::default()
        };
        for at in 0..OLD_KEYS {
            rekey.old[at] = Some(Old {
                key: Key::new([at as u8; 32]),
                generation: at as u16,
                waiting: u32::MAX,
                lost: at % 2 == 1,
            });
        }
        for at in 0..DECLINED {
            rekey.declined[at] = Some([at as u8; 8]);
        }
        for at in 0..OLD_KEYS {
            rekey.removers[at] = Some((at as u16, at as u8));
        }
        let mut out = [0; STORED_MAX];
        assert_eq!(rekey.encode(&mut out), STORED_MAX);
        let read = Rekey::decode(&out).unwrap();
        assert!(
            read.old()
                .map(|old| old.lost)
                .eq((0..OLD_KEYS).map(|at| at % 2 == 1))
        );
        assert_eq!(read.last, rekey.last);
        assert_eq!(read.follows, rekey.follows);
        assert_eq!(read.again(), u32::MAX);
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
        rekey.learned(&g, 3, new(10, 5, 1_030, 1, 2).after(9), 1_020);
        rekey.decline(&g);
        rekey.learned(&g, 3, new(11, 5, 1_031, 1, 2).after(9), 1_040);
        let mut out = [0; STORED_MAX];
        let len = rekey.encode(&mut out);
        let read = Rekey::decode(&out[..len]).unwrap();
        let mut again = [0; STORED_MAX];
        assert_eq!(read.encode(&mut again), len);
        assert_eq!(again[..len], out[..len]);
        let pending = read.pending().unwrap();
        assert_eq!(pending.new, new(11, 5, 1_031, 1, 2).after(9));
        assert_eq!(pending.switch, 1_040 + DECLINE_ROUNDS);
        assert_eq!(read.old().count(), 1);
        assert_eq!(read.declined().count(), 1);
        assert_eq!(read.removing(), Some(2));
        assert!(Rekey::decode(&out[..len - 2]).is_none());

        // The key kept to decline a removal after its switch, and its absence from what was
        // stored before there was one.
        let mut g = group(0, &[(0, 1), (1, 2), (2, 3)]);
        let mut rekey = Rekey::default();
        rekey.learned(&g, 1, new(9, 4, 1_010, 2, 3), 1_000);
        rekey.switch(&mut g).unwrap();
        g.merge(1, signed(1, 2, 2_000), 0);
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
        let len = rekey.encode(&mut out);
        let read = Rekey::decode(&out[..len]).unwrap();
        assert_eq!(read.follows, Some(key_fingerprint(&Key::new([5; 32]))));
        assert_eq!(read.last.unwrap().record, Some(member(3, 100)));
        // What was stored before rivals were ranked by their removers, before the removers, and
        // before the undo too.
        let mut before = rekey.clone();
        before.follows = None;
        before.last = before.last.map(|last| Last {
            record: None,
            ..last
        });
        let len = before.encode(&mut out);
        assert!(Rekey::decode(&out[..len - 4]).is_some_and(|read| read.again() == 0));
        assert!(
            Rekey::decode(&out[..len - 7])
                .is_some_and(|read| read.last.is_some_and(|last| last.record.is_none()))
        );
        before.removers = [None; OLD_KEYS];
        let len = before.encode(&mut out);
        assert!(Rekey::decode(&out[..len - 8]).is_some_and(|read| read.undo_until().is_some()));
        before.undo = None;
        let len = before.encode(&mut out);
        assert!(Rekey::decode(&out[..len - 9]).is_some_and(|read| read.undo_until().is_none()));
    }

    #[test]
    fn the_newest_key_message_of_each_generation_is_kept_for_each_member() {
        extern crate std;
        let key = Key::new([1; 32]);
        let remover = crate::members::tests::key(1);
        let to = |dest, generation, seq, stamp| {
            Message::key(
                1,
                dest,
                seq,
                0,
                stamp,
                generation,
                &[kind::KEY],
                &key,
                &remover,
            )
            .unwrap()
        };
        let mut kept = std::boxed::Box::new(Kept::zeroed());
        kept.keep(&to(4, 4, 5, 100), None);
        kept.keep(&to(4, 4, 6, 90), None);
        kept.keep(&to(4, 5, 7, 200), None);
        assert_eq!(
            kept.get(4, 4).map(Message::seq),
            Some(5),
            "the newer of one generation"
        );
        assert_eq!(kept.get(4, 5).map(Message::seq), Some(7));
        assert_eq!(kept.get(4, 6), None);
        kept.keep(
            &Message::private(2, 5, 10, 9, 300, &[kind::TEXT], &key).unwrap(),
            None,
        );
        assert!(
            (0..8).all(|generation| kept.get(5, generation).is_none()),
            "only key messages"
        );
        for generation in 6..9 {
            kept.keep(&to(4, generation, 10 + u32::from(generation), 300), None);
        }
        assert_eq!(kept.get(4, 4), None, "the oldest generation goes first");
        assert!(kept.get(4, 8).is_some());

        // Two members' key messages for one generation.
        let rival = crate::members::tests::key(3);
        let from = |origin, seq, stamp| {
            Message::key(origin, 6, seq, 0, stamp, 9, &[kind::KEY], &key, &rival).unwrap()
        };
        kept.keep(&from(3, 40, 500), None);
        kept.keep(&from(1, 41, 400), None);
        assert_eq!(
            kept.get(6, 9).map(Message::origin),
            Some(3),
            "the first, unknowing"
        );
        kept.keep(&from(1, 41, 400), Some(1));
        assert_eq!(
            kept.get(6, 9).map(Message::origin),
            Some(1),
            "the remover's"
        );
        kept.keep(&from(3, 42, 600), Some(1));
        assert_eq!(kept.get(6, 9).map(Message::origin), Some(1));
        kept.keep(&from(3, 43, 700), None);
        kept.keep_only(9, 1);
        assert_eq!(kept.get(6, 9).map(Message::origin), Some(1));
        kept.clear();
        assert_eq!(kept.get(4, 5), None);
    }

    #[test]
    fn a_switch_records_who_removed_for_each_generation() {
        let mut g = group(0, &[(0, 1), (1, 2), (2, 3), (3, 4)]);
        let mut rekey = Rekey::default();
        rekey.learned(&g, 1, new(9, 4, 1_010, 2, 3), 1_000);
        rekey.switch(&mut g).unwrap();
        assert_eq!(rekey.remover_of(4), Some(1));
        rekey.start(&g, 3, Key::new([8; 32]), 1_020).unwrap();
        rekey.switch(&mut g).unwrap();
        assert_eq!(rekey.remover_of(5), Some(0));
        assert_eq!(rekey.remover_of(4), Some(1));
        assert_eq!(rekey.remover_of(3), None);
        let mut out = [0; STORED_MAX];
        let len = rekey.encode(&mut out);
        assert_eq!(Rekey::decode(&out[..len]).unwrap().remover_of(4), Some(1));
    }
}
