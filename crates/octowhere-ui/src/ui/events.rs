//! The runtime events the drawer lists, from the 2026-10-04 hand-off: what happened, whether it
//! is still going on, and whether the user has opened it. An event keeps one identity through its
//! life: a GNSS incident from the receiver stopping to its answering again, a conversation's
//! messages while it has unread ones, a removal from its request to its end. Events live in RAM,
//! and a restart loses them.
//!
//! Live events are kept by the state they show: a GNSS incident under way, a removal the mesh
//! shows, this device's own removal while the mesh says so, and a conversation with unread
//! messages. Only settled events are history, which holds at most [`HISTORY`], so that no number
//! of them pushes out an action still open or a conversation still unread (2026-10-05 hand-off).

use heapless::Vec;

use super::{
    gesture::Micros,
    group::view::{
        Decline, IDS, MessagesView, Name, RemovalStage, RemovalView, RemovalsView, Thread,
    },
    screens::GnssHealth,
};

/// The most settled events kept as history. A new one takes the place of the oldest, a read one
/// first; live events never count against it.
pub const HISTORY: usize = 16;
/// The live events besides conversations: a GNSS incident, the two removals the mesh shows, and
/// this device's own removal.
const LIVE: usize = 4;
/// The most conversations with unread messages that keep an event, as many as the inbox lists:
/// the group and every member, and as many devices again that were members at their ids.
pub const CONVERSATIONS: usize = 2 * IDS as usize;
/// The most events the drawer lists.
pub const CAPACITY: usize = HISTORY + LIVE + CONVERSATIONS;

/// Resets that fail before an unanswering receiver counts as a fault.
pub const FAULT_RESETS: u8 = 3;

/// One event's identity for as long as the device runs. Ids are never reused.
pub type Id = u32;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Kind {
    Gnss(Gnss),
    /// A conversation's new messages: how many are unread, the newest told of, and its sender.
    Messages {
        thread: Thread,
        unread: u16,
        newest: u32,
        from: u8,
    },
    /// A removal, by its new key, as last seen.
    Removal(RemovalView),
    /// Another member removed this device: who, and when this device was told.
    Removed {
        by: u8,
        name: Name,
        at: i64,
    },
}

/// Where a GNSS incident is.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gnss {
    /// The receiver stopped answering, and reset `attempt` of [`FAULT_RESETS`] is under way.
    Recovering { attempt: u8 },
    /// `failed` resets, at least [`FAULT_RESETS`], did not bring it back. The resets go on.
    Fault { failed: u8 },
    /// It answers again, which says nothing of a fix.
    Responding,
}

/// Why an event cannot be dismissed yet.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Protected {
    /// A fault or a recovery has not settled.
    Unresolved,
    /// An operation is still running.
    Unfinished,
    /// A removal this device switched to can still be declined.
    Declinable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Event {
    pub id: Id,
    pub kind: Kind,
    /// When it began, or last changed in a way the user is told of. Routine progress leaves it.
    pub at: Micros,
    pub unread: bool,
}

impl Event {
    /// Why it cannot be dismissed or cleared, while it cannot.
    #[must_use]
    pub fn protected(&self) -> Option<Protected> {
        match self.kind {
            Kind::Gnss(Gnss::Recovering { .. } | Gnss::Fault { .. }) => Some(Protected::Unresolved),
            Kind::Removal(removal) => match removal.stage {
                RemovalStage::Pending { .. } => Some(Protected::Unfinished),
                // The mesh says when its day has passed.
                RemovalStage::Switched {
                    decline: Decline::Until(_),
                    ..
                } => Some(Protected::Declinable),
                _ => None,
            },
            Kind::Gnss(Gnss::Responding) | Kind::Messages { .. } | Kind::Removed { .. } => None,
        }
    }

    /// An operation still running, which the drawer keeps at the top.
    #[must_use]
    pub fn ongoing(&self) -> bool {
        self.protected() == Some(Protected::Unfinished)
    }
}

/// Why a dismissal did nothing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Kept {
    Protected(Protected),
    /// What it shows still holds, a conversation's unread messages or a removal the mesh shows,
    /// which would only bring it back.
    Live,
    /// No event has the id, or no longer has it.
    Gone,
}

