//! A set of ids, one bit each. Packets and the flash carry it as its `u32` bits.

use core::ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign, Not};

use crate::IDS;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct Ids(u32);

impl Ids {
    pub const EMPTY: Self = Self(0);
    pub const ALL: Self = Self(u32::MAX);

    #[must_use]
    pub const fn from_bits(bits: u32) -> Self {
        Self(bits)
    }

    #[must_use]
    pub const fn bits(self) -> u32 {
        self.0
    }

    #[must_use]
    pub const fn of(id: u8) -> Self {
        Self(1 << id)
    }

    #[must_use]
    pub const fn contains(self, id: u8) -> bool {
        self.0 & 1 << id != 0
    }

    pub fn insert(&mut self, id: u8) {
        self.0 |= 1 << id;
    }

    pub fn remove(&mut self, id: u8) {
        self.0 &= !(1 << id);
    }

    #[must_use]
    pub const fn with(self, id: u8) -> Self {
        Self(self.0 | 1 << id)
    }

    #[must_use]
    pub const fn without(self, id: u8) -> Self {
        Self(self.0 & !(1 << id))
    }

    #[must_use]
    pub const fn count(self) -> u32 {
        self.0.count_ones()
    }

    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// The lowest id in the set.
    #[must_use]
    pub fn first(self) -> Option<u8> {
        (!self.is_empty()).then(|| self.0.trailing_zeros() as u8)
    }

    /// The ids in the set, lowest first.
    pub fn iter(self) -> impl Iterator<Item = u8> {
        (0..IDS).filter(move |&id| self.contains(id))
    }
}

impl FromIterator<u8> for Ids {
    fn from_iter<I: IntoIterator<Item = u8>>(ids: I) -> Self {
        ids.into_iter().fold(Self::EMPTY, Self::with)
    }
}

impl BitOr for Ids {
    type Output = Self;

    fn bitor(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

impl BitOrAssign for Ids {
    fn bitor_assign(&mut self, other: Self) {
        self.0 |= other.0;
    }
}

impl BitAnd for Ids {
    type Output = Self;

    fn bitand(self, other: Self) -> Self {
        Self(self.0 & other.0)
    }
}

impl BitAndAssign for Ids {
    fn bitand_assign(&mut self, other: Self) {
        self.0 &= other.0;
    }
}

impl Not for Ids {
    type Output = Self;

    fn not(self) -> Self {
        Self(!self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_set_holds_what_goes_in_and_lists_it_in_order() {
        let mut ids = Ids::EMPTY.with(31).with(3);
        ids.insert(0);
        ids.remove(3);
        assert_eq!(ids.bits(), 1 | 1 << 31);
        assert!(ids.contains(31) && !ids.contains(3));
        assert_eq!(ids.count(), 2);
        assert_eq!(ids.first(), Some(0));
        assert!(ids.iter().eq([0, 31]));
        assert_eq!(ids.iter().collect::<Ids>(), ids);
        assert_eq!(Ids::EMPTY.first(), None);
        assert_eq!(!Ids::of(5) & Ids::ALL, Ids::ALL.without(5));
    }
}
