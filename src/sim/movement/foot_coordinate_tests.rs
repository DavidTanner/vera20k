use super::*;
use crate::map::entities::EntityCategory;
use crate::map::tube_facts::{TubeFact, TubeId};
use crate::sim::components::{DriveLocomotionRuntime, NavTargetRef, ShipLocomotionRuntime};
use crate::sim::movement::locomotor::LocomotorState;
use crate::sim::movement::tube_movement::LowBridgeTubeMovementState;
use crate::util::fixed_math::SimFixed;
use serde_json::Value;

fn coord(v: &Value) -> DriveCoord {
    DriveCoord {
        x: v[0].as_i64().unwrap() as i32,
        y: v[1].as_i64().unwrap() as i32,
        z: v[2].as_i64().unwrap() as i32,
    }
}

fn entity(id: u64, kind: LocomotorKind, current: DriveCoord, head: DriveCoord) -> GameEntity {
    let mut e = GameEntity::test_default(id, "E1", "Americans", 0, 0);
    e.category = if kind == LocomotorKind::Walk {
        EntityCategory::Infantry
    } else {
        EntityCategory::Unit
    };
    e.position.rx = (current.x / 256).clamp(0, i32::from(u16::MAX)) as u16;
    e.position.ry = (current.y / 256).clamp(0, i32::from(u16::MAX)) as u16;
    e.position.sub_x = SimFixed::from_num(current.x - i32::from(e.position.rx) * 256);
    e.position.sub_y = SimFixed::from_num(current.y - i32::from(e.position.ry) * 256);
    e.position.exact_z_leptons = Some(current.z);
    e.locomotor = Some(LocomotorState::for_test_kind(kind));
    match kind {
        LocomotorKind::Drive => {
            e.drive_locomotion = Some(DriveLocomotionRuntime {
                head_to: Some(head),
                ..Default::default()
            })
        }
        LocomotorKind::Ship => {
            e.ship_locomotion = Some(ShipLocomotionRuntime {
                head_to: Some(head),
                ..Default::default()
            })
        }
        LocomotorKind::Walk | LocomotorKind::Hover => {
            e.locomotor.as_mut().unwrap().set_step_head(Some(head))
        }
        _ => {}
    }
    e
}

#[test]
fn world_queries_match_all_original_foot_coordinate_rows_and_snapshot() {
    let rows: Value = serde_json::from_str(include_str!(
        "../../../tools/spatial_oracle/foot_navigation_coordinate.json"
    ))
    .unwrap();
    assert_eq!(rows.as_array().unwrap().len(), 44);
    for row in rows.as_array().unwrap() {
        let input = &row["input"];
        let kind = match input["family"].as_str().unwrap() {
            "drive" => LocomotorKind::Drive,
            "ship" => LocomotorKind::Ship,
            "walk" => LocomotorKind::Walk,
            "hover" => LocomotorKind::Hover,
            _ => unreachable!(),
        };
        let mut sim = Simulation::new();
        sim.interner = crate::sim::intern::test_interner();
        let mut e = entity(
            1,
            kind,
            coord(&input["current"]),
            coord(&input["stored_head"]),
        );
        let track = e
            .drive_locomotion
            .as_mut()
            .map(|s| &mut s.track)
            .or_else(|| e.ship_locomotion.as_mut().map(|s| &mut s.track));
        if let Some(track) = track {
            track.turn_index = input["turn_index"].as_i64().unwrap() as i32;
            track.cursor = input["cursor"].as_i64().unwrap() as i32;
        }
        if let Some(tube) = input["tube"].as_array() {
            let exit = (
                tube[0].as_i64().unwrap() as u16,
                tube[1].as_i64().unwrap() as u16,
            );
            sim.resolved_terrain = Some(ResolvedTerrainGrid::from_cells_with_tubes(
                1,
                1,
                vec![],
                vec![TubeFact::explicit((0, 0), exit, 0, vec![0])],
            ));
            e.low_bridge_tube_state = Some(LowBridgeTubeMovementState {
                tube_id: TubeId(0),
                cursor: 0,
                target: NULL_COORD,
            });
        }
        sim.substrate.entities.insert(e);
        assert_eq!(
            sim.foot_navigation_coordinate(1).unwrap(),
            coord(&row["coordinate"]),
            "{input}"
        );
        let bytes = crate::sim::snapshot::GameSnapshot::save(&sim, 1, 0, "Foot coordinate", 0);
        let mut restored = crate::sim::snapshot::GameSnapshot::load(&bytes)
            .unwrap()
            .sim;
        if let Some(terrain) = sim.resolved_terrain.as_ref() {
            // Tube descriptors belong to the immutable map, rehydrated through
            // the production cache rebuild. Only the Foot transit state is saved.
            restored.rebuild_caches_after_load(
                terrain.clone(),
                crate::sim::pathfinding::terrain_speed::TerrainSpeedConfig::default(),
                &crate::sim::runtime::SimResources::empty().rules,
            );
        }
        assert_eq!(
            restored.foot_navigation_coordinate(1).unwrap(),
            coord(&row["coordinate"]),
            "restored {input}"
        );
        if input["tube"].is_array() {
            let e = sim.substrate.entities.get_mut(1).unwrap();
            e.lifecycle.object_alive = false;
            e.lifecycle.in_limbo = true;
            e.locomotor = None;
            assert_eq!(
                sim.foot_navigation_coordinate(1).unwrap(),
                coord(&row["coordinate"])
            );
        }
    }
}

