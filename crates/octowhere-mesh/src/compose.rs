//! What a node's packet carries in its slot, in order: the neighbours, the digests and the ids it
//! asks for; then a summary of its messages, member and gone records, and messages, oldest first;
//! and the positions last. Room is kept for the node's own position throughout, so that a busy
//! packet never crowds it out.

use crate::{
    members::{GONE_LEN, Group, Requests},
    messages::{Name, Store},
    packet::{Builder, Entry, Hdop, Quality, positions_len},
    rekey::OnKey,
    table::Table,
};

/// Member and gone records a packet carries at most, together.
pub const MAX_RECORDS: usize = 3;
/// Positions a packet carries at most: as many as fit beside the neighbours record.
pub const MAX_ENTRIES: usize = 24;
/// Messages a packet carries at most.
pub const MAX_MESSAGES: usize = 8;

/// What a node holds that its packet can carry.
#[derive(Clone, Copy)]
pub struct Sources<'a> {
    pub table: &'a Table,
    pub group: &'a Group,
    pub messages: &'a Store,
    pub requests: &'a Requests,
    /// A summary of the messages, made for this packet.
    pub summary: Option<&'a [u8]>,
    /// This node's word that it is on the group's key, while it is to send it.
    pub on_key: Option<&'a OnKey>,
}

/// What a packet carried, to count as sent once it has gone.
pub struct Carried {
    pub neighbours: u32,
    /// The ids asked for, as a set.
    pub requests: u32,
    /// The ids whose slots went, as a set.
    pub records: u32,
    /// The places of the former members' gone records that went, as a set.
    pub former: u8,
    pub summary: bool,
    /// Whether the node's word that it is on the key went.
    pub on_key: bool,
    entries: [Entry; MAX_ENTRIES],
    positions: usize,
    messages: [Name; MAX_MESSAGES],
    carried: usize,
}

impl Carried {
    /// The positions the packet carried.
    #[must_use]
    pub fn positions(&self) -> &[Entry] {
        &self.entries[..self.positions]
    }

    /// The messages the packet carried.
    #[must_use]
    pub fn messages(&self) -> &[Name] {
        &self.messages[..self.carried]
    }

    /// Counts everything the packet carried as sent, but the summary, which the caller made.
    pub fn sent(
        &self,
        table: &mut Table,
        requests: &mut Requests,
        store: &mut Store,
        group: &mut Group,
    ) {
        table.sent(self.positions());
        requests.sent(self.requests);
        for &name in self.messages() {
            store.sent(name);
        }
        for id in (0..32).filter(|&id| self.records & 1 << id != 0) {
            group.sent(id);
        }
        for at in (0..8).filter(|&at| self.former & 1 << at != 0) {
            group.former_sent(at);
        }
    }
}

