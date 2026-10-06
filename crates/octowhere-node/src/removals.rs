//! What the node keeps of removals beside the group: the removal under way and the keys before
//! it, the key messages kept for members that missed a switch, and what goes to those members
//! under an old key.

use alloc::boxed::Box;
use core::alloc::Allocator;

use crate::write::KeptRow;

#[cfg(feature = "defmt")]
use defmt::{info, warn};
use octowhere_mesh::{
    IDS, Ids,
    members::Group,
    messages::{Message, To},
    pair::Identity,
    rekey::{KEPT_ROW_MAX, Kept, OnKey, Rekey, Switched},
    schedule::{SWEEP_EVERY, is_sweep_round},
    seal::Key,
};

/// How many times a removal notice is sent.
const NOTICE_SENDS: u8 = 3;
/// A member waiting for its key message is sent it under an old key again only after a gap of
/// sweep rounds that doubles with each send, up to 2 to this power, about ten hours: one that
/// declined the removal never takes it, and replays of one of its packets would otherwise spend
/// every send before it is back (owner, 2026-10-04).
const CATCH_UP_DOUBLINGS: u32 = 6;
/// The packets after a switch that carry this node's word that it is on the new key, beside
/// those in sweep rounds.
const ON_KEY_FIRST: u8 = 3;
/// How long after a switch its sweep rounds' packets carry that word.
const ON_KEY_US: i64 = 24 * 3_600 * 1_000_000;

/// The members one packet under an old key catches up, each with the generation of the key
/// message it is sent.
pub type Catching = heapless::Vec<(u8, u16), 4>;

/// A member to send a message under an old key: one that missed a switch, or one removed.
struct CatchUp {
    id: u8,
    key: Key,
    generation: u16,
    /// The generation of the key message it is sent.
    catches_up_with: u16,
}

/// This node's signed word that it is on the group's key, while it is to send it.
struct OnKeySend {
    record: OnKey,
    /// The packets left to carry it whatever the round.
    first: u8,
    /// Until when, on the local timer, sweep rounds' packets carry it.
    until: i64,
}

/// The message telling a member it was removed, held by its remover until the switch, then
/// sent under the old key, which only the members that missed the switch and it still hold.
struct RemovalNotice {
    message: Message,
    key: Key,
    generation: u16,
    /// How many more times it is sent, on hearing the member under the old key.
    left: u8,
}

pub struct Removals<A: Allocator> {
    /// The removal under way, the older keys kept, and those declined.
    pub rekey: Box<Rekey>,
    /// The newest key message to each member, for one that missed a switch.
    kept: Box<Kept, A>,
    /// Members to send a message under an old key: their key message, or the removal notice.
    catch_up: heapless::Vec<CatchUp, 4>,
    /// The message telling the member this device removed that it was, until it has gone.
    removal_notice: Option<Box<RemovalNotice>>,
    /// The round the last header under an old key went out in.
    beacon_round: i64,
    /// The members this device's own removal under way has posted key messages to.
    keys_posted: Ids,
    /// The member this device removed at the last switch, to tell, with the old key and its
    /// generation.
    notify: Option<(u8, Key, u16)>,
    /// How many times each member has been sent its key message under the newest old key, and
    /// the round of the last.
    caught_up: [(u8, i64); IDS as usize],
    on_key: Option<OnKeySend>,
    /// A switch's generation and remover, whose key messages are to be kept for catch-up.
    refill: Option<(u16, u8)>,
    /// The members whose kept key messages the flash holds, and those changed since.
    stored_kept: Ids,
    changed_kept: Ids,
    /// The round of the last switch since the start, and the round now.
    switch_round: Option<u32>,
    round: u32,
}

impl<A: Allocator> Removals<A> {
    pub fn new(rekey: Box<Rekey>, kept: Box<Kept, A>) -> Self {
        Self {
            rekey,
            kept,
            catch_up: heapless::Vec::new(),
            removal_notice: None,
            beacon_round: i64::MIN,
            keys_posted: Ids::EMPTY,
            notify: None,
            caught_up: [(0, 0); IDS as usize],
            on_key: None,
            refill: None,
            stored_kept: Ids::EMPTY,
            changed_kept: Ids::EMPTY,
            switch_round: None,
            round: 0,
        }
    }

