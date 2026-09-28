use super::*;
use crate::rules::ini_parser::IniFile;
use crate::sim::command::{Command, CommandEnvelope};
use crate::sim::movement::FacingClass;
use crate::sim::world::TickResult;
use std::collections::BTreeMap;

fn fixture(kind: &str, facing: u8, rot: u8, deploy_facing: u8) -> (Simulation, RuleSet, u64) {
    fixture_with_sound(kind, facing, rot, deploy_facing, None)
}

fn fixture_with_sound(
    kind: &str,
    facing: u8,
    rot: u8,
    deploy_facing: u8,
    sound: Option<&str>,
) -> (Simulation, RuleSet, u64) {
    let sound_line = sound.map_or(String::new(), |s| format!("DeploySound={s}\n"));
    let text = format!(
        "[InfantryTypes]\n[AircraftTypes]\n[VehicleTypes]\n0={kind}\n[BuildingTypes]\n0=YARD\n[{kind}]\nStrength=1000\nSpeed=5\nROT={rot}\nLocomotor={{4A582741-9839-11d1-B709-00A024DDAFD1}}\nDeploysInto=YARD\n{sound_line}[YARD]\nStrength=1000\nConstructionYard=yes\nFoundation=4x3\nDeployFacing={deploy_facing}\n[Unload]\nRate=0.016\n[Clear]\nBuildable=yes\n"
    );
    let rules = RuleSet::from_ini(&IniFile::from_str(&text)).unwrap();
    let mut sim = Simulation::new();
    crate::sim::arena_fixture::flat_ground(&mut sim, &rules);
    // No house AI/opponent defeat system in these command/locomotor fixtures.
    let id = sim
        .spawn_object(kind, "Americans", 20, 22, facing, &rules, &BTreeMap::new())
        .unwrap();
    (sim, rules, id)
}
fn tick(sim: &mut Simulation, rules: &RuleSet, command: Option<Command>) -> TickResult {
    let cmds: Vec<_> = command
        .into_iter()
        .map(|cmd| {
            CommandEnvelope::new(
                sim.interner.get("Americans").unwrap(),
                sim.session.tick + 1,
                cmd,
            )
        })
        .collect();
    let grid = sim.path_grid.clone();
    sim.advance_tick(
        &cmds,
        Some(rules),
        &BTreeMap::new(),
        grid.as_deref(),
        None,
        22,
    )
}
fn yards(sim: &Simulation) -> usize {
    sim.substrate
        .entities
        .values()
        .filter(|e| !e.dying && sim.interner.resolve(e.type_ref()) == "YARD")
        .count()
}
fn finish(sim: &mut Simulation, rules: &RuleSet, id: u64) -> usize {
    let mut receipts = 0;
    for _ in 0..140 {
        let out = tick(sim, rules, None);
        receipts += usize::from(out.spawned_entities);
        if sim.substrate.entities.get(id).is_none_or(|e| e.dying) {
            break;
        }
    }
    assert_eq!(
        yards(sim),
        1,
        "retained MCV: {:?}",
        sim.substrate.entities.get(id).map(|e| (
            &e.position,
            &e.navigation,
            &e.drive_locomotion,
            &e.movement_target,
            &e.body_facing,
            e.facing,
            e.facing_target,
            e.mcv_deploy_pending,
            &e.mission,
            &e.foot_speed
        ))
    );
    receipts
}
#[test]
fn one_command_turns_and_converts_all_stock_mcv_types() {
    for kind in ["AMCV", "SMCV", "PCV"] {
        for facing in [0, 32, 64, 127, 128, 192, 255] {
            let (mut sim, rules, id) = fixture(kind, facing, 5, 4);
            let accepted = tick(&mut sim, &rules, Some(Command::DeployMcv { entity_id: id }));
            assert!(!accepted.spawned_entities, "acceptance is not conversion");
            assert_eq!(yards(&sim), 0);
            assert_eq!(finish(&mut sim, &rules, id), 1, "one conversion receipt");
            let yard = sim
                .substrate
                .entities
                .values()
                .find(|e| !e.dying && e.category == EntityCategory::Structure)
                .unwrap();
            assert_eq!((yard.position.rx, yard.position.ry), (19, 21));
        }
    }
}
/// Deploy lifts the MCV (`Mark(UP)`, `0x00739670`) before the yard is built
/// and marked. VERA lifts it after, at its UnInit; the cell the MCV stood on
/// must still end like every other yard cell.
#[test]
fn the_cell_the_mcv_stood_on_ends_as_a_yard_cell() {
    let (mut sim, rules, id) = fixture("AMCV", 128, 5, 4);
    tick(&mut sim, &rules, Some(Command::DeployMcv { entity_id: id }));
    finish(&mut sim, &rules, id);
    let yard = sim
        .substrate
        .entities
        .values()
        .find(|e| !e.dying && e.category == EntityCategory::Structure)
        .unwrap()
        .stable_id();
    let bits = |cell: (u16, u16)| {
        sim.substrate
            .raw_cell_occupation
            .ground_bits(cell.0, cell.1)
    };
    assert_ne!(bits((21, 22)), 0);
    assert_eq!(bits((20, 22)), bits((21, 22)));
    assert!(sim.substrate.occupancy.contains_entity(20, 22, yard));
    assert!(!sim.substrate.occupancy.contains_entity(20, 22, id));
}

