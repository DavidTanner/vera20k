//! ProcessMovement publishes the class target; TrackProcess consumes it.
//! This fixed-point scaffold still lacks native double precision, raw ramp
//! speed/full-XYZ distance, and the Foot getter's house/CTF inputs.
//! Exact numeric helpers remain in track_speed_native pending owner migration.

use crate::map::resolved_terrain::ResolvedTerrainGrid;
use crate::rules::locomotor_type::LocomotorKind;
use crate::rules::object_type::ObjectType;
use crate::rules::ruleset::RuleSet;
use crate::sim::game_entity::GameEntity;
use crate::sim::pathfinding::PathGrid;
use crate::sim::pathfinding::terrain_speed::TerrainSpeedConfig;
use crate::util::fixed_math::{SIM_ONE, SIM_ZERO, SimFixed, isqrt_i64};

/// The native Process_Movement fresh arm's publication for its candidate
/// Cell (`track_fresh`), with Road's row when the retained height is two or
/// more levels from the Cell (`0x4B3C84`).
pub(super) fn publish_fresh_track_target(
    entity: &mut GameEntity,
    strength: i32,
    rules: &RuleSet,
    terrain: &ResolvedTerrainGrid,
    terrain_speed: &TerrainSpeedConfig,
    next_cell: (u16, u16),
    road: bool,
) {
    let Some(loco) = entity.locomotor.as_ref() else {
        return;
    };
    let speed_type = loco.speed_type;
    let below_yellow = crate::sim::pathfinding::terrain_speed::is_at_or_below_condition_yellow(
        entity.health.current,
        strength,
        rules.general.condition_yellow,
    );
    let road_row = road.then(|| {
        rules
            .terrain_rules
            .semantics_for_land_type(1)
            .map_or(SIM_ONE, |road| {
                road.speed_costs.speed_multiplier_for(speed_type)
            })
    });
    let xy = super::ground_pose::position_world_xy(&entity.position);
    let requested = crate::sim::pathfinding::terrain_speed::fresh_track_speed_fraction(
        speed_type,
        (xy[0], xy[1]),
        next_cell,
        road_row,
        terrain,
        terrain_speed,
        below_yellow,
    );
    publish_target_fraction(entity, requested);
}

/// Drive4B3DFA..3E21 / Ship twin: a selector below 64 keeps the request on
/// the class target (+50); otherwise it goes to the Foot setter
/// (`SetSpeedFraction`4D3710) when it differs from the applied fraction.
fn publish_target_fraction(entity: &mut GameEntity, requested: SimFixed) {
    let Some(kind) = entity.locomotor.as_ref().map(|loco| loco.kind) else {
        return;
    };
    let (selector, retained) = match kind {
        LocomotorKind::Drive => {
            let state = entity.drive_locomotion.get_or_insert_with(Default::default);
            (state.track.turn_index, &mut state.target_speed_fraction)
        }
        LocomotorKind::Ship => {
            let state = entity.ship_locomotion.get_or_insert_with(Default::default);
            (state.track.turn_index, &mut state.target_speed_fraction)
        }
        _ => return,
    };
    if selector < 64 {
        *retained = requested;
    } else if entity.foot_speed.applied_fraction() != requested {
        entity.foot_speed.set_speed_fraction(requested);
    }
}

