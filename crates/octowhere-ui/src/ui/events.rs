//! The runtime events the drawer lists, from the 2026-10-04 hand-off: what happened, whether it
//! is still going on, and whether the user has opened it. An event keeps one identity through its
//! life: a GNSS incident from the receiver stopping to its answering again, a refresh from its
//! start to its result, a conversation's messages while it has unread ones. Events live in RAM,
//! and a restart loses them.

use heapless::Vec;

use super::{
    gesture::Micros,
    group::view::{MessagesView, RefreshPhase, RefreshView, Thread},
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
    Refresh(RefreshView),
    /// A conversation's new messages: how many are unread, the newest told of, and its sender.
    Messages {
        thread: Thread,
        unread: u16,
        newest: u32,
        from: u8,
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
            Kind::Refresh(refresh) if refresh.is_listening() => Some(Protected::Unfinished),
            Kind::Gnss(Gnss::Responding) | Kind::Refresh(_) | Kind::Messages { .. } => None,
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
    /// The latest refresh an event was made for, so that it is not made again.
    refresh_session: u32,
    /// Counts every change, so that a screen can tell when to look again.
    version: u32,
}

impl Events {
    #[must_use]
    pub fn version(&self) -> u32 {
        self.version
    }

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
            self.version += 1;
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
            self.version += 1;
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
            self.version += 1;
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
        self.version += 1;
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
        self.version += 1;
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
        self.version += 1;
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

    /// Follows the mesh's refresh. A refresh started here begins read; its end is told of. A
    /// refresh first seen after it ended makes no event.
    pub fn refresh(&mut self, refresh: Option<&RefreshView>, now: Micros) -> Option<Id> {
        let refresh = refresh?;
        let held = self.list.iter().find_map(|event| match event.kind {
            Kind::Refresh(held) if held.session == refresh.session => Some((event.id, held)),
            _ => None,
        });
        match held {
            None if refresh.is_listening() && refresh.session > self.refresh_session => {
                self.refresh_session = refresh.session;
                self.add(Kind::Refresh(*refresh), false, now);
                None
            }
            None => None,
            Some((id, held)) => {
                let ends = held.is_listening() && !refresh.is_listening();
                self.change(id, Kind::Refresh(*refresh), ends, now)
            }
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
                        self.version += 1;
                    }
                }
            }
        }
        told
    }
}

/// How long a refresh has run, out of its whole time, on the stage's clock.
#[must_use]
pub fn elapsed(refresh: &RefreshView, now: Micros) -> Option<(Micros, Micros)> {
    let RefreshPhase::Listening { until } = refresh.phase else {
        return None;
    };
    let whole = octowhere_mesh::clock::SWEEP_US as Micros;
    let left = (until.max(0) as Micros).saturating_sub(now).min(whole);
    Some((whole - left, whole))
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

    fn refresh(session: u32, phase: RefreshPhase, heard: u32) -> RefreshView {
        RefreshView {
            session,
            phase,
            heard,
            learned: 0,
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

    #[test]
    fn a_refresh_keeps_one_event_from_start_to_result() {
        let mut events = Events::default();
        let listening = RefreshPhase::Listening { until: 1_000 };
        assert_eq!(events.refresh(Some(&refresh(1, listening, 0)), 0), None);
        let id = events.ordered().next().unwrap().id;
        assert!(!events.get(id).unwrap().unread);
        assert!(events.get(id).unwrap().ongoing());
        assert_eq!(events.refresh(Some(&refresh(1, listening, 0b10)), 10), None);
        let ended = RefreshPhase::Ended { at: 1_000 };
        assert_eq!(
            events.refresh(Some(&refresh(1, ended, 0b10)), 1_000),
            Some(id)
        );
        let event = events.get(id).unwrap();
        assert!(event.unread);
        assert!(!event.ongoing());
        assert_eq!(events.len(), 1);
    }

    #[test]
    fn a_dismissed_or_cleared_refresh_does_not_come_back() {
        let mut events = Events::default();
        let listening = RefreshPhase::Listening { until: 1_000 };
        let ended = RefreshPhase::Ended { at: 1_000 };
        events.refresh(Some(&refresh(1, listening, 0)), 0);
        let id = events.refresh(Some(&refresh(1, ended, 0)), 1_000).unwrap();
        assert_eq!(events.dismiss(id), Ok(()));
        events.refresh(Some(&refresh(1, ended, 0)), 2_000);
        assert!(events.is_empty());
        // A refresh first seen ended makes nothing either.
        events.refresh(Some(&refresh(2, ended, 0)), 3_000);
        assert!(events.is_empty());
    }

    #[test]
    fn going_on_events_cannot_be_dismissed_or_cleared() {
        let mut events = Events::default();
        let fault = events.gnss(&health(true, 3), 0).unwrap();
        events.refresh(
            Some(&refresh(1, RefreshPhase::Listening { until: 100 }, 0)),
            1,
        );
        let running = events.ordered().next().unwrap().id;
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
        let listening = RefreshPhase::Listening { until: 1_000 };
        let ended = RefreshPhase::Ended { at: 1_000 };
        events.refresh(Some(&refresh(1, listening, 0)), 0);
        let first = events.refresh(Some(&refresh(1, ended, 0)), 1).unwrap();
        events.refresh(Some(&refresh(2, listening, 0)), 2);
        let second = events.refresh(Some(&refresh(2, ended, 0)), 3).unwrap();
        events.read(first);
        events.clear_read();
        assert!(events.get(first).is_none());
        assert!(events.get(second).is_some());
    }

    #[test]
    fn running_operations_come_first_then_the_latest_change() {
        let mut events = Events::default();
        let fault = events.gnss(&health(true, 3), 50).unwrap();
        events.refresh(
            Some(&refresh(1, RefreshPhase::Listening { until: 1_000 }, 0)),
            10,
        );
        let order: alloc::vec::Vec<_> = events.ordered().map(|event| event.id).collect();
        assert_eq!(order[1], fault);
        let ended = events
            .refresh(
                Some(&refresh(1, RefreshPhase::Ended { at: 1_000 }, 0)),
                1_000,
            )
            .unwrap();
        let order: alloc::vec::Vec<_> = events.ordered().map(|event| event.id).collect();
        assert_eq!(order, [ended, fault]);
    }

    #[test]
    fn a_full_list_drops_its_oldest_settled_event() {
        let mut events = Events::default();
        let fault = events.gnss(&health(true, 3), 0).unwrap();
        for session in 1..=CAPACITY as u32 {
            let at = Micros::from(session) * 10;
            events.refresh(
                Some(&refresh(
                    session,
                    RefreshPhase::Listening { until: 1_000 },
                    0,
                )),
                at,
            );
            events.refresh(
                Some(&refresh(session, RefreshPhase::Ended { at: 0 }, 0)),
                at + 1,
            );
        }
        assert_eq!(events.len(), CAPACITY);
        assert!(events.get(fault).is_some(), "the fault is still going on");
    }

    #[test]
    fn elapsed_counts_up_to_the_refresh_window() {
        let whole = octowhere_mesh::clock::SWEEP_US as Micros;
        let view = refresh(
            1,
            RefreshPhase::Listening {
                until: (whole + 1_000) as i64,
            },
            0,
        );
        assert_eq!(elapsed(&view, 1_000), Some((0, whole)));
        assert_eq!(elapsed(&view, 28_000_000), Some((27_999_000, whole)));
        assert_eq!(elapsed(&view, whole * 2), Some((whole, whole)));
    }
}
