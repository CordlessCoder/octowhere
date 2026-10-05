//! The changes to its state a node stores, one at a time.

use alloc::boxed::Box;

use octowhere_mesh::{
    members::{Group, Name, Slot},
    rekey::{KEPT_ROW_MAX, Rekey},
};

/// The key messages kept for one member, as `Kept::encode_row` writes them.
pub type KeptRow = heapless::Vec<u8, KEPT_ROW_MAX>;

/// One change to the mesh's state to save.
pub enum GroupWrite {
    Identity([u8; 32]),
    Name(Name),
    /// The end of a new block of message sequence numbers.
    Sequence(u32),
    /// The group's removals: the one under way, the old keys and those declined.
    Rekey(Box<Rekey>),
    /// The whole group, replacing what was stored, and with a change of key its removals.
    Group(Box<Group>, Option<Box<Rekey>>),
    /// What the id `id` holds: a member's record, a gone record, or nothing.
    Slot {
        id: u8,
        slot: Option<Slot>,
    },
    /// The key messages kept for the member `id`, to catch it up after a restart, or with
    /// none, nothing kept for it.
    Kept {
        id: u8,
        row: Option<Box<KeptRow>>,
    },
    /// Forget the group: its key, the members and this device's id, and the key messages kept.
    Leave,
}

#[cfg(feature = "defmt")]
impl defmt::Format for GroupWrite {
    fn format(&self, f: defmt::Formatter) {
        match self {
            Self::Identity(_) => defmt::write!(f, "Identity"),
            Self::Name(name) => defmt::write!(f, "Name({})", name),
            Self::Sequence(end) => defmt::write!(f, "Sequence({})", end),
            Self::Rekey(rekey) => defmt::write!(
                f,
                "Rekey(pending={} old={})",
                rekey.pending().is_some(),
                rekey.old().count()
            ),
            Self::Group(group, rekey) => defmt::write!(
                f,
                "Group(own={} members={} generation={} rekey={})",
                group.own(),
                group.count(),
                group.generation(),
                rekey.is_some()
            ),
            Self::Slot { id, slot } => match slot {
                Some(Slot::Member(member)) => defmt::write!(f, "Member({}, {})", id, member.name),
                Some(Slot::Gone(_)) => defmt::write!(f, "Gone({})", id),
                None => defmt::write!(f, "Empty({})", id),
            },
            Self::Kept { id, row } => {
                defmt::write!(
                    f,
                    "Kept({}, {=usize} bytes)",
                    id,
                    row.as_ref().map_or(0, |row| row.len())
                )
            }
            Self::Leave => defmt::write!(f, "Leave"),
        }
    }
}
