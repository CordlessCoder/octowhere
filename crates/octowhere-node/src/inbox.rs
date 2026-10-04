//! The messages a node shows the screens: its own, the group's and those to it, each with how far
//! it has gone. The store holds the messages themselves, sealed; this holds their text and
//! follows the store.

use alloc::boxed::Box;
use core::alloc::Allocator;

use octowhere_mesh::{
    members,
    messages::{Name, Store},
};

use crate::view::{At, Carriage, MessageView, MessagesView};

/// The other member of a message, by its device's fingerprint and its name.
pub type Peer = Option<([u8; 8], members::Name)>;

pub struct Inbox<A: Allocator> {
    view: Box<MessagesView, A>,
    /// The number the next message shown takes.
    next: u32,
    /// The timebase second this node first had a timebase at. A message stamped before came back
    /// from another member after a restart.
    since: Option<u32>,
    /// Whether the view changed since the screens were last shown it.
    changed: bool,
    /// Acknowledgements of this device's messages not shown yet, by who sent each and the
    /// sequence number it acknowledges. After a restart one can come back before its message.
    early_acks: heapless::Deque<(u8, u32), 8>,
}

impl<A: Allocator> Inbox<A> {
    pub fn new(view: Box<MessagesView, A>) -> Self {
        Self {
            view,
            next: 1,
            since: None,
            changed: true,
            early_acks: heapless::Deque::new(),
        }
    }

    pub fn view(&self) -> &MessagesView {
        &self.view
    }

    /// Whether the view changed since [`Inbox::shown`].
    pub fn changed(&self) -> bool {
        self.changed
    }

    /// Notes that the screens have the view.
    pub fn shown(&mut self) {
        self.changed = false;
    }

    /// Notes the timebase second this node has a timebase at, the first time it does.
    pub fn started(&mut self, second: u32) {
        self.since.get_or_insert(second);
    }

    fn number(&mut self) -> u32 {
        let id = self.next;
        self.next = self.next.wrapping_add(1).max(1);
        id
    }

    fn by_name(&mut self, (origin, seq): Name) -> Option<&mut MessageView> {
        self.view
            .find_mut(|message| message.from == origin && message.seq == seq && seq != 0)
    }

    fn update(&mut self, name: Name, change: impl FnOnce(&mut MessageView) -> bool) {
        if let Some(message) = self.by_name(name)
            && change(message)
        {
            self.changed = true;
        }
    }

    /// Shows a message made here, queued at `at`, to `peer`'s device if it is private, and
    /// returns its number.
    pub fn queue(&mut self, from: u8, to: Option<u8>, text: &[u8], at: At, peer: Peer) -> u32 {
        let id = self.number();
        let mut message = MessageView::new(id, at, from, to, text);
        if let Some((device, name)) = peer {
            message.set_peer(device, name);
        }
        self.view.push(message);
        self.changed = true;
        id
    }

    /// Gives the queued message `id` its sequence number, once it is made.
    pub fn posted(&mut self, id: u32, seq: u32) {
        if let Some(message) = self.view.get_mut(id) {
            message.seq = seq;
        }
    }

    /// Drops the queued message `id`, which could not be made.
    pub fn dropped(&mut self, id: u32) {
        self.view.retain(|message| message.id != id);
        self.changed = true;
    }

    /// A packet of this device's carried message `name`.
    pub fn sent(&mut self, name: Name) {
        self.update(name, |message| {
            let queued = message.carriage == Carriage::Queued;
            if queued {
                message.carriage = Carriage::Sent;
            }
            queued
        });
    }

    /// Another member's packet carried this device's message `name`.
    pub fn relayed(&mut self, name: Name) {
        self.update(name, |message| {
            let earlier = matches!(message.carriage, Carriage::Queued | Carriage::Sent);
            if earlier {
                message.carriage = Carriage::Relayed;
            }
            earlier
        });
    }

    /// Member `by` acknowledged this device's message numbered `seq`.
    pub fn delivered(&mut self, own: u8, by: u8, seq: u32) {
        match self
            .by_name((own, seq))
            .filter(|message| message.to == Some(by))
        {
            Some(message) => {
                message.carriage = Carriage::Delivered;
                self.changed = true;
            }
            None => {
                if self.early_acks.is_full() {
                    self.early_acks.pop_front();
                }
                _ = self.early_acks.push_back((by, seq));
            }
        }
    }

    /// Shows a message that arrived: another member's, or after a restart this device's own,
    /// which another member carried. One stamped before this node had a timebase came back
    /// after a restart, and is not counted unread.
    #[expect(clippy::too_many_arguments)]
    pub fn arrived(
        &mut self,
        name: Name,
        own: u8,
        to: Option<u8>,
        stamp: u32,
        at: At,
        text: &[u8],
        peer: Peer,
    ) {
        if self.by_name(name).is_some() {
            return;
        }
        let (origin, seq) = name;
        let recovered = self.since.is_none_or(|since| stamp < since);
        let mut message = MessageView::new(self.number(), at, origin, to, text);
        message.seq = seq;
        if let Some((device, name)) = peer {
            message.set_peer(device, name);
        }
        message.recovered = recovered;
        if origin == own {
            let acked = to.is_some_and(|to| self.early_acks.iter().any(|&ack| ack == (to, seq)));
            message.carriage = if acked {
                Carriage::Delivered
            } else {
                Carriage::Relayed
            };
        } else {
            message.carriage = Carriage::Received;
            message.unread = !recovered;
        }
        self.view.push(message);
        self.changed = true;
    }

    pub fn read(&mut self, id: u32) {
        if let Some(message) = self.view.get_mut(id)
            && message.unread
        {
            message.unread = false;
            self.changed = true;
        }
    }

    /// Drops the messages the store no longer holds: past the horizon, pushed out by newer ones,
    /// or forgotten. A message still queued stays.
    pub fn prune(&mut self, store: &Store) {
        let before = self.view.len();
        self.view.retain(|message| {
            message.carriage == Carriage::Queued && message.seq == 0
                || store.get((message.from, message.seq)).is_some()
        });
        self.changed |= self.view.len() != before;
    }

    pub fn clear(&mut self) {
        self.view.clear();
        self.early_acks.clear();
        self.changed = true;
    }
}
