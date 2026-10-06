//! Removing a member: a new group key, sent to each remaining member in a key message and taken
//! up at a switch round, and the old key kept until every remaining member has been heard on the
//! new one (`context/LORA-PROTOCOL.md`, "Removing a member").

use bytemuck::Zeroable;
use sha2::{Digest, Sha256};

use crate::identity::{Identity, SIGNATURE_LEN, verify};
use crate::members::{Group, Member, PUBLIC_LEN, RECORD_MAX_LEN, fingerprint};
use crate::messages::{BODY_MAX, FIXED_LEN, Message, To, kind};
use crate::packet::MAX_PACKET;
use crate::schedule::{REST_TIMES, ROUND_S, ROUND_US, airtime_us};
use crate::seal::Key;
use crate::{IDS, Ids};

/// A key message's plaintext: its kind, the key, its generation, the switch round, the id and
/// fingerprint of the member removed, and the fingerprint of the key it follows.
pub const KEY_LEN: usize = 1 + 32 + 2 + 4 + 1 + 8 + 8;
/// The key messages a remover sends in each packet, which sets how far off the switch is. A
/// signed key message takes most of a packet.
pub const KEYS_PER_PACKET: u32 = 1;
/// How long a remover takes over each packet of key messages: the packet, at its largest, and
/// the rest after it.
pub const KEY_PACKET_US: i64 = (1 + REST_TIMES) * airtime_us(MAX_PACKET);
/// Rounds past the remover's own key messages for them to cross the hops, and for those lost on
/// the way to be asked for again (owner, 2026-10-05). A member that still misses the switch is
/// caught up the next time it is heard under the old key.
pub const HOP_ROUNDS: u32 = 3;
/// The older keys kept for members not yet heard on a newer one.
pub const OLD_KEYS: usize = 4;
/// The rounds a device that learns of a removal has to decline it, however late it learns.
pub const DECLINE_ROUNDS: u32 = 3;
/// A round's length in seconds.
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
#[must_use]
pub fn is_newer(a: u16, b: u16) -> bool {
    a != b && a.wrapping_sub(b) <= u16::MAX / 2
}

/// How many rounds from now a removal's switch is, for a remover sending `messages` key and
/// removal messages: the rounds it takes to send them, [`HOP_ROUNDS`], and the round it is in.
#[must_use]
pub const fn switch_rounds(messages: u32) -> u32 {
    let sending = messages.div_ceil(KEYS_PER_PACKET) as u64 * KEY_PACKET_US as u64;
    sending.div_ceil(ROUND_US as u64) as u32 + HOP_ROUNDS + 1
}

/// The furthest a key message's switch can be past the round it arrives in: as far as a removal
/// from the largest group needs, and a round for clocks that disagree.
pub const SWITCH_AHEAD: u32 = switch_rounds(IDS as u32) + 1;

/// A removal learned and not yet switched to.
#[derive(Clone, Debug)]
pub struct Pending {
    pub new: NewKey,
    pub remover: u8,
    /// The round this device switches at: the key's own [`NewKey::switch`], or later, so that a
    /// removal learned late still leaves its user [`DECLINE_ROUNDS`] to decline it.
    pub own_switch: u32,
}

/// A key the group switched away from, kept for the members not yet heard on a newer one.
#[derive(Clone)]
pub struct Old {
    pub key: Key,
    pub generation: u16,
    /// The members not yet heard on a newer key.
    pub waiting: Ids,
    /// The generation of the key message that catches up a member still on this key: the
    /// removal after it, or the rival that won over it or over a key before it.
    pub catches_up_with: u16,
}

/// A key this device switched to: a rival that wins over it undoes it and every switch since.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Took {
    pub generation: u16,
    pub remover: u8,
    /// The fingerprint of the key it replaced, which a rival of it replaces too. Missing from
    /// what was stored before it was kept for each switch.
    pub follows: Option<[u8; 8]>,
    /// The member it removed, given its place back when a rival wins over the key.
    pub removed: Option<Removed>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Removed {
    pub id: u8,
    pub fingerprint: [u8; 8],
    /// Its record before the switch, put back as it was. Only the newest switch's is stored;
    /// without one, the record comes back from the nodes that never removed it.
    pub record: Option<Member>,
}

/// The key a removal's switch left, kept for [`UNDO_ROUNDS`] so that this device's user can
/// still decline the removal and go back to it.
#[derive(Clone)]
pub(crate) struct Undo {
    pub key: Key,
    pub generation: u16,
    /// The generation the removal switched to. A rival of it has the same.
    pub new_generation: u16,
    /// The group's switch round: a message stamped from its start was made under a newer key.
    pub switched: u32,
    /// The last round the removal can be declined in.
    pub until: u32,
    /// The ids whose slots changed since this device's switch.
    pub changed: Ids,
    /// The id of the member removed.
    pub removed: u8,
    /// Whether another member made the removal. This device's own is kept only so that a rival
    /// winning over it can still be declined.
    pub theirs: bool,
}

/// The removal last switched to, as this device can still decline it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Declinable {
    /// The group's switch round.
    pub switched: u32,
    /// The last round it can be declined in.
    pub until: u32,
    /// The member it removed, and that member's device.
    pub removed: u8,
    pub fingerprint: [u8; 8],
    /// The member's record before the switch, unless what was stored predates keeping it.
    pub record: Option<Member>,
}

