//! What the group screens know of the mesh, as the firmware publishes it: this device, its
//! group, the pairing under way, a refresh, the wait of a device that founded a group, and how
//! the last leave or rename went. And what the screens ask of the mesh in return.

use heapless::Vec;
pub use octowhere_mesh::{
    IDS,
    members::{MAC_LEN, NAME_LEN, Name},
    pair::{CANDIDATES, Done, End, Phase, Reason, Role},
};

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
        self.radio = other.radio;
        self.mac = other.mac;
        self.name = other.name;
        self.group = other.group;
        self.sessions = other.sessions;
        self.pairing.clone_from(&other.pairing);
        self.answered = other.answered;
        self.answer = other.answer;
        self.refresh = other.refresh;
        self.recovery = other.recovery;
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
}