/// The yard an MCV deploys into in frame D builds up from its type's Buildup
/// control and completes at D + 1 + (count - 1) * rate: Deploy's ready byte
/// commences the queued Construction mission in D and its first visit is D+1
/// (`sim::building_construction`); the tactical capture ledger pins this
/// route.
#[test]
fn a_deployed_yard_completes_its_buildup_after_the_conversion_frame() {
    let (mut sim, mut rules, id) = fixture("SMCV", 0, 5, 4);
    rules.set_buildup_control_for_test("YARD", [0, 31, 1]);
    tick(&mut sim, &rules, Some(Command::DeployMcv { entity_id: id }));
    let mut converted = None;
    let mut completed = None;
    for _ in 0..200 {
        let frame = sim.session.binary_frame;
        tick(&mut sim, &rules, None);
        let yard = sim
            .substrate
            .entities
            .values()
            .find(|e| !e.dying && e.category == EntityCategory::Structure);
        if let Some(yard) = yard {
            converted.get_or_insert(frame);
            if yard.building_up.is_none() {
                completed = Some(frame);
                break;
            }
        }
    }
    let converted = converted.expect("the MCV converts");
    assert_eq!(completed, Some(converted + 1 + 30));
}

#[test]
fn duplicate_orders_do_not_restart_the_turn_and_zero_rot_still_completes() {
    for rot in [0, 1, 5, 10] {
        let (mut sim, rules, id) = fixture("AMCV", 64, rot, 6);
        tick(&mut sim, &rules, Some(Command::DeployMcv { entity_id: id }));
        let mut receipts = 0;
        for _ in 0..160 {
            let out = tick(&mut sim, &rules, Some(Command::DeployMcv { entity_id: id }));
            receipts += usize::from(out.spawned_entities);
            if yards(&sim) == 1 {
                break;
            }
        }
        assert_eq!((yards(&sim), receipts), (1, 1), "ROT={rot}");
    }
}
#[test]
fn turn_completion_converts_before_the_next_mission_retry() {
    let (mut sim, rules, id) = fixture("AMCV", 64, 10, 4);
    tick(&mut sim, &rules, Some(Command::DeployMcv { entity_id: id }));
    tick(&mut sim, &rules, None);
    let e = sim.substrate.entities.get(id).unwrap();
    assert!(e.mcv_deploy_pending);
    assert_eq!(e.facing, 64);
    let due = e.mission.dispatch_timer();
    for _ in 0..10 {
        let before = sim.session.binary_frame;
        let out = tick(&mut sim, &rules, None);
        if out.spawned_entities {
            assert!(!due.due(before), "conversion is the Drive edge callback");
            assert_eq!(yards(&sim), 1);
            return;
        }
    }
    panic!("turn completion never retried deployment");
}
#[test]
fn native_stop_retains_pending_deployment_and_death_cannot_spawn_a_yard() {
    let (mut sim, rules, id) = fixture("AMCV", 64, 5, 4);
    tick(&mut sim, &rules, Some(Command::DeployMcv { entity_id: id }));
    tick(&mut sim, &rules, None);
    tick(&mut sim, &rules, Some(Command::Stop { entity_id: id }));
    assert_eq!(finish(&mut sim, &rules, id), 1);
    let (mut sim, rules, id) = fixture("AMCV", 64, 5, 4);
    tick(&mut sim, &rules, Some(Command::DeployMcv { entity_id: id }));
    tick(&mut sim, &rules, None);
    sim.uninit_with_rules(id, &rules);
    for _ in 0..50 {
        assert!(!tick(&mut sim, &rules, None).spawned_entities);
    }
    assert_eq!(yards(&sim), 0);
}
#[test]
fn pending_turn_roundtrips_through_save_and_hashes_its_latches() {
    let (mut sim, rules, id) = fixture("AMCV", 0, 5, 4);
    tick(&mut sim, &rules, Some(Command::DeployMcv { entity_id: id }));
    tick(&mut sim, &rules, None);
    let hash = sim.state_hash();
    sim.substrate
        .entities
        .get_mut(id)
        .unwrap()
        .mcv_deploy_pending = false;
    assert_ne!(hash, sim.state_hash());
    sim.substrate
        .entities
        .get_mut(id)
        .unwrap()
        .mcv_deploy_pending = true;
    let data = crate::sim::snapshot::GameSnapshot::save_validated(&sim, 1, 2, "pending MCV", 0);
    let mut restored = crate::sim::snapshot::GameSnapshot::load(&data).unwrap().sim;
    restored.restore_after_snapshot_load().unwrap();
    // A save carries no map: the load re-installs the scenario's cells.
    crate::sim::arena_fixture::flat_ground(&mut restored, &rules);
    // Retail's save reader deliberately reinitializes Scenario RNG. Align the
    // control run to that documented load contract before comparing continuation.
    sim.scenario_rng = crate::sim::rng::SimRng::new(0);
    assert_eq!(sim.state_hash(), restored.state_hash());
    for _ in 0..80 {
        let a = tick(&mut sim, &rules, None);
        let b = tick(&mut restored, &rules, None);
        assert_eq!(a.spawned_entities, b.spawned_entities);
        assert_eq!(sim.state_hash(), restored.state_hash());
    }
    assert_eq!(yards(&restored), 1);
}
#[test]
fn facing_matches_original_drive_oracle() {
    let vectors: serde_json::Value =
        serde_json::from_str(include_str!("../../tools/mcv_deploy_oracle.json")).unwrap();
    for row in vectors["turns"].as_array().unwrap() {
        let rate = row["rate"].as_u64().unwrap() as u16;
        if rate % 256 != 0 {
            continue;
        } // native sub-byte rates are outside rules ROT constructor
        let mut facing =
            FacingClass::new(row["start"].as_u64().unwrap() as u16, (rate / 256) as u8);
        let target = row["target"].as_u64().unwrap() as u16;
        facing.set(target, 100);
        if row["duplicate_at_one"].as_bool().unwrap_or(false) {
            facing.set(target, 101);
        }
        assert_eq!(
            facing.current(100 + row["elapsed"].as_u64().unwrap() as u32),
            row["current"].as_u64().unwrap() as u16,
            "{row}"
        );
    }
}