/// What declining a removal after its switch did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct Reverted {
    /// From when, in timebase seconds, messages were made under the key left; the caller
    /// forgets them.
    pub since: u32,
    /// The ids whose records were forgotten, the member removed among them.
    pub forgotten: Ids,
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
    undo: Option<Undo>,
    /// The last keys switched to, newest first. A node keeps only the key messages of a
    /// generation's remover for catch-up.
    took: [Option<Took>; OLD_KEYS],
    /// The members this device removed under a key a rival won over, to remove again.
    again: Ids,
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

    /// The removal last switched to, while this device can still decline it and holds the
    /// member it removed.
    #[must_use]
    pub fn declinable(&self) -> Option<Declinable> {
        let undo = self.undo.as_ref().filter(|undo| undo.theirs)?;
        let removed = self.took[0]
            .filter(|took| took.generation == undo.new_generation)?
            .removed?;
        Some(Declinable {
            switched: undo.switched,
            until: undo.until,
            removed: removed.id,
            fingerprint: removed.fingerprint,
            record: removed.record,
        })
    }

    /// The last round the removal last switched to can be declined in.
    #[must_use]
    pub fn undo_until(&self) -> Option<u32> {
        self.undo
            .as_ref()
            .filter(|undo| undo.theirs)
            .map(|undo| undo.until)
    }

    /// Notes the ids whose slots changed, which declining the last removal forgets. Returns
    /// whether that changed what is kept.
    pub fn changed(&mut self, ids: Ids) -> bool {
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
        self.took_at(generation).map(|took| took.remover)
    }

    fn took_at(&self, generation: u16) -> Option<&Took> {
        self.took
            .iter()
            .flatten()
            .find(|took| took.generation == generation)
    }

    /// Whether `new`, from `remover`, wins over the key this device switched to at its
    /// generation: the group's key, or one switched past since, which an old key holds. A
    /// device that did not switch to that key cannot rank it.
    fn wins_over_ours(&self, group: &Group, remover: u8, new: &NewKey) -> bool {
        let Some(ours) = self.took_at(new.generation) else {
            return false;
        };
        let key = if new.generation == group.generation() {
            Some(group.key())
        } else {
            // The key the next switch replaced.
            self.took_at(new.generation.wrapping_add(1))
                .and_then(|next| next.follows)
                .and_then(|follows| {
                    self.old()
                        .find(|old| key_fingerprint(&old.key) == follows)
                        .map(|old| &old.key)
                })
        };
        ours.follows == Some(new.follows)
            && key.is_some_and(|key| {
                new.key != *key && beats((remover, &new.key), (ours.remover, key))
            })
    }

    /// The members this device removed under a key a rival won over: it removes each again
    /// once that member's record is back.
    #[must_use]
    pub fn again(&self) -> Ids {
        self.again
    }

    /// Whether some member has not been heard on the newest key since a switch.
    #[must_use]
    pub fn is_waiting(&self) -> bool {
        self.old().any(|old| !old.waiting.is_empty())
    }

    /// Starts removing the member `id` with the fresh key `key`, in round `round`. Returns the
    /// new key, which goes to every remaining member, or `None` when `id` is not another
    /// member or a removal is under way.
    pub fn start(&mut self, group: &Group, id: u8, key: Key, round: u32) -> Option<NewKey> {
        if self.pending.is_some() || id == group.own() {
            return None;
        }
        let removed = group.member(id)?;
        let remaining = group.ids().without(id).without(group.own());
        let new = NewKey {
            key,
            generation: group.generation().wrapping_add(1),
            switch: round + switch_rounds(remaining.count() + 1),
            removed: id,
            fingerprint: fingerprint(&removed.public),
            follows: key_fingerprint(group.key()),
        };
        self.pending = Some(Pending {
            new: new.clone(),
            remover: group.own(),
            own_switch: new.switch,
        });
        self.removing = Some((new.generation, id));
        self.again.remove(id);
        Some(new)
    }

    /// Takes a key message from `remover`, opened, in round `round`. Only the key after the
    /// group's is taken, or a rival that wins over the group's key or over one this device
    /// switched past since: a member that missed several switches takes them one at a time,
    /// and a part of the group that switched past a rival's generation goes back to it.
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
            1 if new.follows == key_fingerprint(group.key()) => {}
            // Wrapping: a generation up to half the range ahead is newer.
            ahead if (1..=u16::MAX / 2).contains(&ahead) => return Learned::Later,
            _ if !self.wins_over_ours(group, remover, &new) => return Learned::Ignored,
            _ => {}
        }
        if let Some(pending) = &self.pending
            && !is_newer(pending.new.generation, current)
            && is_newer(new.generation, pending.new.generation)
        {
            // It follows a key the rival under way wins over, unless this device declines it.
            return Learned::Later;
        }
        if let Some(pending) = &self.pending
            && pending.new.generation == new.generation
            && (pending.new.key == new.key
                || !beats((remover, &new.key), (pending.remover, &pending.new.key)))
        {
            return Learned::Ignored;
        }
        if let Some(pending) = &self.pending
            && pending.remover == group.own()
            && pending.new.generation != new.generation
        {
            // This device's removal follows the key a rival beats, so it is made again after.
            self.again.insert(pending.new.removed);
        }
        let own_switch = new.switch.max(round.saturating_add(DECLINE_ROUNDS));
        self.pending = Some(Pending {
            new,
            remover,
            own_switch,
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
            .is_some_and(|pending| round >= pending.own_switch)
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
        compact(&mut self.old);
        for took in &mut self.took {
            took.take_if(|took| is_newer(took.generation, undo.generation));
        }
        compact(&mut self.took);
        let changed = undo.changed | group.take_changed();
        let forgotten = group.forget_changed(changed);
        group.take_changed();
        group.rekey(undo.key, undo.generation);
        self.removing = None;
        // A rival this device lost to, made again, would only race the key gone back to.
        self.again = Ids::EMPTY;
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
    /// member until it is heard on the new one. A rival of a key this device switched to undoes
    /// that switch and every one since: the members they removed come back, and this device
    /// removes its own again.
    pub fn switch(&mut self, group: &mut Group) -> Option<Switched> {
        let pending = self.pending.take()?;
        let new = pending.new;
        let ours = pending.remover == group.own();
        let rival = !is_newer(new.generation, group.generation());
        // What changed before the switch is no business of declining it.
        group.take_changed();
        let mut taken = [None; OLD_KEYS];
        if rival {
            for (took, undone) in self.took.iter_mut().zip(&mut taken) {
                *undone = took.take_if(|took| !is_newer(new.generation, took.generation));
            }
            compact(&mut self.took);
        }
        let undone_keys = taken.iter().flatten();
        // A rival removing a member a key undone removed leaves it gone.
        let again = undone_keys
            .clone()
            .filter_map(|took| took.removed)
            .find(|removed| removed.fingerprint == new.fingerprint);
        let mut restored = Ids::EMPTY;
        for removed in undone_keys.clone().filter_map(|took| took.removed) {
            if removed.fingerprint != new.fingerprint
                && group.put_back(removed.id, removed.record, &removed.fingerprint)
            {
                restored.insert(removed.id);
            }
        }
        // This device's removal lost to a rival of its generation, before the switch or after,
        // that removes another member.
        let mut undone = match self.removing {
            Some((generation, id))
                if !ours && generation == new.generation && id != new.removed =>
            {
                Ids::of(id)
            }
            _ => Ids::EMPTY,
        };
        // The same for each of its keys undone, the one before a removal it started since too.
        for took in undone_keys.clone() {
            if let Some(removed) = took.removed
                && took.remover == group.own()
                && removed.id != new.removed
            {
                undone.insert(removed.id);
            }
        }
        let at = new.switch.saturating_mul(ROUND_S);
        let (removed, record) = match again {
            Some(removed) => (Some(removed.id), removed.record),
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
            // Going back past this device's later switches, the key it leaves is not the one
            // the rival replaced, so neither is a state to decline back to: such a rival is
            // declined before it switches, or not at all.
            _ if rival && new.generation != group.generation() => None,
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
                until: pending.own_switch.saturating_add(UNDO_ROUNDS),
                changed,
                removed,
                theirs: !ours,
            }),
            (None, None) => None,
        };
        self.wait_only_for(group.ids());
        if rival {
            // A member on a key undone, or on a rival of one, is sent this key.
            for old in self.old.iter_mut().flatten() {
                if !is_newer(new.generation, old.generation)
                    && is_newer(old.catches_up_with, new.generation)
                {
                    old.catches_up_with = new.generation;
                }
            }
        }
        let waiting = group.ids().without(group.own());
        if !waiting.is_empty() {
            self.old.rotate_right(1);
            self.old[0] = Some(Old {
                key: group.key().clone(),
                generation: group.generation(),
                waiting,
                catches_up_with: new.generation,
            });
        }
        group.rekey(new.key, new.generation);
        self.took.rotate_right(1);
        self.took[0] = Some(Took {
            generation: new.generation,
            remover: pending.remover,
            follows: Some(new.follows),
            removed: removed.map(|id| Removed {
                id,
                fingerprint: new.fingerprint,
                record,
            }),
        });
        self.removing = ours.then_some((new.generation, new.removed));
        self.again |= undone;
        Some(Switched {
            removed,
            remover: pending.remover,
            restored,
            undone,
            rival,
        })
    }

    /// Takes a member's word, checked, that it is on the key of `generation`, the group's: it
    /// needs no key that key replaced, or that a member on it would be caught up from. Returns
    /// whether that changed what is kept.
    pub fn on_key(&mut self, id: u8, generation: u16) -> bool {
        let mut changed = false;
        for old in self.old.iter_mut() {
            if let Some(held) = old
                && !is_newer(held.catches_up_with, generation)
                && held.waiting.contains(id)
            {
                changed = true;
                held.waiting.remove(id);
                if held.waiting.is_empty() {
                    *old = None;
                }
            }
        }
        if changed {
            compact(&mut self.old);
        }
        changed
    }

    /// Stops waiting for anyone outside `members`: a member removed or gone is never heard on a
    /// newer key. Returns whether that changed what is kept.
    pub fn wait_only_for(&mut self, members: Ids) -> bool {
        let mut changed = false;
        for old in self.old.iter_mut() {
            if let Some(held) = old
                && !(held.waiting & !members).is_empty()
            {
                changed = true;
                held.waiting &= members;
                if held.waiting.is_empty() {
                    *old = None;
                }
            }
        }
        if changed {
            compact(&mut self.old);
        }
        changed
    }

    /// Whether `sender` is a member still waited for under the old key with generation
    /// `generation`.
    #[must_use]
    pub fn is_waiting_for(&self, generation: u16, sender: u8) -> bool {
        self.old()
            .any(|old| old.generation == generation && old.waiting.contains(sender))
    }

    /// Whether `id`, a member on the key of `generation`, is still waited for under a key that
    /// one replaced or would catch it up from.
    #[must_use]
    pub fn is_waiting_for_under_older(&self, id: u8, generation: u16) -> bool {
        self.old()
            .any(|old| !is_newer(old.catches_up_with, generation) && old.waiting.contains(id))
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
        self.wait_only_for(Ids::ALL.without(id))
    }
}

