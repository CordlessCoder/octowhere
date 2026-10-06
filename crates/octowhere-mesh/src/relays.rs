//! Which neighbours a node expects to pass on the messages it sent, and what it sends again when
//! it does not hear them do so (owner, 2026-10-05). By the cancel rule, a neighbour passes a
//! packet's new messages on when it has neighbours of its own that the packet's sender does not
//! report, so hearing it pass them on is the sign it heard them. Along a chain of relays a
//! message lost on one hop stops there, and is otherwise asked for only once its loss shows.

use crate::{IDS, Ids, messages::MessageId};

/// How long a node waits to hear a neighbour pass a message on before sending it again: past
/// the neighbour's rest after its own last packet, at the largest, and its backoff.
pub const RELAY_WAIT_US: i64 = 10_000_000;
/// How many times a node sends a message again for a neighbour it does not hear pass it on.
pub const RESENDS: u8 = 3;
/// Bench: how many times a message goes again, in place of [`RESENDS`].
pub static BENCH_RESENDS: core::sync::atomic::AtomicU8 = core::sync::atomic::AtomicU8::new(RESENDS);
/// The messages a node waits on at once. Past that the oldest is dropped, and is left to the
/// summaries.
const WAITING: usize = 16;
/// The recent messages a node remembers who carried, newest kept.
const CARRIED: usize = 32;

#[derive(Clone, Copy, Debug)]
struct Waiting {
    name: MessageId,
    /// The neighbours not yet heard passing it on.
    expected: Ids,
    /// The local time it goes again unless they are heard, or `None` once it is due, until
    /// [`Relays::sent`] says it has gone.
    until: Option<i64>,
    /// How many times it has gone again.
    resends: u8,
}

/// Bench: answers to a message carried again, delivered at no cost: (to, from, message).
pub struct BenchAnswers(core::cell::UnsafeCell<heapless::Vec<(u8, u8, MessageId), 4096>>);
// SAFETY: the bench runs every node on one thread.
unsafe impl Sync for BenchAnswers {}
pub static BENCH_ANSWERS: BenchAnswers =
    BenchAnswers(core::cell::UnsafeCell::new(heapless::Vec::new()));
/// Bench: whether a node answers a message carried again by a sender it heard carry it before.
pub static BENCH_ANSWER: core::sync::atomic::AtomicBool =
    core::sync::atomic::AtomicBool::new(false);
/// Bench: whether a message sent again names the neighbours still waited for, and a named
/// neighbour that holds it answers so, at no cost.
pub static BENCH_NAMED: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);
/// Bench: the neighbours each message sent again named: (sender, message, named).
pub struct BenchNamed(core::cell::UnsafeCell<heapless::Vec<(u8, MessageId, Ids), 4096>>);
// SAFETY: the bench runs every node on one thread.
unsafe impl Sync for BenchNamed {}
pub static BENCH_NAMED_SETS: BenchNamed =
    BenchNamed(core::cell::UnsafeCell::new(heapless::Vec::new()));

impl BenchNamed {
    #[allow(clippy::mut_from_ref)]
    fn get(&self) -> &mut heapless::Vec<(u8, MessageId, Ids), 4096> {
        // SAFETY: as above, and no reference outlives a call.
        unsafe { &mut *self.0.get() }
    }

    pub fn clear(&self) {
        self.get().clear();
    }

    /// Answers, for the node at id `own`, each message it held already that `sender` sent again
    /// naming it.
    pub fn answer(&self, own: u8, sender: u8, held: impl Iterator<Item = MessageId>) {
        if !BENCH_NAMED.load(core::sync::atomic::Ordering::Relaxed) {
            return;
        }
        for name in held {
            if self
                .get()
                .iter()
                .any(|&(from, at, named)| from == sender && at == name && named.contains(own))
            {
                let _ = BENCH_ANSWERS.get().push((sender, own, name));
            }
        }
    }
}

impl BenchAnswers {
    #[allow(clippy::mut_from_ref)]
    fn get(&self) -> &mut heapless::Vec<(u8, u8, MessageId), 4096> {
        // SAFETY: as above, and no reference outlives a call.
        unsafe { &mut *self.0.get() }
    }

    pub fn clear(&self) {
        self.get().clear();
    }
}

/// What a node expects its neighbours to pass on.
#[derive(Clone, Debug, Default)]
pub struct Relays {
    /// Each id's neighbours, as its last packet reported them.
    reported: [Ids; IDS as usize],
    /// The neighbours heard carrying each recent message, which hold it and so pass it on no
    /// more: the one this node had it from, first.
    carriers: heapless::Deque<(MessageId, Ids), CARRIED>,
    waiting: heapless::Vec<Waiting, WAITING>,
}