#[test]
fn hover_projects_altitude_once_and_at_coord_ignores_tube_exit() {
    let raw = DriveCoord::cell(5, 5, 37);
    let mut e = entity(1, LocomotorKind::Hover, raw, NULL_COORD);
    e.locomotor.as_mut().unwrap().altitude = SimFixed::from_num(125);
    assert_eq!(navigation_coordinate(&e, None).unwrap().z, 37);
    // Exact Object Z already includes height. Only a legacy coarse position
    // needs the independent controller displacement projected into it.
    e.position.exact_z_leptons = None;
    e.position.z = 1;
    assert_eq!(navigation_coordinate(&e, None).unwrap().z, 229);
    e.low_bridge_tube_state = Some(LowBridgeTubeMovementState {
        tube_id: TubeId(0),
        cursor: 0,
        target: NULL_COORD,
    });
    let terrain = ResolvedTerrainGrid::from_cells_with_tubes(
        1,
        1,
        vec![],
        vec![TubeFact::explicit((0, 0), (9, 10), 0, vec![0])],
    );
    assert_eq!(
        navigation_coordinate(&e, Some(&terrain)).unwrap(),
        DriveCoord::cell(9, 10, 0)
    );
    let at = crate::sim::movement::at_coord::AtCoordQuery::from_entity(&e).unwrap();
    assert_eq!(at.head_z(), 229);
    assert!(at.matches(DriveCoord { z: 229, ..raw }));
}

#[test]
fn raw_copy_classes_have_foot_coordinates_but_no_at_coord_match() {
    for kind in [
        LocomotorKind::Fly,
        LocomotorKind::Rocket,
        LocomotorKind::Teleport,
    ] {
        let current = DriveCoord::cell(8, 6, 917);
        let e = entity(1, kind, current, NULL_COORD);
        assert_eq!(navigation_coordinate(&e, None).unwrap(), current);
        assert!(crate::sim::movement::at_coord::AtCoordQuery::from_entity(&e).is_none());
    }
}

#[test]
fn production_drive_reaim_reads_retained_target_head() {
    let mut sim = Simulation::new();
    sim.interner = crate::sim::intern::test_interner();
    let mut mover = entity(
        1,
        LocomotorKind::Drive,
        DriveCoord::cell(3, 4, 0),
        NULL_COORD,
    );
    mover.navigation.nav_com = Some(NavTargetRef::Entity { id: 2 });
    mover.drive_locomotion.as_mut().unwrap().destination = Some(DriveCoord::cell(7, 4, 0));
    let target_head = DriveCoord::cell(8, 4, 416);
    sim.substrate.entities.insert(mover);
    sim.substrate.entities.insert(entity(
        2,
        LocomotorKind::Walk,
        DriveCoord::cell(7, 4, 0),
        target_head,
    ));
    let destination = |sim: &Simulation| {
        sim.substrate
            .entities
            .get(1)
            .unwrap()
            .drive_locomotion
            .as_ref()
            .unwrap()
            .destination
    };
    // Drive 0x4B05D0 is reached only after a track end in the same Process;
    // an ordinary visit leaves +34 alone.
    sim.process_ground_locomotor_for_test(1, None, None, None)
        .unwrap();
    assert_eq!(destination(&sim), Some(DriveCoord::cell(7, 4, 0)));
    assert!(
        sim.begin_track_end_continuation(
            1,
            crate::sim::movement::track_process::TrackFamily::Drive,
            None
        )
        .unwrap()
    );
    assert_eq!(destination(&sim), Some(target_head));
}

