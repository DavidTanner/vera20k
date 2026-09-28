use std::collections::BTreeMap;

use crate::rules::ini_parser::IniFile;
use crate::rules::ruleset::RuleSet;
use crate::rules::team_ai_ini::{TeamAiDefinitionSource, TeamAiIniRegistry};
use crate::sim::house_state::HouseState;
use crate::sim::snapshot::GameSnapshot;
use crate::sim::team_script_vm::{TeamAiInstallDiagnostic, TeamScriptDefinition};
use crate::util::native_x87::NativeF64Bits;

use super::{MasterFrameTestRung, Simulation};

fn zero_ai_trigger_comparison() -> String {
    "00".repeat(32)
}

/// A computer house `Computer` with two E1 at cells (10,10) and (12,10) on
/// flat ground, and a new team of TeamType `TT`: TaskForce `F` (2 E1),
/// script `S` (guard 15 frames, then success), AI trigger `A` (weight 40
/// within 10..60, success delta 5).
fn recruiting_team_fixture() -> (Simulation, RuleSet, u64, [u64; 2]) {
    let rules = RuleSet::from_ini(&IniFile::from_str(
        "[General]\nAITriggerSuccessWeightDelta=5\n\
         [InfantryTypes]\n0=E1\n[E1]\nStrength=100\n",
    ))
    .expect("minimal rules");
    let comparison = zero_ai_trigger_comparison();
    let aimd = IniFile::from_str(&format!(
        "[TeamTypes]\n0=TT\n[TT]\nScript=S\nTaskForce=F\n\
         [ScriptTypes]\n0=S\n[S]\n0=5,1\n1=49,0\n\
         [TaskForces]\n0=F\n[F]\n0=2,E1\n\
         [AITriggerTypes]\nA=Trigger,TT,<all>,2,4,<none>,{comparison},40,10,60,1,0,1,0,<none>,1,1,1\n"
    ));
    let registry = TeamAiIniRegistry::from_sources(&aimd, &IniFile::from_str(""), true);
    let mut sim = Simulation::with_seed(0xA11CE);
    crate::sim::arena_fixture::flat_ground(&mut sim, &rules);
    sim.intern_rule_type_ids(&rules);
    sim.resolve_type_handles(&rules);
    sim.install_team_ai_registry(&registry, &rules)
        .expect("clean fixed AIMD installs");
    let owner = sim.interner.intern("Computer");
    sim.houses
        .insert(owner, HouseState::new(owner, 0, None, false, 0, 10));
    sim.session.house_order = vec![owner];
    let members = [(10, 10), (12, 10)].map(|(rx, ry)| {
        sim.spawn_object_at_height("E1", "Computer", rx, ry, 0, 0, &rules)
            .expect("E1 spawns")
    });
    let team_type = sim.interner.get("TT").expect("TT interned");
    let team = sim
        .team_script_vm
        .construct_team(team_type, owner, true, sim.session.binary_frame as i32)
        .expect("no Max= limit");
    (sim, rules, team, members)
}

fn trigger_weight(sim: &Simulation) -> NativeF64Bits {
    let (_, record) = sim
        .team_script_vm
        .ai_triggers_in_order()
        .next()
        .expect("trigger A");
    record.weight()
}

#[test]
fn a_computer_team_recruits_forms_and_succeeds_in_the_master_frame() {
    let (mut sim, rules, team_id, members) = recruiting_team_fixture();
    let heights = BTreeMap::new();

    sim.advance_tick(&[], Some(&rules), &heights, None, None, 67);
    let trace = sim.take_master_frame_test_trace();
    assert_eq!(
        &trace[..4],
        &[
            MasterFrameTestRung::SessionCommands,
            MasterFrameTestRung::Triggers,
            MasterFrameTestRung::TeamScript,
            MasterFrameTestRung::LogicVector,
        ],
        "native TeamClass AI must finish before any live LogicClass object visit"
    );

    let mut formed_with = None;
    for _ in 0..40 {
        let Some(team) = sim.team_script_vm.team(team_id) else {
            break;
        };
        if team.formed() && formed_with.is_none() {
            formed_with = Some(team.members().collect::<Vec<_>>());
        }
        sim.advance_tick(&[], Some(&rules), &heights, None, None, 67);
    }
    let mut recruited = formed_with.expect("the team formed");
    recruited.sort_unstable();
    assert_eq!(recruited, members, "both E1 were recruited");
    assert!(
        sim.team_script_vm.team(team_id).is_none(),
        "the script's end destroys the team"
    );
    for member in members {
        assert_eq!(sim.team_script_vm.team_for_member(member), None);
    }
    assert_eq!(
        trigger_weight(&sim),
        NativeF64Bits::from_bits(45.0f64.to_bits()),
        "its destruction after action 49 counts as the trigger's success"
    );
}

