//! ProcessMovement publishes the class target; TrackProcess consumes it.
//! The speed prefix works in `SimFixed`, not native binary64 (see
//! `drive_locomotion::apply_track_speed_prefix`).

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

/// Drive 0x004B0FBA..0x004B1087 / Ship 0x006A068A..0x006A0757: the 3-D
/// distance from the raw Foot coordinate (+0x9C) to the class destination
/// (+0x34). The destination's own Z is replaced (0x004B101D) by the ground
/// under its XY (0x00578080, slope-interpolated) plus the bridge height when
/// Map[destination] (0x00565730) has the structural flag 0x100. A null
/// destination is looked up as (0, 0); there is no null check.
///
/// PRECISION: native sums binary64 squares, takes `Sqrt_Approx` (0x004CAC40,
/// binary32) and truncates. The integer square root floors, so the two differ
/// by one lepton where binary32 rounding reaches the next integer, which
/// matters only at exactly SlowdownDistance.
///
/// RESIDUAL: Ship adds the global at 0xB0782C, Drive the one at 0x8A07C4;
/// both are taken as the deck height (the executed rows supply 416 for both).
fn braking_distance(
    entity: &GameEntity,
    destination: Option<crate::sim::components::DriveCoord>,
    terrain: Option<&ResolvedTerrainGrid>,
    grid: Option<&PathGrid>,
) -> i32 {
    let destination =
        destination.unwrap_or(crate::sim::components::DriveCoord { x: 0, y: 0, z: 0 });
    let structural = crate::sim::cell_rect::get_cellclass_fallback_leptons(
        terrain,
        destination.x,
        destination.y,
    )
    .bridge_flags_0x1180()
        & crate::map::bridge_facts::BRIDGE_FLAG_STRUCTURAL
        != 0;
    // Headless fixtures without map cells read a level-0 ground.
    let ground = super::ground_pose::ground_surface_z_at(
        [destination.x, destination.y],
        false,
        terrain,
        grid,
    )
    .unwrap_or(0);
    let destination_z = super::ground_pose::z_at_height(ground, 0, structural);
    let current = super::ground_pose::position_world_coord(&entity.position);
    let delta = |a: i32, b: i32| i64::from(a.wrapping_sub(b));
    let (dx, dy, dz) = (
        delta(current.x, destination.x),
        delta(current.y, destination.y),
        delta(current.z, destination_z),
    );
    isqrt_i64(dx * dx + dy * dy + dz * dz) as i32
}

