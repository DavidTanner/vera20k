//! Installed-locomotor state: which class a unit runs, and how it got there.
//!
//! Class identity and the retail CLSID table are
//! [`crate::rules::locomotor_type::LocomotorKind`]; capability and base-slot
//! defaults live in [`crate::sim::substrate::locomotion`]. This module owns the
//! *installed* side: the piggyback stash over the object a unit's type
//! constructed at spawn, and its power.
//!
//! ## Dependency rules
//! - Part of sim/ — depends on rules/ and `sim::substrate::locomotion` only.
//! - sim/ NEVER depends on render/, ui/, sidebar/, audio/, net/.

pub mod piggyback;
pub mod power;

pub use piggyback::{BeginOutcome, EndOutcome, LocomotorRuntimePayload, StashedLocomotor};