#[test]
fn a_recruiting_team_survives_save_load() {
    let (mut original, rules, team_id, _) = recruiting_team_fixture();
    let heights = BTreeMap::new();
    original.advance_tick(&[], Some(&rules), &heights, None, None, 67);
    assert_eq!(
        original
            .team_script_vm
            .team(team_id)
            .unwrap()
            .member_count(),
        1,
        "one recruit per short entry per update"
    );

    let bytes = GameSnapshot::save_validated(&original, 0, 0, "team_vm_test", 0);
    let mut restored = GameSnapshot::load(&bytes).expect("snapshot").sim;
    restored
        .restore_after_snapshot_load()
        .expect("references resolve");
    // A save carries no map: the load re-installs the scenario's cells.
    crate::sim::arena_fixture::flat_ground(&mut restored, &rules);
    // Retail's save reader reinitializes the Scenario RNG; align the control
    // run to that load contract before comparing the continuation.
    original.scenario_rng = crate::sim::rng::SimRng::new(0);
    assert_eq!(original.state_hash(), restored.state_hash());

    for _ in 0..40 {
        let expected = original.advance_tick(&[], Some(&rules), &heights, None, None, 67);
        let actual = restored.advance_tick(&[], Some(&rules), &heights, None, None, 67);
        assert_eq!(expected.state_hash, actual.state_hash);
    }
    assert!(restored.team_script_vm.team(team_id).is_none());
    assert_eq!(trigger_weight(&original), trigger_weight(&restored));
}

#[test]
fn team_member_order_is_hashed() {
    let mut forward = Simulation::with_seed(7);
    let mut reverse = Simulation::with_seed(7);
    for (sim, members) in [
        (&mut forward, vec![19, 7, 11]),
        (&mut reverse, vec![11, 7, 19]),
    ] {
        let owner = sim.interner.intern("Americans");
        let script = sim.interner.intern("TEAM_OPENING");
        sim.team_script_vm.register_script(TeamScriptDefinition {
            id: script,
            source: TeamAiDefinitionSource::FixedAimd,
            actions: Vec::new(),
        });
        sim.team_script_vm.create_team(owner, script, members, 0);
    }

    assert_ne!(forward.state_hash(), reverse.state_hash());
}

#[test]
fn production_install_boundary_resolves_aimd_without_creating_a_team() {
    let rules = RuleSet::from_ini(&IniFile::from_str(
        "[InfantryTypes]\n0=E1\n[E1]\nStrength=100\n",
    ))
    .expect("minimal rules");
    let comparison = zero_ai_trigger_comparison();
    let aimd = IniFile::from_str(&format!(
        "[TeamTypes]\n0=TT\n[TT]\nScript=S\nTaskForce=F\n\
         [ScriptTypes]\n0=S\n[S]\n0=2,0\n\
         [TaskForces]\n0=F\n[F]\n0=1,E1\n\
         [AITriggerTypes]\nA=Trigger,TT,<all>,2,4,<none>,{comparison},40,10,40,1,0,1,0,<none>,1,1,1\n"
    ));
    let registry = TeamAiIniRegistry::from_sources(&aimd, &IniFile::from_str(""), true);
    let mut sim = Simulation::new();
    sim.intern_rule_type_ids(&rules);
    sim.resolve_type_handles(&rules);

    let diagnostics = sim
        .install_team_ai_registry(&registry, &rules)
        .expect("clean fixed AIMD installs");

    assert!(diagnostics.is_empty());
    assert_eq!(sim.team_script_vm.registry_counts(), (1, 1, 1, 1));
    assert!(
        sim.team_script_vm.team(1).is_none(),
        "definition installation must not allocate a live TeamClass"
    );
}

