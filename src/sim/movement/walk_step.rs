//! Paid Walk75BFA9..75C0CB step, between admission/completion and placement.
use crate::map::resolved_terrain::ResolvedTerrainGrid;
use crate::sim::game_entity::GameEntity;
use crate::sim::pathfinding::PathGrid;
use crate::util::fixed_math::{SIM_ONE, SIM_ZERO, SimFixed};

/// Walk75BCE3..75BD16/75C1EA: with no head or destination, a Process visit
/// stops a positive Foot speed and clears +68A. Refusal and Stop owners do not
/// share this tail. Native controls: foot_scold_latch.json paid_tails.
pub(super) fn finish_idle(entity: &mut GameEntity) {
    if !entity.locomotor.as_ref().is_some_and(|loco| {
        loco.kind == crate::rules::locomotor_type::LocomotorKind::Walk
            && loco.step_head().is_none()
            && loco.walk_destination().is_none()
    }) {
        return;
    }
    if entity.foot_speed.applied_fraction > SIM_ZERO {
        entity.foot_speed.applied_fraction = SIM_ZERO;
    }
    entity.navigation.path_runtime.clear_scold_latch();
}

/// Publish Foot speed/facing and the provisional numeric step. The caller owns
/// the ordinary completion and boundary transactions; same-cell height is paid
/// here before that continuation. No new retained movement representation.
pub(super) fn advance(
    entity: &mut GameEntity,
    adjusted_speed: SimFixed,
    prone_crawls: Option<bool>,
    native_frame: u32,
    terrain: Option<&ResolvedTerrainGrid>,
    grid: Option<&PathGrid>,
) {
    let head = entity
        .locomotor
        .as_ref()
        .and_then(|loco| loco.step_head())
        .expect("paid Walk requires its admitted head");
    entity.foot_speed.applied_fraction = SIM_ONE; // owner +578 / Foot4D3710
    let speed = super::foot_speed::owner_current_speed_from_fraction(
        adjusted_speed,
        entity.foot_speed.applied_fraction,
    );
    entity.foot_speed.cached_current_speed = speed;
    let speed = prone_crawls.map_or(speed, |crawls| {
        crate::sim::infantry::apply_prone_speed(SimFixed::from_num(speed), crawls).to_num::<i32>()
    });
    entity.navigation.path_runtime.path_blocked = false; //75BFD1; retain grace timer.
    if let Some(target) = entity.movement_target.as_mut() {
        target.current_speed = adjusted_speed;
    }
    let current = super::ground_pose::position_world_xy(&entity.position);
    let desired = crate::util::direction_tables::facing16_from_delta(
        head.x.wrapping_sub(current[0]),
        head.y.wrapping_sub(current[1]),
    );
    // Walk75AE00 snaps the body, but SI still supplies the desired direction to
    // the displacement even if snap's equality branch retains an old target.
    entity.body_facing.snap(desired, native_frame);
    let proposed = crate::util::native_trig::facing_step_world_xy(current, desired, speed);
    entity.position.sub_x = SimFixed::from_num(proposed[0] - i32::from(entity.position.rx) * 256);
    entity.position.sub_y = SimFixed::from_num(proposed[1] - i32::from(entity.position.ry) * 256);
    if proposed[0] / 256 == i32::from(entity.position.rx)
        && proposed[1] / 256 == i32::from(entity.position.ry)
    {
        super::ground_pose::commit_ground_height(
            &mut entity.position,
            entity.on_bridge,
            terrain,
            grid,
        );
        //75C22F follows the same-cell SetCoords/SetHeight callbacks. A
        //boundary step retains the byte until its world placement completes.
        entity.navigation.path_runtime.clear_scold_latch();
    }
}

#[cfg(test)]
#[path = "walk_step_tests.rs"]
mod tests;
