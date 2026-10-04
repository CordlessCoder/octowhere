//! What a node takes from a packet of its group: the positions, member and gone records, and
//! messages it carries, and what its digests, requests and summary ask of this node.

use crate::{
    IDS,
    identity::Identity,
    members::{Group, Merged, Name, Requests, Slot},
    messages::{self, Insert, Message, Store, Summaries, To},
    packet::Record,
    rekey::Rekey,
    table::{Merge, Table},
};

/// Member and gone records a packet takes at most: two fit, since each is signed.
const RECORDS: usize = 4;
/// Messages a packet takes at most: it has room for about a dozen of the shortest.
const MESSAGES: usize = 16;

/// What a node keeps that a packet changes.
pub struct State<'a> {
    pub group: &'a mut Group,
    pub table: &'a mut Table,
    pub messages: &'a mut Store,
    pub requests: &'a mut Requests,
    pub summaries: &'a mut Summaries,
    pub rekey: &'a mut Rekey,
    pub me: &'a Identity,
}

/// When a packet arrived, as the node judges it.
#[derive(Clone, Copy)]
pub struct When {
    /// The round its sender's slot was in.
    pub round: i64,
    /// UTC seconds on the node's own timebase, if it has one.
    pub now: Option<u32>,
    /// UTC seconds from GNSS or the RTC, for a record's merge without a timebase.
    pub utc: u32,
    /// The start of the round, in seconds, by which its messages are judged.
    pub round_s: u32,
}

/// Something a packet changed that the node reports.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    /// The member at the id has a new record.
    Changed(u8, Name),
    /// Another device keeps this node's id, so this node moved to the lowest free one.
    Renumbered { from: u8, to: u8 },
    /// A member went, and the slot at the id holds its gone record.
    Went(u8),
}

/// What a packet changed, for the node to store, act on and report.
pub struct Absorbed {
    /// Positions it carried, and how many of them were news.
    pub entries: usize,
    pub news: usize,
    /// The ids its sender reports hearing, as a set.
    pub neighbours: u32,
    /// The ids whose slots changed, to store, as a set.
    pub changed: u32,
    /// This node's new id, when another device keeps the old one.
    pub renumbered: Option<u8>,
    /// Whether the removal state changed.
    pub rekey_changed: bool,
    /// A key message to this node, older than the store keeps, that catches it up.
    pub late_key: Option<Message>,
    /// Whether it carried a summary of its sender's messages.
    pub summary: bool,
    /// Messages it carried.
    pub carried: usize,
    carried_names: [messages::Name; MESSAGES],
    events: [Option<Event>; RECORDS],
    arrivals: [messages::Name; MESSAGES],
    arrived: usize,
}

impl Absorbed {
    /// What it changed that the node reports, in order.
    pub fn events(&self) -> impl Iterator<Item = Event> + '_ {
        self.events.iter().flatten().copied()
    }

    /// The messages new to this node.
    #[must_use]
    pub fn arrivals(&self) -> &[messages::Name] {
        &self.arrivals[..self.arrived]
    }

    /// Every message it carried, new to this node or not.
    #[must_use]
    pub fn carried_names(&self) -> &[messages::Name] {
        &self.carried_names[..self.carried]
    }
}