    /// Takes back the key messages kept for the member `id` from the flash, at the start.
    pub fn restore_kept(&mut self, id: u8, row: &[u8]) {
        // Kept or not, the row is in the flash, and goes once it is not wanted.
        self.stored_kept.insert(id);
        if self.kept.restore_row(id, row) {
            info!(
                "[REKEY] the key messages for {} are back from the flash",
                id
            );
        } else {
            warn!("[REKEY] the key messages stored for {} are unreadable", id);
        }
    }

    /// The round now, which holds a member's key messages off the flash until it has had a
    /// round after the switch to say it is on the new key.
    pub fn at_round(&mut self, round: u32) {
        self.round = round;
    }

    /// Whether key messages are kept for the member `id` and it is still waited for.
    fn kept_wanted(&self, id: u8) -> bool {
        self.kept.holds(id) && self.rekey.old().any(|old| old.waiting.contains(id))
    }

    /// Whether the round after the last switch has passed, by which a member there has said it
    /// is on the new key: one still waited for missed it.
    fn past_switch(&self) -> bool {
        self.switch_round
            .is_none_or(|switched| self.round > switched.saturating_add(1))
    }

    /// The next member whose kept key messages the flash is due: wanted there and not stored
    /// as they are, or stored and no longer wanted. Until the round after a switch, what is
    /// stored stays as it was, while the store refills what the switch dropped.
    pub fn kept_due(&self) -> Option<u8> {
        if !self.past_switch() {
            return None;
        }
        (0..IDS).find(|&id| {
            let stored = self.stored_kept.contains(id);
            if self.kept_wanted(id) {
                !stored || self.changed_kept.contains(id)
            } else {
                stored
            }
        })
    }

    /// What the flash is to hold for the member `id`: its kept key messages, or `None` to delete
    /// them. Apart from the step, since a row is about as long as a page of stack.
    #[inline(never)]
    pub fn kept_row(&self, id: u8) -> Option<Box<KeptRow>> {
        if !self.kept_wanted(id) {
            return None;
        }
        let mut row = Box::new(KeptRow::new());
        row.resize(KEPT_ROW_MAX, 0).expect("the row's own size");
        let len = self
            .kept
            .encode_row(id, (&mut row[..]).try_into().expect("a whole row"));
        row.truncate(len);
        Some(row)
    }

    /// The row of the member `id` went to the flash: stored, or with `stores` false, deleted.
    pub fn kept_queued(&mut self, id: u8, stores: bool) {
        info!("[REKEY] the key messages for {} stored={}", id, stores);
        self.changed_kept.remove(id);
        if stores {
            self.stored_kept.insert(id);
        } else {
            self.stored_kept.remove(id);
        }
    }

    /// A write to the flash was lost, so every row it may have held is written again.
    pub fn kept_lost(&mut self) {
        self.changed_kept = Ids::ALL;
    }

    /// Forgets the removals of a group this device no longer belongs to.
    pub fn forget(&mut self) {
        self.rekey.clear();
        self.kept.clear();
        self.refill = None;
        self.catch_up.clear();
        self.removal_notice = None;
        self.keys_posted = Ids::EMPTY;
        self.notify = None;
        self.caught_up = [(0, 0); IDS as usize];
        self.on_key = None;
    }

    /// This device started a removal, whose key messages are yet to be posted.
    pub fn started(&mut self) {
        self.keys_posted = Ids::EMPTY;
    }

    /// The key message of this device's removal under way to `member` is posted.
    pub fn posted(&mut self, member: u8) {
        self.keys_posted.insert(member);
    }

    /// The members of `group` still owed a key message by this device's removal under way.
    pub fn keys_due(&self, group: &Group) -> Ids {
        match self.rekey.pending() {
            Some(pending) if pending.remover == group.own() => {
                group
                    .ids()
                    .without(pending.new.removed)
                    .without(group.own())
                    & !self.keys_posted
            }
            _ => Ids::EMPTY,
        }
    }

