//! The location mesh in `context/LORA-PROTOCOL.md`: the slot schedule, the packet's header and
//! records, its sealing under the group key, the table of positions, the timebase a node places
//! its slots on, the group's members, pairing, messages, and removing a member. It has no radio
//! or board dependency, so it builds and tests on the host.

#![no_std]

pub mod bits;
pub mod clock;
pub mod identity;
pub mod members;
pub mod messages;
pub mod packet;
pub mod pair;
pub mod rekey;
pub mod schedule;
pub mod seal;
pub mod table;

/// All zeroes is a valid value of a type that has it, which lets a large one be made in place.
pub use bytemuck::Zeroable;

/// The number of ids, and of slots in a round. Ids are five bits on the air, and sets of them
/// are `u32`.
pub const IDS: u8 = 32;
/// How far ahead of a node's clock a member or gone record, a position or a message may be
/// stamped: one stamped later would win every merge until then, a clock's error kept for ever.
pub const AHEAD_S: u32 = 3_600;
