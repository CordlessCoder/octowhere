//! What changed in the group and its removals that the flash does not hold yet, and the order
//! it goes there in.

/// A write the flash is due, in the order [`Unsaved::due`] gives them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Due {
    /// The whole group, with the removals when they changed too.
    Group {
        rekey: bool,
    },
    Rekey,
    /// One member's slot.
    Slot(u8),
}

#[derive(Default)]
pub struct Unsaved {
    /// The ids whose member records changed, as a set.
    slots: u32,
    /// The group's key or this node's id changed, so the whole group is to be stored.
    group: bool,
    rekey: bool,
}

impl Unsaved {
    pub fn slot(&mut self, id: u8) {
        self.slots |= 1 << id;
    }

    pub fn group(&mut self) {
        self.group = true;
    }

    pub fn rekey(&mut self) {
        self.rekey = true;
    }

    /// The group and its removals both, as a change of key needs in one write, or after a write
    /// whose result is lost.
    pub fn everything(&mut self) {
        self.group = true;
        self.rekey = true;
    }

    /// The group was stored whole elsewhere, or is gone, so its slots need no write. The
    /// removals keep their own.
    pub fn group_replaced(&mut self) {
        self.slots = 0;
        self.group = false;
    }

    #[must_use]
    pub fn rekey_due(&self) -> bool {
        self.rekey
    }

    /// The next write the flash is due, with `has_group` saying whether the node holds a group.
    /// A change of key goes with the group in one write: apart, a restart between the two would
    /// pair the key with removals made for another. The removals go before any slot, so that a
    /// restart never keeps a change that declining the last removal would not forget.
    #[must_use]
    pub fn due(&self, has_group: bool) -> Option<Due> {
        if self.group {
            return has_group.then_some(Due::Group { rekey: self.rekey });
        }
        if self.rekey {
            return Some(Due::Rekey);
        }
        (has_group && self.slots != 0).then(|| Due::Slot(self.slots.trailing_zeros() as u8))
    }

    /// `due` was queued.
    pub fn queued(&mut self, due: Due) {
        match due {
            Due::Group { .. } => *self = Self::default(),
            Due::Rekey => self.rekey = false,
            Due::Slot(id) => self.slots &= !(1 << id),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Queues every write due until none is, and returns them in order.
    fn drain(unsaved: &mut Unsaved, has_group: bool) -> Vec<Due> {
        let mut writes = Vec::new();
        while let Some(due) = unsaved.due(has_group) {
            unsaved.queued(due);
            writes.push(due);
        }
        writes
    }

    #[test]
    fn slots_go_in_id_order_after_the_removals() {
        let mut unsaved = Unsaved::default();
        unsaved.slot(5);
        unsaved.slot(2);
        unsaved.rekey();
        assert_eq!(
            drain(&mut unsaved, true),
            [Due::Rekey, Due::Slot(2), Due::Slot(5)]
        );
        assert_eq!(unsaved.due(true), None);
    }

    #[test]
    fn the_whole_group_takes_the_slots_and_the_removals_with_it() {
        let mut unsaved = Unsaved::default();
        unsaved.slot(3);
        unsaved.everything();
        assert_eq!(drain(&mut unsaved, true), [Due::Group { rekey: true }]);
        unsaved.group();
        assert_eq!(drain(&mut unsaved, true), [Due::Group { rekey: false }]);
    }

    #[test]
    fn without_a_group_only_the_removals_go_and_never_ahead_of_a_key() {
        let mut unsaved = Unsaved::default();
        unsaved.slot(1);
        unsaved.rekey();
        assert_eq!(drain(&mut unsaved, false), [Due::Rekey]);
        unsaved.everything();
        assert_eq!(unsaved.due(false), None);
        assert_eq!(drain(&mut unsaved, true), [Due::Group { rekey: true }]);
    }

    #[test]
    fn a_replaced_group_leaves_only_the_removals() {
        let mut unsaved = Unsaved::default();
        unsaved.slot(4);
        unsaved.everything();
        unsaved.group_replaced();
        assert_eq!(drain(&mut unsaved, true), [Due::Rekey]);
    }
}