/// A conversation with unread messages, kept apart from the history and smaller than an
/// [`Event`], since there can be many.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Conversation {
    id: Id,
    thread: Thread,
    unread: u16,
    /// The newest message told of, and its sender.
    newest: u32,
    from: u8,
    at: Micros,
    /// Its event is unread: told of, and not opened since.
    fresh: bool,
}

impl Conversation {
    fn event(&self) -> Event {
        Event {
            id: self.id,
            kind: Kind::Messages {
                thread: self.thread,
                unread: self.unread,
                newest: self.newest,
                from: self.from,
            },
            at: self.at,
            unread: self.fresh,
        }
    }
}

/// What the live events in the list show: the GNSS incident under way, the removals the mesh
/// shows, by their new keys, and when it said another member removed this device.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct Sources {
    incident: Option<Id>,
    shown: [Option<[u8; 8]>; 2],
    removed: Option<i64>,
}

impl Sources {
    /// Whether `event` is live: what it shows still holds, so it is not history.
    fn hold(&self, event: &Event) -> bool {
        match event.kind {
            Kind::Gnss(_) => self.incident == Some(event.id),
            Kind::Removal(removal) => self.shown.contains(&Some(removal.key)),
            Kind::Removed { at, .. } => self.removed == Some(at),
            Kind::Messages { .. } => false,
        }
    }
}

/// Where an event is held.
#[derive(Clone, Copy, Debug)]
enum Slot {
    List(u8),
    Conversation(u8),
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Events {
    /// The live events but conversations, and the history, in the order they were made.
    list: Vec<Event, { HISTORY + LIVE }>,
    /// The conversations with unread messages.
    conversations: Vec<Conversation, CONVERSATIONS>,
    next: Id,
    sources: Sources,
    /// The events of two removals that compete, the winner first, while the mesh shows both.
    rivals: Option<[Id; 2]>,
}

impl Events {
    #[must_use]
    pub fn get(&self, id: Id) -> Option<Event> {
        self.list
            .iter()
            .copied()
            .find(|event| event.id == id)
            .or_else(|| {
                self.conversations
                    .iter()
                    .find(|conversation| conversation.id == id)
                    .map(Conversation::event)
            })
    }

    fn event(&self, slot: Slot) -> Event {
        match slot {
            Slot::List(i) => self.list[usize::from(i)],
            Slot::Conversation(i) => self.conversations[usize::from(i)].event(),
        }
    }

    /// The events in the drawer's order: operations still running first, then the latest
    /// change first.
    pub fn ordered(&self) -> impl Iterator<Item = Event> + '_ {
        let mut order: Vec<Slot, CAPACITY> = (0..self.list.len() as u8)
            .map(Slot::List)
            .chain((0..self.conversations.len() as u8).map(Slot::Conversation))
            .collect();
        order.sort_unstable_by_key(|&slot| {
            let event = self.event(slot);
            (
                core::cmp::Reverse(event.ongoing()),
                core::cmp::Reverse(event.at),
                core::cmp::Reverse(event.id),
            )
        });
        order.into_iter().map(|slot| self.event(slot))
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.list.len() + self.conversations.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// How many settled events are kept as history, at most [`HISTORY`].
    #[must_use]
    pub fn history(&self) -> usize {
        self.list
            .iter()
            .filter(|event| !self.sources.hold(event))
            .count()
    }

    #[must_use]
    pub fn unread(&self) -> usize {
        self.list.iter().filter(|event| event.unread).count()
            + self
                .conversations
                .iter()
                .filter(|conversation| conversation.fresh)
                .count()
    }

    #[must_use]
    pub fn ongoing(&self) -> usize {
        self.list.iter().filter(|event| event.ongoing()).count()
    }

    /// Marks an event read, as opening it does.
    pub fn read(&mut self, id: Id) {
        if let Some(event) = self.list.iter_mut().find(|event| event.id == id) {
            event.unread = false;
        }
        if let Some(conversation) = self
            .conversations
            .iter_mut()
            .find(|conversation| conversation.id == id)
        {
            conversation.fresh = false;
        }
    }

    /// Whether [`mark_all_read`](Self::mark_all_read) would change anything.
    #[must_use]
    pub fn can_mark_all_read(&self) -> bool {
        self.unread() > 0
    }