#[test]
fn moving_mcv_finishes_committed_segment_then_deploys_once() {
    let (mut sim, rules, id) = fixture("AMCV", 64, 5, 4);
    tick(
        &mut sim,
        &rules,
        Some(Command::Move {
            entity_id: id,
            target_rx: 30,
            target_ry: 22,
            queue: false,
        }),
    );
    for _ in 0..8 {
        tick(&mut sim, &rules, None);
    }
    assert!(
        sim.substrate
            .entities
            .get(id)
            .unwrap()
            .movement_target
            .is_some()
    );
    let position = sim.substrate.entities.get(id).unwrap().position.clone();
    tick(&mut sim, &rules, Some(Command::DeployMcv { entity_id: id }));
    assert_eq!(yards(&sim), 0);
    assert_eq!(finish(&mut sim, &rules, id), 1);
    let yard = sim
        .substrate
        .entities
        .values()
        .find(|e| !e.dying && e.category == EntityCategory::Structure)
        .unwrap();
    assert!(yard.position.rx < 29, "must abandon the old destination");
    assert!(
        yard.position.rx.abs_diff(position.rx) <= 3,
        "finish the committed segment only"
    );
}
#[test]
fn placement_is_rechecked_on_turn_completion() {
    let (mut sim, rules, id) = fixture("AMCV", 64, 5, 4);
    tick(&mut sim, &rules, Some(Command::DeployMcv { entity_id: id }));
    tick(&mut sim, &rules, None);
    // A structure arrives after the initial attempt has already accepted the turn.
    let blocker = sim
        .spawn_object("YARD", "Americans", 19, 21, 0, &rules, &BTreeMap::new())
        .unwrap();
    for _ in 0..35 {
        tick(&mut sim, &rules, None);
    }
    let mcv = sim
        .substrate
        .entities
        .get(id)
        .expect("blocked MCV survives");
    assert!(!mcv.dying && !mcv.mcv_deploy_pending);
    assert_eq!(yards(&sim), 1, "only the blocking building exists");
    sim.uninit_with_rules(blocker, &rules);
    for _ in 0..35 {
        assert!(!tick(&mut sim, &rules, None).spawned_entities);
    }
    assert_eq!(yards(&sim), 0, "failed deploy must not keep retrying");
}
#[test]
fn move_order_during_turn_prevents_conversion_at_the_old_site() {
    let (mut sim, rules, id) = fixture("AMCV", 64, 5, 4);
    sim.path_grid = Some(std::sync::Arc::new(crate::sim::pathfinding::PathGrid::new(
        64, 64,
    )));
    tick(&mut sim, &rules, Some(Command::DeployMcv { entity_id: id }));
    tick(&mut sim, &rules, None);
    tick(
        &mut sim,
        &rules,
        Some(Command::Move {
            entity_id: id,
            target_rx: 40,
            target_ry: 22,
            queue: false,
        }),
    );
    for _ in 0..10 {
        assert!(!tick(&mut sim, &rules, None).spawned_entities);
    }
    assert_eq!(yards(&sim), 0);
    assert!(!sim.substrate.entities.get(id).unwrap().dying);
}