/// Fills `builder` with what `from` holds, for round `round` of the timebase whose base is
/// `base`, and returns what went in.
pub fn compose(builder: &mut Builder, round: i64, base: u32, from: Sources) -> Carried {
    let Sources {
        table,
        group,
        messages,
        requests,
        summary,
        on_key,
    } = from;
    let mut carried = Carried {
        neighbours: table.neighbours(round),
        requests: requests.pending(),
        records: 0,
        former: 0,
        summary: false,
        on_key: false,
        entries: [Entry {
            id: 0,
            latitude: 0,
            longitude: 0,
            stamp: 0,
            quality: Quality::Reserved,
            hdop: Hdop::from_milli(0),
        }; MAX_ENTRIES],
        positions: 0,
        messages: [(0, 0); MAX_MESSAGES],
        carried: 0,
    };
    let _ = builder.neighbours(carried.neighbours);
    let _ = builder.members_digest(group.digest());
    if !messages.is_empty() {
        let _ = builder.messages_digest(messages.digest());
    }
    if carried.requests != 0 {
        let _ = builder.request(carried.requests);
    }
    carried.on_key = on_key.is_some_and(|on_key| builder.on_key(on_key).is_ok());
    let own_room = if table.entry(group.own()).is_some() {
        positions_len(1)
    } else {
        0
    };
    carried.summary = summary.is_some_and(|summary| {
        builder.room() >= 2 + summary.len() + own_room && builder.summary(summary).is_ok()
    });
    for id in group.unsent_in_turn() {
        if carried.records.count_ones() as usize >= MAX_RECORDS {
            break;
        }
        let Some(slot) = group.slot(id).copied() else {
            continue;
        };
        if builder.room() < slot.record_len() + own_room || builder.slot(id, &slot).is_err() {
            break;
        }
        carried.records |= 1 << id;
    }
    for (at, id, gone) in group.former_unsent() {
        if (carried.records.count_ones() + carried.former.count_ones()) as usize >= MAX_RECORDS
            || builder.room() < 2 + GONE_LEN + own_room
            || builder.gone(id, &gone).is_err()
        {
            break;
        }
        carried.former |= 1 << at;
    }
    let mut after = None;
    while let Some(message) = messages.next_unsent(after.as_ref()) {
        if carried.carried == MAX_MESSAGES
            || builder.room() < message.record_len() + own_room
            || builder.message(message).is_err()
        {
            break;
        }
        carried.messages[carried.carried] = message.name();
        carried.carried += 1;
        after = Some(*message);
    }
    let room = builder.room_for_entries().min(MAX_ENTRIES);
    let n = table.digest(base, &mut carried.entries[..room]);
    if n > 0 && builder.positions(&carried.entries[..n]).is_ok() {
        carried.positions = n;
    }
    carried
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::{boxed::Box, vec::Vec};

    use super::*;
    use crate::{
        IDS, Zeroable,
        members::tests::member,
        messages::{Message, kind},
        packet::{Header, MAX_PLAIN, Plain, Record, Source, Timebase},
        seal::Key,
    };

    const NOW: u32 = 1_790_000_000;
    const ROUND: i64 = 39_800_000;

    fn header() -> Header {
        Header {
            sender: 0,
            timebase: Timebase {
                source: Source::Gps,
                hops: 0,
            },
            base: NOW,
            phase: 0,
            notice: false,
        }
    }

    /// A group of `count` members with this node at id 0, every other record waiting to go.
    fn group(count: u8) -> Group {
        let mut members = [None; IDS as usize];
        for id in 0..count {
            members[usize::from(id)] = Some(member(id + 1, NOW - 100));
        }
        let mut group = Group::new(Key::new([5; 32]), 0, members).unwrap();
        group.ask(u32::MAX);
        group
    }

    fn entry(id: u8) -> Entry {
        Entry {
            id,
            latitude: 533_498_000 + i32::from(id) * 1_000,
            longitude: -62_603_000,
            stamp: NOW - 10,
            quality: Quality::Autonomous,
            hdop: Hdop::from_milli(900),
        }
    }

    /// A table holding this node's position, and those of `others`.
    fn table(others: &[u8]) -> Table {
        let mut table = Table::new(0);
        table.set_own(entry(0));
        for &id in others {
            table.merge(entry(id), Some(NOW));
        }
        table
    }

    /// A store of `count` messages from member 3, stamped newest first, so that sending order
    /// differs from the order they were inserted in.
    fn store(count: u32) -> Box<Store> {
        let mut store = Box::new(Store::zeroed());
        for n in 0..count {
            let message =
                Message::to_group(3, n + 1, n, NOW - n, &[kind::TEXT, b'h', b'i']).unwrap();
            store.insert(message, NOW);
        }
        store
    }

    /// Composes a packet from `from`, and returns what it carried and its records' kinds in
    /// order.
    fn compose_kinds(from: Sources) -> (Carried, Vec<&'static str>) {
        let mut buf = [0; MAX_PLAIN];
        let mut builder = Builder::new(&mut buf, &header());
        let carried = compose(&mut builder, ROUND, NOW, from);
        let len = builder.finish();
        let plain = Plain::parse(&buf[..len]).unwrap();
        let kinds = plain
            .records()
            .map(|record| match record {
                Record::Positions(_) => "positions",
                Record::Neighbours(_) => "neighbours",
                Record::Members(_) => "members digest",
                Record::Request(_) => "request",
                Record::Member(..) => "member",
                Record::Gone(..) => "gone",
                Record::OnKey(_) => "on key",
                Record::Message(_) => "message",
                Record::Messages(_) => "messages digest",
                Record::Summary(_) => "summary",
                Record::Other(..) => "other",
            })
            .collect();
        (carried, kinds)
    }

    #[test]
    fn a_word_on_the_key_goes_ahead_of_the_records() {
        let (table, group, store) = (table(&[2, 3]), group(6), store(2));
        let on_key = OnKey::new(0, 1, group.key(), &crate::members::tests::key(1));
        let (carried, kinds) = compose_kinds(Sources {
            table: &table,
            group: &group,
            messages: &store,
            requests: &Requests::default(),
            summary: None,
            on_key: Some(&on_key),
        });
        assert!(carried.on_key);
        assert_eq!(
            kinds[..4],
            ["neighbours", "members digest", "messages digest", "on key"]
        );
    }

    #[test]
    fn records_go_in_order_as_far_as_room_allows() {
        let (table, group, store) = (table(&[2, 3]), group(6), store(2));
        let (carried, kinds) = compose_kinds(Sources {
            table: &table,
            group: &group,
            messages: &store,
            requests: &Requests::default(),
            summary: Some(&[1, 2, 3]),
            on_key: None,
        });
        // A signed member record takes over half a packet, so one goes.
        assert_eq!(
            kinds,
            [
                "neighbours",
                "members digest",
                "messages digest",
                "summary",
                "member",
                "message",
                "message",
                "positions",
            ]
        );
        assert_eq!(carried.records, 1, "the lowest id waiting, this node's own");
        assert_eq!(carried.positions().len(), 3);
        assert!(carried.summary);
    }

    #[test]
    fn messages_go_oldest_first() {
        let (table, group, store) = (table(&[]), group(1), store(4));
        let (carried, _) = compose_kinds(Sources {
            table: &table,
            group: &group,
            messages: &store,
            requests: &Requests::default(),
            summary: None,
            on_key: None,
        });
        assert_eq!(carried.messages(), [(3, 4), (3, 3), (3, 2), (3, 1)]);
    }

    #[test]
    fn a_full_packet_keeps_room_for_the_own_position() {
        let (table, group, store) = (table(&[]), group(1), store(40));
        let (carried, kinds) = compose_kinds(Sources {
            table: &table,
            group: &group,
            messages: &store,
            requests: &Requests::default(),
            summary: None,
            on_key: None,
        });
        assert!(carried.messages().len() < 40);
        assert_eq!(kinds.last(), Some(&"positions"));
        assert_eq!(carried.positions().first().map(|entry| entry.id), Some(0));
    }

    #[test]
    fn a_summary_waits_where_it_would_crowd_out_the_own_position() {
        let group = group(1);
        let store = store(0);
        let mut buf = [0; MAX_PLAIN];
        let mut builder = Builder::new(&mut buf, &header());
        let _ = builder.neighbours(0);
        let _ = builder.members_digest(group.digest());
        // One byte longer than leaves room for the node's own position.
        let summary = std::vec![0; builder.room() - 2 - positions_len(1) + 1];
        for (own_position, carries) in [(true, false), (false, true)] {
            let table = if own_position {
                table(&[])
            } else {
                Table::new(0)
            };
            let (carried, _) = compose_kinds(Sources {
                table: &table,
                group: &group,
                messages: &store,
                requests: &Requests::default(),
                summary: Some(&summary),
                on_key: None,
            });
            assert_eq!(carried.summary, carries, "own position: {own_position}");
        }
    }

    #[test]
    fn records_asked_for_again_keep_no_other_waiting() {
        let (mut table, mut group, mut store) = (table(&[]), group(3), store(0));
        let mut requests = Requests::default();
        let mut went = 0;
        for _ in 0..3 {
            let (carried, _) = compose_kinds(Sources {
                table: &table,
                group: &group,
                messages: &store,
                requests: &requests,
                summary: None,
                on_key: None,
            });
            carried.sent(&mut table, &mut requests, &mut store, &mut group);
            went |= carried.records;
            // A neighbour whose digest differs asks for every id it holds again.
            group.ask(0b011);
        }
        assert_eq!(
            went, 0b111,
            "every record waiting went within three packets"
        );
    }

    #[test]
    fn what_went_counts_as_sent() {
        let (mut table, mut group, mut store) = (table(&[2]), group(6), store(3));
        let mut requests = Requests::default();
        let (carried, _) = compose_kinds(Sources {
            table: &table,
            group: &group,
            messages: &store,
            requests: &requests,
            summary: None,
            on_key: None,
        });
        let unsent = group.unsent();
        carried.sent(&mut table, &mut requests, &mut store, &mut group);
        assert_eq!(group.unsent(), unsent & !carried.records);
        assert!(store.next_unsent(None).is_none());
        let (again, _) = compose_kinds(Sources {
            table: &table,
            group: &group,
            messages: &store,
            requests: &requests,
            summary: None,
            on_key: None,
        });
        assert_eq!(again.records & carried.records, 0);
        assert!(again.messages().is_empty());
    }
}
