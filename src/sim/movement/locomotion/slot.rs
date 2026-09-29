//! The installed-locomotor slot: the single authority for which locomotor class
//! a unit runs.
//!
//! Natively a unit holds exactly one locomotor interface pointer, created once
//! in its class constructor from the type's `Locomotor=` CLSID and linked to the
//! owner. There is no second slot and no re-selection: no stock Yuri's Revenge
//! unit is constructed with one locomotor and later permanently swapped to
//! another. The Rust equivalent is therefore a resolved class, not a pointer —
//! the per-class runtime state lives in [`super::super::locomotor`], and a
//! temporary override (the Magnetron lift) is a *stash*, not a replacement, so
//! it belongs to the piggyback mechanism rather than here.

use crate::rules::locomotor_type::LocomotorKind;

/// The locomotor class installed on a unit at spawn.
///
/// A newtype rather than a bare [`LocomotorKind`] so that "the class this unit
/// was built with" cannot be silently confused with "the class currently
/// driving it" — those differ while a piggyback stash is active, and conflating
/// them is what made the previous `kind`/`primary_kind` pair ambiguous.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub struct LocomotorSlot {
    installed: LocomotorKind,
}

impl LocomotorSlot {
    /// Install `kind` as this unit's locomotor.
    pub const fn new(installed: LocomotorKind) -> Self {
        Self { installed }
    }

    /// The installed class.
    pub const fn installed(self) -> LocomotorKind {
        self.installed
    }
}
