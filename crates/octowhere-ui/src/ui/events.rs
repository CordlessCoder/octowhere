//! The runtime events the drawer lists, from the 2026-10-04 hand-off: what happened, whether it
//! is still going on, and whether the user has opened it. An event keeps one identity through its
//! life: a GNSS incident from the receiver stopping to its answering again, a conversation's
//! messages while it has unread ones, a removal from its request to its end. Events live in RAM,
//! and a restart loses them.

use heapless::Vec;

use super::{
    gesture::Micros,
    group::view::{Decline, MessagesView, Name, RemovalStage, RemovalView, RemovalsView, Thread},
    screens::GnssHealth,
};

/// The most events held. A new event takes the place of the oldest settled one when the list is
/// full, a read one first; an event still going on is never dropped.
pub const CAPACITY: usize = 16;

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
    /// No event has the id, or no longer has it.
    Gone,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Events {
    /// In the order they were made.
    list: Vec<Event, CAPACITY>,
    next: Id,
    /// The GNSS incident under way, until the receiver answers again.
    incident: Option<Id>,
    /// The events of two removals that compete, the winner first, while the mesh shows both.
    rivals: Option<[Id; 2]>,
}

impl Events {
    #[must_use]
    pub fn get(&self, id: Id) -> Option<&Event> {
        self.list.iter().find(|event| event.id == id)
    }

    /// The events in the drawer's order: operations still running first, then the latest
    /// change first.
    pub fn ordered(&self) -> impl Iterator<Item = &Event> {
        let mut order: Vec<&Event, CAPACITY> = self.list.iter().collect();
        order.sort_unstable_by_key(|event| {
            (
                core::cmp::Reverse(event.ongoing()),
                core::cmp::Reverse(event.at),
                core::cmp::Reverse(event.id),
            )
        });
        order.into_iter()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.list.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.list.is_empty()
    }

    #[must_use]
    pub fn unread(&self) -> usize {
        self.list.iter().filter(|event| event.unread).count()
    }

    #[must_use]
    pub fn ongoing(&self) -> usize {
        self.list.iter().filter(|event| event.ongoing()).count()
    }

    /// Marks an event read, as opening it does.
    pub fn read(&mut self, id: Id) {
        if let Some(event) = self.list.iter_mut().find(|event| event.id == id)
            && event.unread
        {
            event.unread = false;
        }
    }

    /// Whether [`mark_all_read`](Self::mark_all_read) would change anything.
    #[must_use]
    pub fn can_mark_all_read(&self) -> bool {
        self.unread() > 0
    }

    pub fn mark_all_read(&mut self) {
        if self.can_mark_all_read() {
            self.list.iter_mut().for_each(|event| event.unread = false);
        }
    }

    fn clearable(event: &Event) -> bool {
        !event.unread && event.protected().is_none()
    }

    /// Whether [`clear_read`](Self::clear_read) would remove anything.
    #[must_use]
    pub fn can_clear_read(&self) -> bool {
        self.list.iter().any(Self::clearable)
    }

    /// Removes the read events that have settled. Unread ones, and any still going on, stay.
    pub fn clear_read(&mut self) {
        if self.can_clear_read() {
            self.list.retain(|event| !Self::clearable(event));
        }
    }

    /// Removes one settled event, read or not, as its detail's DISMISS does. Its eligibility is
    /// checked now, by its id.
    pub fn dismiss(&mut self, id: Id) -> Result<(), Kept> {
        let index = self
            .list
            .iter()
            .position(|event| event.id == id)
            .ok_or(Kept::Gone)?;
        if let Some(protected) = self.list[index].protected() {
            return Err(Kept::Protected(protected));
        }
        self.list.remove(index);
        Ok(())
    }

    fn get_mut(&mut self, id: Id) -> Option<&mut Event> {
        self.list.iter_mut().find(|event| event.id == id)
    }

    /// Adds an event, making room if the list is full, and returns its id, or `None` when every
    /// event held is still going on.
    fn add(&mut self, kind: Kind, unread: bool, now: Micros) -> Option<Id> {
        if self.list.is_full() {
            let oldest = |unread: bool| {
                self.list
                    .iter()
                    .enumerate()
                    .filter(|(_, event)| event.protected().is_none() && event.unread == unread)
                    .min_by_key(|(_, event)| (event.at, event.id))
                    .map(|(index, _)| index)
            };
            let index = oldest(false).or_else(|| oldest(true))?;
            self.list.remove(index);
        }
        let id = self.next;
        self.next += 1;
        let pushed = self.list.push(Event {
            id,
            kind,
            at: now,
            unread,
        });
        debug_assert!(pushed.is_ok(), "the list had room");
        Some(id)
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
        let current = self.incident.and_then(|id| Some((id, self.get(id)?.kind)));
        if !health.recovering {
            let (id, _) = current?;
            self.incident = None;
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
                self.incident = Some(id);
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
    /// it or reads its messages. A message taken back after a restart is never news. Returns an
    /// event the user should be told of.
    pub fn messages(&mut self, messages: &MessagesView, own: u8, now: Micros) -> Option<Id> {
        let mut told = None;
        let mut threads: Vec<Thread, 33> = Vec::new();
        for message in messages.iter() {
            let thread = message.thread(own);
            if !threads.contains(&thread) {
                _ = threads.push(thread);
            }
        }
        for thread in threads {
            let unread = messages
                .thread(thread, own)
                .filter(|message| message.unread)
                .count() as u16;
            let newest = messages
                .thread(thread, own)
                .rfind(|message| message.unread && !message.recovered)
                .map(|message| (message.id, message.from));
            let held = self.list.iter().find_map(|event| match event.kind {
                Kind::Messages {
                    thread: held,
                    newest,
                    ..
                } if held == thread => Some((event.id, newest)),
                _ => None,
            });
            match (held, newest) {
                (None, Some((newest, from))) => {
                    let kind = Kind::Messages {
                        thread,
                        unread,
                        newest,
                        from,
                    };
                    told = self.add(kind, true, now).or(told);
                }
                (None, None) => {}
                (Some((id, was)), Some((newest, from))) => {
                    let kind = Kind::Messages {
                        thread,
                        unread,
                        newest,
                        from,
                    };
                    told = self.change(id, kind, newest != was, now).or(told);
                }
                (Some((id, _)), None) => {
                    if let Some(event) = self.get_mut(id)
                        && let Kind::Messages { unread: held, .. } = &mut event.kind
                        && (*held != unread || event.unread)
                    {
                        *held = unread;
                        event.unread &= unread > 0;
                    }
                }
            }
        }
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
    fn a_full_list_drops_its_oldest_settled_event() {
        let mut events = Events::default();
        let fault = events.gnss(&health(true, 3), 0).unwrap();
        for at in 1..=CAPACITY as i64 {
            let removed = RemovalsView {
                removed_by: Some((2, Name::from_mac(&[0, 0, 0, 0, 0, 2]), at)),
                ..RemovalsView::default()
            };
            events.removals(&removed, None, at as Micros * 10);
        }
        assert_eq!(events.len(), CAPACITY);
        assert!(events.get(fault).is_some(), "the fault is still going on");
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