/// One TrackProcess invocation consumes its retained class target, even when
/// callbacks have changed the path, terrain, health, or destination request.
pub(super) fn advance(
    entity: &mut GameEntity,
    object: Option<&ObjectType>,
    rules: Option<&RuleSet>,
    grid: Option<&PathGrid>,
) -> i32 {
    let Some(loco) = entity.locomotor.as_ref() else {
        return 0;
    };
    let kind = loco.kind;
    if !matches!(kind, LocomotorKind::Drive | LocomotorKind::Ship) {
        return 0;
    }
    let speed = super::foot_speed::adjusted_speed(
        entity,
        object,
        rules.map_or(1.0, |r| r.general.veteran_speed),
    );
    let target = entity.movement_target.as_ref();
    let class_goal = if kind == LocomotorKind::Drive {
        entity
            .drive_locomotion
            .as_ref()
            .and_then(|d| d.destination.or(d.head_to))
    } else {
        entity
            .ship_locomotion
            .as_ref()
            .and_then(|d| d.destination.or(d.head_to))
    };
    let goal = target.and_then(|t| t.final_goal.or_else(|| t.path.last().copied()));
    let current = super::ground_pose::position_world_xy(&entity.position);
    let goal_xy = goal
        .map(|(x, y)| [i32::from(x) * 256 + 128, i32::from(y) * 256 + 128])
        .or_else(|| class_goal.map(|c| [c.x, c.y]))
        .unwrap_or(current);
    let dx = i64::from(goal_xy[0]) - i64::from(current[0]);
    let dy = i64::from(goal_xy[1]) - i64::from(current[1]);
    let mut distance = SimFixed::from_num(isqrt_i64(dx * dx + dy * dy) as i32);
    if loco.movement_zone.is_water_mover()
        && grid
            .and_then(|g| g.cell(entity.position.rx, entity.position.ry))
            .is_some_and(|cell| cell.bridge_deck_level_if_any().is_some())
    {
        distance += SimFixed::from_num(crate::util::lepton::BRIDGE_DECK_HEIGHT_LEPTONS);
    }
    let accel = object.map_or(SIM_ZERO, |o| o.accel_factor);
    let decel = object.map_or(SIM_ZERO, |o| o.decel_factor);
    let slowdown = object.map_or(SIM_ZERO, |o| SimFixed::from_num(o.slowdown_distance));
    let unit_passive = entity.category == crate::map::entities::EntityCategory::Unit
        && object.is_some_and(|object| object.passive);
    match kind {
        LocomotorKind::Drive => {
            if let Some(drive) = entity.drive_locomotion.as_ref() {
                super::drive_locomotion::update_drive_speed_fraction(
                    drive,
                    &mut entity.foot_speed,
                    entity.drive_accelerates,
                    unit_passive,
                    speed / SimFixed::from_num(15),
                    accel,
                    decel,
                    slowdown,
                    distance,
                );
            }
        }
        LocomotorKind::Ship => {
            if let Some(ship) = entity.ship_locomotion.as_ref() {
                super::drive_locomotion::update_ship_speed_fraction(
                    ship,
                    &mut entity.foot_speed,
                    entity.drive_accelerates,
                    unit_passive,
                    speed / SimFixed::from_num(15),
                    accel,
                    decel,
                    slowdown,
                    distance,
                );
            }
        }
        _ => unreachable!(),
    }
    super::foot_speed::owner_current_speed_from_fraction(
        speed,
        entity.foot_speed.applied_fraction(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::ini_parser::IniFile;
    use crate::sim::components::{DriveLocomotionRuntime, ShipLocomotionRuntime};
    use crate::sim::movement::locomotor::LocomotorState;

    fn entity(kind: LocomotorKind, selector: i32, target: SimFixed) -> GameEntity {
        let mut entity = GameEntity::test_default(1, "MTNK", "Americans", 8, 8);
        entity.locomotor = Some(LocomotorState::for_test_kind(kind));
        match kind {
            LocomotorKind::Drive => {
                let mut state = DriveLocomotionRuntime::default();
                state.track.turn_index = selector;
                state.track_valid = true;
                state.target_speed_fraction = target;
                entity.drive_locomotion = Some(state);
            }
            LocomotorKind::Ship => {
                let mut state = ShipLocomotionRuntime::default();
                state.track.turn_index = selector;
                state.track_valid = true;
                state.target_speed_fraction = target;
                entity.ship_locomotion = Some(state);
            }
            _ => unreachable!(),
        }
        entity
    }

    fn retained(entity: &GameEntity) -> SimFixed {
        match entity.locomotor.as_ref().unwrap().kind {
            LocomotorKind::Drive => {
                entity
                    .drive_locomotion
                    .as_ref()
                    .unwrap()
                    .target_speed_fraction
            }
            LocomotorKind::Ship => {
                entity
                    .ship_locomotion
                    .as_ref()
                    .unwrap()
                    .target_speed_fraction
            }
            _ => unreachable!(),
        }
    }

    #[test]
    fn retained_track_reads_new_crate_factor_without_a_new_move_order() {
        let rules = RuleSet::from_ini(&IniFile::from_str(
            "[VehicleTypes]\n0=MTNK\n[MTNK]\nSpeed=4\nStrength=100\n",
        ))
        .unwrap();
        let object = rules.object("MTNK").unwrap();
        for kind in [LocomotorKind::Drive, LocomotorKind::Ship] {
            let mut mover = entity(kind, 0, SIM_ONE);
            mover.drive_accelerates = false;
            mover.movement_target = Some(crate::sim::components::MovementTarget {
                speed: SimFixed::from_num(150),
                ..Default::default()
            });
            assert_eq!(advance(&mut mover, Some(object), Some(&rules), None), 10);
            assert!(mover.foot_speed.accept_speed_crate(
                crate::util::native_x87::NativeF64Bits::from_bits(1.2_f64.to_bits())
            ));
            assert_eq!(advance(&mut mover, Some(object), Some(&rules), None), 11);
            assert_eq!(
                mover.movement_target.as_ref().unwrap().speed,
                SimFixed::from_num(150)
            );
        }
    }

    #[test]
    fn live_type_speed_wins_over_the_path_speed_cache() {
        let rules = RuleSet::from_ini(&IniFile::from_str(
            "[VehicleTypes]\n0=MTNK\n[MTNK]\nStrength=100\nSpeed=6\nAccelerates=no\n",
        ))
        .unwrap();
        for kind in [LocomotorKind::Drive, LocomotorKind::Ship] {
            for cached_speed in [15, 1500] {
                let mut mover = entity(kind, 0, SIM_ONE);
                mover.drive_accelerates = false;
                mover.movement_target = Some(crate::sim::components::MovementTarget {
                    speed: SimFixed::from_num(cached_speed),
                    ..Default::default()
                });
                assert_eq!(
                    advance(&mut mover, rules.object("MTNK"), Some(&rules), None),
                    15
                );
            }
        }
    }

    #[test]
    fn production_prefix_passive_special_and_nonaccelerating_gates_match_original() {
        let corpus: serde_json::Value = serde_json::from_str(include_str!(
            "../../../tools/spatial_oracle/track_speed_native.json"
        ))
        .unwrap();
        let fixed = |value: &serde_json::Value| {
            let bits = u64::from_str_radix(value.as_str().unwrap(), 16).unwrap();
            let native = f64::from_bits(bits);
            let fixed = SimFixed::from_num(native);
            assert_eq!(
                fixed.to_num::<f64>(),
                native,
                "gate case must be exactly representable"
            );
            fixed
        };
        let mut compared = [0; 2];
        for case in corpus["prefixes"].as_array().unwrap() {
            let input = &case["input"];
            let accelerates = input["accelerates"].as_bool().unwrap_or(true);
            let passive = input["passive"].as_bool().unwrap_or(false);
            let selector = input["selector"].as_i64().unwrap_or(1) as i32;
            if accelerates && !passive && selector < 64 {
                continue;
            }
            let kind = if input["family"] == "drive" {
                LocomotorKind::Drive
            } else {
                LocomotorKind::Ship
            };
            let rules = RuleSet::from_ini(&IniFile::from_str(&format!(
                "[VehicleTypes]\n0=MTNK\n[MTNK]\nStrength=100\nSpeed=6\nPassive={}\n",
                if passive { "yes" } else { "no" },
            )))
            .unwrap();
            let mut entity = entity(kind, selector, fixed(&input["target_bits"]));
            entity.drive_accelerates = accelerates;
            entity
                .foot_speed
                .set_speed_fraction(fixed(&input["applied_bits"]));
            advance(&mut entity, rules.object("MTNK"), Some(&rules), None);
            assert_eq!(
                retained(&entity),
                fixed(&case["output"]["target_bits"]),
                "{input}"
            );
            assert_eq!(
                entity.foot_speed.applied_fraction(),
                fixed(&case["output"]["applied_bits"]),
                "{input}"
            );
            compared[usize::from(kind == LocomotorKind::Ship)] += 1;
        }
        assert_eq!(compared, [14, 14]);
    }
}