#[test]
fn continuation_result_and_rotation_edge_match_original_blocks() {
    let v: serde_json::Value =
        serde_json::from_str(include_str!("../../tools/mcv_deploy_oracle.json")).unwrap();
    for row in v["mission_branches"].as_array().unwrap() {
        let kind = row["kind"].as_str().unwrap();
        if kind == "state0" {
            continue;
        } // state0 includes the separate path reset/radio handoff
        let (mut sim, _rules, id) = fixture("AMCV", 64, 5, 4);
        let e = sim.substrate.entities.get_mut(id).unwrap();
        e.dying = row["alive"].as_u64().unwrap() == 0;
        e.mcv_deploy_pending = row["flag"].as_u64().unwrap() != 0;
        e.navigation.nav_com = (row["nav"].as_u64().unwrap() != 0)
            .then_some(crate::sim::components::NavTargetRef::cell(30, 30));
        e.mission
            .set_handler_state(if kind == "initial" { 1 } else { 2 });
        if kind == "initial" {
            finish_initial_attempt(&mut sim, id);
        } else {
            finish_retry(&mut sim, id, row["result"].as_u64().unwrap() != 0);
        }
        let e = sim.substrate.entities.get(id).unwrap();
        assert_eq!(
            e.mission.handler_state(),
            row["out_state"].as_u64().unwrap() as u32,
            "{row}"
        );
        assert_eq!(
            e.mcv_deploy_pending,
            row["out_flag"].as_u64().unwrap() != 0,
            "{row}"
        );
    }
}

#[test]
fn conversion_receipt_rebuilds_navigation_in_the_conversion_frame() {
    let (mut sim, rules, id) = fixture("AMCV", 64, 5, 4);
    sim.resolved_terrain = Some(crate::sim::deploy_tests::mcv_deploy_terrain_with(|_| {}));
    assert!(sim.rebuild_dynamic_navigation(&rules));
    assert!(sim.path_grid.as_ref().unwrap().is_walkable(19, 21));
    let accepted = tick(&mut sim, &rules, Some(Command::DeployMcv { entity_id: id }));
    assert!(!accepted.spawned_entities);
    assert!(sim.path_grid.as_ref().unwrap().is_walkable(19, 21));
    for _ in 0..50 {
        let result = tick(&mut sim, &rules, None);
        if result.spawned_entities {
            assert_eq!(yards(&sim), 1);
            assert!(!sim.path_grid.as_ref().unwrap().is_walkable(19, 21));
            return;
        }
        assert!(sim.path_grid.as_ref().unwrap().is_walkable(19, 21));
    }
    panic!("missing conversion frame");
}
#[test]
fn retail_mcv_and_target_rules_deploy_with_one_command() {
    let Some((rules_ini, art_ini)) = crate::rules::retail_ini_fixture::retail_rules_and_art()
    else {
        return;
    };
    let mut rules = RuleSet::from_ini(&rules_ini).unwrap();
    let art = crate::rules::art_data::ArtRegistry::from_ini(&art_ini);
    rules.merge_art_data(&art);
    for kind in ["AMCV", "SMCV", "PCV"] {
        for facing in [0, 64, 128, 192] {
            let mut sim = Simulation::new();
            crate::sim::arena_fixture::flat_ground(&mut sim, &rules);
            let id = sim
                .spawn_object(kind, "Americans", 20, 22, facing, &rules, &BTreeMap::new())
                .unwrap();
            tick(&mut sim, &rules, Some(Command::DeployMcv { entity_id: id }));
            let mut converted = false;
            for _ in 0..140 {
                let result = tick(&mut sim, &rules, None);
                if result.spawned_entities {
                    converted = true;
                    break;
                }
            }
            assert!(converted, "{kind}, facing={facing}");
            let target = rules.object(kind).unwrap().deploys_into.as_deref().unwrap();
            assert_eq!(
                sim.substrate
                    .entities
                    .values()
                    .filter(|e| !e.dying && sim.interner.resolve(e.type_ref()) == target)
                    .count(),
                1
            );
        }
    }
}

#[test]
fn active_track_and_same_cell_destination_preserve_the_rotation_latch() {
    let (mut sim, rules, id) = fixture("AMCV", 128, 5, 4);
    let e = sim.substrate.entities.get_mut(id).unwrap();
    e.mcv_deploy_pending = true;
    let drive = e.drive_locomotion.get_or_insert_with(Default::default);
    drive.turn_latched = true;
    drive.track.turn_index = 3;
    drive.track_valid = true;
    assert!(
        drive.head_to.is_none(),
        "the native +63/selector gate is independent of Head_To"
    );
    sim.process_ground_locomotor_for_test(id, Some(&rules), None, None)
        .unwrap();
    assert!(
        sim.substrate
            .entities
            .get(id)
            .unwrap()
            .drive_locomotion
            .as_ref()
            .unwrap()
            .turn_latched
    );
    assert_eq!(yards(&sim), 0);
    let e = sim.substrate.entities.get_mut(id).unwrap();
    e.drive_locomotion.as_mut().unwrap().track_valid = false;
    e.navigation.nav_com = Some(crate::sim::components::NavTargetRef::cell(20, 22));
    sim.process_ground_locomotor_for_test(id, Some(&rules), None, None)
        .unwrap();
    assert!(
        sim.substrate
            .entities
            .get(id)
            .unwrap()
            .drive_locomotion
            .as_ref()
            .unwrap()
            .turn_latched
    );
    assert_eq!(yards(&sim), 0);
    let e = sim.substrate.entities.get_mut(id).unwrap();
    e.navigation.nav_com = None;
    // A retained selector alone cannot override the cleared +63 authority.
    assert_eq!(e.drive_locomotion.as_ref().unwrap().track.turn_index, 3);
    assert!(!e.drive_locomotion.as_ref().unwrap().track_valid);
    sim.process_ground_locomotor_for_test(id, Some(&rules), None, None)
        .unwrap();
    assert_eq!(yards(&sim), 1);
}