/// The most bytes [`Rekey::encode`] writes, in its order.
pub const STORED_MAX: usize = 1 // whether a removal is under way
    + 1 // its remover
    + 4 // its switch round
    + KEY_LEN // its new key
    + 1 // how many old keys
    + OLD_KEYS * (32 + 2 + 4) // each: key, generation, the members it waits for
    + 1 // how many keys declined
    + DECLINED * 8 // each one's fingerprint
    + 4 // this device's removal: whether, generation, id
    + 1 // whether a switch is recorded
    + 2 // its generation
    + 1 // the member it removed
    + 8 // that member's fingerprint
    + 1 // whether a key is kept to decline it
    + 32 // that key
    + 2 // its generation
    + 2 // the new key's generation
    + 4 // the round it switched
    + 4 // the last round it can be declined in
    + 4 // the ids whose slots changed since
    + 1 // the member removed
    + 1 // whether another member made it
    + 1 // how many generations' removers
    + OLD_KEYS * 3 // each: generation, remover
    + 1 // which old keys a rival won over
    + 1 // whether the key the group's key followed is known
    + 8 // its fingerprint
    + 1 // the length of the removed member's record
    + RECORD_MAX_LEN // that record
    + 4 // the members to remove again
    + OLD_KEYS * (1 + 8 + 1 + 1 + 8) // each switch's: what it followed, the member it removed
    + OLD_KEYS * 2; // the generation each old key catches up with

