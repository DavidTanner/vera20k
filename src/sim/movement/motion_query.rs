//! The active locomotor's two motion queries over existing retained state:
//! ILocomotion+10 `Is_Moving` ([`is_moving`]) and +80 `Is_Moving_Now`
//! ([`is_moving_now`]). Native callers dispatch one slot or the other, and
//! the two answer differently in every family except Teleport. No order/path
//! presence, movement phase or speed is substituted for a native query here.
use super::track_process::TrackFamily;
use crate::rules::locomotor_type::LocomotorKind;
use crate::sim::game_entity::GameEntity;

/// Drive4AFB80, Ship69F290, Walk75AB30, Fly4CCA90, Jumpjet54AE50 and
/// Hover514C30.
/// Evidence: locomotor_moving and air_locomotor_moving native corpora.
pub(crate) fn is_moving(entity: &GameEntity) -> Option<bool> {
    let locomotor = entity.locomotor.as_ref()?;
    match locomotor.active_kind() {
        LocomotorKind::Drive => Some(super::track_head::motion_state(entity, TrackFamily::Drive).0),
        LocomotorKind::Ship => Some(super::track_head::motion_state(entity, TrackFamily::Ship).0),
        LocomotorKind::Walk => locomotor.walk_is_moving(),
        LocomotorKind::Fly => locomotor
            .fly_runtime()
            .map(|state| state.moving() || entity.flight_attitude.blocks_landing()),
        LocomotorKind::Jumpjet => locomotor.jumpjet_runtime().map(|state| state.moving),
        LocomotorKind::Hover => locomotor
            .hover_runtime()
            .map(super::hover::HoverRuntime::is_moving),
        // Teleport Is_Moving 0x718080 reads the +0x30 request byte: Move_To
        // raises it and the warp's Stop_Moving (0x00719725) clears it, so it
        // is up only for an armed warp not yet processed.
        LocomotorKind::Teleport => {
            Some(entity.teleport_state.as_ref().is_some_and(|state| {
                state.phase == super::teleport_movement::TeleportPhase::Relocate
            }))
        }
        // Rocket destination storage still requires its native
        // producer/lifecycle migration.
        _ => None,
    }
}

/// `Is_Moving_Now` (ILocomotion+0x80) of the active locomotor: Fly
/// `0x004CCAC0`, Rocket `0x00661F90`, and the readiness families of
/// [`super::ready_producer::ready_state_for`] (Drive `0x004AFC20`, Ship,
/// Walk, Hover `0x00514C80`, Teleport and Jumpjet `0x0054D0D0`). No locomotor
/// answers false. `rules` gives Drive and Ship the owner's speed getter.
pub(crate) fn is_moving_now(
    entity: &GameEntity,
    rules: Option<super::SpeedRules<'_>>,
    binary_frame: u32,
) -> bool {
    let Some(locomotor) = entity.locomotor.as_ref() else {
        return false;
    };
    match locomotor.active_kind() {
        LocomotorKind::Fly => locomotor
            .fly_runtime()
            .is_some_and(super::fly_height::FlyRuntime::is_moving_now),
        LocomotorKind::Rocket => entity
            .rocket_state
            .as_ref()
            .is_some_and(|rocket| rocket.phase.is_moving_now()),
        _ => super::ready_producer::ready_state_for(entity, rules, binary_frame)
            .is_some_and(super::locomotor_ready::LocomotorReadyState::is_moving_now),
    }
}

#[cfg(test)]
#[path = "motion_query_tests.rs"]
pub(crate) mod tests;