/// Takes the records of a packet from `sender`, which arrived `when`, into `state`.
pub fn absorb<'p>(
    records: impl Iterator<Item = Record<'p>>,
    sender: u8,
    when: When,
    state: State,
) -> Absorbed {
    let State {
        group,
        table,
        messages,
        requests,
        summaries,
        rekey,
        me,
    } = state;
    let own = group.own();
    let mut absorbed = Absorbed {
        entries: 0,
        news: 0,
        neighbours: 0,
        changed: 0,
        renumbered: None,
        rekey_changed: rekey.heard(sender),
        late_key: None,
        summary: false,
        carried: 0,
        carried_names: [(0, 0); MESSAGES],
        events: [None; RECORDS],
        arrivals: [(0, 0); MESSAGES],
        arrived: 0,
    };
    let mut events = 0;
    let mut event = |absorbed: &mut Absorbed, event: Event| {
        if let Some(slot) = absorbed.events.get_mut(events) {
            *slot = Some(event);
            events += 1;
        }
    };
    let mut stamps = [None; IDS as usize];
    let mut heard_records = [None; RECORDS];
    let (mut theirs, mut asked, mut their_messages, mut summary) = (None, None, 0, None);
    for record in records {
        match record {
            Record::Positions(positions) => {
                for entry in positions {
                    absorbed.entries += 1;
                    if let Some(stamp) = stamps.get_mut(usize::from(entry.id)) {
                        *stamp = Some(entry.stamp);
                    }
                    if matches!(table.merge(entry, when.now), Merge::New | Merge::Newer) {
                        absorbed.news += 1;
                    }
                }
            }
            Record::Neighbours(set) => absorbed.neighbours = set,
            Record::Members(digest) => theirs = Some(digest),
            Record::Request(ids) => asked = Some(ids),
            Record::Member(id, member) => {
                push(&mut heard_records, (id, Slot::Member(member)));
                match group.merge(id, member, when.now.unwrap_or(when.utc)) {
                    Merged::Unchanged => {}
                    Merged::Changed { vacated } => {
                        event(&mut absorbed, Event::Changed(id, member.name));
                        absorbed.changed |= 1 << id;
                        if let Some(vacated) = vacated {
                            absorbed.changed |= 1 << vacated;
                            // A member that moved is waited for at its new id, if at all.
                            absorbed.rekey_changed |= rekey.went(vacated);
                        }
                    }
                    Merged::Renumbered { from, to } => {
                        event(&mut absorbed, Event::Renumbered { from, to });
                        group.sign_own(me);
                        // Ahead of the positions, which come later in the packet.
                        table.renumber(to);
                        absorbed.renumbered = Some(to);
                    }
                    Merged::Went { at } => absorbed.changed |= 1 << at,
                }
            }
            Record::Gone(id, gone) => {
                push(&mut heard_records, (id, Slot::Gone(gone)));
                if let Merged::Went { at } =
                    group.merge_gone(id, gone, when.now.unwrap_or(when.utc))
                {
                    event(&mut absorbed, Event::Went(at));
                    absorbed.changed |= 1 << at;
                    absorbed.rekey_changed |= rekey.went(at);
                }
            }
            Record::Message(message) => {
                if let Some(slot) = absorbed.carried_names.get_mut(absorbed.carried) {
                    *slot = message.name();
                    absorbed.carried += 1;
                }
                match messages.insert(message, when.round_s) {
                    Insert::New => {
                        if let Some(slot) = absorbed.arrivals.get_mut(absorbed.arrived) {
                            *slot = message.name();
                            absorbed.arrived += 1;
                        }
                    }
                    // Catching up a member away for longer than the horizon.
                    Insert::Old if message.is_key() && message.to() == To::Member(own) => {
                        absorbed.late_key = Some(message);
                    }
                    _ => {}
                }
            }
            Record::Messages(digest) => their_messages = digest,
            Record::Summary(body) => summary = Some(body),
            Record::Other(..) => {}
        }
    }
    if table.covered_by(sender, &stamps, absorbed.neighbours, when.round) {
        for (id, slot) in heard_records.iter().flatten() {
            group.covered(*id, slot);
        }
        for &name in absorbed.carried_names() {
            messages.sent(name);
        }
    }
    requests.heard(group, sender, theirs, asked);
    // A packet with no members digest is no full account of its sender.
    if theirs.is_some() {
        summaries.heard(messages, sender, their_messages, summary);
    }
    absorbed.summary = summary.is_some();
    absorbed
}