#[test]
fn missing_foot_receiver_is_an_error_and_tube_does_not_hide_missing_descriptor() {
    let mut e = entity(1, LocomotorKind::Drive, NULL_COORD, NULL_COORD);
    e.locomotor = None;
    assert!(navigation_coordinate(&e, None).is_err());
    e.low_bridge_tube_state = Some(LowBridgeTubeMovementState {
        tube_id: TubeId(0),
        cursor: 0,
        target: NULL_COORD,
    });
    e.locomotor = None;
    assert!(
        navigation_coordinate(&e, None)
            .unwrap_err()
            .contains("TubeClass")
    );
}

#[test]
fn flight_queries_follow_live_altitude_producers_and_keep_jumpjet_exact_z() {
    use crate::sim::movement::{air_movement, rocket_movement};
    let mut sim = Simulation::new();
    sim.interner = crate::sim::intern::test_interner();
    let base = DriveCoord::cell(8, 6, 104);
    // Fly reads GetHeight against actual terrain. Rocket still owns its
    // independent displacement controller and receives the same starting Z.
    let terrain = crate::map::resolved_terrain::ResolvedTerrainGrid::from_cells(
        16,
        16,
        (0..16)
            .flat_map(|ry| {
                (0..16).map(move |rx| {
                    crate::sim::world::common_raw_test_terrain_cell(rx, ry, 1, false)
                })
            })
            .collect(),
    );
    assert_eq!(
        super::super::ground_pose::ground_surface_z_at(
            [base.x, base.y],
            false,
            Some(&terrain),
            None,
        ),
        Some(104)
    );
    let mut fly = entity(1, LocomotorKind::Fly, base, NULL_COORD);
    let loco = fly.locomotor.as_mut().unwrap();

    loco.set_fly_target_height(1000);
    sim.substrate.entities.insert(fly);
    sim.substrate
        .entities
        .insert(entity(2, LocomotorKind::Rocket, base, NULL_COORD));
    assert!(rocket_movement::attach_rocket_state(
        &mut sim.substrate.entities,
        2,
        (8, 6),
        (18, 6),
        SimFixed::from_num(250)
    ));
    sim.substrate
        .entities
        .get_mut(2)
        .unwrap()
        .rocket_state
        .as_mut()
        .unwrap()
        .phase = rocket_movement::RocketPhase::Ascent;
    air_movement::tick_air_movement(
        &mut sim.substrate.entities,
        &[1],
        1,
        1,
        Some(&terrain),
        None,
    );
    rocket_movement::process_rocket(sim.substrate.entities.get_mut(2).unwrap(), 1);
    for id in [1, 2] {
        let e = sim.substrate.entities.get(id).unwrap();
        let altitude = if id == 1 {
            e.locomotor.as_ref().unwrap().altitude
        } else {
            e.rocket_state.as_ref().unwrap().altitude
        };
        assert!(altitude > SimFixed::from_num(0));
        assert_eq!(
            sim.foot_navigation_coordinate(id).unwrap().z,
            104 + altitude.to_num::<i32>()
        );
    }
    let mut jumpjet = entity(
        3,
        LocomotorKind::Jumpjet,
        DriveCoord { z: 604, ..base },
        NULL_COORD,
    );
    jumpjet.locomotor.as_mut().unwrap().altitude = SimFixed::from_num(500);
    assert_eq!(navigation_coordinate(&jumpjet, None).unwrap().z, 604);
}
