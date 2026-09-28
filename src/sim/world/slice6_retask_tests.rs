//! Slice 6 — verb API + dispatch-adoption integration tests.
//!
//! Two jobs:
//!   1. `replay_hash_stable_through_slice6` — the behavior-preserving gate. A
//!      scripted skirmish drives every retasking command site (Move / Stop /
//!      Attack / ForceAttack / ForceAttackCell / AttackMove) and asserts the
//!      end-of-run `state_hash()` equals the committed baseline. At the slice's
//!      introduction, this exposed wrong `DockTeardown` subsets and dropped
//!      legacy-field clears. Changes require causal behavior or hash-composition
//!      evidence before updating the Rust regression receipt.
//!   2. The verb-write + retaliation-gate tripwires (added below the gate).

use super::*;
use crate::map::entities::{EntityCategory, MapEntity};
use crate::rules::ini_parser::IniFile;
use crate::rules::ruleset::RuleSet;
use crate::sim::combat::AttackTarget;
use crate::sim::command::{Command, CommandEnvelope};
use crate::sim::components::OrderIntent;
use crate::sim::mission::{MissionId, MissionType};
use crate::sim::pathfinding::PathGrid;
use crate::sim::replay::{ReplayHeader, ReplayLog, ReplayRunner};
use std::collections::BTreeMap;

fn slice6_rules() -> RuleSet {
    // Two attack-capable vehicles + an infantry; ranges short enough that no
    // auto-combat fires during the scripted window (commands drive everything,
    // while constructor and Walk head-selection draws still use Scenario RNG).
    let ini: IniFile = IniFile::from_str(
        "[InfantryTypes]\n0=E1\n\n\
         [VehicleTypes]\n0=MTNK\n\n\
         [AircraftTypes]\n\n\
         [BuildingTypes]\n0=GACNST\n\n\
         [E1]\nLocomotor={4A582744-9839-11d1-B709-00A024DDAFD1}\nStrength=125\nArmor=flak\nSpeed=4\nPrimary=M60\n\n\
         [MTNK]\nLocomotor={4A582741-9839-11d1-B709-00A024DDAFD1}\nStrength=300\nArmor=heavy\nSpeed=6\nPrimary=105mm\n\n\
         [GACNST]\nStrength=1000\nArmor=wood\nFoundation=4x3\n\n\
         [M60]\nDamage=25\nROF=20\nRange=5\nWarhead=SA\n\n\
         [105mm]\nDamage=65\nROF=50\nRange=6\nWarhead=AP\n\n\
         [SA]\nVerses=100%,100%,100%,90%,70%,25%,100%,25%,25%,0%,0%\n\n\
         [AP]\nVerses=100%,100%,90%,75%,75%,75%,60%,30%,20%,0%,0%\n",
    );
    RuleSet::from_ini(&ini).expect("slice6 test rules should parse")
}

fn cmd_envelope(
    sim: &Simulation,
    owner: &str,
    execute_tick: u64,
    payload: Command,
) -> CommandEnvelope {
    let owner_id = sim
        .interner
        .get(owner)
        .unwrap_or_else(|| panic!("owner '{owner}' not interned"));
    CommandEnvelope::new(owner_id, execute_tick, payload)
}

fn unit(owner: &str, type_id: &str, cx: u16, cy: u16, cat: EntityCategory) -> MapEntity {
    MapEntity {
        owner: owner.to_string(),
        type_id: type_id.to_string(),
        health: 256,
        cell_x: cx,
        cell_y: cy,
        facing: 64,
        category: cat,
        sub_cell: 0,
        veterancy: 0,
        high: false,
        mission: None,
        recruitable_a: true,
        recruitable_b: true,
        structure_upgrades: [None, None, None],
        structure_ai_sellable: false,
        structure_ai_repairable: false,
    }
}

// Rust regression receipts from `bridge-fv-host-v15`, not native full-replay
// goldens. IDLE/Stop (`0x004C74CB..0x004C76BB`) clears target/destination but
// writes no ordinary mission. Attack therefore keeps its frame-5 dispatch and
// ends with 11 visits, rather than the old invented Stop's 5. All three streams,
// paid Walk rows, positions, health and per-frame replay equality are unchanged.
// Original executed Stop/mission/Drive evidence is in
// `tools/spatial_oracle/fv_cell_attack/paid_conditional*.json` and its README.
// Historical counter/ROT rewrites and hash-schema projections described old
// behavior; Git preserves them without mutating current state to imitate it.
const SLICE6_BASELINE_HASH: u64 = 0xE5B5_A210_4739_1CDE;
const SLICE6_FINAL_STREAM_STATES: (u64, u64, u64) = (
    0x37A0_8362_3CAA_3D37,
    0x4CB6_FE1C_CB45_47FF,
    0x1CE8_1848_7043_6163,
);