    pub fn mark_all_read(&mut self) {
        self.list.iter_mut().for_each(|event| event.unread = false);
        self.conversations
            .iter_mut()
            .for_each(|conversation| conversation.fresh = false);
    }

    fn clearable(sources: &Sources, event: &Event) -> bool {
        !event.unread && event.protected().is_none() && !sources.hold(event)
    }

    /// Whether [`clear_read`](Self::clear_read) would remove anything.
    #[must_use]
    pub fn can_clear_read(&self) -> bool {
        self.list
            .iter()
            .any(|event| Self::clearable(&self.sources, event))
    }

    /// Removes the read events that have settled. Unread ones, and live ones, stay.
    pub fn clear_read(&mut self) {
        let sources = self.sources;
        self.list.retain(|event| !Self::clearable(&sources, event));
    }

    /// Removes one settled event, read or not, as its detail's DISMISS does. Its eligibility is
    /// checked now, by its id.
    pub fn dismiss(&mut self, id: Id) -> Result<(), Kept> {
        if self
            .conversations
            .iter()
            .any(|conversation| conversation.id == id)
        {
            return Err(Kept::Live);
        }
        let index = self
            .list
            .iter()
            .position(|event| event.id == id)
            .ok_or(Kept::Gone)?;
        let event = &self.list[index];
        if let Some(protected) = event.protected() {
            return Err(Kept::Protected(protected));
        }
        if self.sources.hold(event) {
            return Err(Kept::Live);
        }
        self.list.remove(index);
        Ok(())
    }

    fn get_mut(&mut self, id: Id) -> Option<&mut Event> {
        self.list.iter_mut().find(|event| event.id == id)
    }

    /// Drops the oldest event of the history, a read one first, if there is one.
    fn evict(&mut self) -> bool {
        let oldest = |unread: bool| {
            self.list
                .iter()
                .enumerate()
                .filter(|(_, event)| {
                    !self.sources.hold(event)
                        && event.protected().is_none()
                        && event.unread == unread
                })
                .min_by_key(|(_, event)| (event.at, event.id))
                .map(|(index, _)| index)
        };
        let Some(index) = oldest(false).or_else(|| oldest(true)) else {
            return false;
        };
        self.list.remove(index);
        true
    }

    /// Keeps the history to [`HISTORY`].
    fn trim(&mut self) {
        while self.history() > HISTORY && self.evict() {}
    }

    /// Puts `event` in the list, making room in the history if the list is full: live events
    /// are at most [`LIVE`], so a full list holds history to drop.
    fn push(&mut self, event: Event) -> bool {
        if self.list.is_full() {
            self.evict();
        }
        self.list.push(event).is_ok()
    }

    /// Adds an event, and returns its id, or `None` should the list have no room, which its
    /// bounds rule out.
    fn add(&mut self, kind: Kind, unread: bool, now: Micros) -> Option<Id> {
        let id = self.next;
        let pushed = self.push(Event {
            id,
            kind,
            at: now,
            unread,
        });
        debug_assert!(pushed, "the list had room");
        pushed.then(|| {
            self.next += 1;
            id
        })
    }

    /// Changes event `id` to `kind`. A material change marks it unread and moves its time to
    /// now; returns `id` then, for the user to be told.
    fn change(&mut self, id: Id, kind: Kind, material: bool, now: Micros) -> Option<Id> {
        let event = self.get_mut(id)?;
        if event.kind == kind {
            return None;
        }
        event.kind = kind;
        if material {
            event.unread = true;
            event.at = now;
        }
        material.then_some(id)
    }

    /// Follows the receiver's health. Returns an event the user should be told of: a new
    /// incident, its escalation to a fault, or the receiver answering again. Later resets update
    /// the incident quietly.
    pub fn gnss(&mut self, health: &GnssHealth, now: Micros) -> Option<Id> {
        let told = self.follow_gnss(health, now);
        self.trim();
        told
    }

