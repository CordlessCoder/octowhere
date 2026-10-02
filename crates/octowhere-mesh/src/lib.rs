//! The location mesh in `context/LORA-PROTOCOL.md`: the slot schedule, the packet's header and
//! records, its sealing under the group key, the table of positions, and the timebase a node
//! places its slots on. It has no radio or board dependency, so it builds and tests on the host.

#![no_std]

pub mod bits;
pub mod clock;
pub mod members;
pub mod packet;
pub mod pair;
pub mod schedule;
pub mod seal;
pub mod table;

/// The number of ids, and of slots in a round.
pub const IDS: u8 = 32;