impl Rekey {
    /// Writes what a restart has to keep: the removal under way, the old keys with the members
    /// each waits for and the generation each catches up with, the keys declined, this
    /// device's removals and those it makes again, the last switches with who removed, what
    /// each followed and whom it removed, the newest one's record, and the key kept to decline
    /// it.
    pub fn encode(&self, out: &mut [u8; STORED_MAX]) -> usize {
        let mut len = 0;
        let mut put = |bytes: &[u8]| {
            out[len..len + bytes.len()].copy_from_slice(bytes);
            len += bytes.len();
        };
        match &self.pending {
            Some(pending) => {
                put(&[1, pending.remover]);
                put(&pending.own_switch.to_be_bytes());
                put(&pending.new.encode());
            }
            None => put(&[0]),
        }
        put(&[self.old().count() as u8]);
        for old in self.old() {
            put(old.key.bytes());
            put(&old.generation.to_be_bytes());
            put(&old.waiting.bits().to_be_bytes());
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
        let newest = self.took[0];
        let last = newest.and_then(|took| Some((took.generation, took.removed?)));
        match last {
            Some((generation, removed)) => {
                put(&[1]);
                put(&generation.to_be_bytes());
                put(&[removed.id]);
                put(&removed.fingerprint);
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
                put(&undo.changed.bits().to_be_bytes());
                put(&[undo.removed, u8::from(undo.theirs)]);
            }
            None => put(&[0]),
        }
        put(&[self.took.iter().flatten().count() as u8]);
        for took in self.took.iter().flatten() {
            put(&took.generation.to_be_bytes());
            put(&[took.remover]);
        }
        // Which old keys a rival won over, as what was stored before each kept its generation
        // to catch up with read it.
        put(&[self.old().enumerate().fold(0, |lost, (at, old)| {
            lost | u8::from(old.catches_up_with != old.generation.wrapping_add(1)) << at
        })]);
        match newest.and_then(|took| took.follows) {
            Some(follows) => {
                put(&[1]);
                put(&follows);
            }
            None => put(&[0]),
        }
        match last.and_then(|(_, removed)| Some((removed.id, removed.record?))) {
            Some((id, record)) => {
                let mut bytes = [0; RECORD_MAX_LEN];
                let record_len = record.encode(id, &mut bytes);
                put(&[record_len as u8]);
                put(&bytes[..record_len]);
            }
            None => put(&[0]),
        }
        put(&self.again.bits().to_be_bytes());
        for took in self.took.iter().flatten() {
            match took.follows {
                Some(follows) => {
                    put(&[1]);
                    put(&follows);
                }
                None => put(&[0]),
            }
            match took.removed {
                Some(removed) => {
                    put(&[1, removed.id]);
                    put(&removed.fingerprint);
                }
                None => put(&[0]),
            }
        }
        for old in self.old() {
            put(&old.catches_up_with.to_be_bytes());
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
            let own_switch = u32::from_be_bytes(take(4)?.try_into().ok()?);
            rekey.pending = Some(Pending {
                new: NewKey::decode(take(KEY_LEN)?)?,
                remover,
                own_switch,
            });
        }
        let count = usize::from(take(1)?[0]);
        if count > OLD_KEYS {
            return None;
        }
        for old in rekey.old.iter_mut().take(count) {
            let key = Key::new(take(32)?.try_into().ok()?);
            let generation = u16::from_be_bytes(take(2)?.try_into().ok()?);
            let waiting = Ids::from_bits(u32::from_be_bytes(take(4)?.try_into().ok()?));
            *old = Some(Old {
                key,
                generation,
                waiting,
                catches_up_with: generation.wrapping_add(1),
            });
        }
        let count = usize::from(take(1)?[0]);
        if count > DECLINED {
            return None;
        }
        for declined in rekey.declined.iter_mut().take(count) {
            *declined = Some(take(8)?.try_into().ok()?);
        }
        let removing = take(4)?;
        if removing[0] == 1 {
            rekey.removing = Some((u16::from_be_bytes([removing[1], removing[2]]), removing[3]));
        }
        let mut last = None;
        if take(1)?[0] == 1 {
            let generation = u16::from_be_bytes(take(2)?.try_into().ok()?);
            let removed = Removed {
                id: take(1)?[0],
                fingerprint: take(8)?.try_into().ok()?,
                record: None,
            };
            last = Some((generation, removed));
        }
        // Missing from what was stored before declining after a switch was possible.
        if take(1).is_some_and(|flag| flag[0] == 1) {
            rekey.undo = Some(Undo {
                key: Key::new(take(32)?.try_into().ok()?),
                generation: u16::from_be_bytes(take(2)?.try_into().ok()?),
                new_generation: u16::from_be_bytes(take(2)?.try_into().ok()?),
                switched: u32::from_be_bytes(take(4)?.try_into().ok()?),
                until: u32::from_be_bytes(take(4)?.try_into().ok()?),
                changed: Ids::from_bits(u32::from_be_bytes(take(4)?.try_into().ok()?)),
                removed: take(1)?[0],
                theirs: take(1)?[0] == 1,
            });
        }
        // Missing, as the undo was, from what was stored before them.
        let mut took_count = 0;
        if let Some(count) = take(1).map(|count| usize::from(count[0])) {
            if count > OLD_KEYS {
                return None;
            }
            took_count = count;
            for took in rekey.took.iter_mut().take(count) {
                let generation = u16::from_be_bytes(take(2)?.try_into().ok()?);
                *took = Some(Took {
                    generation,
                    remover: take(1)?[0],
                    follows: None,
                    removed: None,
                });
            }
        }
        // Missing, as the rest after it, from what was stored before rivals were ranked by
        // their removers.
        if let Some(lost) = take(1).map(|lost| lost[0]) {
            for (at, old) in rekey.old.iter_mut().enumerate() {
                if let Some(old) = old
                    && lost & 1 << at != 0
                {
                    old.catches_up_with = old.generation;
                }
            }
        }
        if take(1).is_some_and(|flag| flag[0] == 1)
            && let Some(took) = &mut rekey.took[0]
        {
            took.follows = Some(take(8)?.try_into().ok()?);
        }
        if let Some(len) = take(1).map(|len| usize::from(len[0]))
            && len > 0
        {
            let (id, record) = Member::decode(take(len)?)?;
            match &mut last {
                Some((_, removed)) if removed.id == id => removed.record = Some(record),
                _ => return None,
            }
        }
        // The last switch's member, on the switch it names. Lost from what was stored before
        // the switches' removers were kept.
        if let Some((generation, removed)) = last
            && let Some(took) = rekey
                .took
                .iter_mut()
                .flatten()
                .find(|took| took.generation == generation)
        {
            took.removed = Some(removed);
        }
        if let Some(again) = take(4) {
            rekey.again = Ids::from_bits(u32::from_be_bytes(again.try_into().ok()?));
        }
        // Missing, as the rest after it, from what was stored before each switch kept what it
        // followed and whom it removed, and each old key the generation it catches up with.
        for took in rekey.took.iter_mut().take(took_count).flatten() {
            let Some(flag) = take(1) else { break };
            took.follows = match flag[0] {
                1 => Some(take(8)?.try_into().ok()?),
                _ => None,
            };
            took.removed = match take(1)?[0] {
                1 => {
                    let id = take(1)?[0];
                    let fingerprint: [u8; 8] = take(8)?.try_into().ok()?;
                    let record = took
                        .removed
                        .filter(|removed| removed.id == id)
                        .and_then(|removed| removed.record);
                    Some(Removed {
                        id,
                        fingerprint,
                        record,
                    })
                }
                _ => None,
            };
        }
        for old in rekey.old.iter_mut().flatten() {
            let Some(generation) = take(2) else { break };
            old.catches_up_with = u16::from_be_bytes(generation.try_into().ok()?);
        }
        rest.is_empty().then_some(rekey)
    }
}

/// The most bytes [`Kept::encode_row`] writes: a count, then each message's length and record.
pub const KEPT_ROW_MAX: usize = 1 + OLD_KEYS * (1 + FIXED_LEN + BODY_MAX);

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
    /// otherwise the first. Returns whether that changed what is kept.
    pub fn keep(&mut self, message: &Message, remover: Option<u8>) -> bool {
        let (To::Member(dest), Some(generation)) = (message.to(), message.generation()) else {
            return false;
        };
        let Some(held) = self.messages.get_mut(usize::from(dest)) else {
            return false;
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
        let Some(at) = at else { return false };
        let kept = &held[at];
        let replace = if kept.seq == 0 || kept.generation() != Some(generation) {
            true
        } else if kept.origin == message.origin {
            kept.stamp <= message.stamp
        } else {
            remover == Some(message.origin)
        };
        let changed = replace && kept.name() != message.name();
        if replace {
            held[at] = *message;
        }
        changed
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

    /// Drops the key messages of generations after `generation`, those of keys a rival undid.
    pub fn forget_after(&mut self, generation: u16) {
        for kept in self.messages.iter_mut().flatten() {
            if kept.seq != 0 && kept.generation().is_some_and(|of| is_newer(of, generation)) {
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

    /// Whether any key message is kept for the member `id`.
    #[must_use]
    pub fn holds(&self, id: u8) -> bool {
        self.messages
            .get(usize::from(id))
            .is_some_and(|held| held.iter().any(|kept| kept.seq != 0))
    }

    /// Writes the key messages kept for the member `id`, to store, and returns their length.
    pub fn encode_row(&self, id: u8, out: &mut [u8; KEPT_ROW_MAX]) -> usize {
        let mut at = 1;
        let mut count = 0;
        for kept in self.messages[usize::from(id)]
            .iter()
            .filter(|kept| kept.seq != 0)
        {
            let len = kept.encode(&mut out[at + 1..]);
            out[at] = len as u8;
            at += 1 + len;
            count += 1;
        }
        out[0] = count;
        at
    }

    /// Keeps the key messages of a row [`encode_row`](Self::encode_row) wrote for the member
    /// `id`. Returns false, keeping none, when `row` is not one: a message in it to another
    /// member, or not a key message.
    pub fn restore_row(&mut self, id: u8, row: &[u8]) -> bool {
        let Some((&count, mut rest)) = row.split_first() else {
            return false;
        };
        let mut read = [Message::zeroed(); OLD_KEYS];
        if usize::from(count) > OLD_KEYS {
            return false;
        }
        for slot in &mut read[..usize::from(count)] {
            let Some((&len, after)) = rest.split_first() else {
                return false;
            };
            let Some((record, after)) = after.split_at_checked(usize::from(len)) else {
                return false;
            };
            match Message::decode(record) {
                Some(message)
                    if message.is_key()
                        && message.to() == To::Member(id)
                        && message.generation().is_some() =>
                {
                    *slot = message;
                }
                _ => return false,
            }
            rest = after;
        }
        for message in &read[..usize::from(count)] {
            self.keep(message, None);
        }
        true
    }
}

/// What a switch did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct Switched {
    pub removed: Option<u8>,
    pub remover: u8,
    /// The members the keys a rival won over removed, given their places back.
    pub restored: Ids,
    /// The members this device removed under the losing keys: it removes them again under the
    /// new key once their records are back.
    pub undone: Ids,
    /// Whether the key was a rival of one this device switched to, which it undid.
    pub rival: bool,
}

/// Moves the filled slots to the front, newest first, with the gaps after.
fn compact<T>(slots: &mut [Option<T>]) {
    let mut kept = 0;
    for at in 0..slots.len() {
        if let Some(held) = slots[at].take() {
            slots[kept] = Some(held);
            kept += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::members::tests::{member, signed};
    use crate::members::{Name, fingerprint};

    /// The test group at generation 3, so that a key following it is generation 4.
    fn group(own: u8, ids: &[(u8, u8)]) -> Group {
        crate::members::tests::group_at(3, own, ids)
    }

    /// Holds the device `of` as gone from id `id`: a key that names it removes nobody.
    fn left(g: &mut Group, id: u8, of: u8) {
        g.merge_gone(id, crate::members::tests::left(id, of, 50), None);
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
        // Six key messages and the removal notice, one a packet at about 4 s each, fit a round.
        assert_eq!(switch_rounds(7), 1 + HOP_ROUNDS + 1);
        // 31 take about 124 s, three rounds: 5.25 minutes in all.
        assert_eq!(switch_rounds(31), 3 + HOP_ROUNDS + 1);
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
            rekey.learned(&g, 1, new(9, 4, 1_010, 2, 3), 1_005),
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
        assert_eq!(old.waiting, Ids::of(1).with(3));
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
            rekey.old().all(|old| old.waiting == Ids::of(1)),
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
            rekey.learned(&g, 2, next.clone(), 1_015),
            Learned::Later,
            "the switch to the key it follows comes first"
        );
        assert_eq!(
            rekey.learned(&g, 2, new(9, 4, 1_010, 3, 4).after(7), 1_005),
            Learned::Later,
            "it follows a key that won elsewhere"
        );
        assert!(rekey.pending().is_none());
        rekey.learned(&g, 2, new(8, 4, 1_010, 1, 2), 1_005);
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
            rekey.learned(&g, 7, new(9, 4, 1_010, 2, 3), 1_005),
            Learned::Ignored
        );
        assert_eq!(
            rekey.learned(&g, 1, new(9, 2, 1_010, 2, 3), 1_005),
            Learned::Ignored
        );
        assert_eq!(
            rekey.learned(&g, 1, new(5, 3, 1_010, 2, 3), 1_005),
            Learned::Ignored,
            "the current key is no rival of itself"
        );
        assert_eq!(
            rekey.learned(&g, 1, new(9, 4, 1_010, 0, 1), 1_005),
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
            rekey.learned(&g, 1, new(9, 4, 1_010, 2, 99), 1_005),
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
            rekey.learned(&g, 1, new(9, 4, 1_010, 6, 99), 1_005),
            Learned::Pending,
            "a member gone by the time the key arrives"
        );
    }

    #[test]
    fn a_declined_removal_is_not_asked_again() {
        let mut g = group(0, &[(0, 1), (1, 2), (2, 3)]);
        let mut rekey = Rekey::default();
        rekey.learned(&g, 1, new(9, 4, 1_010, 2, 3), 1_005);
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
        rekey.learned(&g, 3, losing.clone(), 1_005);
        assert_eq!(
            rekey.learned(&g, 1, winning.clone(), 1_007),
            Learned::Pending
        );
        assert_eq!(rekey.pending().unwrap().remover, 1);
        assert_eq!(
            rekey.learned(&g, 3, losing.clone(), 1_005),
            Learned::Ignored
        );

        // After switching to the higher's: the lower's still wins, and the member the higher's
        // removed has its record back as it was.
        let mut g = group(0, &[(0, 1), (1, 2), (2, 3), (3, 4)]);
        let record = *g.member(2).unwrap();
        let mut rekey = Rekey::default();
        rekey.learned(&g, 3, losing, 1_005);
        rekey.switch(&mut g).unwrap();
        assert!(g.gone(2).is_some());
        assert_eq!(
            rekey.learned(&g, 1, winning.clone(), 1_020),
            Learned::Pending
        );
        let switched = rekey.switch(&mut g).unwrap();
        assert_eq!(switched.removed, Some(3));
        assert_eq!(switched.restored, Ids::of(2));
        assert_eq!(g.member(2), Some(&record));
        assert!(*g.key() == Key::new([20; 32]));
        assert_eq!(
            rekey.old().count(),
            2,
            "both older keys wait for their members"
        );
        let lost = rekey.old().next().unwrap();
        assert!(lost.key == Key::new([21; 32]) && lost.catches_up_with == lost.generation);
        assert!(
            rekey.old().all(|old| old.catches_up_with == 4),
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
        assert_eq!(rekey.learned(&g, 1, rival.clone(), 1_007), Learned::Pending);
        let switched = rekey.switch(&mut g).unwrap();
        assert_eq!(switched.removed, Some(0));
        assert_eq!(switched.undone, Ids::of(3), "3 is a member again");
        assert_eq!(rekey.removing(), None);

        // After the switch.
        let mut g = group(2, &members);
        let mut rekey = Rekey::default();
        rekey.start(&g, 3, Key::new([30; 32]), 1_000).unwrap();
        rekey.switch(&mut g).unwrap();
        assert_eq!(rekey.removing(), Some(3));
        assert_eq!(rekey.learned(&g, 1, rival, 1_020), Learned::Pending);
        let switched = rekey.switch(&mut g).unwrap();
        assert_eq!(switched.restored, Ids::of(3));
        assert_eq!(switched.undone, Ids::of(3));
        assert!(g.member(3).is_some(), "with its record, to remove again");
        let mut out = [0; STORED_MAX];
        let len = rekey.encode(&mut out);
        let mut read = Rekey::decode(&out[..len]).unwrap();
        assert_eq!(read.again(), Ids::of(3), "across a restart");
        read.start(&g, 3, Key::new([32; 32]), 1_030).unwrap();
        assert_eq!(read.again(), Ids::EMPTY);

        // A later removal by another member undoes nothing.
        let mut g = group(2, &members);
        let mut rekey = Rekey::default();
        rekey.start(&g, 3, Key::new([30; 32]), 1_000).unwrap();
        rekey.switch(&mut g).unwrap();
        rekey.learned(&g, 1, new(40, 5, 1_030, 0, 1).after(30), 1_025);
        assert_eq!(rekey.switch(&mut g).unwrap().undone, Ids::EMPTY);

        // A rival from a higher id loses.
        let g = group(2, &members);
        let mut rekey = Rekey::default();
        rekey.start(&g, 1, Key::new([30; 32]), 1_000).unwrap();
        assert_eq!(
            rekey.learned(&g, 3, new(31, 4, 1_012, 0, 1), 1_007),
            Learned::Ignored
        );
    }

    #[test]
    fn a_removal_can_be_declined_for_a_day_after_its_switch() {
        let mut g = group(0, &[(0, 1), (1, 2), (2, 3), (3, 4)]);
        let mut rekey = Rekey::default();
        rekey.learned(&g, 1, new(9, 4, 1_010, 2, 3), 1_005);
        rekey.switch(&mut g).unwrap();
        let until = 1_010 + UNDO_ROUNDS;
        assert_eq!(rekey.undo_until(), Some(until));
        // A member renamed after the switch, under the new key.
        g.merge(3, signed(3, 4, 2_000), None);
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
                forgotten: Ids::of(2).with(3),
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
        rekey.learned(&g, 1, new(9, 4, 1_010, 2, 3), 1_005);
        rekey.switch(&mut g).unwrap();
        rekey.learned(&g, 1, new(10, 5, 1_030, 3, 4).after(9), 1_025);
        assert_eq!(rekey.undo(&mut g, 1_020, 2), None);
        rekey.switch(&mut g).unwrap();
        assert_eq!(rekey.undo(&mut g, 1_040, 2), None);
        assert_eq!(
            rekey
                .undo(&mut g, 1_040, 3)
                .map(|reverted| reverted.forgotten),
            Some(Ids::of(3))
        );
        assert_eq!(g.generation(), 4);
        assert_eq!(g.gone(2).map(|gone| gone.changed), Some(1_010 * ROUND_S));
    }

    #[test]
    fn declining_after_a_rival_won_goes_back_before_both() {
        let (losing, winning) = (new(21, 4, 1_010, 2, 3), new(20, 4, 1_012, 3, 4));
        let mut g = group(0, &[(0, 1), (1, 2), (2, 3), (3, 4)]);
        let mut rekey = Rekey::default();
        rekey.learned(&g, 3, losing.clone(), 1_005);
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
        assert_eq!(reverted.forgotten, Ids::of(2).with(3));
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
        rekey.learned(&g, 3, new(21, 4, 1_010, 2, 3), 1_005);
        rekey.switch(&mut g).unwrap();
        rekey.learned(&g, 1, new(20, 4, 1_012, 2, 3), 1_020);
        let switched = rekey.switch(&mut g).unwrap();
        assert_eq!(switched.removed, Some(2));
        assert_eq!(switched.restored, Ids::EMPTY);
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

    /// The part of the group this device is in switched twice, and a rival of the first wins.
    /// This device goes back past both: the members they removed come back, and a member still
    /// on any older key is sent the rival.
    #[test]
    fn a_rival_of_a_key_switched_past_goes_back_past_every_switch_since() {
        let mut g = group(0, &[(0, 1), (1, 2), (2, 3), (3, 4), (4, 5), (5, 6)]);
        let (four, five) = (*g.member(4).unwrap(), *g.member(5).unwrap());
        let mut rekey = Rekey::default();
        // Member 2 removes 4, then 5.
        rekey.learned(&g, 2, new(21, 4, 1_010, 4, 5), 1_005);
        rekey.switch(&mut g).unwrap();
        rekey.learned(&g, 2, new(22, 5, 1_020, 5, 6).after(21), 1_015);
        rekey.switch(&mut g).unwrap();
        // Member 1, apart, removed 3 at the first one's generation.
        let rival = new(20, 4, 1_012, 3, 4);
        assert_eq!(
            rekey.learned(&g, 3, new(23, 4, 1_012, 1, 2), 1_030),
            Learned::Ignored,
            "a higher remover's loses"
        );
        assert_eq!(
            rekey.learned(&g, 1, rival.clone().after(7), 1_030),
            Learned::Ignored,
            "it follows another key"
        );
        assert_eq!(rekey.learned(&g, 1, rival, 1_030), Learned::Pending);
        assert_eq!(
            rekey.pending().unwrap().own_switch,
            1_030 + DECLINE_ROUNDS,
            "time to decline it"
        );
        assert_eq!(
            rekey.learned(&g, 2, new(24, 6, 1_034, 1, 2).after(22), 1_031),
            Learned::Later,
            "it follows a key the rival undoes"
        );
        let switched = rekey.switch(&mut g).unwrap();
        assert_eq!(
            (switched.removed, switched.restored, switched.undone),
            (Some(3), Ids::of(4).with(5), Ids::EMPTY)
        );
        assert!(switched.rival);
        assert_eq!(
            (g.generation(), g.member(4), g.member(5)),
            (4, Some(&four), Some(&five))
        );
        assert!(*g.key() == Key::new([20; 32]) && g.gone(3).is_some());
        assert_eq!(rekey.old().count(), 3);
        assert!(
            rekey.old().all(|old| old.catches_up_with == 4),
            "a member on any older key is sent the rival"
        );
        assert_eq!(
            rekey.undo_until(),
            None,
            "declined before its switch, or not at all"
        );
        assert_eq!((rekey.remover_of(4), rekey.remover_of(5)), (Some(1), None));
        assert!(rekey.on_key(1, 4), "a member on the rival's key");
        assert!(!rekey.is_waiting_for(5, 1) && !rekey.is_waiting_for(3, 1));
        let mut out = [0; STORED_MAX];
        let len = rekey.encode(&mut out);
        let read = Rekey::decode(&out[..len]).unwrap();
        assert!(
            read.old()
                .map(|old| old.catches_up_with)
                .eq(rekey.old().map(|old| old.catches_up_with)),
            "across a restart"
        );
        assert_eq!(read.took, rekey.took);
    }

    /// The same on the device that made both removals: it makes both again under the rival.
    #[test]
    fn a_remover_whose_switched_removals_a_rival_undoes_makes_them_again() {
        let mut g = group(2, &[(0, 1), (1, 2), (2, 3), (3, 4), (4, 5), (5, 6)]);
        let mut rekey = Rekey::default();
        rekey.start(&g, 4, Key::new([21; 32]), 1_000).unwrap();
        rekey.switch(&mut g).unwrap();
        rekey.start(&g, 5, Key::new([22; 32]), 1_010).unwrap();
        rekey.switch(&mut g).unwrap();
        assert_eq!(
            rekey.learned(&g, 1, new(20, 4, 1_012, 3, 4), 1_030),
            Learned::Pending
        );
        let switched = rekey.switch(&mut g).unwrap();
        assert_eq!(switched.undone, Ids::of(4).with(5));
        assert_eq!(rekey.again(), Ids::of(4).with(5));
        assert!(g.member(4).is_some() && g.member(5).is_some());
    }

    /// A rival is ranked only against a key this device still keeps, four switches back at most.
    #[test]
    fn a_rival_older_than_the_keys_kept_is_not_taken() {
        let ids: [(u8, u8); 9] = core::array::from_fn(|id| (id as u8, id as u8 + 1));
        let mut g = group(0, &ids);
        let mut rekey = Rekey::default();
        let mut follows = 5;
        let mut switch = |g: &mut Group, rekey: &mut Rekey, at: u8| {
            let key = new(
                30 + at,
                4 + u16::from(at),
                1_010 + 10 * u32::from(at),
                3 + at,
                4 + at,
            );
            rekey.learned(g, 2, key.after(follows), 1_005 + 10 * u32::from(at));
            rekey.switch(g).unwrap();
            follows = 30 + at;
        };
        for at in 0..4 {
            switch(&mut g, &mut rekey, at);
        }
        let rival = new(20, 4, 1_012, 1, 2);
        let (mut g4, mut rekey4) = (g.clone(), rekey.clone());
        assert_eq!(
            rekey4.learned(&g4, 1, rival.clone(), 1_100),
            Learned::Pending,
            "four switches back"
        );
        assert!(rekey4.switch(&mut g4).unwrap().rival);
        assert_eq!(g4.generation(), 4);
        switch(&mut g, &mut rekey, 4);
        assert_eq!(rekey.learned(&g, 1, rival, 1_100), Learned::Ignored);
    }

    /// This device switched for its own removal and started another on that key. A rival of the
    /// first wins: both removals are to be made again under it.
    #[test]
    fn a_rival_over_two_removals_of_this_device_has_both_made_again() {
        let mut g = group(2, &[(0, 1), (1, 2), (2, 3), (3, 4), (4, 5)]);
        let mut rekey = Rekey::default();
        rekey.start(&g, 3, Key::new([30; 32]), 1_000).unwrap();
        rekey.switch(&mut g).unwrap();
        rekey.start(&g, 4, Key::new([40; 32]), 1_010).unwrap();
        assert_eq!(
            rekey.learned(&g, 1, new(31, 4, 1_012, 0, 1), 1_020),
            Learned::Pending
        );
        rekey.switch(&mut g).unwrap();
        assert!(*g.key() == Key::new([31; 32]));
        assert_eq!(rekey.again(), Ids::of(3).with(4));
    }

    #[test]
    fn a_key_that_removes_nobody_leaves_the_last_removal_to_decline() {
        let mut g = group(0, &[(0, 1), (1, 2), (2, 3), (3, 4)]);
        left(&mut g, 6, 99);
        let mut rekey = Rekey::default();
        rekey.learned(&g, 1, new(9, 4, 1_010, 2, 3), 1_005);
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
        rekey.learned(&g, 3, new(21, 4, 1_010, 2, 3), 1_005);
        rekey.switch(&mut g).unwrap();
        rekey.learned(&g, 1, new(20, 4, 1_012, 6, 99), 1_020);
        assert_eq!(rekey.switch(&mut g).unwrap().restored, Ids::of(2));
        assert_eq!(rekey.undo_until(), None);
        assert_eq!(rekey.undo(&mut g, 1_030, 2), None);
        assert_eq!(g.generation(), 4);
    }

    #[test]
    fn declining_a_rival_of_a_key_that_removed_nobody_goes_back_to_that_key() {
        let mut g = group(0, &[(0, 1), (1, 2), (2, 3), (3, 4), (4, 5)]);
        left(&mut g, 6, 99);
        let mut rekey = Rekey::default();
        rekey.learned(&g, 1, new(9, 4, 1_010, 2, 3), 1_005);
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
        assert_eq!((switched.removed, switched.undone), (Some(3), Ids::EMPTY));
        assert!(g.gone(3).is_some());

        // Before it.
        let mut g = group(2, &[(0, 1), (1, 2), (2, 3), (3, 4)]);
        let mut rekey = Rekey::default();
        rekey.start(&g, 3, Key::new([30; 32]), 1_000).unwrap();
        rekey.learned(&g, 1, rival, 1_007);
        assert_eq!(rekey.switch(&mut g).unwrap().undone, Ids::EMPTY);
    }

    #[test]
    fn declining_keeps_an_older_rival_of_the_key_gone_back_to() {
        let mut g = group(0, &[(0, 1), (1, 2), (2, 3), (3, 4)]);
        let mut rekey = Rekey::default();
        rekey.learned(&g, 1, new(9, 4, 1_010, 2, 3), 1_005);
        rekey.old[0] = Some(Old {
            key: Key::new([20; 32]),
            generation: 3,
            waiting: Ids::of(3),
            catches_up_with: 3,
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
                own_switch: 1_010,
            }),
            removing: Some((4, 2)),
            again: Ids::ALL,
            undo: Some(Undo {
                key: Key::new([5; 32]),
                generation: 3,
                new_generation: 4,
                switched: 1_010,
                until: 2_930,
                changed: Ids::ALL,
                removed: 2,
                theirs: true,
            }),
            ..Rekey::default()
        };
        for (at, old) in rekey.old.iter_mut().enumerate() {
            *old = Some(Old {
                key: Key::new([at as u8; 32]),
                generation: at as u16,
                waiting: Ids::ALL,
                catches_up_with: (at as u16).wrapping_sub(1),
            });
        }
        for (at, declined) in rekey.declined.iter_mut().enumerate() {
            *declined = Some([at as u8; 8]);
        }
        for (at, took) in rekey.took.iter_mut().enumerate() {
            // Only the newest switch's record is stored.
            let record = (at == 0).then(|| Member {
                name: Name::new(b"Sixteen letters!").unwrap(),
                ..member(3, 0)
            });
            *took = Some(Took {
                generation: 4 - at as u16,
                remover: at as u8,
                follows: Some([at as u8; 8]),
                removed: Some(Removed {
                    id: 2 + at as u8,
                    fingerprint: [10 + at as u8; 8],
                    record,
                }),
            });
        }
        let mut out = [0; STORED_MAX];
        assert_eq!(rekey.encode(&mut out), STORED_MAX);
        let read = Rekey::decode(&out).unwrap();
        assert!(
            read.old()
                .map(|old| old.catches_up_with)
                .eq(rekey.old().map(|old| old.catches_up_with))
        );
        assert_eq!(read.took, rekey.took);
        assert_eq!(read.again(), Ids::ALL);
    }

    /// The bytes [`Rekey::encode`] writes after what was stored before each switch kept what
    /// it followed and whom it removed, and each old key the generation it catches up with.
    fn kept_since(rekey: &Rekey) -> usize {
        rekey
            .took
            .iter()
            .flatten()
            .map(|took| 2 + took.follows.map_or(0, |_| 8) + took.removed.map_or(0, |_| 9))
            .sum::<usize>()
            + 2 * rekey.old().count()
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
        rekey.learned(&g, 3, new(10, 5, 1_030, 1, 2).after(9), 1_025);
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
        assert_eq!(pending.own_switch, 1_040 + DECLINE_ROUNDS);
        assert_eq!(read.old().count(), 1);
        assert_eq!(read.declined().count(), 1);
        assert_eq!(read.removing(), Some(2));
        assert!(Rekey::decode(&out[..len - 1]).is_none());

        // The key kept to decline a removal after its switch, and its absence from what was
        // stored before there was one.
        let mut g = group(0, &[(0, 1), (1, 2), (2, 3)]);
        let mut rekey = Rekey::default();
        rekey.learned(&g, 1, new(9, 4, 1_010, 2, 3), 1_005);
        rekey.switch(&mut g).unwrap();
        g.merge(1, signed(1, 2, 2_000), None);
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
            (Ids::of(1).with(2), 2, true)
        );
        assert!(read.undo(&mut g, 1_010, 2).is_some());
        assert!(*g.key() == Key::new([5; 32]));
        assert_eq!(g.generation(), 3);
        let len = rekey.encode(&mut out);
        let read = Rekey::decode(&out[..len]).unwrap();
        let took = read.took[0].unwrap();
        assert_eq!(took.follows, Some(key_fingerprint(&Key::new([5; 32]))));
        assert_eq!(took.removed.unwrap().record, Some(member(3, 100)));
        // What was stored before each switch kept what it followed and whom it removed, before
        // rivals were ranked by their removers, before the removers, and before the undo too.
        let len = len - kept_since(&rekey);
        let read = Rekey::decode(&out[..len]).unwrap();
        assert_eq!(read.took, rekey.took, "the newest switch's alone");
        assert!(
            read.old()
                .all(|old| old.catches_up_with == old.generation + 1)
        );
        let mut before = rekey.clone();
        if let Some(took) = &mut before.took[0] {
            took.follows = None;
            took.removed = took.removed.map(|removed| Removed {
                record: None,
                ..removed
            });
        }
        let len = before.encode(&mut out) - kept_since(&before);
        assert!(Rekey::decode(&out[..len - 4]).is_some_and(|read| read.again().is_empty()));
        assert!(Rekey::decode(&out[..len - 7]).is_some_and(|read| {
            read.took[0]
                .is_some_and(|took| took.removed.is_some_and(|removed| removed.record.is_none()))
        }));
        before.took = [None; OLD_KEYS];
        let len = before.encode(&mut out) - kept_since(&before);
        assert!(Rekey::decode(&out[..len - 8]).is_some_and(|read| read.undo_until().is_some()));
        before.undo = None;
        let len = before.encode(&mut out) - kept_since(&before);
        assert!(Rekey::decode(&out[..len - 9]).is_some_and(|read| read.undo_until().is_none()));
    }

    #[test]
    fn a_members_kept_row_comes_back_from_its_bytes() {
        extern crate std;
        let key = Key::new([1; 32]);
        let remover = crate::members::tests::key(1);
        let to = |dest, generation, seq| {
            Message::key(
                1,
                dest,
                seq,
                0,
                100,
                generation,
                &[kind::KEY],
                &key,
                &remover,
            )
            .unwrap()
        };
        let mut kept = std::boxed::Box::new(Kept::zeroed());
        assert!(!kept.holds(4));
        for (generation, seq) in [(4, 5), (5, 6), (6, 7), (7, 8)] {
            kept.keep(&to(4, generation, seq), None);
        }
        kept.keep(&to(9, 4, 20), None);
        let mut row = [0; KEPT_ROW_MAX];
        let len = kept.encode_row(4, &mut row);
        let mut back = std::boxed::Box::new(Kept::zeroed());
        assert!(back.restore_row(4, &row[..len]));
        for generation in 4..8 {
            assert_eq!(
                back.get(4, generation).map(|message| message.name()),
                kept.get(4, generation).map(|message| message.name()),
            );
        }
        assert!(!back.holds(9), "only the row's own member");
        let mut empty = [0; KEPT_ROW_MAX];
        assert_eq!(kept.encode_row(3, &mut empty), 1);
        assert!(back.restore_row(3, &empty[..1]));
        assert!(!back.holds(3));
    }

    #[test]
    fn a_kept_row_for_another_member_or_cut_short_is_refused() {
        extern crate std;
        let key = Key::new([1; 32]);
        let remover = crate::members::tests::key(1);
        let message = Message::key(1, 4, 5, 0, 100, 4, &[kind::KEY], &key, &remover).unwrap();
        let mut kept = std::boxed::Box::new(Kept::zeroed());
        kept.keep(&message, None);
        let mut row = [0; KEPT_ROW_MAX];
        let len = kept.encode_row(4, &mut row);
        let mut back = std::boxed::Box::new(Kept::zeroed());
        assert!(!back.restore_row(5, &row[..len]), "another member's");
        assert!(!back.restore_row(4, &row[..len - 1]), "cut short");
        assert!(!back.holds(4) && !back.holds(5));
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
        kept.forget_after(7);
        assert!(
            kept.get(4, 7).is_some() && kept.get(4, 8).is_none() && kept.get(6, 9).is_none(),
            "a rival at generation 7 undid the keys after it"
        );
        kept.clear();
        assert_eq!(kept.get(4, 5), None);
    }

    #[test]
    fn a_switch_records_who_removed_for_each_generation() {
        let mut g = group(0, &[(0, 1), (1, 2), (2, 3), (3, 4)]);
        let mut rekey = Rekey::default();
        rekey.learned(&g, 1, new(9, 4, 1_010, 2, 3), 1_005);
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
