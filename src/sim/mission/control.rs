//! Compatibility path for rules-owned immutable mission-control data.
//!
//! Canonical definitions and INI parsing live in `rules::mission_data`.
//! Runtime mission scheduling remains in the parent `sim::mission` module.

pub use crate::rules::mission_data::{MissionControl, MissionControlEntry};

use super::MissionId;
use crate::rules::ruleset::RuleSet;

/// `0x005B36E0`: whether MissionControl lets an object on `mission` be
/// recruited: always with none (-1), else the mission's `Recruitable=`
/// (`MissionControl[m]+6`, constructor 1). Read by `FootClass::IsRecruitable
/// @ 0x004DA230` and `TeamClass::Can_Add @ 0x006EA72C`.
pub(crate) fn mission_recruitable(rules: &RuleSet, mission: MissionId) -> bool {
    if mission == MissionId::NONE {
        return true;
    }
    mission
        .known()
        .and_then(|mission| rules.mission_control.entry(mission))
        .is_some_and(|entry| entry.recruitable)
}
