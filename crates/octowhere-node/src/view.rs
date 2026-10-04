//! What the group screens know of the mesh, as the firmware publishes it: this device, its
//! group, the pairing under way, a refresh, the wait of a device that founded a group, and how
//! the last leave or rename went. And what the screens ask of the mesh in return.

use heapless::Vec;
pub use octowhere_mesh::{
    IDS,
    members::{MAC_LEN, NAME_LEN, Name},
    messages::TEXT_MAX,
    pair::{CANDIDATES, Done, End, Phase, Reason, Role},
};
use octowhere_mesh::{Zeroable, messages};

pub type Mac = [u8; MAC_LEN];

/// A time on the stage's clock, in microseconds. Signed, since a member can have joined before
/// the clock started.
pub type At = i64;

/// A member's position, as this device last received it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Position {
    Never,
    /// Received, but this device has no UTC to age it by.
    Unknown,
    /// Observed at this time, whoever relayed it.
    At(At),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MemberView {
    pub name: Name,
    pub mac: Mac,
    /// When it joined, if the device that added it knew UTC.
    pub joined: Option<At>,
    /// When this device last heard a packet the member sent itself. `None` for this device.
    pub heard: Option<At>,
    pub position: Position,
    /// Where its newest position put it, in degrees × 10⁷, latitude then longitude.
    pub coordinates: Option<(i32, i32)>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GroupView {
    /// This device's id.
    pub own: u8,
    pub members: [Option<MemberView>; IDS as usize],
}

impl GroupView {
    /// The ids taken, in order, with their members.
    pub fn members(&self) -> impl Iterator<Item = (u8, &MemberView)> {
        (0..IDS).filter_map(|id| Some((id, self.members[usize::from(id)].as_ref()?)))
    }

    #[must_use]
    pub fn member(&self, id: u8) -> Option<&MemberView> {
        self.members.get(usize::from(id))?.as_ref()
    }

    #[must_use]
    pub fn count(&self) -> usize {
        self.members.iter().flatten().count()
    }

    #[must_use]
    pub fn is_full(&self) -> bool {
        self.count() == usize::from(IDS)
    }
}

/// Why the mesh would not start a pairing it was asked for.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Refused {
    /// Joining needs this device in no group.
    InGroup,
    /// No true random source for the keys.
    NoRandom,
    /// Adding: a member is being removed, and the group is about to change its key.
    Removing,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PairingView {
    /// Counts up from 1 with each pairing asked for since boot.
    pub session: u32,
    pub role: Role,
    pub phase: Phase,
    /// When the phase times out, while it can.
    pub deadline: Option<At>,
    /// Adding: the devices announcing, in the order first heard.
    pub candidates: Vec<Mac, CANDIDATES>,
    pub peer: Option<Mac>,
    /// The other device's name, once it has sent it.
    pub peer_name: Option<Name>,
    /// The group the pairing holds, as this device's id and the member count.
    pub group: Option<(u8, u8)>,
    pub refused: Option<Refused>,
}

/// Where a refresh is. It listens throughout for three rounds, sending in its slot as usual.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RefreshPhase {
    Listening {
        until: At,
    },
    /// It ran its whole time.
    Ended {
        at: At,
    },
    /// A pairing took the radio first.
    Interrupted {
        at: At,
    },
}

/// The refresh under way, or the last one, for as long as this device keeps its group.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RefreshView {
    /// Counts up from 1 with each refresh started since boot.
    pub session: u32,
    pub phase: RefreshPhase,
    /// The ids of the devices it heard packets from directly, as a set.
    pub heard: u32,
    /// The ids of the members the group gained while it ran, as a set. A member learned from
    /// another's packet need not be among those heard.
    pub learned: u32,
}

impl RefreshView {
    #[must_use]
    pub fn is_listening(&self) -> bool {
        matches!(self.phase, RefreshPhase::Listening { .. })
    }
}

/// Where the wait of a device that founded a group without hearing the last acknowledgement is.
/// The group is not this device's until it is stored.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecoveryPhase {
    /// Listening for the joining device under the group's key.
    Listening {
        until: At,
    },
    /// The joining device was heard, and the group is being stored.
    Storing,
    /// The group could not be stored, and is tried again while the wait lasts.
    SaveFailed {
        until: At,
    },
    Stored,
    /// Nothing was heard in the wait.
    Expired,
    /// The joining device was heard, but the group could not be stored in the wait.
    NotStored,
}