#[test]
fn mcv_deploy_sound_occurs_once_with_conversion_at_the_source_cell() {
    use crate::sim::world::SimSoundEvent;
    for facing in [64, 128] {
        let (mut sim, rules, id) = fixture_with_sound("AMCV", facing, 5, 4, Some("PlaceBuilding"));
        tick(&mut sim, &rules, Some(Command::DeployMcv { entity_id: id }));
        let mut sounds = 0;
        for _ in 0..50 {
            // Repeated D while waiting must not repeat the conversion cue.
            let command = (yards(&sim) == 0).then_some(Command::DeployMcv { entity_id: id });
            sim.sound_events.clear();
            let result = tick(&mut sim, &rules, command);
            let cues: Vec<_> = sim
                .sound_events
                .iter()
                .filter_map(|event| match event {
                    SimSoundEvent::EntityDeployed {
                        deploy_sound_id,
                        rx,
                        ry,
                    } => Some((sim.interner.resolve(*deploy_sound_id), *rx, *ry)),
                    _ => None,
                })
                .collect();
            assert_eq!(cues.len(), usize::from(result.spawned_entities));
            if !cues.is_empty() {
                assert_eq!(cues, vec![("PlaceBuilding", 20, 22)]);
            }
            sounds += cues.len();
        }
        assert_eq!(sounds, 1);
    }
}

#[test]
fn blocked_or_unconfigured_mcv_deploy_does_not_emit_deploy_sound() {
    use crate::sim::world::SimSoundEvent;
    for blocked in [false, true] {
        let (mut sim, rules, id) =
            fixture_with_sound("AMCV", 64, 5, 4, blocked.then_some("PlaceBuilding"));
        tick(&mut sim, &rules, Some(Command::DeployMcv { entity_id: id }));
        tick(&mut sim, &rules, None);
        if blocked {
            sim.spawn_object("YARD", "Americans", 19, 21, 0, &rules, &BTreeMap::new())
                .unwrap();
        }
        for _ in 0..50 {
            tick(&mut sim, &rules, None);
            assert!(
                !sim.sound_events
                    .iter()
                    .any(|e| matches!(e, SimSoundEvent::EntityDeployed { .. }))
            );
        }
        if !blocked {
            assert_eq!(
                yards(&sim),
                1,
                "an unconfigured cue must not prevent conversion"
            );
            assert!(sim.substrate.entities.get(id).is_none_or(|e| e.dying));
        }
    }
}

/// A computer (or human) house whose AutoBaseBuilding latch is set, in a
/// multiplayer game, with a BuildConst MCV at (20, 22) already facing its
/// DeployFacing. The house keeps the base plan of a yard it lost, so the
/// deploy only re-anchors it.
fn house_fixture(human: bool, land: &str) -> (Simulation, RuleSet, u64) {
    let text = format!(
        "[InfantryTypes]\n[AircraftTypes]\n[VehicleTypes]\n0=AMCV\n[BuildingTypes]\n0=YARD\n\
         [AI]\nBuildConst=YARD\n\
         [AMCV]\nStrength=1000\nSpeed=5\nROT=5\nLocomotor={{4A582741-9839-11d1-B709-00A024DDAFD1}}\nDeploysInto=YARD\n\
         [YARD]\nStrength=1000\nConstructionYard=yes\nFoundation=4x3\nDeployFacing=4\n\
         [Hunt]\nRate=0.016\n[Guard]\nRate=0.016\n[Unload]\nRate=0.016\n{land}"
    );
    let rules = RuleSet::from_ini(&IniFile::from_str(&text)).unwrap();
    let mut sim = Simulation::new();
    crate::sim::arena_fixture::flat_ground(&mut sim, &rules);
    sim.session.game_mode_nonzero = true;
    let owner = sim.interner.intern("Americans");
    let mut house = crate::sim::house_state::HouseState::new(owner, 0, None, human, 5000, 10);
    house.ai_activation.auto_base_building = true;
    house.base_plan.nodes = vec![crate::sim::base_plan::BasePlanNode {
        type_or_control: 0,
        packed_cell: 0,
        filled: true,
        retry_count: 0,
    }];
    sim.houses.insert(owner, house);
    sim.session.house_order.push(owner);
    let id = sim
        .spawn_object("AMCV", "Americans", 20, 22, 128, &rules, &BTreeMap::new())
        .unwrap();
    (sim, rules, id)
}

const BUILDABLE: &str = "[Clear]\nBuildable=yes\n";