    /// The group switched, to the key `group` now has, at local time `now`; `old` is the key
    /// before it and its generation.
    pub fn switched(
        &mut self,
        group: &Group,
        switched: &Switched,
        old: (Key, u16),
        me: &Identity,
        now: i64,
    ) {
        if switched.remover == group.own()
            && let Some(removed) = switched.removed
        {
            self.notify = Some((removed, old.0, old.1));
        }
        self.kept.keep_only(group.generation(), switched.remover);
        self.changed_kept = Ids::ALL;
        self.switch_round = Some(self.round);
        self.refill = Some((group.generation(), switched.remover));
        self.caught_up = [(0, 0); IDS as usize];
        // Signed once a switch: about 35 ms on the board.
        self.carry_on_key(group, me, now);
        self.keys_posted = Ids::EMPTY;
    }

    /// The group went back to the key before the last switch, declined after it.
    pub fn reverted(&mut self) {
        self.kept.clear();
        self.catch_up.clear();
        self.caught_up = [(0, 0); IDS as usize];
        self.on_key = None;
    }

    /// Every member is on the new key: the key messages kept for catch-up go, and only the
    /// member removed is still sent anything under the old key.
    pub fn all_on_new_key(&mut self) {
        self.kept.clear();
        // The member removed is no member, so nobody waits for it; it is still told.
        let told = self
            .removal_notice
            .as_ref()
            .map(|notice| notice.message.to());
        self.catch_up.retain(|up| told == Some(To::Member(up.id)));
    }

    /// The last switch's generation and remover, once after it: the key messages of theirs the
    /// store holds are to be kept for catch-up.
    pub fn take_refill(&mut self) -> Option<(u16, u8)> {
        self.refill.take()
    }

    /// Starts carrying this node's word that it is on `group`'s key: in its next few packets,
    /// and in sweep rounds' for a day from local time `now`.
    pub fn carry_on_key(&mut self, group: &Group, me: &Identity, now: i64) {
        self.on_key = Some(OnKeySend {
            record: OnKey::new(group.own(), group.generation(), group.key(), me),
            first: ON_KEY_FIRST,
            until: now + ON_KEY_US,
        });
    }

    /// The word that this node is on the group's key, if the packet of `round`, sent at local
    /// time `send_at`, is to carry it.
    pub fn on_key_for(&self, round: i64, send_at: i64) -> Option<&OnKey> {
        self.on_key
            .as_ref()
            .filter(|on| on.first > 0 || (is_sweep_round(round) && send_at < on.until))
            .map(|on| &on.record)
    }

    /// Whether this node's word that it is on the group's key is still to go in its next
    /// packets, whatever the round.
    #[must_use]
    pub fn on_key_first(&self) -> bool {
        self.on_key.as_ref().is_some_and(|on| on.first > 0)
    }

    /// Whether a sweep round's packet sent at local time `now` carries that word.
    #[must_use]
    pub fn on_key_in_sweeps(&self, now: i64) -> bool {
        self.on_key.as_ref().is_some_and(|on| now < on.until)
    }

    /// A packet carried the word that this node is on the group's key.
    pub fn carried_on_key(&mut self) {
        if let Some(on) = &mut self.on_key {
            on.first = on.first.saturating_sub(1);
        }
    }

    /// Whether the member this device removed at the last switch is yet to be told.
    pub fn to_tell(&self) -> bool {
        self.notify.is_some()
    }

    /// Takes the member to tell it was removed, with the old key and its generation.
    pub fn take_to_tell(&mut self) -> Option<(u8, Key, u16)> {
        self.notify.take()
    }

    /// Puts back the member to tell, whose message cannot be made yet.
    pub fn tell_later(&mut self, removed: (u8, Key, u16)) {
        self.notify = Some(removed);
    }

    /// Queues `message`, which tells the member `id` it was removed, to go under the old key
    /// `key` of `generation`.
    pub fn tell(&mut self, message: Message, (id, key, generation): (u8, Key, u16)) {
        self.removal_notice = Some(Box::new(RemovalNotice {
            message,
            key: key.clone(),
            generation,
            left: NOTICE_SENDS,
        }));
        let _ = self.catch_up.push(CatchUp {
            id,
            key,
            generation,
            catches_up_with: generation.wrapping_add(1),
        });
    }

    /// Whether a packet under an old key may be due: a member is to be caught up, or waited for.
    pub fn sends_old(&self) -> bool {
        !self.catch_up.is_empty() || self.rekey.is_waiting()
    }

    /// Whether a member is to be sent a message under an old key.
    pub fn catching_up(&self) -> bool {
        !self.catch_up.is_empty()
    }