#[test]
fn production_install_refuses_fixed_resolution_loss_but_keeps_scenario_omissions() {
    let rules = RuleSet::from_ini(&IniFile::from_str(
        "[InfantryTypes]\n0=E1\n[E1]\nStrength=100\n",
    ))
    .expect("minimal rules");
    let comparison = zero_ai_trigger_comparison();
    let fixed_with_unknown = IniFile::from_str(&format!(
        "[TeamTypes]\n0=TT\n[TT]\nScript=S\nTaskForce=F\n\
         [ScriptTypes]\n0=S\n[S]\n0=2,0\n\
         [TaskForces]\n0=F\n[F]\n0=1,GHOST\n\
         [AITriggerTypes]\nA=Trigger,TT,<all>,2,4,<none>,{comparison},40,10,40,1,0,1,0,<none>,1,1,1\n"
    ));
    let fixed_registry =
        TeamAiIniRegistry::from_sources(&fixed_with_unknown, &IniFile::from_str(""), true);
    assert!(fixed_registry.fixed_source_is_complete());

    let mut fixed_sim = Simulation::new();
    fixed_sim.intern_rule_type_ids(&rules);
    fixed_sim.resolve_type_handles(&rules);
    let fixed_diagnostics = fixed_sim
        .install_team_ai_registry(&fixed_registry, &rules)
        .expect_err("a fixed-origin resolution loss refuses the install");

    assert_eq!(
        fixed_diagnostics,
        vec![TeamAiInstallDiagnostic::UnknownTaskForceMember {
            task_force_id: "F".to_string(),
            member_type: "GHOST".to_string(),
            source: TeamAiDefinitionSource::FixedAimd,
        }]
    );
    assert!(fixed_diagnostics[0].is_fixed_source_refusal());
    assert_eq!(
        fixed_sim.team_script_vm.registry_counts(),
        (0, 0, 0, 0),
        "a fixed-origin resolution refusal must not install a partial registry"
    );

    let clean_fixed = IniFile::from_str(&format!(
        "[TeamTypes]\n0=TT\n[TT]\nScript=S\nTaskForce=F\n\
         [ScriptTypes]\n0=S\n[S]\n0=2,0\n\
         [TaskForces]\n0=F\n[F]\n0=1,E1\n\
         [AITriggerTypes]\nA=Trigger,TT,<all>,2,4,<none>,{comparison},40,10,40,1,0,1,0,<none>,1,1,1\n"
    ));
    let scenario = IniFile::from_str("[TaskForces]\n0=MAP_F\n[MAP_F]\n0=1,GHOST\n");
    let scenario_registry = TeamAiIniRegistry::from_sources(&clean_fixed, &scenario, true);
    assert!(scenario_registry.fixed_source_is_complete());

    let mut scenario_sim = Simulation::new();
    scenario_sim.intern_rule_type_ids(&rules);
    scenario_sim.resolve_type_handles(&rules);
    let scenario_diagnostics = scenario_sim
        .install_team_ai_registry(&scenario_registry, &rules)
        .expect("scenario-origin omissions still install");

    assert_eq!(
        scenario_diagnostics,
        vec![TeamAiInstallDiagnostic::UnknownTaskForceMember {
            task_force_id: "MAP_F".to_string(),
            member_type: "GHOST".to_string(),
            source: TeamAiDefinitionSource::Scenario,
        }]
    );
    assert!(!scenario_diagnostics[0].is_fixed_source_refusal());
    assert_eq!(
        scenario_sim.team_script_vm.registry_counts(),
        (2, 1, 1, 1),
        "scenario-origin omissions remain diagnosed, nonfatal overlays"
    );
}