    fn follow_gnss(&mut self, health: &GnssHealth, now: Micros) -> Option<Id> {
        let current = self
            .sources
            .incident
            .and_then(|id| Some((id, self.get(id)?.kind)));
        if !health.recovering {
            let (id, _) = current?;
            self.sources.incident = None;
            return self.change(id, Kind::Gnss(Gnss::Responding), true, now);
        }
        let phase = if health.failed_resets >= FAULT_RESETS {
            Gnss::Fault {
                failed: health.failed_resets,
            }
        } else {
            Gnss::Recovering {
                attempt: health.failed_resets + 1,
            }
        };
        match current {
            None => {
                let id = self.add(Kind::Gnss(phase), true, now)?;
                self.sources.incident = Some(id);
                Some(id)
            }
            Some((id, Kind::Gnss(was))) => {
                let escalates =
                    matches!(was, Gnss::Recovering { .. }) && matches!(phase, Gnss::Fault { .. });
                self.change(id, Kind::Gnss(phase), escalates, now)
            }
            Some(_) => None,
        }
    }
}

impl Events {
    /// Follows the messages of member `own`'s device: a conversation with unread messages has an
    /// event, told of when a newer message arrives than it was last, unread until the user opens
    /// it or reads its messages, and history once they are read. A message taken back after a
    /// restart is never news. Returns an event the user should be told of.
    pub fn messages(&mut self, messages: &MessagesView, own: u8, now: Micros) -> Option<Id> {
        // The conversations with unread messages, newest first, as many as there is room for.
        let mut threads: Vec<Thread, CONVERSATIONS> = Vec::new();
        for message in messages.iter().rev().filter(|message| message.unread) {
            let thread = message.thread(own);
            if !threads.contains(&thread) && threads.push(thread).is_err() {
                break;
            }
        }
        let mut i = 0;
        while let Some(held) = self.conversations.get(i).copied() {
            if messages
                .thread(held.thread, own)
                .any(|message| message.unread)
            {
                i += 1;
                continue;
            }
            self.conversations.remove(i);
            let pushed = self.push(Event {
                kind: Kind::Messages {
                    thread: held.thread,
                    unread: 0,
                    newest: held.newest,
                    from: held.from,
                },
                unread: false,
                ..held.event()
            });
            debug_assert!(pushed, "the list had room");
        }
        let mut told = None;
        for thread in threads {
            let unread = messages
                .thread(thread, own)
                .filter(|message| message.unread)
                .count() as u16;
            let newest = messages
                .thread(thread, own)
                .rfind(|message| message.unread && !message.recovered)
                .map(|message| (message.id, message.from));
            if let Some(held) = self
                .conversations
                .iter_mut()
                .find(|held| held.thread == thread)
            {
                held.unread = unread;
                if let Some((newest, from)) = newest
                    && newest != held.newest
                {
                    (held.newest, held.from, held.at, held.fresh) = (newest, from, now, true);
                    told = Some(held.id);
                }
                continue;
            }
            // Only messages taken back after a restart make no event. Past the bound one waits
            // for room, untold; the inbox and the unread arc still have it.
            let Some((newest, from)) = newest else {
                continue;
            };
            if self.conversations.is_full() {
                continue;
            }
            // One read before keeps its event, out of the history again.
            let id = match self.list.iter().position(
                |event| matches!(event.kind, Kind::Messages { thread: held, .. } if held == thread),
            ) {
                Some(index) => self.list.remove(index).id,
                None => {
                    self.next += 1;
                    self.next - 1
                }
            };
            _ = self.conversations.push(Conversation {
                id,
                thread,
                unread,
                newest,
                from,
                at: now,
                fresh: true,
            });
            told = Some(id);
        }
        self.trim();
        told
    }
}

impl Events {
    /// Follows the group's removals as member `own`'s device knows them, `None` in no group.
    /// Each request has an event by its new key: another member's is told of when it comes and
    /// when this device switches to it, and a rival whenever one is learned. This device's own
    /// begins read, as does one first seen switched, which a restart brought back. Being
    /// removed is an event of its own. Returns an event the user should be told of.
    pub fn removals(
        &mut self,
        removals: &RemovalsView,
        own: Option<u8>,
        now: Micros,
    ) -> Option<Id> {
        let mut told = None;
        let shown = [removals.current, removals.rival];
        // Live while the mesh shows them, so that none of them is taken for history and dropped
        // only to come back as news.
        self.sources.shown = shown.map(|removal| removal.map(|removal| removal.key));
        self.sources.removed = removals.removed_by.map(|(_, _, at)| at);
        for removal in shown.into_iter().flatten() {
            let theirs = Some(removal.remover) != own;
            match self
                .removal(removal.key)
                .map(|event| (event.id, event.kind))
            {
                None => {
                    let news = theirs && !matches!(removal.stage, RemovalStage::Switched { .. });
                    if let Some(id) = self.add(Kind::Removal(removal), news, now)
                        && news
                    {
                        told = Some(id);
                    }
                }
                Some((id, was)) => {
                    let switched = matches!(removal.stage, RemovalStage::Switched { .. })
                        && !matches!(
                            was,
                            Kind::Removal(RemovalView {
                                stage: RemovalStage::Switched { .. },
                                ..
                            })
                        );
                    told = self
                        .change(id, Kind::Removal(removal), theirs && switched, now)
                        .or(told);
                }
            }
        }
        self.settle_removals(&shown, own.is_some());
        let ids = shown.map(|removal| Some(self.removal(removal?.key)?.id));
        self.rivals = match ids {
            [Some(winner), Some(rival)] => Some([winner, rival]),
            _ => None,
        };
        if let Some((by, name, at)) = removals.removed_by {
            let known = self
                .list
                .iter()
                .any(|event| matches!(event.kind, Kind::Removed { at: held, .. } if held == at));
            if !known {
                told = self.add(Kind::Removed { by, name, at }, true, now).or(told);
            }
        }
        self.trim();
        told
    }