impl Relays {
    /// Takes a packet from `sender`, which reported hearing `neighbours` and carried the
    /// messages `carried`, as heard by the node at id `own`.
    pub fn heard(&mut self, own: u8, sender: u8, neighbours: Ids, carried: &[MessageId]) {
        if let Some(reported) = self.reported.get_mut(usize::from(sender)) {
            *reported = neighbours;
        }
        for &name in carried {
            match self.carriers.iter_mut().find(|(held, _)| *held == name) {
                Some((_, carriers)) => {
                    if carriers.contains(sender)
                        && BENCH_ANSWER.load(core::sync::atomic::Ordering::Relaxed)
                    {
                        let _ = BENCH_ANSWERS.get().push((sender, own, name));
                    }
                    carriers.insert(sender)
                }
                None => {
                    if self.carriers.is_full() {
                        self.carriers.pop_front();
                    }
                    let _ = self.carriers.push_back((name, Ids::of(sender)));
                }
            }
        }
        let reported = &self.reported;
        for waiting in &mut self.waiting {
            let (origin, seq) = waiting.name;
            // A later message from its origin shows the neighbour heard this one, or its gap
            // does, which the neighbour asks after itself.
            if carried
                .iter()
                .any(|&(from, at)| from == origin && at >= seq)
            {
                waiting.expected.remove(sender);
            }
            if !carried.contains(&waiting.name) {
                continue;
            }
            // Another relay reached what a neighbour was to reach.
            for id in waiting.expected.iter() {
                let beyond = beyond(reported, own, id).without(sender);
                if (beyond & !neighbours).is_empty() {
                    waiting.expected.remove(id);
                }
            }
        }
        self.waiting.retain(|waiting| !waiting.expected.is_empty());
    }

    /// Takes a packet the node at id `own` sent at local time `now`, reporting `neighbours` and
    /// carrying `carried`: each neighbour with a neighbour of its own outside them is to pass
    /// the messages on, unless it was heard carrying them already.
    pub fn sent(&mut self, own: u8, neighbours: Ids, carried: &[MessageId], now: i64) {
        let relaying = neighbours
            .iter()
            .filter(|&id| !(beyond(&self.reported, own, id) & !neighbours).is_empty())
            .collect::<Ids>();
        for &name in carried {
            let carriers = self
                .carriers
                .iter()
                .find(|(held, _)| *held == name)
                .map_or(Ids::EMPTY, |&(_, carriers)| carriers);
            let expected = relaying & !carriers;
            if let Some(waiting) = self.waiting.iter_mut().find(|held| held.name == name) {
                waiting.expected = expected;
                waiting.until = Some(now + RELAY_WAIT_US);
                continue;
            }
            if expected.is_empty() {
                continue;
            }
            if self.waiting.is_full() {
                self.waiting.remove(0);
            }
            let _ = self.waiting.push(Waiting {
                name,
                expected,
                until: Some(now + RELAY_WAIT_US),
                resends: 0,
            });
        }
        self.waiting.retain(|waiting| !waiting.expected.is_empty());
    }

    /// Bench: notes that the node at id `own` sends `name` again, naming the neighbours it still
    /// waits for.
    pub fn bench_named(&self, own: u8, name: MessageId) {
        if !BENCH_NAMED.load(core::sync::atomic::Ordering::Relaxed) {
            return;
        }
        let Some(waiting) = self.waiting.iter().find(|waiting| waiting.name == name) else {
            return;
        };
        let sets = BENCH_NAMED_SETS.get();
        sets.retain(|&(from, at, _)| !(from == own && at == name));
        if sets.is_full() {
            sets.remove(0);
        }
        let _ = sets.push((own, name, waiting.expected));
    }

    /// Bench: takes the answers addressed to the node at id `own`.
    pub fn bench_answered(&mut self, own: u8) {
        let answers = BENCH_ANSWERS.get();
        answers.retain(|&(to, from, name)| {
            if to != own {
                return true;
            }
            if let Some(waiting) = self.waiting.iter_mut().find(|waiting| waiting.name == name) {
                waiting.expected.remove(from);
            }
            false
        });
        self.waiting.retain(|waiting| !waiting.expected.is_empty());
    }