#[test]
fn production_install_refuses_unknown_fixed_ai_trigger_object() {
    let rules = RuleSet::from_ini(&IniFile::from_str(
        "[InfantryTypes]\n0=E1\n[E1]\nStrength=100\n",
    ))
    .expect("minimal rules");
    let comparison = zero_ai_trigger_comparison();
    let fixed = IniFile::from_str(&format!(
        "[TeamTypes]\n0=TT\n[TT]\nScript=S\nTaskForce=F\n\
         [ScriptTypes]\n0=S\n[S]\n0=2,0\n\
         [TaskForces]\n0=F\n[F]\n0=1,E1\n\
         [AITriggerTypes]\nA=Trigger,TT,<all>,2,4,GHOST,{comparison},40,10,40,1,0,1,0,<none>,1,1,1\n"
    ));
    let registry = TeamAiIniRegistry::from_sources(&fixed, &IniFile::from_str(""), true);
    assert!(registry.fixed_source_is_complete());
    let mut sim = Simulation::new();
    sim.intern_rule_type_ids(&rules);
    sim.resolve_type_handles(&rules);

    let diagnostics = sim
        .install_team_ai_registry(&registry, &rules)
        .expect_err("a fixed-origin resolution loss refuses the install");

    assert_eq!(
        diagnostics,
        vec![TeamAiInstallDiagnostic::UnknownAiTriggerObject {
            trigger_id: "A".to_string(),
            object_type: "GHOST".to_string(),
            source: TeamAiDefinitionSource::FixedAimd,
        }]
    );
    assert!(diagnostics[0].is_fixed_source_refusal());
    assert_eq!(
        sim.team_script_vm.registry_counts(),
        (0, 0, 0, 0),
        "unknown fixed AITrigger token-6 references must refuse the whole registry install"
    );
}

#[test]
fn production_install_refuses_fixed_resolution_loss_masked_by_same_identity_map_overlays() {
    let rules = RuleSet::from_ini(&IniFile::from_str(
        "[InfantryTypes]\n0=E1\n[E1]\nStrength=100\n",
    ))
    .expect("minimal rules");
    let comparison = zero_ai_trigger_comparison();
    let fixed = IniFile::from_str(&format!(
        "[TeamTypes]\n0=TT\n[TT]\nScript=MISSING_SCRIPT\nTaskForce=F\nPriority=5\n\
         [ScriptTypes]\n0=S\n[S]\n0=2,0\n\
         [TaskForces]\n0=F\n[F]\n0=1,GHOST\n\
         [AITriggerTypes]\nA=Fixed bad,TT,<all>,2,4,GHOST,{comparison},40,10,40,1,0,1,0,<none>,1,1,1\n"
    ));
    let scenario = IniFile::from_str(&format!(
        "[TeamTypes]\n0=TT\n[TT]\nPriority=20\n\
         [TaskForces]\n0=F\n[F]\n0=1,E1\n\
         [AITriggerTypes]\nA=Map repair,TT,<all>,2,4,E1,{comparison},40,10,40,1,0,1,0,<none>,1,1,1\n"
    ));
    let registry = TeamAiIniRegistry::from_sources(&fixed, &scenario, true);
    assert!(registry.fixed_source_is_complete());
    let mut sim = Simulation::new();
    sim.intern_rule_type_ids(&rules);
    sim.resolve_type_handles(&rules);

    let diagnostics = sim
        .install_team_ai_registry(&registry, &rules)
        .expect_err("a fixed-origin resolution loss refuses the install");

    assert_eq!(
        diagnostics,
        vec![
            TeamAiInstallDiagnostic::UnknownTaskForceMember {
                task_force_id: "F".to_string(),
                member_type: "GHOST".to_string(),
                source: TeamAiDefinitionSource::FixedAimd,
            },
            TeamAiInstallDiagnostic::UnknownAiTriggerObject {
                trigger_id: "A".to_string(),
                object_type: "GHOST".to_string(),
                source: TeamAiDefinitionSource::FixedAimd,
            },
        ]
    );
    assert!(
        diagnostics
            .iter()
            .all(TeamAiInstallDiagnostic::is_fixed_source_refusal)
    );
    assert_eq!(
        sim.team_script_vm.registry_counts(),
        (0, 0, 0, 0),
        "map repair/relabeling cannot erase fixed-AIMD resolution obligations"
    );
}
