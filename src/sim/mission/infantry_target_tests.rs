//! Original E1 class setter controls for the native response/Rescue consumer.
//! Reciprocal+2A8 rows and unconsumed+68E/timer auxiliary words are excluded;
//! this compares the represented ordinary class receiver, not the whole setter.

use crate::map::entities::EntityCategory;
use crate::rules::ini_parser::IniFile;
use crate::rules::native_processing::{RulesLayerKind, RulesLayerStack};
use crate::rules::ruleset::RuleSet;
use crate::sim::animation::{Animation, SequenceKind};
use crate::sim::combat::{AttackTarget, TargetKind};
use crate::sim::components::Health;
use crate::sim::game_entity::GameEntity;
use crate::sim::rng::SimRng;
use crate::sim::world::Simulation;
use serde_json::Value;

fn corpus() -> Value {
    let metadata: Value = serde_json::from_str(include_str!(
        "../../../tools/spatial_oracle/base_defense_response.meta.json"
    ))
    .unwrap();
    assert_eq!(metadata["schema_version"], 1);
    assert_eq!(
        metadata["native_sha256"],
        "1cdd1180e49024fbda8ad568caac2e86e856063ff67ab38f62b7d2c7bb84298c"
    );
    serde_json::from_str(include_str!(
        "../../../tools/spatial_oracle/base_defense_response.json"
    ))
    .unwrap()
}

fn target(label: &str) -> Option<TargetKind> {
    match label {
        "null" => None,
        "new_target" => Some(TargetKind::Entity(2)),
        "old_target" => Some(TargetKind::Entity(3)),
        _ => panic!("undeclared native target {label}"),
    }
}

#[test]
fn native_infantry_target_assignment_matches_58_ordinary_original_rows() {
    compare_original_infantry_target_rows(false);
}

#[test]
fn ordered_attack_uses_the_class_setter_for_46_original_infantry_rows() {
    // Event4C7467 calls virtual+3C8 before its destination setter. Replay
    // eligible positive target rows through the production command consumer;
    // the class outputs remain the original51B1F0 execution, not Rust goldens.
    compare_original_infantry_target_rows(true);
}