/// One TrackProcess invocation consumes its retained class target, even when
/// callbacks have changed the path, terrain, health, or destination request.
pub(super) fn advance(
    entity: &mut GameEntity,
    object: Option<&ObjectType>,
    rules: Option<&RuleSet>,
    terrain: Option<&ResolvedTerrainGrid>,
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
    let destination = match kind {
        LocomotorKind::Drive => entity.drive_locomotion.as_ref().and_then(|d| d.destination),
        _ => entity.ship_locomotion.as_ref().and_then(|s| s.destination),
    };
    let prefix = super::drive_locomotion::TrackSpeedPrefix {
        accelerates: entity.drive_accelerates,
        unit_passive: entity.category == crate::map::entities::EntityCategory::Unit
            && object.is_some_and(|object| object.passive),
        selector: match kind {
            LocomotorKind::Drive => entity.drive_locomotion.as_ref().map(|d| d.track.turn_index),
            _ => entity.ship_locomotion.as_ref().map(|s| s.track.turn_index),
        }
        .unwrap_or(-1),
        raw_type_speed: object.map_or(0, |o| {
            crate::util::fixed_math::ra2_speed_to_leptons_per_frame(o.speed)
        }),
        accel: object.map_or(SIM_ZERO, |o| o.accel_factor),
        decel: object.map_or(SIM_ZERO, |o| o.decel_factor),
        slowdown_distance: object.map_or(0, |o| o.slowdown_distance),
        distance: braking_distance(entity, destination, terrain, grid),
        sinking: entity.sinking.is_active(),
        // RESIDUAL: Foot+0x6B5's producers are not ported, so the crush
        // slowdown never caps the target.
        crush_slowdown: false,
    };
    let target = match kind {
        LocomotorKind::Drive => entity
            .drive_locomotion
            .as_mut()
            .map(|d| &mut d.target_speed_fraction),
        _ => entity
            .ship_locomotion
            .as_mut()
            .map(|s| &mut s.target_speed_fraction),
    };
    if let Some(target) = target {
        super::drive_locomotion::apply_track_speed_prefix(&prefix, target, &mut entity.foot_speed);
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
            assert_eq!(
                advance(&mut mover, Some(object), Some(&rules), None, None),
                10
            );
            assert!(mover.foot_speed.accept_speed_crate(
                crate::util::native_x87::NativeF64Bits::from_bits(1.2_f64.to_bits())
            ));
            assert_eq!(
                advance(&mut mover, Some(object), Some(&rules), None, None),
                11
            );
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
                    advance(&mut mover, rules.object("MTNK"), Some(&rules), None, None),
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
            advance(&mut entity, rules.object("MTNK"), Some(&rules), None, None);
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

    fn native_f64(value: &serde_json::Value) -> f64 {
        f64::from_bits(u64::from_str_radix(value.as_str().unwrap(), 16).unwrap())
    }

    /// Drive 0x004B1087 / Ship 0x006A0757 observed distances over a flat
    /// level-2 map (ground 208), the destination cell structural when a row
    /// says `bridge`. The destination's own Z never matters.
    #[test]
    fn braking_distance_matches_original_rows() {
        let corpus: serde_json::Value = serde_json::from_str(include_str!(
            "../../../tools/spatial_oracle/track_speed_native.json"
        ))
        .unwrap();
        let mut compared = 0;
        for case in corpus["prefixes"].as_array().unwrap() {
            let Some(native) = case["output"]["distance"].as_i64() else {
                continue;
            };
            let input = &case["input"];
            let current: [i32; 3] =
                serde_json::from_value(input["current"].clone()).unwrap_or([2176, 2176, 208]);
            let destination: [i32; 3] =
                serde_json::from_value(input["destination"].clone()).unwrap_or([2688, 2176, -731]);
            let bridge = input["bridge"].as_bool().unwrap_or(false);
            let cells = (0..16)
                .flat_map(|y| {
                    (0..16).map(move |x| {
                        let mut cell = crate::map::resolved_terrain::test_flat_cell(x, y);
                        cell.level = 2;
                        if bridge
                            && (i32::from(x), i32::from(y))
                                == (destination[0] / 256, destination[1] / 256)
                        {
                            cell.bridge_facts.raw_flags |=
                                crate::map::bridge_facts::BRIDGE_FLAG_STRUCTURAL;
                        }
                        cell
                    })
                })
                .collect();
            let terrain = ResolvedTerrainGrid::from_cells(16, 16, cells);
            let mut mover = GameEntity::test_default(1, "MTNK", "Americans", 0, 0);
            crate::sim::movement::ground_pose::set_position_world_xy(
                &mut mover.position,
                [current[0], current[1]],
            );
            mover.position.exact_z_leptons = Some(current[2]);
            let destination = crate::sim::components::DriveCoord {
                x: destination[0],
                y: destination[1],
                z: destination[2],
            };
            assert_eq!(
                braking_distance(&mover, Some(destination), Some(&terrain), None),
                native as i32,
                "{input}"
            );
            compared += 1;
        }
        assert_eq!(compared, 88);
    }

    /// Every executed Drive/Ship prefix row through the one production port,
    /// within `SimFixed` precision (see `apply_track_speed_prefix`).
    #[test]
    fn speed_prefix_matches_original_rows_within_fixed_point() {
        use crate::sim::components::FootSpeedState;
        use crate::sim::movement::drive_locomotion::{TrackSpeedPrefix, apply_track_speed_prefix};
        let corpus: serde_json::Value = serde_json::from_str(include_str!(
            "../../../tools/spatial_oracle/track_speed_native.json"
        ))
        .unwrap();
        let integer = |input: &serde_json::Value, key: &str, fallback: i32| {
            input[key].as_i64().map_or(fallback, |value| value as i32)
        };
        let mut compared = 0;
        for case in corpus["prefixes"].as_array().unwrap() {
            let input = &case["input"];
            let expected = &case["output"];
            let prefix = TrackSpeedPrefix {
                accelerates: input["accelerates"].as_bool().unwrap_or(true),
                unit_passive: input["passive"].as_bool().unwrap_or(false),
                selector: integer(input, "selector", 1),
                raw_type_speed: integer(input, "raw", 17),
                accel: SimFixed::from_num(native_f64(&input["accel_bits"])),
                decel: SimFixed::from_num(native_f64(&input["decel_bits"])),
                slowdown_distance: integer(input, "slowdown", 500),
                // A bypassed row never evaluates geometry.
                distance: expected["distance"].as_i64().map_or(i32::MIN, |d| d as i32),
                sinking: input["sinking"].as_bool().unwrap_or(false),
                crush_slowdown: input["crush"].as_bool().unwrap_or(false),
            };
            let mut target = SimFixed::from_num(native_f64(&input["target_bits"]));
            let mut owner = FootSpeedState::default();
            owner.set_speed_fraction(SimFixed::from_num(native_f64(&input["applied_bits"])));
            apply_track_speed_prefix(&prefix, &mut target, &mut owner);
            // A brake step carries raw speed x half a step of Deceleration=.
            let tolerance = (f64::from(prefix.raw_type_speed) + 2.0) / 131_072.0;
            for (actual, key) in [
                (target, "target_bits"),
                (owner.applied_fraction(), "applied_bits"),
            ] {
                let native = native_f64(&expected[key]);
                assert!(
                    (actual.to_num::<f64>() - native).abs() <= tolerance,
                    "{key}: {actual} vs native {native} for {input}"
                );
            }
            compared += 1;
        }
        assert_eq!(compared, 116);
    }
}