fn yard_cells(sim: &Simulation) -> Vec<(u16, u16)> {
    sim.substrate
        .entities
        .values()
        .filter(|e| !e.dying && sim.interner.resolve(e.type_ref()) == "YARD")
        .map(|e| (e.position.rx, e.position.ry))
        .collect()
}

#[test]
fn try_to_deploy_sites_match_the_original_table() {
    let vectors: serde_json::Value =
        serde_json::from_str(include_str!("../../tools/mcv_deploy_oracle.json")).unwrap();
    let native: Vec<(i16, i16)> = vectors["try_to_deploy_sites"]
        .as_array()
        .unwrap()
        .iter()
        .map(|site| {
            (
                site[0].as_i64().unwrap() as i16,
                site[1].as_i64().unwrap() as i16,
            )
        })
        .collect();
    assert_eq!(native, TRY_TO_DEPLOY_SITES);
}

#[test]
fn try_to_deploy_admits_a_clear_spot_where_the_unit_stands() {
    let (mut sim, rules, id) = house_fixture(false, BUILDABLE);
    assert!(try_to_deploy(&mut sim, id, &rules, None));
    let mcv = sim.substrate.entities.get(id).unwrap();
    assert!(mcv.navigation.nav_com.is_none());
    assert!(sim.substrate.occupancy.contains_entity(20, 22, id));
}

/// A blocker inside the unit's own foundation also refuses the first four
/// northern sites; `(0, -3)` is the first whose foundation misses it, and the
/// destination is that site's cell, not its origin.
#[test]
fn try_to_deploy_drives_to_the_first_clear_site_in_table_order() {
    let (mut sim, rules, id) = house_fixture(false, BUILDABLE);
    sim.spawn_object("AMCV", "Americans", 20, 21, 0, &rules, &BTreeMap::new())
        .unwrap();
    assert!(!try_to_deploy(&mut sim, id, &rules, None));
    let mcv = sim.substrate.entities.get(id).unwrap();
    assert_eq!(
        mcv.navigation.nav_com,
        Some(crate::sim::components::NavTargetRef::cell(20, 19))
    );
    assert!(sim.substrate.occupancy.contains_entity(20, 22, id));
}

#[test]
fn with_no_site_a_computer_mcv_scatters_and_a_human_one_waits() {
    for human in [false, true] {
        let (mut sim, rules, id) = house_fixture(human, "[Clear]\nBuildable=no\n");
        let rng = sim.scenario_rng.logical_state();
        assert!(!try_to_deploy(&mut sim, id, &rules, None));
        let mcv = sim.substrate.entities.get(id).unwrap();
        let scattered = mcv.movement_target.is_some() || mcv.navigation.nav_com.is_some();
        assert_eq!(scattered, !human, "human={human}");
        assert_eq!(
            sim.scenario_rng.logical_state() == rng,
            human,
            "human={human}"
        );
    }
}

/// UnitClass::AI queues Hunt for a base-building computer house with no
/// Construction Yard; Mission_Hunt's deploy arm unpacks it where it stands.
#[test]
fn a_computer_house_without_a_yard_hunts_and_deploys_its_mcv() {
    let (mut sim, rules, _id) = house_fixture(false, BUILDABLE);
    for _ in 0..8 {
        tick(&mut sim, &rules, None);
    }
    assert_eq!(yard_cells(&sim), [(19, 21)]);

    let (mut sim, rules, id) = house_fixture(true, BUILDABLE);
    for _ in 0..8 {
        tick(&mut sim, &rules, None);
    }
    assert!(
        yard_cells(&sim).is_empty(),
        "a human's MCV waits for orders"
    );
    assert!(sim.substrate.entities.get(id).is_some_and(|e| !e.dying));
}

/// With a yard already standing, Mission_Guard's arm queues Unload and the
/// second MCV deploys through Mission_Unload.
#[test]
fn a_computer_mcv_on_guard_unloads_when_its_house_has_a_yard() {
    let (mut sim, rules, id) = house_fixture(false, BUILDABLE);
    sim.spawn_object("YARD", "Americans", 4, 4, 0, &rules, &BTreeMap::new())
        .unwrap();
    let owner = sim.interner.get("Americans").unwrap();
    assert_eq!(sim.houses[&owner].build_const_order.len(), 1);
    assert!(guard_queues_unload(&sim, id, &rules));
    for _ in 0..8 {
        tick(&mut sim, &rules, None);
    }
    assert_eq!(yard_cells(&sim).len(), 2);
    assert!(yard_cells(&sim).contains(&(19, 21)));
}