    /// The next message due to go again at local time `now`, for a neighbour not heard passing
    /// it on. It waits until it has gone, as [`Relays::sent`] takes; one sent [`RESENDS`] times
    /// is given up.
    pub fn due(&mut self, now: i64) -> Option<MessageId> {
        self.waiting.retain(|waiting| {
            waiting.until.is_none_or(|until| until > now)
                || waiting.resends < BENCH_RESENDS.load(core::sync::atomic::Ordering::Relaxed)
        });
        let waiting = self
            .waiting
            .iter_mut()
            .find(|waiting| waiting.until.is_some_and(|until| until <= now))?;
        waiting.resends += 1;
        waiting.until = None;
        Some(waiting.name)
    }

    /// Gives up the message `name`, which can no longer go.
    pub fn forget(&mut self, name: MessageId) {
        self.waiting.retain(|waiting| waiting.name != name);
    }

    /// When the next message is due to go again, on the local timer.
    #[must_use]
    pub fn next(&self) -> Option<i64> {
        self.waiting
            .iter()
            .filter_map(|waiting| waiting.until)
            .min()
    }
}

/// The neighbours `id` reported, but for itself and the node at `own`.
fn beyond(reported: &[Ids; IDS as usize], own: u8, id: u8) -> Ids {
    reported
        .get(usize::from(id))
        .copied()
        .unwrap_or(Ids::EMPTY)
        .without(own)
        .without(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: i64 = 1_000_000_000;

    /// Node 1 in a chain 0, 1, 2, 3.
    fn chain() -> Relays {
        let mut relays = Relays::default();
        relays.heard(1, 0, Ids::of(1), &[]);
        relays.heard(1, 2, Ids::of(1).with(3), &[]);
        relays
    }

    #[test]
    fn a_neighbour_with_neighbours_of_its_own_is_to_pass_a_message_on() {
        let mut relays = chain();
        relays.sent(1, Ids::of(0).with(2), &[(0, 7)], NOW);
        assert_eq!(relays.next(), Some(NOW + RELAY_WAIT_US), "2 reaches 3");
        relays.heard(1, 2, Ids::of(1).with(3), &[(0, 7)]);
        assert_eq!(relays.next(), None, "2 passed it on");
    }

    #[test]
    fn a_message_goes_again_until_heard_passed_on_or_given_up() {
        let mut relays = chain();
        relays.sent(1, Ids::of(0).with(2), &[(0, 7)], NOW);
        assert_eq!(relays.due(NOW + RELAY_WAIT_US - 1), None);
        let mut at = NOW + RELAY_WAIT_US;
        for _ in 0..RESENDS {
            assert_eq!(relays.due(at), Some((0, 7)));
            assert_eq!(relays.due(at), None, "once until it has gone");
            relays.sent(1, Ids::of(0).with(2), &[(0, 7)], at);
            at += RELAY_WAIT_US;
        }
        assert_eq!(relays.due(at), None, "given up");
        assert_eq!(relays.next(), None);
    }

    #[test]
    fn nothing_is_expected_where_every_neighbour_is_reached() {
        let mut relays = Relays::default();
        relays.heard(1, 0, Ids::of(1).with(2), &[]);
        relays.heard(1, 2, Ids::of(0).with(1), &[]);
        relays.sent(1, Ids::of(0).with(2), &[(0, 7)], NOW);
        assert_eq!(relays.next(), None);
        let mut end = chain();
        end.heard(1, 2, Ids::of(1), &[]);
        end.sent(1, Ids::of(0).with(2), &[(0, 7)], NOW);
        assert_eq!(end.next(), None, "the end of a chain passes nothing on");
    }

    #[test]
    fn the_neighbour_a_message_came_from_is_not_expected_to_pass_it_on() {
        let mut relays = chain();
        relays.heard(1, 0, Ids::of(1).with(9), &[(0, 7)]);
        relays.heard(1, 2, Ids::of(1).with(3), &[(0, 7)]);
        relays.sent(1, Ids::of(0).with(2), &[(0, 7)], NOW);
        assert_eq!(relays.next(), None, "0 and 2 both carried it");
    }

    #[test]
    fn a_later_message_or_another_relay_reaching_beyond_answers_for_one() {
        let mut relays = chain();
        relays.sent(1, Ids::of(0).with(2), &[(0, 7)], NOW);
        relays.heard(1, 2, Ids::of(1).with(3), &[(0, 9)]);
        assert_eq!(relays.next(), None, "a later message from its origin");

        let mut relays = chain();
        relays.heard(1, 4, Ids::of(1).with(3), &[]);
        relays.sent(1, Ids::of(0).with(2).with(4), &[(0, 7)], NOW);
        relays.heard(1, 4, Ids::of(1).with(3), &[(0, 7)]);
        assert_eq!(relays.next(), None, "4 reached 3, which 2 was to reach");
    }
}
