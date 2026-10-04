//! A node's flash as the simulation keeps it: what its group writes stored, as the firmware's
//! settings store keeps them, which a restart starts from.

use octowhere_mesh::{
    IDS,
    members::{Group, Name, Slot},
    pair::Identity,
    rekey::Rekey,
    seal::Key,
};
use octowhere_node::{GroupWrite, Start};

#[derive(Clone)]
pub struct Stored {
    seed: [u8; 32],
    mac: [u8; 6],
    name: Name,
    sequence: Option<u32>,
    /// The group's key, generation and this node's id.
    group: Option<(Key, u16, u8)>,
    slots: [Option<Slot>; IDS as usize],
    rekey: Option<Box<Rekey>>,
}

impl Stored {
    pub fn new(start: &Start) -> Self {
        let mut stored = Self {
            seed: start.me.seed(),
            mac: start.me.mac,
            name: start.me.name,
            sequence: start.sequence,
            group: None,
            slots: [None; IDS as usize],
            rekey: start.rekey.clone(),
        };
        if let Some(group) = &start.group {
            stored.take_group(group);
        }
        stored
    }

    fn take_group(&mut self, group: &Group) {
        self.group = Some((group.key().clone(), group.generation(), group.own()));
        for id in 0..IDS {
            self.slots[usize::from(id)] = group.slot(id).copied();
        }
    }

    pub fn apply(&mut self, write: &GroupWrite) {
        match write {
            GroupWrite::Identity(seed) => self.seed = *seed,
            GroupWrite::Name(name) => self.name = *name,
            GroupWrite::Sequence(end) => self.sequence = Some(*end),
            GroupWrite::Rekey(rekey) => self.rekey = Some(rekey.clone()),
            GroupWrite::Group(group, rekey) => {
                self.take_group(group);
                if let Some(rekey) = rekey {
                    self.rekey = Some(rekey.clone());
                }
            }
            GroupWrite::Slot { id, slot } => {
                if let Some(held) = self.slots.get_mut(usize::from(*id)) {
                    *held = *slot;
                }
            }
            GroupWrite::Leave => {
                self.group = None;
                self.slots = [None; IDS as usize];
                self.rekey = None;
            }
        }
    }

    /// What a node starts with after a restart.
    pub fn start(&self) -> Start {
        Start {
            me: Identity::new(self.seed, self.mac, self.name),
            group: self.group.as_ref().and_then(|(key, generation, own)| {
                Group::restore(key.clone(), *generation, *own, self.slots).map(Box::new)
            }),
            sequence: self.sequence,
            rekey: self.rekey.clone(),
        }
    }
}