/// Retail Dustbowl with a skirmish computer house (`Russians`) whose MCV
/// stands on flat, empty buildable ground nearest the map centre, set up as
/// `ScenarioClass::Create_Houses` sets up a computer slot (`MaxIQLevels`, an
/// AI player). Returns the scenario, the house, the MCV and its cell.
fn retail_dustbowl_computer_mcv() -> (
    crate::headless_scenario::HeadlessScenario,
    crate::sim::intern::InternedId,
    u64,
    (u16, u16),
) {
    let dir = std::env::var("RA2_DIR")
        .ok()
        .filter(|path| !path.trim().is_empty())
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            crate::util::config::GameConfig::load()
                .expect("set RA2_DIR or provide config.toml for this ignored test")
                .paths
                .ra2_dir
        });
    let mut scenario =
        crate::headless_scenario::load(&dir, "Dustbowl.mmx", 0x00C0_FFEE).expect("Dustbowl loads");
    let crate::sim::runtime::SimRuntime {
        simulation: sim,
        resources,
    } = &mut scenario.runtime;
    let rules = &resources.rules;
    sim.session.game_mode_nonzero = true;
    let owner = sim.interner.intern("Russians");
    let mut house =
        crate::sim::house_state::HouseState::new(owner, 1, Some(owner), false, 10_000, 10);
    house.current_iq = rules.general.max_iq_levels;
    sim.houses.insert(owner, house);
    sim.session.house_order.push(owner);
    sim.ai_players
        .push(crate::sim::ai::AiPlayerState::new(owner));

    let (width, height) = crate::rules::foundation::foundation_dimensions(
        &rules.object("NACNST").expect("retail NACNST").foundation,
    );
    // Retail `[Clear]` and `[Rough]` both read `Buildable=yes`.
    let buildable = [
        crate::rules::terrain_rules::LandType::Clear.as_index(),
        crate::rules::terrain_rules::LandType::Rough.as_index(),
    ];
    // Nearest the map centre first, well inside the playfield.
    let mut cells: Vec<(u16, u16)> = (43..103_u16)
        .flat_map(|y| (43..103_u16).map(move |x| (x, y)))
        .collect();
    cells.sort_by_key(|&(x, y)| x.abs_diff(73).max(y.abs_diff(73)));
    let (mcv, cell) = cells
        .into_iter()
        .find_map(|(x, y)| {
            let terrain = sim.resolved_terrain.as_ref()?;
            let overlays = sim.overlay_grid.as_ref()?;
            let level = terrain.cell(x, y)?.level;
            let site = (x - 1..x - 1 + width).all(|cx| {
                (y - 1..y - 1 + height).all(|cy| {
                    terrain.cell(cx, cy).is_some_and(|cell| {
                        cell.level == level
                            && cell.slope_type == 0
                            && buildable.contains(&cell.yr_cell_land_type)
                            && cell.terrain_object_occupation.is_none()
                            && !cell.has_bridge_deck
                            && !cell.bridge_facts.has_structural_bridge()
                    }) && overlays.cell(cx, cy).overlay_id.is_none()
                })
            });
            let alone = sim.substrate.entities.values().all(|entity| {
                entity.position.rx.abs_diff(x) > 5 || entity.position.ry.abs_diff(y) > 5
            });
            if !site || !alone {
                return None;
            }
            let id = sim.spawn_object("SMCV", "Russians", x, y, 0, rules, &resources.height_map)?;
            Some((id, (x, y)))
        })
        .expect("flat, empty buildable ground for the yard");
    sim.resolve_type_handles(rules);
    (scenario, owner, mcv, cell)
}

fn retail_frame(scenario: &mut crate::headless_scenario::HeadlessScenario) {
    scenario
        .runtime
        .advance_frame(
            &[],
            crate::headless_scenario::SIM_TICK_MS,
            crate::sim::world::TickLane::Ordinary,
        )
        .expect("retail frame");
}

fn retail_yard_at(scenario: &crate::headless_scenario::HeadlessScenario) -> Option<(u16, u16)> {
    let sim = scenario.sim();
    sim.substrate
        .entities
        .values()
        .find(|e| !e.dying && sim.interner.resolve(e.type_ref()) == "NACNST")
        .map(|e| (e.position.rx, e.position.ry))
}

/// A computer house's MCV on retail Dustbowl through the production frame.
/// HouseClass::Update's activation sets `+0x1F3`, UnitClass::AI queues Hunt
/// for the yard-less house, and Mission_Hunt's TryToDeploy admits the spot
/// where the MCV stands: every foundation cell is flat, empty `[Clear]` or
/// `[Rough]` ground, both of which retail marks `Buildable=yes`. Deploy then
/// unpacks NACNST one cell north-west of the MCV.
#[test]
#[ignore = "requires a retail RA2/YR install (RA2_DIR or config.toml)"]
fn retail_dustbowl_a_computer_mcv_deploys_where_it_stands() {
    let (mut scenario, owner, mcv, (x, y)) = retail_dustbowl_computer_mcv();
    retail_frame(&mut scenario);
    let sim = scenario.sim();
    assert!(sim.houses[&owner].ai_activation.auto_base_building);
    let mut frames = 1;
    while retail_yard_at(&scenario).is_none() && frames < 600 {
        retail_frame(&mut scenario);
        frames += 1;
    }
    assert_eq!(
        retail_yard_at(&scenario),
        Some((x - 1, y - 1)),
        "after {frames} frames"
    );
    let sim = scenario.sim();
    assert!(sim.substrate.entities.get(mcv).is_none_or(|e| e.dying));
    assert_eq!(sim.houses[&owner].base_center, Some((x - 1, y - 1)));
}