impl RecoveryPhase {
    #[must_use]
    pub fn is_final(self) -> bool {
        matches!(self, Self::Stored | Self::Expired | Self::NotStored)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RecoveryView {
    /// The pairing that founded the group.
    pub session: u32,
    pub phase: RecoveryPhase,
    /// The joining device's name, once it has sent one, and its address.
    pub peer_name: Option<Name>,
    pub peer: Mac,
    /// The founded group's members.
    pub count: u8,
}

/// How the last leave or rename went: whether it reached the flash.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Answer {
    Left(bool),
    Renamed(bool),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MeshView {
    /// The radio answered at boot.
    pub radio: bool,
    pub mac: Mac,
    /// This device's stored name.
    pub name: Name,
    /// The stored group.
    pub group: Option<GroupView>,
    /// Pairings asked for since boot.
    pub sessions: u32,
    /// The latest of them.
    pub pairing: Option<PairingView>,
    /// Leaves and renames answered since boot, and the last answer.
    pub answered: u32,
    pub answer: Option<Answer>,
    pub refresh: Option<RefreshView>,
    /// A founding's wait, under way or ended, until another pairing starts.
    pub recovery: Option<RecoveryView>,
}

impl Default for MeshView {
    fn default() -> Self {
        let mac = [0; MAC_LEN];
        Self {
            radio: false,
            mac,
            name: Name::from_mac(&mac),
            group: None,
            sessions: 0,
            pairing: None,
            answered: 0,
            answer: None,
            refresh: None,
            recovery: None,
        }
    }
}

impl MeshView {
    /// Makes this view a copy of `other` a field at a time, which keeps a whole view's copy off
    /// the stack.
    pub fn copy_from(&mut self, other: &Self) {
        // Named in full, so that a field added later cannot be left out: the views are swapped,
        // and one left out would show a value two views old.
        let Self {
            radio,
            mac,
            name,
            group,
            sessions,
            pairing,
            answered,
            answer,
            refresh,
            recovery,
        } = other;
        self.radio = *radio;
        self.mac = *mac;
        self.name = *name;
        self.group = *group;
        self.sessions = *sessions;
        self.pairing.clone_from(pairing);
        self.answered = *answered;
        self.answer = *answer;
        self.refresh = *refresh;
        self.recovery = *recovery;
    }

    /// The pairing asked for after `sessions` had started, once the mesh has taken it up.
    #[must_use]
    pub fn session_after(&self, sessions: u32) -> Option<&PairingView> {
        self.pairing
            .as_ref()
            .filter(|pairing| pairing.session > sessions)
    }

    /// Whether a pairing is under way, which holds the screen awake.
    #[must_use]
    pub fn pairing_active(&self) -> bool {
        self.pairing
            .as_ref()
            .is_some_and(|pairing| !pairing.phase.is_final() && pairing.refused.is_none())
    }
}

/// A message's text: 1 to [`TEXT_MAX`] printable ASCII characters.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Text {
    len: u8,
    bytes: [u8; TEXT_MAX],
}

impl Text {
    #[must_use]
    pub fn new(text: &[u8]) -> Option<Self> {
        if !messages::is_text(text) {
            return None;
        }
        let mut bytes = [0; TEXT_MAX];
        bytes[..text.len()].copy_from_slice(text);
        Some(Self {
            len: text.len() as u8,
            bytes,
        })
    }

    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes[..usize::from(self.len)]
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        core::str::from_utf8(self.as_bytes()).unwrap_or_default()
    }
}

#[cfg(feature = "defmt")]
impl defmt::Format for Text {
    fn format(&self, f: defmt::Formatter) {
        defmt::write!(f, "{=[u8]:a}", self.as_bytes());
    }
}

/// The most messages the screens are shown: as many as a node's store holds.
pub const MESSAGES: usize = messages::CAPACITY;

/// How far a message has gone, as far as this device can tell.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[repr(u8)]
pub enum Carriage {
    /// Made here and waiting: no packet has carried it yet.
    #[default]
    Queued = 0,
    /// A packet of this device's carried it.
    Sent,
    /// Another member's packet carried it, which is a relay and not a delivery.
    Relayed,
    /// Its destination acknowledged it. Only a private message is.
    Delivered,
    /// Another member's message, readable here.
    Received,
}

/// A message this device can read: one of its own, one to the whole group, or one to it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct MessageView {
    /// This device's number for it, the same through its life, never 0.
    pub id: u32,
    /// Its origin's sequence number, 0 while it waits to be made here.
    pub seq: u32,
    /// When it was sent, on the stage's clock.
    pub at: At,
    pub from: u8,
    /// Its destination's id, or `None` for the whole group.
    pub to: Option<u8>,
    pub carriage: Carriage,
    /// Arrived and not yet read.
    pub unread: bool,
    /// Taken back from another member after this device restarted, so not new when it came,
    /// and whether it was read before is unknown.
    pub recovered: bool,
    len: u8,
    text: [u8; TEXT_MAX],
}