    /// Settles the removals the mesh no longer shows. One not yet switched to is moot. One
    /// switched to can no longer be declined: a later removal replaced it, or with no group
    /// there is nothing to go back to.
    fn settle_removals(&mut self, shown: &[Option<RemovalView>; 2], grouped: bool) {
        let gone = |key: [u8; 8]| !shown.iter().flatten().any(|removal| removal.key == key);
        self.list.retain(|event| match event.kind {
            Kind::Removal(removal) if gone(removal.key) => match removal.stage {
                RemovalStage::Pending { .. } => false,
                RemovalStage::Switched {
                    decline: Decline::Until(_),
                    ..
                } => grouped,
                _ => true,
            },
            _ => true,
        });
        for event in &mut self.list {
            if let Kind::Removal(removal) = &mut event.kind
                && gone(removal.key)
                && let RemovalStage::Switched { decline, .. } = &mut removal.stage
                && matches!(decline, Decline::Until(_))
            {
                *decline = Decline::Later;
            }
        }
    }

    /// The event of the removal with the new key `key`.
    #[must_use]
    pub fn removal(&self, key: [u8; 8]) -> Option<&Event> {
        self.list
            .iter()
            .find(|event| matches!(event.kind, Kind::Removal(held) if held.key == key))
    }

    /// The two competing removals' events, the winner first, if `id` is one of them, and which.
    #[must_use]
    pub fn rivals_of(&self, id: Id) -> Option<([Id; 2], usize)> {
        let ids = self.rivals?;
        Some((ids, ids.iter().position(|&held| held == id)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::group::view::MessageView;

    fn health(recovering: bool, failed_resets: u8) -> GnssHealth {
        GnssHealth {
            recovering,
            failed_resets,
            last_response: None,
            last_fix: None,
        }
    }

    #[test]
    fn one_gnss_incident_runs_from_recovering_to_responding() {
        let mut events = Events::default();
        assert_eq!(events.gnss(&health(false, 0), 0), None);
        let id = events
            .gnss(&health(true, 0), 10)
            .expect("a new incident is told");
        assert_eq!(
            events.get(id).unwrap().kind,
            Kind::Gnss(Gnss::Recovering { attempt: 1 })
        );
        events.read(id);
        // Later attempts update it quietly.
        assert_eq!(events.gnss(&health(true, 1), 20), None);
        assert_eq!(events.gnss(&health(true, 2), 30), None);
        let event = events.get(id).unwrap();
        assert_eq!(event.kind, Kind::Gnss(Gnss::Recovering { attempt: 3 }));
        assert!(!event.unread);
        assert_eq!(event.at, 10);
        // The third failure escalates it, unread again.
        assert_eq!(events.gnss(&health(true, 3), 40), Some(id));
        assert!(events.get(id).unwrap().unread);
        assert_eq!(events.gnss(&health(true, 4), 50), None);
        assert_eq!(events.gnss(&health(false, 0), 60), Some(id));
        assert_eq!(events.get(id).unwrap().kind, Kind::Gnss(Gnss::Responding));
        assert_eq!(events.len(), 1);
        // The next stop is a new incident.
        let next = events.gnss(&health(true, 0), 70).unwrap();
        assert_ne!(next, id);
        assert_eq!(events.len(), 2);
    }

    /// A GNSS incident that has settled, made unread at `at`.
    fn settled(events: &mut Events, at: Micros) -> Id {
        let id = events.gnss(&health(true, 0), at).unwrap();
        events.gnss(&health(false, 0), at + 1);
        id
    }

    #[test]
    fn going_on_events_cannot_be_dismissed_or_cleared() {
        let mut events = Events::default();
        let fault = events.gnss(&health(true, 3), 0).unwrap();
        let running = events
            .removals(&shown(Some(removal(1, 2, PENDING)), None), Some(0), 1)
            .unwrap();
        assert_eq!(
            events.dismiss(fault),
            Err(Kept::Protected(Protected::Unresolved))
        );
        assert_eq!(
            events.dismiss(running),
            Err(Kept::Protected(Protected::Unfinished))
        );
        events.mark_all_read();
        assert_eq!(events.unread(), 0);
        assert!(!events.can_clear_read());
        events.clear_read();
        assert_eq!(events.len(), 2);
        assert_eq!(events.dismiss(99), Err(Kept::Gone));
    }

    #[test]
    fn clearing_keeps_unread_events() {
        let mut events = Events::default();
        let first = settled(&mut events, 0);
        let second = settled(&mut events, 2);
        events.read(first);
        events.clear_read();
        assert!(events.get(first).is_none());
        assert!(events.get(second).is_some());
    }

    #[test]
    fn running_operations_come_first_then_the_latest_change() {
        let mut events = Events::default();
        let fault = events.gnss(&health(true, 3), 50).unwrap();
        let removal_at = |stage| shown(Some(removal(1, 2, stage)), None);
        let pending = events.removals(&removal_at(PENDING), Some(0), 10).unwrap();
        let order: alloc::vec::Vec<_> = events.ordered().map(|event| event.id).collect();
        assert_eq!(order, [pending, fault]);
        let switched = RemovalStage::Switched {
            at: 1_000,
            decline: Decline::Until(2_000),
        };
        events.removals(&removal_at(switched), Some(0), 1_000);
        let order: alloc::vec::Vec<_> = events.ordered().map(|event| event.id).collect();
        assert_eq!(order, [pending, fault], "switched, it is the latest change");
    }

    #[test]
    fn a_full_history_drops_its_oldest_settled_event() {
        let mut events = Events::default();
        let fault = events.gnss(&health(true, 3), 0).unwrap();
        for at in 1..=HISTORY as i64 + 4 {
            let removed = RemovalsView {
                removed_by: Some((2, Name::from_mac(&[0, 0, 0, 0, 0, 2]), at)),
                ..RemovalsView::default()
            };
            events.removals(&removed, None, at as Micros * 10);
        }
        // The fault and the latest notice are live; the rest are history.
        assert_eq!(events.history(), HISTORY);
        assert_eq!(events.len(), HISTORY + 2);
        assert!(events.get(fault).is_some(), "the fault is still going on");
        let kept: alloc::vec::Vec<i64> = events
            .ordered()
            .filter_map(|event| match event.kind {
                Kind::Removed { at, .. } => Some(at),
                _ => None,
            })
            .collect();
        assert_eq!(kept.len(), HISTORY + 1);
        assert_eq!(
            kept,
            (4..=20).rev().collect::<alloc::vec::Vec<_>>(),
            "the oldest went"
        );
    }

    /// A store of messages, `unread` to member 0 from each member in `from`, privately.
    fn store(from: core::ops::Range<u8>, unread: bool) -> alloc::boxed::Box<MessagesView> {
        let mut store = MessagesView::boxed();
        for (id, from) in (1..).zip(from) {
            let mut message = MessageView::new(id, i64::from(id), from, Some(0), b"Hi");
            message.set_peer([from; 8], Name::from_mac(&[0, 0, 0, 0, 0, from]));
            message.unread = unread;
            store.push(message);
        }
        store
    }

    /// The hand-off's capacity check: a full history, a fault, a removal and its rival under
    /// way, this device removed, and a conversation with unread messages from every member, all
    /// at once. Each stays in the drawer, once.
    #[test]
    fn live_events_and_unread_conversations_stay_beyond_a_full_history() {
        let mut events = Events::default();
        for at in 0..HISTORY as Micros + 3 {
            events.gnss(&health(true, 0), at * 10);
            events.gnss(&health(false, 0), at * 10 + 1);
        }
        assert_eq!(events.history(), HISTORY);
        let fault = events.gnss(&health(true, 3), 1_000).unwrap();
        let views = RemovalsView {
            current: Some(removal(1, 2, PENDING)),
            rival: Some(removal(2, 3, PENDING)),
            removed_by: Some((4, Name::from_mac(&[0, 0, 0, 0, 0, 4]), 1_000)),
        };
        events.removals(&views, Some(0), 1_001);
        let store = store(1..IDS, true);
        events.messages(&store, 0, 1_002);
        assert_eq!(events.history(), HISTORY);
        assert_eq!(events.len(), HISTORY + 4 + usize::from(IDS) - 1);
        assert!(events.get(fault).is_some());
        assert!(events.removal([1; 8]).is_some() && events.removal([2; 8]).is_some());
        let conversations = events
            .ordered()
            .filter(|event| matches!(event.kind, Kind::Messages { unread: 1, .. }))
            .count();
        assert_eq!(conversations, usize::from(IDS) - 1);
        // Nothing comes again, or is told again, on the next pass.
        assert_eq!(events.removals(&views, Some(0), 1_003), None);
        assert_eq!(events.messages(&store, 0, 1_004), None);
        assert_eq!(events.len(), HISTORY + 4 + usize::from(IDS) - 1);
        // Neither clearing nor dismissing drops what is live.
        events.mark_all_read();
        events.clear_read();
        assert_eq!(events.history(), 0);
        assert_eq!(events.len(), 4 + usize::from(IDS) - 1);
        let removed = events
            .ordered()
            .find(|event| matches!(event.kind, Kind::Removed { .. }))
            .unwrap();
        assert_eq!(events.dismiss(removed.id), Err(Kept::Live));
    }

    #[test]
    fn a_conversation_read_settles_into_the_history_and_comes_back_when_written_to() {
        let mut events = Events::default();
        let mut store = store(1..2, true);
        let id = events.messages(&store, 0, 10).expect("told");
        assert_eq!(
            events.dismiss(id),
            Err(Kept::Live),
            "it has unread messages"
        );
        store.get_mut(1).unwrap().unread = false;
        assert_eq!(events.messages(&store, 0, 20), None);
        assert_eq!(events.history(), 1);
        let settled = events.get(id).unwrap();
        assert!(!settled.unread);
        assert!(matches!(settled.kind, Kind::Messages { unread: 0, .. }));
        let mut again = MessageView::new(2, 30, 1, Some(0), b"Again");
        again.set_peer([1; 8], Name::from_mac(&[0, 0, 0, 0, 0, 1]));
        again.unread = true;
        store.push(again);
        assert_eq!(
            events.messages(&store, 0, 30),
            Some(id),
            "the same event, told"
        );
        assert_eq!((events.len(), events.history()), (1, 0));
    }

    fn removal(key: u8, remover: u8, stage: RemovalStage) -> RemovalView {
        RemovalView {
            key: [key; 8],
            remover,
            remover_name: Name::from_mac(&[0, 0, 0, 0, 0, remover]),
            removed: 3,
            removed_name: Name::from_mac(&[0, 0, 0, 0, 0, 3]),
            device: [3; 8],
            stage,
        }
    }

    const PENDING: RemovalStage = RemovalStage::Pending {
        since: 0,
        switch: Some(1_000),
    };

    fn shown(current: Option<RemovalView>, rival: Option<RemovalView>) -> RemovalsView {
        RemovalsView {
            current,
            rival,
            removed_by: None,
        }
    }

    #[test]
    fn anothers_request_is_told_and_this_devices_own_is_not() {
        let mut events = Events::default();
        let theirs = shown(Some(removal(1, 2, PENDING)), None);
        let id = events.removals(&theirs, Some(0), 10).expect("told");
        assert!(events.get(id).unwrap().unread);
        assert!(events.get(id).unwrap().ongoing());
        // Nothing new: told once.
        assert_eq!(events.removals(&theirs, Some(0), 20), None);
        let mut events = Events::default();
        let own = shown(Some(removal(1, 0, PENDING)), None);
        assert_eq!(events.removals(&own, Some(0), 10), None);
        assert!(!events.ordered().next().unwrap().unread);
    }

    #[test]
    fn its_switch_is_told_and_its_day_to_decline_protects_it() {
        let mut events = Events::default();
        let id = events
            .removals(&shown(Some(removal(1, 2, PENDING)), None), Some(0), 10)
            .unwrap();
        events.read(id);
        let switched = RemovalStage::Switched {
            at: 1_000,
            decline: Decline::Until(2_000),
        };
        let told = events.removals(&shown(Some(removal(1, 2, switched)), None), Some(0), 1_000);
        assert_eq!(told, Some(id));
        assert_eq!(
            events.get(id).unwrap().protected(),
            Some(Protected::Declinable)
        );
        let expired = RemovalStage::Switched {
            at: 1_000,
            decline: Decline::Expired,
        };
        assert_eq!(
            events.removals(&shown(Some(removal(1, 2, expired)), None), Some(0), 3_000),
            None
        );
        // Shown still, it would only come back.
        assert_eq!(events.dismiss(id), Err(Kept::Live));
        events.removals(&shown(None, None), Some(0), 4_000);
        assert_eq!(events.dismiss(id), Ok(()));
    }

    #[test]
    fn a_switch_restored_after_a_restart_is_not_news() {
        let mut events = Events::default();
        let switched = RemovalStage::Switched {
            at: 1_000,
            decline: Decline::Until(2_000),
        };
        let views = shown(Some(removal(1, 2, switched)), None);
        assert_eq!(events.removals(&views, Some(0), 10), None);
        let event = events.ordered().next().unwrap();
        assert!(!event.unread);
        assert_eq!(event.protected(), Some(Protected::Declinable));
    }

    #[test]
    fn a_removal_the_mesh_stops_showing_settles() {
        let switched = RemovalStage::Switched {
            at: 1_000,
            decline: Decline::Until(2_000),
        };
        let mut events = Events::default();
        events.removals(&shown(Some(removal(1, 2, switched)), None), Some(0), 10);
        // A later removal replaces it: it can no longer be declined, and stays listed.
        events.removals(&shown(Some(removal(2, 3, PENDING)), None), Some(0), 20);
        let first = events.removal([1; 8]).unwrap();
        assert!(matches!(
            first.kind,
            Kind::Removal(RemovalView {
                stage: RemovalStage::Switched {
                    decline: Decline::Later,
                    ..
                },
                ..
            })
        ));
        assert_eq!(first.protected(), None);
        // Out of the group, the request under way is moot and goes.
        events.removals(&RemovalsView::default(), None, 30);
        assert!(events.removal([2; 8]).is_none());
        assert!(events.removal([1; 8]).is_some());
    }

    #[test]
    fn two_competing_removals_are_paired_winner_first() {
        let mut events = Events::default();
        events.removals(&shown(Some(removal(1, 2, PENDING)), None), Some(0), 10);
        let views = shown(
            Some(removal(2, 3, PENDING)),
            Some(removal(1, 2, RemovalStage::Lost)),
        );
        let told = events.removals(&views, Some(0), 20).unwrap();
        let lost = events.removal([1; 8]).unwrap().id;
        assert_eq!(events.rivals_of(told), Some(([told, lost], 0)));
        assert_eq!(events.rivals_of(lost), Some(([told, lost], 1)));
        events.removals(&shown(Some(removal(2, 3, PENDING)), None), Some(0), 30);
        assert_eq!(events.rivals_of(told), None);
    }

    #[test]
    fn being_removed_is_told_once() {
        let mut events = Events::default();
        let views = RemovalsView {
            removed_by: Some((2, Name::from_mac(&[0, 0, 0, 0, 0, 2]), 50)),
            ..RemovalsView::default()
        };
        let id = events.removals(&views, Some(0), 60).unwrap();
        assert!(matches!(
            events.get(id).unwrap().kind,
            Kind::Removed { by: 2, .. }
        ));
        assert_eq!(events.removals(&views, Some(0), 70), None);
        assert_eq!(events.len(), 1);
    }
}