fn push<T>(into: &mut [Option<T>], value: T) {
    if let Some(free) = into.iter_mut().find(|held| held.is_none()) {
        *free = Some(value);
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::boxed::Box;

    use super::*;
    use crate::{
        Zeroable,
        members::{
            MISMATCHES,
            tests::{key, member, signed},
        },
        messages::kind,
        packet::{Builder, Header, MAX_PLAIN, Plain, Source, Timebase},
        seal::Key,
    };

    const NOW: u32 = 1_790_000_000;
    const ROUND: i64 = 39_800_000;
    const WHEN: When = When {
        round: ROUND,
        now: Some(NOW),
        utc: NOW,
        round_s: NOW,
    };

    /// This node, device 1 at id 0, and device 2 at id 1.
    struct Node {
        group: Group,
        table: Table,
        messages: Box<Store>,
        requests: Requests,
        summaries: Summaries,
        rekey: Rekey,
        me: Identity,
    }

    impl Node {
        fn new() -> Self {
            let mut members = [None; IDS as usize];
            members[0] = Some(member(1, NOW - 100));
            members[1] = Some(member(2, NOW - 100));
            Self {
                group: Group::new(Key::new([5; 32]), 0, members).unwrap(),
                table: Table::new(0),
                messages: Box::new(Store::zeroed()),
                requests: Requests::default(),
                summaries: Summaries::default(),
                rekey: Rekey::default(),
                me: key(1),
            }
        }

        /// Takes a packet from id 1 holding what `fill` writes.
        fn absorb(&mut self, fill: impl FnOnce(&mut Builder)) -> Absorbed {
            let mut buf = [0; MAX_PLAIN];
            let header = Header {
                sender: 1,
                timebase: Timebase {
                    source: Source::Gps,
                    hops: 0,
                },
                base: NOW,
                phase: 0,
                notice: false,
            };
            let mut builder = Builder::new(&mut buf, &header);
            fill(&mut builder);
            let len = builder.finish();
            let plain = Plain::parse(&buf[..len]).unwrap();
            absorb(
                plain.records(),
                1,
                WHEN,
                State {
                    group: &mut self.group,
                    table: &mut self.table,
                    messages: &mut self.messages,
                    requests: &mut self.requests,
                    summaries: &mut self.summaries,
                    rekey: &mut self.rekey,
                    me: &self.me,
                },
            )
        }
    }

    #[test]
    fn a_newer_record_is_stored_and_reported() {
        let mut node = Node::new();
        let mut record = signed(1, 2, NOW - 10);
        record.name = Name::new(b"Roger Saved").unwrap();
        record.sign(1, &key(2));
        let absorbed = node.absorb(|builder| builder.member(1, &record).unwrap());
        assert_eq!(absorbed.changed, 1 << 1);
        assert_eq!(
            absorbed.events().collect::<std::vec::Vec<_>>(),
            [Event::Changed(1, record.name)]
        );
        assert_eq!(absorbed.renumbered, None);
    }

    #[test]
    fn records_a_full_account_carried_as_held_are_no_longer_news() {
        let mut node = Node::new();
        node.group.ask(1 << 1);
        let held = *node.group.slot(1).unwrap();
        let Slot::Member(record) = held else {
            panic!("id 1 holds a member");
        };
        node.absorb(|builder| {
            builder.neighbours(0).unwrap();
            builder.member(1, &record).unwrap();
        });
        assert_eq!(node.group.unsent() & 1 << 1, 0);
    }

    #[test]
    fn only_a_full_account_of_its_sender_makes_a_summary_due() {
        for members_digest in [true, false] {
            let mut node = Node::new();
            let held = Message::to_group(1, 1, 0, NOW - 5, &[kind::TEXT, b'h', b'i']).unwrap();
            node.messages.insert(held, NOW);
            let digest = node.group.digest();
            // Their messages digest, absent, differs from this node's.
            for _ in 0..MISMATCHES {
                node.absorb(|builder| {
                    if members_digest {
                        builder.members_digest(digest).unwrap();
                    }
                });
            }
            assert_eq!(
                node.summaries.pending(),
                members_digest,
                "members digest: {members_digest}"
            );
        }
    }

    #[test]
    fn a_new_message_arrives_once() {
        let mut node = Node::new();
        let message = Message::to_group(1, 1, 0, NOW - 5, &[kind::TEXT, b'h', b'i']).unwrap();
        let absorbed = node.absorb(|builder| builder.message(&message).unwrap());
        assert_eq!(absorbed.arrivals(), [message.name()]);
        assert_eq!(absorbed.carried, 1);
        let again = node.absorb(|builder| builder.message(&message).unwrap());
        assert!(again.arrivals().is_empty());
    }
}