impl MessageView {
    #[must_use]
    pub fn new(id: u32, at: At, from: u8, to: Option<u8>, text: &[u8]) -> Self {
        let mut bytes = [0; TEXT_MAX];
        let len = text.len().min(TEXT_MAX);
        bytes[..len].copy_from_slice(&text[..len]);
        Self {
            id,
            seq: 0,
            at,
            from,
            to,
            carriage: Carriage::Queued,
            unread: false,
            recovered: false,
            len: len as u8,
            text: bytes,
        }
    }

    #[must_use]
    pub fn text(&self) -> &str {
        core::str::from_utf8(&self.text[..usize::from(self.len)]).unwrap_or_default()
    }

    /// The conversation it belongs to, seen from member `own`: the group, or the other member.
    #[must_use]
    pub fn thread(&self, own: u8) -> Thread {
        match self.to {
            None => Thread::Group,
            Some(to) if self.from == own => Thread::Member(to),
            Some(_) => Thread::Member(self.from),
        }
    }
}

/// A conversation: the whole group's, or this device's with one member.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Thread {
    Group,
    Member(u8),
}

/// The messages this device can read, oldest first by when each was sent. All zeroes is none, so
/// the firmware makes it in place in PSRAM: it is tens of kilobytes.
#[repr(C)]
pub struct MessagesView {
    len: u16,
    messages: [MessageView; MESSAGES],
}

// SAFETY: every field is an integer, an array of them, an `Option<u8>` or a `bool`, whose all-zero
// value is valid, or a `Carriage`, whose zero is `Queued`.
unsafe impl Zeroable for MessageView {}
// SAFETY: as above, and `len` 0 holds none.
unsafe impl Zeroable for MessagesView {}

impl MessagesView {
    /// None, made in place on the heap.
    #[must_use]
    pub fn boxed() -> alloc::boxed::Box<Self> {
        // SAFETY: `Zeroable` promises that all zeroes is a valid view.
        unsafe { alloc::boxed::Box::new_zeroed().assume_init() }
    }

    pub fn iter(&self) -> impl DoubleEndedIterator<Item = &MessageView> + Clone {
        self.messages[..usize::from(self.len)].iter()
    }

    #[must_use]
    pub fn get(&self, id: u32) -> Option<&MessageView> {
        self.iter().find(|message| message.id == id)
    }

    pub fn get_mut(&mut self, id: u32) -> Option<&mut MessageView> {
        self.find_mut(|message| message.id == id)
    }

    pub fn find_mut(
        &mut self,
        mut found: impl FnMut(&MessageView) -> bool,
    ) -> Option<&mut MessageView> {
        self.messages[..usize::from(self.len)]
            .iter_mut()
            .find(|message| found(message))
    }

    #[must_use]
    pub fn len(&self) -> usize {
        usize::from(self.len)
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Adds `message` in order of time, after any sent at the same time, dropping the oldest
    /// when full.
    pub fn push(&mut self, message: MessageView) {
        if self.len() == MESSAGES {
            self.messages.copy_within(1.., 0);
            self.len -= 1;
        }
        let len = self.len();
        let at = self.messages[..len]
            .iter()
            .rposition(|held| held.at <= message.at)
            .map_or(0, |before| before + 1);
        self.messages.copy_within(at..len, at + 1);
        self.messages[at] = message;
        self.len += 1;
    }

    /// Keeps only the messages `keep` returns true for, in order.
    pub fn retain(&mut self, mut keep: impl FnMut(&MessageView) -> bool) {
        let mut kept = 0;
        for at in 0..self.len() {
            if keep(&self.messages[at]) {
                self.messages[kept] = self.messages[at];
                kept += 1;
            }
        }
        self.len = kept as u16;
    }

    pub fn clear(&mut self) {
        self.len = 0;
    }

    /// Makes this a copy of `other`, copying only the messages it holds.
    pub fn copy_from(&mut self, other: &Self) {
        let len = other.len();
        self.messages[..len].copy_from_slice(&other.messages[..len]);
        self.len = other.len;
    }

    /// The messages of `thread`, seen from member `own`, oldest first.
    pub fn thread(&self, thread: Thread, own: u8) -> impl DoubleEndedIterator<Item = &MessageView> {
        self.iter()
            .filter(move |message| message.thread(own) == thread)
    }

    /// How many messages are unread.
    #[must_use]
    pub fn unread(&self) -> usize {
        self.iter().filter(|message| message.unread).count()
    }
}

/// What the screens ask of the mesh.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Request {
    Add,
    Join,
    /// Adding: picks the device announcing from this address.
    Choose(Mac),
    Accept,
    Decline,
    Mismatch,
    Cancel,
    Leave,
    Rename(Name),
    /// Listens throughout for three rounds, to find group members near now.
    Refresh,
    /// Sends text to one member, privately, or with `None` to the whole group.
    Send {
        to: Option<u8>,
        text: Text,
    },
    /// Counts the message this device numbered so as read.
    Read(u32),
}