#[test]
fn replay_hash_stable_through_slice6() {
    let rules = slice6_rules();
    let heights: BTreeMap<(u16, u16), u8> = BTreeMap::new();
    let grid = PathGrid::new(64, 64);
    let mut sim = Simulation::new();
    // id 1: Americans MTNK (the unit we retask). id 2: enemy MTNK (Soviet, hostile
    // by default — no alliance entry). id 3: Americans E1 (second attacker).
    sim.spawn_from_map(
        &[
            unit("Americans", "MTNK", 3, 3, EntityCategory::Unit),
            unit("Soviet", "MTNK", 25, 3, EntityCategory::Unit),
            unit("Americans", "E1", 5, 5, EntityCategory::Infantry),
        ],
        Some(&rules),
        &heights,
    );
    let mut diagnostic = super::global_parity_harness_tests::replay_diagnostic_file("slice6");
    super::global_parity_harness_tests::record_replay_diagnostic(
        &mut diagnostic,
        &sim,
        None,
        &[],
        &[],
    );

    // (execute_tick, command) — apply_due_commands fires each when self.session.tick+1 == tick.
    let script: &[(u64, Command)] = &[
        (
            1,
            Command::Move {
                entity_id: 1,
                target_rx: 10,
                target_ry: 10,
                queue: false,
            },
        ),
        (
            3,
            Command::AttackMove {
                entity_id: 1,
                target_rx: 15,
                target_ry: 3,
                queue: false,
            },
        ),
        (
            5,
            Command::ForceAttackCell {
                attacker_id: 1,
                target_rx: 18,
                target_ry: 3,
            },
        ),
        (
            7,
            Command::ForceAttack {
                attacker_id: 1,
                target_id: 2,
            },
        ),
        (
            9,
            Command::Attack {
                attacker_id: 3,
                target_id: 2,
            },
        ),
        (11, Command::Stop { entity_id: 1 }),
    ];

    let mut log = ReplayLog::new(ReplayHeader {
        version: 1,
        pixel_conversion_bounds: sim.session.pixel_conversion_bounds,
        tick_hz: 15,
        seed: sim.session.seed,
        map_name: "slice6_retask".to_string(),
        rules_hash: rules.simulation_config_hash(),
    });
    let mut stopped_head = None;
    let walk_vectors: Vec<serde_json::Value> = serde_json::from_str(include_str!(
        "../../../tools/spatial_oracle/walk_paid_step.json"
    ))
    .unwrap();
    let paid_steps: Vec<_> = walk_vectors
        .iter()
        .filter(|row| {
            row["input"]["name"]
                .as_str()
                .unwrap()
                .starts_with("slice6_paid_step_")
        })
        .collect();
    assert_eq!(paid_steps.len(), 5);
    for tick in 0..16u64 {
        let due: Vec<CommandEnvelope> = script
            .iter()
            .filter(|(t, _)| *t == tick + 1)
            .map(|(t, c)| cmd_envelope(&sim, "Americans", *t, c.clone()))
            .collect();
        let mut advance = || sim.advance_tick(&due, Some(&rules), &heights, Some(&grid), None, 67);
        let (result, draws) = if diagnostic.is_some() {
            crate::sim::rng::trace_draws(advance)
        } else {
            (advance(), Vec::new())
        };
        super::global_parity_harness_tests::record_replay_diagnostic(
            &mut diagnostic,
            &sim,
            Some(&result),
            &due,
            &draws,
        );
        assert!(result.frame_committed, "retask frame {tick} must commit");
        assert_eq!(
            result.executed_commands,
            due.len(),
            "scripted envelope at {tick} must be consumed"
        );
        log.record_tick(tick, due, result.state_hash);
        if tick >= 11 {
            let row = paid_steps[(tick - 11) as usize];
            let infantry = sim.substrate.entities.get(3).unwrap();
            let coord = crate::sim::movement::ground_pose::position_world_coord(&infantry.position);
            assert_eq!(
                [coord.x, coord.y, coord.z],
                std::array::from_fn::<_, 3, _>(|i| row["proposed"][i].as_i64().unwrap() as i32),
                "native paid Walk frame {}",
                tick + 1
            );
            assert_eq!(
                u64::from(
                    infantry
                        .body_facing
                        .unwrap()
                        .current(sim.session.binary_frame)
                ),
                row["facing"].as_u64().unwrap()
            );
            assert_eq!(infantry.foot_speed.cached_current_speed, 10);
        }

        if tick >= 10 {
            let tank = sim.substrate.entities.get(1).expect("retasked tank lives");
            assert_eq!(
                tank.mission.current(),
                MissionId::from_known(MissionType::Attack)
            );
            assert_eq!(tank.mission.queued(), MissionId::NONE);
            assert_eq!(tank.mission.mission_start_frame(), 5);
            assert_eq!(tank.mission.ai_counter(), tick as u32 - 4);
            assert_eq!(
                (
                    tank.mission.dispatch_timer().start_frame(),
                    tank.mission.dispatch_timer().delay()
                ),
                (5, 14),
                "Stop retains Attack's dispatch timer and counter"
            );
            let drive = tank.drive_locomotion.as_ref().expect("Drive owner");
            if tick == 10 {
                stopped_head = drive.head_to;
                assert!(
                    stopped_head.is_some(),
                    "Stop must exercise an already committed segment"
                );
            }
            assert!(
                tank.navigation.nav_com.is_none(),
                "Stop clears owner NavCom"
            );
            assert!(
                drive.destination.is_none(),
                "Stop clears the class destination"
            );
            assert!(
                drive.head_to.is_none() || drive.head_to == stopped_head,
                "Stop must not select a new head from abandoned orders"
            );
            assert_eq!(
                tank.navigation.path_replay.cursor as usize,
                tank.navigation.path_replay.directions.len(),
                "Stop exhausts the abandoned direction suffix"
            );
            assert!(
                tank.attack_target.is_none(),
                "Stop retires the previous attack"
            );
        }
    }

    // Unlike the old single-run hash gate, execute every recorded command a
    // second time through ReplayRunner and compare each committed frame.
    let mut replay = Simulation::new();
    replay.spawn_from_map(
        &[
            unit("Americans", "MTNK", 3, 3, EntityCategory::Unit),
            unit("Soviet", "MTNK", 25, 3, EntityCategory::Unit),
            unit("Americans", "E1", 5, 5, EntityCategory::Infantry),
        ],
        Some(&rules),
        &heights,
    );
    let replayed = ReplayRunner::run_fixture_with_overlay_registry(
        &mut replay,
        &log,
        Some(&rules),
        &heights,
        Some(&grid),
        None,
        67,
    );
    assert_eq!(replayed.len(), log.ticks.len());
    for (frame, (actual, recorded)) in replayed.iter().zip(&log.ticks).enumerate() {
        assert_eq!(*actual, recorded.state_hash, "retask replay frame {frame}");
    }
    assert_eq!(
        replay.scenario_rng.logical_state(),
        sim.scenario_rng.logical_state()
    );
    assert_eq!(
        replay.main_rng.logical_state(),
        sim.main_rng.logical_state()
    );
    assert_eq!(
        replay.mapgen_rng.logical_state(),
        sim.mapgen_rng.logical_state()
    );
    for (id, health) in [(1, 300), (2, 300), (3, 125)] {
        assert_eq!(
            sim.substrate
                .entities
                .get(id)
                .expect("actor lives")
                .health
                .current,
            health,
            "retask window must not turn into a combat/death fixture"
        );
    }
    super::global_parity_harness_tests::print_replay_summary("slice6", &sim);
    assert_eq!(
        [1, 2, 3].map(|id| sim.substrate.entities.get(id).unwrap().mission.ai_counter()),
        [11, 16, 7],
        "Stop retains Attack; Unit/Infantry Commence precedes the Techno counter increment"
    );
    assert_eq!(
        sim.substrate
            .entities
            .get(3)
            .unwrap()
            .body_facing
            .unwrap()
            .rot_per_frame(),
        0x7F00
    );
    assert_eq!(
        (
            sim.scenario_rng.state(),
            sim.main_rng.state(),
            sim.mapgen_rng.state()
        ),
        SLICE6_FINAL_STREAM_STATES,
        "absolute retask stream receipts remain unchanged by the native Stop correction"
    );
    assert_eq!(
        sim.state_hash(),
        SLICE6_BASELINE_HASH,
        "scripted-retask state drifted: establish the changed behavior, RNG producer, or hash composition before updating"
    );
}

