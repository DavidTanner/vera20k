//! The member virtuals `TeamClass` calls, each through its class's port.
//!
//! Team code orders its members only through these: `Queue_Mission`
//! (`vt+0x1E8`), `Ready_To_Commence`/`Commence` (`vt+0x200`/`vt+0x1EC`),
//! `Assign_Target` (`vt+0x3C8`), `Set_Destination` (`vt+0x480`),
//! `Enter_Idle_Mode` (`vt+0x484`) and `Set_ArchiveTarget` (`0x0070C610`).
//!
//! RESIDUALS:
//! - `Set_Destination` for an aircraft (`0x0041AA80`) is not given. Unit
//!   callers, including Jumpjet, use the shared class setter; Infantry uses
//!   its class setter and retains that owner's Jumpjet limitations.
//!   Trigger: an aircraft member in a team ordered to move or to
//!   join up. Effect: it stays where it is and never joins, so a team of
//!   them never finishes action 53 or 54 (`Coordinate_Move`) and passes
//!   action 0 without attacking.
//!   Frequency: the previously counted 24 retail AIMD teams mixed Aircraft
//!   and now-supported Unit members, so that count no longer bounds this gap.
//!   Downstream: a stuck aircraft team keeps its members and its `Max=`
//!   slot.
//! - `Enter_Idle_Mode` for an aircraft (`0x004176F0`) is not called for a
//!   member leaving its team; see [`Simulation::team_member_enter_idle_mode`].

use crate::map::entities::EntityCategory;
use crate::rules::overlay_types::OverlayTypeRegistry;
use crate::rules::ruleset::RuleSet;
use crate::sim::mission::authority::LiveReadyInputProvider;
use crate::sim::mission::{MissionId, MissionType};
use crate::sim::world::Simulation;

use super::TeamTarget;

impl Simulation {
    /// `vt+0x1E8` `Queue_Mission(mission, 0)`: `MissionClass::Queue_Mission
    /// @ 0x005B35E0`, behind the Aircraft filter `0x0041BA90`.
    pub(super) fn team_member_queue_mission(
        &mut self,
        id: u64,
        mission: MissionType,
        rules: &RuleSet,
    ) {
        self.team_member_queue_mission_id(id, MissionId::from_known(mission), rules);
    }

    /// [`Self::team_member_queue_mission`] with a raw mission (script
    /// action 11's argument).
    pub(super) fn team_member_queue_mission_id(
        &mut self,
        id: u64,
        mission: MissionId,
        rules: &RuleSet,
    ) {
        let now = self.session.binary_frame;
        let readiness = LiveReadyInputProvider { rules };
        let _ = self.mission_queue_exact(id, mission, 0, now, &readiness);
    }

    /// `vt+0x200` then `vt+0x1EC`: the queued mission commences at once when
    /// the member is ready for it.
    pub(super) fn team_member_commence(&mut self, id: u64, rules: &RuleSet) {
        if self.mission_ready_to_commence(id, rules) {
            let now = self.session.binary_frame;
            let _ = self.mission_commence_exact(id, now);
        }
    }

    /// `vt+0x3C8` `Assign_Target(NULL)`: `TechnoClass::Assign_Target @
    /// 0x006FCDB0`, Infantry's `0x0051B1F0`.
    pub(super) fn team_member_clear_target(&mut self, id: u64, rules: &RuleSet) {
        let _ = self.assign_target_represented(id, None, Some(rules));
    }

    /// `vt+0x3C8` `Assign_Target(target)`.
    pub(super) fn team_member_assign_target(
        &mut self,
        id: u64,
        target: TeamTarget,
        rules: &RuleSet,
    ) {
        let _ = self.assign_target_represented(id, Some(target.target_kind()), Some(rules));
    }

    /// `Set_ArchiveTarget(NULL)` (`0x0070C610`), a plain store.
    pub(super) fn team_member_clear_archive_target(&mut self, id: u64) {
        if let Some(entity) = self.substrate.entities.get_mut(id) {
            entity.set_archive_target(None);
        }
    }

    /// `vt+0x480` `Set_Destination(target, 1)`: Infantry `0x0051AA40`, Unit
    /// `0x00741970` (see the module residuals for the receivers and targets
    /// without a port).
    pub(super) fn team_member_set_destination(
        &mut self,
        id: u64,
        target: Option<TeamTarget>,
        rules: &RuleSet,
        registry: Option<&OverlayTypeRegistry>,
    ) {
        let Some(target) = target else {
            self.assign_null_destination(id, Some(rules), None);
            return;
        };
        let requested = match target {
            TeamTarget::Cell { x, y } => {
                crate::sim::components::NavTargetRef::cell(x as u16, y as u16)
            }
            TeamTarget::Object(id) => crate::sim::components::NavTargetRef::object(id),
        };
        let Some(category) = self
            .substrate
            .entities
            .get(id)
            .map(|entity| entity.category)
        else {
            return;
        };
        match category {
            EntityCategory::Infantry => {
                let _ = self.set_infantry_destination(id, requested, rules, registry);
            }
            EntityCategory::Unit if self.unit_setter_receiver(id, Some(rules)) => {
                self.set_unit_destination(id, requested, rules, true);
            }
            _ => {}
        }
    }

    /// `vt+0x484` `Enter_Idle_Mode(0, 1)`: Unit `0x00738970`
    /// ([`Self::unit_enter_idle_mode`]), Infantry `0x0051CBA0` (VERA's Foot
    /// selector, [`crate::sim::world::queue_foot_enter_idle_mode`]).
    ///
    /// RESIDUAL: the Aircraft arm (`0x004176F0`, `aircraft::enter_idle_mode_now`)
    /// is not called, so an aircraft leaving a team keeps its mission.
    /// Trigger: an aircraft member leaves its team alive. Effect: it does not
    /// return to its idle mission.
    pub(crate) fn team_member_enter_idle_mode(&mut self, id: u64, rules: &RuleSet) {
        match self
            .substrate
            .entities
            .get(id)
            .map(|entity| entity.category)
        {
            Some(EntityCategory::Unit) => {
                self.unit_enter_idle_mode(id, Some(rules), false);
            }
            Some(EntityCategory::Infantry) => {
                crate::sim::world::queue_foot_enter_idle_mode(self, id, rules);
            }
            _ => {}
        }
    }
}