    /// The old key a packet under one goes out under, with whom it catches up and the generation
    /// of the key message each is sent: the key a member waiting for its key message is on, or
    /// in a sweep round while any member is waited for, the newest old key, so that parts of the
    /// group on rival keys still hear each other.
    pub fn old_packet(&self, round: i64) -> Option<(&Key, u16, Catching)> {
        if let Some(first) = self.catch_up.first() {
            let ids = self
                .catch_up
                .iter()
                .filter(|up| up.key == first.key)
                .map(|up| (up.id, up.catches_up_with))
                .collect();
            return Some((&first.key, first.generation, ids));
        }
        (is_sweep_round(round) && round != self.beacon_round && self.rekey.is_waiting())
            .then(|| self.rekey.old().next())
            .flatten()
            .map(|old| (&old.key, old.generation, heapless::Vec::new()))
    }

    /// What a packet under an old key carries to the member `id`: the removal notice, if it is
    /// the member removed, or else the key message of `catches_up_with`.
    pub fn message_for(&self, id: u8, catches_up_with: u16) -> Option<&Message> {
        match &self.removal_notice {
            Some(notice) if notice.message.to() == To::Member(id) => Some(&notice.message),
            _ => self.kept.get(id, catches_up_with),
        }
    }

    /// A packet under an old key went out in `round` with the messages of the members in
    /// `caught`; those in `lost` had none left to send.
    pub fn sent_old(&mut self, caught: Ids, lost: Ids, round: i64) {
        self.catch_up.retain(|up| !(caught | lost).contains(up.id));
        for id in caught.iter() {
            let (sends, last) = &mut self.caught_up[usize::from(id)];
            *sends = sends.saturating_add(1);
            *last = round;
        }
        if let Some(notice) = &mut self.removal_notice
            && let To::Member(id) = notice.message.to()
            && caught.contains(id)
        {
            notice.left -= 1;
            if notice.left == 0 {
                self.removal_notice = None;
            }
        }
        self.beacon_round = round;
    }

    /// Takes word, in `round`, of `sender` on the old key `key` of `generation`: unless it is the
    /// member removed, which is sent its notice, the key message of `catches_up_with` goes to it
    /// when it is waited for and not sent it too recently.
    pub fn heard_on_old(
        &mut self,
        sender: u8,
        (key, generation, catches_up_with): (Key, u16, u16),
        round: i64,
    ) {
        if let Some(notice) = &self.removal_notice
            && notice.message.to() == To::Member(sender)
            && notice.generation == generation
        {
            if notice.left > 0 && self.catch_up.iter().all(|up| up.id != sender) {
                let _ = self.catch_up.push(CatchUp {
                    id: sender,
                    key: notice.key.clone(),
                    generation,
                    catches_up_with,
                });
            }
            return;
        }
        if !self.rekey.is_waiting_for(generation, sender) {
            info!("[REKEY] heard {} on generation {}", sender, generation);
            return;
        }
        let (sends, last) = self.caught_up[usize::from(sender)];
        let due =
            last + SWEEP_EVERY * (1 << u32::from(sends.saturating_sub(1)).min(CATCH_UP_DOUBLINGS));
        if sends > 0 && round < due {
            info!(
                "[REKEY] {} is on generation {}; sent its key message {} times, again from round {}",
                sender, generation, sends, due
            );
            return;
        }
        if self.kept.get(sender, catches_up_with).is_none() {
            info!(
                "[REKEY] {} is on generation {}; no key message is held for it",
                sender, generation
            );
            return;
        }
        if self.catch_up.iter().all(|up| up.id != sender)
            && self
                .catch_up
                .push(CatchUp {
                    id: sender,
                    key,
                    generation,
                    catches_up_with,
                })
                .is_ok()
        {
            info!(
                "[REKEY] {} is on generation {}; its key message goes to it",
                sender, generation
            );
        }
    }

    /// Keeps `message`, a key message whose signature the caller checked, for catch-up.
    pub fn keep(&mut self, message: &Message, remover: Option<u8>) {
        if self.kept.keep(message, remover)
            && let To::Member(dest) = message.to()
        {
            self.changed_kept.insert(dest);
        }
    }

    /// The key message of `generation` kept for the member `id`.
    pub fn kept(&self, id: u8, generation: u16) -> Option<&Message> {
        self.kept.get(id, generation)
    }
}