#[test]
fn slice6_move_command_retasks_via_mission_substrate_and_clears_state() {
    // A Move command must route through the compatibility boundary: the mission
    // substrate's `current` becomes Move (checked BEFORE any tick-tail shadow
    // refresh) AND the legacy conflicting fields are cleared.
    let rules = slice6_rules();
    let heights: BTreeMap<(u16, u16), u8> = BTreeMap::new();
    let grid = PathGrid::new(64, 64);
    let mut sim = Simulation::new();
    sim.spawn_from_map(
        &[unit("Americans", "MTNK", 3, 3, EntityCategory::Unit)],
        Some(&rules),
        &heights,
    );
    // Seed a conflicting prior order the Move must tear down.
    {
        let e = sim.substrate.entities.get_mut(1).expect("unit");
        e.attack_target = Some(AttackTarget::new(2));
        e.order_intent = Some(OrderIntent::Guard {
            anchor_rx: 3,
            anchor_ry: 3,
        });
    }

    let issued = sim.apply_command(
        "Americans",
        &Command::Move {
            entity_id: 1,
            target_rx: 10,
            target_ry: 10,
            queue: false,
        },
        Some(&rules),
        Some(&grid),
        &heights,
    );
    assert!(issued, "move command should issue");

    let e = sim.substrate.entities.get(1).expect("unit");
    assert_eq!(
        e.mission.queued(),
        MissionId::from_known(MissionType::Move),
        "the command queued Move through the exact authority (host promotes later)"
    );
    assert!(
        e.attack_target.is_none(),
        "Move tore down the attack target"
    );
    assert!(e.order_intent.is_none(), "Move tore down the order intent");
}