fn compare_original_infantry_target_rows(via_command: bool) {
    let Some(ini) = crate::rules::retail_ini_fixture::retail_ini("rulesmd.ini") else {
        return;
    };
    let Some(art) = crate::rules::retail_ini_fixture::retail_ini("artmd.ini") else {
        return;
    };
    let registry = crate::rules::infantry_sequence::parse_infantry_sequence_registry(&art);
    let rules: std::collections::BTreeMap<_, _> = ["yes", "no"]
        .into_iter()
        .map(|raw| {
            let mut layers = RulesLayerStack::new(ini.clone());
            layers.push(
                RulesLayerKind::Scenario,
                IniFile::from_str(&format!("[E1]\nDeployFire={raw}\n")),
            );
            let mut rules =
                RuleSet::from_processed_rules(&layers.process_with_fixed_art(&art).unwrap())
                    .unwrap();
            rules.install_art_data(crate::rules::art_data::ArtRegistry::from_ini(&art));
            rules.bind_animation_sequences(&registry);
            (raw, rules)
        })
        .collect();
    let native = corpus();
    let evidence = &native["infantry_assignment"];
    assert_eq!(evidence["rows"].as_array().unwrap().len(), 61);
    let mut compared = 0;
    for row in evidence["rows"].as_array().unwrap() {
        let before = &row["before"];
        if before["pair2a8"] != "null" {
            continue;
        }
        let input = &row["input"];
        if via_command
            && (input["health"].as_i64().unwrap() <= 0
                || !matches!(
                    input["target"].as_str().unwrap(),
                    "assign" | "replace" | "same"
                ))
        {
            // A command refuses an inactive source before the class setter;
            // null-target command ordering has a separate event path.
            continue;
        }
        let name = input["name"].as_str().unwrap();
        let rules = &rules[input["deploy_fire_raw"].as_str().unwrap()];
        let mut sim = Simulation::new();
        let house = sim.interner.intern("Receiver");
        let mut actor = GameEntity::new_at_frame_zero_for_test(
            1,
            10,
            10,
            0,
            0,
            house,
            Health {
                current: input["health"].as_i64().unwrap() as i32,
            },
            sim.interner.intern("E1"),
            EntityCategory::Infantry,
            0,
            0,
            false,
        );
        actor.lifecycle.in_limbo = false;
        let doing = before["doing"].as_i64().unwrap() as i32;
        actor
            .mission_leaf
            .set_infantry_doing_verified(doing)
            .unwrap();
        actor
            .mission_leaf
            .set_foot_firing_sequence(before["firing_latch"].as_u64().unwrap() as u8);
        actor.infantry.as_mut().unwrap().is_prone = before["prone"].as_u64().unwrap() != 0;
        actor.set_object_is_falling_down_for_test(input["falling"].as_u64().unwrap() as u8);
        actor.passively_acquired_target = before["passive"].as_u64().unwrap() != 0;
        actor.attack_target =
            target(before["target"].as_str().unwrap()).map(|target| match target {
                TargetKind::Entity(id) => AttackTarget::new(id),
                _ => unreachable!(),
            });
        actor.navigation.path_replay.directions =
            vec![before["path_field"].as_i64().unwrap() as u8, 2, 3];
        let kind =
            crate::rules::infantry_sequence::action_kind(doing).unwrap_or(SequenceKind::Stand);
        let mut animation = Animation::new(kind);
        animation.frame_index = before["frame"].as_u64().unwrap() as u16;
        animation.elapsed_frames = before["action_timer"][2].as_u64().unwrap() as u16;
        actor.animation = Some(animation);
        sim.substrate.entities.insert(actor);
        for id in [2, 3] {
            let mut recipient = GameEntity::new_at_frame_zero_for_test(
                id,
                11,
                10,
                0,
                0,
                house,
                Health { current: 100 },
                sim.interner.intern("MTNK"),
                EntityCategory::Unit,
                0,
                0,
                true,
            );
            if via_command {
                // The event admits only out-of-limbo object tokens. This
                // additional caller premise does not change the represented
                // class setter's alive-target results.
                recipient.lifecycle.in_limbo = false;
            }
            sim.substrate.entities.insert(recipient);
        }
        let requested = match input["target"].as_str().unwrap() {
            "assign" | "replace" | "same" => Some(TargetKind::Entity(2)),
            "clear" | "same_null" => None,
            other => panic!("undeclared setter input {other}"),
        };
        sim.session.binary_frame = input["frame"].as_u64().unwrap() as u32;
        sim.scenario_rng = SimRng::new(input["seed"].as_u64().unwrap());
        assert_eq!(
            sim.scenario_rng.native_state_hex(),
            row["rng_before"],
            "{name}"
        );
        if via_command {
            assert!(sim.order_actor_admits(1), "{name}: actor admission");
            assert!(sim.order_object_token_admits(2), "{name}: target admission");
            assert!(
                sim.apply_command(
                    "Receiver",
                    &crate::sim::command::Command::ForceAttack {
                        attacker_id: 1,
                        target_id: 2,
                    },
                    Some(rules),
                    None,
                ),
                "{name}"
            );
        } else {
            sim.assign_target_represented(1, requested, Some(rules))
                .unwrap();
        }

        let after = &row["after"];
        let actor = sim.substrate.entities.get(1).unwrap();
        let leaf = actor.mission_leaf.as_infantry().unwrap();
        assert_eq!(
            leaf.doing(),
            after["doing"].as_i64().unwrap() as i32,
            "{name}"
        );
        assert_eq!(
            leaf.firing_sequence_latch(),
            after["firing_latch"].as_u64().unwrap() as u8,
            "{name}",
        );
        assert_eq!(
            actor.attack_target.as_ref().map(|attack| attack.target),
            target(after["target"].as_str().unwrap()),
            "{name}",
        );
        assert_eq!(
            actor.passively_acquired_target,
            after["passive"].as_u64().unwrap() != 0,
            "{name}",
        );
        assert_eq!(
            actor.infantry.as_ref().unwrap().is_prone,
            after["prone"].as_u64().unwrap() != 0,
            "{name}",
        );
        assert_eq!(
            actor.animation.as_ref().unwrap().frame_index,
            after["frame"].as_u64().unwrap() as u16,
            "{name}",
        );
        let head = actor
            .navigation
            .path_replay
            .remaining_directions()
            .first()
            .copied();
        let expected_head = (after["path_field"].as_i64().unwrap() != -1)
            .then(|| after["path_field"].as_u64().unwrap() as u8);
        assert_eq!(head, expected_head, "{name}");
        assert_eq!(
            sim.scenario_rng.native_state_hex(),
            row["rng_after"],
            "{name}"
        );
        compared += 1;
    }
    assert_eq!(compared, if via_command { 46 } else { 58 });
}