/// The deployed yard then builds from its House's BasePlan
/// (`sim::ai_base_building`): each building it places is of a node's type
/// and stands on the cell that node keeps.
#[test]
#[ignore = "requires a retail RA2/YR install (RA2_DIR or config.toml)"]
fn retail_dustbowl_the_computer_yard_places_its_planned_buildings() {
    let (mut scenario, owner, _mcv, _cell) = retail_dustbowl_computer_mcv();
    let mut frames = 0;
    while retail_yard_at(&scenario).is_none() && frames < 600 {
        retail_frame(&mut scenario);
        frames += 1;
    }
    assert!(retail_yard_at(&scenario).is_some(), "the yard stands");
    let placed =
        |scenario: &crate::headless_scenario::HeadlessScenario| -> Vec<(u64, String, (u16, u16))> {
            let sim = scenario.sim();
            let mut buildings: Vec<_> = sim
                .substrate
                .entities
                .values()
                .filter(|e| {
                    !e.dying
                        && !e.lifecycle.in_limbo
                        && e.owner() == owner
                        && e.category == crate::map::entities::EntityCategory::Structure
                        && sim.interner.resolve(e.type_ref()) != "NACNST"
                })
                .map(|e| {
                    (
                        e.stable_id(),
                        sim.interner.resolve(e.type_ref()).to_string(),
                        (e.position.rx, e.position.ry),
                    )
                })
                .collect();
            buildings.sort();
            buildings
        };
    let start = frames;
    let mut seen: Vec<(u64, String, (u16, u16))> = Vec::new();
    while seen.len() < 3 && frames < start + 8000 {
        retail_frame(&mut scenario);
        frames += 1;
        for building in placed(&scenario) {
            if seen.iter().any(|known| known.0 == building.0) {
                continue;
            }
            let sim = scenario.sim();
            let rules = &scenario.runtime.resources.rules;
            let house = &sim.houses[&owner];
            let index = rules
                .building_type_index(&building.1)
                .expect("a BuildingType");
            let node_cell = crate::sim::base_plan::pack_base_plan_cell(
                i32::from(building.2.0),
                i32::from(building.2.1),
            );
            assert!(
                house
                    .base_plan
                    .nodes
                    .iter()
                    .any(|node| node.type_or_control == index && node.packed_cell == node_cell),
                "frame {frames}: {building:?} stands on a node of its type"
            );
            eprintln!("frame {frames}: placed {} at {:?}", building.1, building.2);
            seen.push(building);
        }
    }
    assert_eq!(
        seen.len(),
        3,
        "three buildings placed after {} frames",
        frames - start
    );
}

/// The yard's `-1` nodes become base defenses (`sim::ai_base_defense`): the
/// first defense it places is one of the Soviet list's and stands on the
/// cell its node took from the site search.
#[test]
#[ignore = "requires a retail RA2/YR install (RA2_DIR or config.toml)"]
fn retail_dustbowl_the_computer_yard_places_a_base_defense() {
    let (mut scenario, owner, _mcv, _cell) = retail_dustbowl_computer_mcv();
    let mut frames = 0;
    while retail_yard_at(&scenario).is_none() && frames < 600 {
        retail_frame(&mut scenario);
        frames += 1;
    }
    assert!(retail_yard_at(&scenario).is_some(), "the yard stands");
    let defense = |scenario: &crate::headless_scenario::HeadlessScenario| {
        let sim = scenario.sim();
        let rules = &scenario.runtime.resources.rules;
        let defenses = rules.base_defense_types(1);
        sim.substrate
            .entities
            .values()
            .filter(|e| {
                !e.dying
                    && !e.lifecycle.in_limbo
                    && e.owner() == owner
                    && e.category == crate::map::entities::EntityCategory::Structure
            })
            .map(|e| {
                (
                    sim.interner.resolve(e.type_ref()).to_string(),
                    (e.position.rx, e.position.ry),
                )
            })
            .find(|(name, _)| defenses.iter().any(|id| id.eq_ignore_ascii_case(name)))
    };
    let start = frames;
    let mut placed = None;
    while placed.is_none() && frames < start + 8000 {
        retail_frame(&mut scenario);
        frames += 1;
        placed = defense(&scenario);
    }
    let (name, cell) =
        placed.unwrap_or_else(|| panic!("no defense after {} frames", frames - start));
    eprintln!("frame {frames}: placed {name} at {cell:?}");
    let sim = scenario.sim();
    let rules = &scenario.runtime.resources.rules;
    let index = rules.building_type_index(&name).expect("a BuildingType");
    let node_cell =
        crate::sim::base_plan::pack_base_plan_cell(i32::from(cell.0), i32::from(cell.1));
    assert!(
        sim.houses[&owner]
            .base_plan
            .nodes
            .iter()
            .any(|node| node.type_or_control == index && node.packed_cell == node_cell),
        "{name} at {cell:?} stands on a node of its type"
    );
}
