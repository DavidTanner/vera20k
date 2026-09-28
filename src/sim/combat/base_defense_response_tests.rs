//! Focused native-contract tests for the House base-defence responder.

use super::*;
use crate::map::entities::EntityCategory;
use crate::map::resolved_terrain::{ResolvedTerrainCell, ResolvedTerrainGrid};
use crate::rules::ini_parser::IniFile;
use crate::rules::object_type::ObjectCategory;
use crate::rules::team_ai_ini::TeamAiDefinitionSource;
use crate::rules::terrain_rules::{SpeedCostProfile, TerrainClass};
use crate::sim::entity_store::EntityStore;
use crate::sim::game_entity::GameEntity;
use crate::sim::house_state::HouseState;
use crate::sim::intern::{test_intern, test_interner};
use crate::sim::pathfinding::PathGrid;
use crate::sim::pathfinding::zone_map::ZoneGrid;
use crate::sim::rng::SimRng;
use crate::sim::team_script_vm::{
    TeamMemberTypeIdentity, TeamScriptAction, TeamScriptDefinition, TeamScriptMember,
    TeamTaskForceDefinition, TeamTaskForceEntry, TeamTypeDefinition,
};
use crate::sim::timer::CdTimer;

fn threat(cost: i32, distance: i32, range: i32, speed: i32) -> ThreatFacts {
    ThreatFacts {
        cost,
        speed_leptons_per_frame: speed,
        current_coord: [0, 0, 0],
        attacker_coord: [distance, 0, 0],
        primary_range_leptons: range,
        existing_target: ExistingTargetDisposition::NoneOrUnarmed,
        in_non_base_defense_team: false,
        mission_is_harvest: false,
    }
}

fn clear_terrain(width: u16, height: u16) -> ResolvedTerrainGrid {
    let cells = (0..height)
        .flat_map(|ry| {
            (0..width).map(move |rx| ResolvedTerrainCell {
                rx,
                ry,
                source_tile_index: 0,
                source_sub_tile: 0,
                final_tile_index: 0,
                final_sub_tile: 0,
                is_wood_bridge_repair_tile: false,
                level: 0,
                filled_clear: false,
                tileset_index: Some(0),
                land_type: 0,
                yr_cell_land_type: 0,
                slope_type: 0,
                template_height: 0,
                height_in_pixels: 0,
                render_offset_x: 0,
                render_offset_y: 0,
                terrain_class: TerrainClass::Clear,
                speed_costs: SpeedCostProfile::default(),
                is_water: false,
                is_cliff_like: false,
                is_rough: false,
                is_road: false,
                accepts_smudge: false,
                allows_tiberium: false,
                variant: 0,
                has_ramp: false,
                canonical_ramp: None,
                ground_walk_blocked: false,
                terrain_object_blocks: false,
                terrain_object_occupation: None,
                overlay_blocks: false,
                overlay_zone_type: None,
                outside_playfield: false,
                zone_type: 0,
                base_ground_walk_blocked: false,
                base_build_blocked: false,
                base_land_type: 0,
                base_yr_cell_land_type: 0,
                base_terrain_class: TerrainClass::Clear,
                base_speed_costs: SpeedCostProfile::default(),
                has_bridge_deck: false,
                bridge_walkable: false,
                bridge_transition: false,
                bridge_deck_level: 0,
                bridge_layer: None,
                bridge_facts: crate::map::bridge_facts::BridgeCellFacts::default(),
                tube_index: None,
                radar_left: [0; 3],
                radar_right: [0; 3],
                has_damaged_data: false,
                bridgehead_anchor_class_at_load: None,
            })
        })
        .collect();
    ResolvedTerrainGrid::from_cells(width, height, cells)
}

#[test]
fn gsi_04_05_response_delay_is_the_native_ftol() {
    assert_eq!(response_delay_frames(0.25), 225);
    assert_eq!(response_delay_frames(-0.25), -225);
}

/// A game-mode-nonzero world at `frame` whose Victim and Enemy houses are
/// computer players, with the victim building (1) and the attacking unit (2).
/// Fixture names go through the thread-local test interner; [`respond`]
/// hands the world a copy of it.
fn response_world(frame: u32, attacker_at: (u16, u16)) -> Simulation {
    let mut sim = Simulation::new();
    sim.session.binary_frame = frame;
    sim.session.game_mode_nonzero = true;
    for (name, side) in [("Victim", 0), ("Enemy", 1)] {
        let id = test_intern(name);
        sim.houses
            .insert(id, HouseState::new(id, side, None, false, 0, 10));
    }
    let mut victim = GameEntity::test_default(1, "VICTIM", "Victim", 4, attacker_at.1);
    victim.category = EntityCategory::Structure;
    sim.substrate.entities.insert(victim);
    let mut attacker =
        GameEntity::test_default(2, "ATTACKER", "Enemy", attacker_at.0, attacker_at.1);
    attacker.lifecycle.in_limbo = false;
    attacker.lifecycle.cell_marked = true;
    sim.substrate.entities.insert(attacker);
    sim
}

/// An 8x8 clear map with its zone grid, which a positive scan needs.
fn with_clear_map(sim: &mut Simulation) {
    let terrain = clear_terrain(8, 8);
    let path_grid = PathGrid::from_resolved_terrain(&terrain);
    sim.zone_grid = Some(ZoneGrid::build_with_terrain(
        &path_grid,
        &terrain,
        &[],
        8,
        8,
    ));
    sim.resolved_terrain = Some(terrain);
    sim.session.map_width = 8;
    sim.session.map_height = 8;
}

/// The victim building (1) calls for help against the attacker (2).
fn respond(sim: &mut Simulation, rules: &RuleSet) {
    sim.interner = test_interner();
    respond_to_base_attack(sim, rules, 1, 2);
}

#[test]
fn gsi_04_05_zero_budget_still_suspends_low_priority_teams_before_scan_exit() {
    let rules = RuleSet::from_ini(&IniFile::from_str(
        "[General]\nComputerBaseDefenseResponse=0\nSuspendPriority=1\nSuspendDelay=2\n\
         [VehicleTypes]\n0=ATTACKER\n\
         [BuildingTypes]\n0=VICTIM\n\
         [ATTACKER]\nStrength=100\nArmor=heavy\nCost=100\n\
         [VICTIM]\nStrength=100\nArmor=wood\n",
    ))
    .expect("zero-budget response fixture");
    let mut sim = response_world(9, (6, 4));
    let victim_owner = test_intern("Victim");
    let member_type = test_intern("DEFENDER");
    let script_id = test_intern("LOW_SCRIPT");
    let task_force_id = test_intern("LOW_TASK_FORCE");
    let team_type_id = test_intern("LOW_TEAM");
    let teams = &mut sim.team_script_vm;
    let member_identity = TeamMemberTypeIdentity {
        category: ObjectCategory::Infantry,
        id: member_type,
    };
    teams.register_script(TeamScriptDefinition {
        id: script_id,
        source: TeamAiDefinitionSource::FixedAimd,
        actions: vec![TeamScriptAction {
            action_id: 2,
            argument: 0,
        }],
    });
    teams.register_task_force(TeamTaskForceDefinition {
        id: task_force_id,
        source: TeamAiDefinitionSource::FixedAimd,
        group: -1,
        entries: vec![TeamTaskForceEntry {
            member_type: member_identity,
            count: 1,
        }],
    });
    teams.register_team_type(TeamTypeDefinition {
        id: team_type_id,
        script_id,
        task_force_id,
        priority: 0,
        is_base_defense: false,
        suicide: false,
        aggressive: false,
        combined_movement_zone: crate::rules::locomotor_type::MovementZone::Fly,
        base_zone_relation_enforced: true,
        transport_crossing_required: false,
    });
    let team_id = teams.create_team_from_type(
        victim_owner,
        team_type_id,
        &[TeamScriptMember {
            entity_id: 99,
            member_type: member_identity,
        }],
        None,
        0,
    );
    sim.scenario_rng = SimRng::new(0x0405);
    let rng_before = sim.scenario_rng.logical_state();

    respond(&mut sim, &rules);

    assert!(
        sim.team_script_vm
            .team(team_id)
            .unwrap()
            .members()
            .is_empty()
    );
    assert_eq!(
        sim.team_script_vm
            .team(team_id)
            .unwrap()
            .response_suspension_state(),
        (true, true, true, 9, 1800)
    );
    assert_eq!(sim.scenario_rng.logical_state(), rng_before);
    assert_eq!(
        sim.substrate
            .entities
            .get(2)
            .unwrap()
            .base_defense_response
            .cooldown,
        CdTimer::from_raw(-1, 0)
    );
}

#[test]
fn gsi_04_05_positive_transaction_queues_in_order_and_arms_only_on_overshoot() {
    let rules = RuleSet::from_ini(&IniFile::from_str(
        "[General]\nComputerBaseDefenseResponse=3\nBaseDefenseDelay=.25\n\
         [VehicleTypes]\n0=ATTACKER\n1=DEFENDER\n\
         [BuildingTypes]\n0=VICTIM\n\
         [ATTACKER]\nStrength=100\nArmor=heavy\nCost=50\n\
         [DEFENDER]\nStrength=100\nArmor=heavy\nCost=100\nSpeed=4\nMovementZone=Normal\nPrimary=DEFENDERGUN\n\
         [VICTIM]\nStrength=100\nArmor=wood\n\
         [DEFENDERGUN]\nDamage=10\nRange=5\nWarhead=DEFENDERWH\n\
         [DEFENDERWH]\nVerses=100%,100%,100%,100%,100%,100%,100%,100%,100%,100%,100%\n",
    ))
    .expect("positive response fixture");
    let mut sim = response_world(41, (5, 0));
    with_clear_map(&mut sim);
    sim.substrate
        .entities
        .insert(GameEntity::test_default(3, "DEFENDER", "Victim", 2, 0));
    sim.substrate
        .entities
        .insert(GameEntity::test_default(4, "DEFENDER", "Victim", 3, 0));
    let seed = 0x504F_5349;
    sim.scenario_rng = SimRng::new(seed);
    let mut expected_rng = SimRng::new(seed);
    let expected_missions = [
        response_mission(expected_rng.next_range_u32_inclusive(0, 99), false),
        response_mission(expected_rng.next_range_u32_inclusive(0, 99), false),
    ];

    respond(&mut sim, &rules);

    for (id, expected_mission) in [3_u64, 4].into_iter().zip(expected_missions) {
        let responder = sim.substrate.entities.get(id).unwrap();
        let expected_mission = match expected_mission {
            ResponseMission::Rescue => MissionType::Rescue,
            ResponseMission::AreaGuard => MissionType::AreaGuard,
        };
        assert_eq!(
            responder.mission.queued(),
            MissionId::from_known(expected_mission)
        );
        assert_eq!(responder.archive_target(), Some(TargetKind::Entity(1)));
        assert_eq!(
            responder.attack_target.as_ref().map(|target| target.target),
            Some(TargetKind::Entity(2))
        );
    }
    assert_eq!(
        sim.scenario_rng.logical_state(),
        expected_rng.logical_state()
    );
    let attacker = sim.substrate.entities.get(2).unwrap();
    assert_eq!(
        attacker.base_defense_response.cooldown,
        CdTimer::started(41, 225)
    );
}

/// The weapon-0 peek is GetFireError itself (vt+0x3BC at `0x007084B8`), and
/// only ILLEGAL refuses. A `LandTargeting=1` defender's shot at an attacker
/// on land is ILLEGAL (T40), so it is never recruited; an ordinary defender
/// beside it is.
#[test]
fn gsi_04_05_a_defender_whose_weapon_zero_is_illegal_is_not_recruited() {
    let rules = RuleSet::from_ini(&IniFile::from_str(
        "[General]\nComputerBaseDefenseResponse=3\nBaseDefenseDelay=.25\n\
         [VehicleTypes]\n0=ATTACKER\n1=DEFENDER\n2=SEAGUARD\n\
         [BuildingTypes]\n0=VICTIM\n\
         [ATTACKER]\nStrength=100\nArmor=heavy\nCost=50\n\
         [DEFENDER]\nStrength=100\nArmor=heavy\nCost=100\nSpeed=4\nMovementZone=Normal\nPrimary=DEFENDERGUN\n\
         [SEAGUARD]\nStrength=100\nArmor=heavy\nCost=100\nSpeed=4\nMovementZone=Normal\nPrimary=DEFENDERGUN\n\
         LandTargeting=1\n\
         [VICTIM]\nStrength=100\nArmor=wood\n\
         [DEFENDERGUN]\nDamage=10\nRange=5\nWarhead=DEFENDERWH\n\
         [DEFENDERWH]\nVerses=100%,100%,100%,100%,100%,100%,100%,100%,100%,100%,100%\n",
    ))
    .expect("illegal responder fixture");
    let mut sim = response_world(41, (5, 0));
    with_clear_map(&mut sim);
    sim.substrate
        .entities
        .insert(GameEntity::test_default(3, "SEAGUARD", "Victim", 2, 0));
    sim.substrate
        .entities
        .insert(GameEntity::test_default(4, "DEFENDER", "Victim", 3, 0));
    let seed = 0x504F_5349;
    sim.scenario_rng = SimRng::new(seed);
    let mut expected_rng = SimRng::new(seed);
    expected_rng.next_range_u32_inclusive(0, 99);

    respond(&mut sim, &rules);

    let target_of = |id: u64| {
        sim.substrate
            .entities
            .get(id)
            .unwrap()
            .attack_target
            .as_ref()
            .map(|target| target.target)
    };
    assert_eq!(target_of(3), None);
    assert_eq!(target_of(4), Some(TargetKind::Entity(2)));
    assert_eq!(
        sim.scenario_rng.logical_state(),
        expected_rng.logical_state()
    );
}

#[test]
fn gsi_04_05_threat_preserves_special_targets_and_signed_integer_math() {
    let mut facts = threat(100, 1024, 256, 128);
    assert_eq!(evaluate_target_threat(facts), (100_i32 << 10) / 6);
    facts.attacker_coord = [256, 0, 0];
    assert_eq!(evaluate_target_threat(facts), 100_i32 << 10);
    facts.existing_target = ExistingTargetDisposition::RequestedAttacker;
    assert_eq!(evaluate_target_threat(facts), -100);
    facts.existing_target = ExistingTargetDisposition::OtherArmedTarget;
    assert_eq!(evaluate_target_threat(facts), 0);
    facts.existing_target = ExistingTargetDisposition::NoneOrUnarmed;
    facts.mission_is_harvest = true;
    assert_eq!(evaluate_target_threat(facts), 0);
}

#[test]
fn gsi_04_05_threat_uses_wrapping_base_and_native_sqrt_distance() {
    let facts = threat(i32::MAX, 513, 0, 256);
    assert_eq!(evaluate_target_threat(facts), 1);
    let diagonal = ThreatFacts {
        attacker_coord: [256, 256, 256],
        primary_range_leptons: 0,
        speed_leptons_per_frame: 1,
        cost: 2,
        ..threat(2, 0, 0, 1)
    };
    assert_eq!(distance_3d_leptons([0, 0, 0], [256, 256, 256]), 443);
    assert_eq!(evaluate_target_threat(diagonal), 4);
}

#[test]
fn gsi_04_05_negative_scores_debit_budget_with_class_specific_anchor_order() {
    let mut selection = ResponseSelection::new(500);
    selection.consider(1, -2, ResponderClass::Infantry, true);
    assert_eq!(selection.remaining_budget(), 300);
    selection.consider(2, -2, ResponderClass::Unit, true);
    assert_eq!(selection.remaining_budget(), 298);
    assert!(selection.can_scan());
    selection.consider(3, -298, ResponderClass::Unit, false);
    assert!(!selection.can_scan());
}

#[test]
fn gsi_04_05_first_six_leave_minimum_zero_and_seventh_only_exposes_minimum() {
    let mut selection = ResponseSelection::new(1);
    for (id, score) in [(1, 9), (2, 2), (3, 8), (4, 3), (5, 7), (6, 4)] {
        selection.consider(id, score, ResponderClass::Unit, false);
    }
    selection.consider(7, 100, ResponderClass::Unit, false);
    let (_, ranked) = selection.into_ranked();
    assert_eq!(
        ranked
            .iter()
            .map(|entry| entry.entity_id)
            .collect::<Vec<_>>(),
        [1, 3, 5, 6, 4, 2]
    );
}

#[test]
fn gsi_04_05_replacement_overwrites_every_old_minimum_with_duplicates() {
    let mut selection = ResponseSelection::new(1);
    for (id, score) in [(1, 9), (2, 2), (3, 2), (4, 8), (5, 7), (6, 6)] {
        selection.consider(id, score, ResponderClass::Unit, false);
    }
    selection.consider(7, 100, ResponderClass::Unit, false);
    selection.consider(8, 5, ResponderClass::Unit, false);
    let (_, ranked) = selection.into_ranked();
    assert_eq!(
        ranked.iter().filter(|entry| entry.entity_id == 8).count(),
        2
    );
    assert_eq!(
        ranked
            .iter()
            .map(|entry| entry.entity_id)
            .collect::<Vec<_>>(),
        [1, 4, 5, 6, 8, 8]
    );
}

#[test]
fn gsi_04_05_stable_descending_sort_retains_equal_score_order() {
    let mut selection = ResponseSelection::new(1);
    for (id, score) in [(1, 4), (2, 9), (3, 4), (4, 7)] {
        selection.consider(id, score, ResponderClass::Infantry, false);
    }
    let (_, ranked) = selection.into_ranked();
    assert_eq!(
        ranked
            .iter()
            .map(|entry| entry.entity_id)
            .collect::<Vec<_>>(),
        [2, 4, 1, 3]
    );
}

#[test]
fn gsi_04_05_draw_boundary_and_strict_budget_overshoot_are_literal() {
    assert_eq!(response_mission(65, false), ResponseMission::Rescue);
    assert_eq!(response_mission(66, false), ResponseMission::AreaGuard);
    assert_eq!(response_mission(0, true), ResponseMission::AreaGuard);

    assert_eq!(add_assigned_cost(0, 100, 100), (100, false));
    assert_eq!(add_assigned_cost(100, 1, 100), (101, true));
    assert_eq!(add_assigned_cost(i32::MAX, 1, -1), (i32::MIN, false));
}

/// `TechnoClass::Is_Armed @ 0x00701120` consults exactly ONE weapon slot:
/// `GetCurrentWeapon (vt+0x3F4, 0x0070E1A0)` asks `GetWeapon(0)` for a plain
/// type and `GetWeapon(CurrentWeaponNumber)` for a `TurretCount>0` one. A
/// `Secondary=` alone never arms an object, an 18-slot weapon array that is
/// entirely unauthored never arms one either, and `BuildingClass::Is_Armed
/// @ 0x00458DB0` arms an occupied building unconditionally.
#[test]
fn gsi_04_05_is_armed_reads_one_slot_not_the_whole_weapon_array() {
    let rules = RuleSet::from_ini(&IniFile::from_str(
        "[InfantryTypes]\n0=ARMED\n1=UNARMED\n\
         [VehicleTypes]\n0=GUNNER\n1=TURRETNOGUN\n\
         [WeaponTypes]\n0=Gun\n\
         [ARMED]\nStrength=100\nArmor=none\nCost=100\nPrimary=Gun\n\
         [UNARMED]\nStrength=100\nArmor=none\nCost=100\n\
         [GUNNER]\nStrength=100\nArmor=none\nCost=100\nTurretCount=4\nWeaponCount=2\n\
         Weapon1=Gun\nWeapon2=Gun\n\
         [TURRETNOGUN]\nStrength=100\nArmor=none\nCost=100\nTurretCount=4\nWeaponCount=0\n\
         Primary=Gun\n\
         [Gun]\nDamage=10\nROF=10\nRange=5\nWarhead=WH\n\
         [WH]\nVerses=100%,100%,100%,100%,100%,100%,100%,100%,100%,100%,100%\n",
    ))
    .expect("is_armed fixture");

    let armed = GameEntity::test_default(1, "ARMED", "Armed", 1, 1);
    let unarmed = GameEntity::test_default(2, "UNARMED", "Unarmed", 2, 1);
    let gunner = GameEntity::test_default(3, "GUNNER", "Gunner", 3, 1);
    // `TurretCount>0` takes the `WeaponN=` branch of `TechnoTypeClass::ReadINI
    // @ 0x007128B2` and never reads `Primary=`, and `WeaponCount=0` leaves
    // every slot NULL — so `GetWeapon(0)` has no `WeaponType`.
    let turret_no_gun = GameEntity::test_default(4, "TURRETNOGUN", "Empty turret", 4, 1);

    assert!(is_armed(&armed, rules.object("ARMED").expect("ARMED")));
    assert!(!is_armed(
        &unarmed,
        rules.object("UNARMED").expect("UNARMED")
    ));
    assert!(is_armed(&gunner, rules.object("GUNNER").expect("GUNNER")));
    assert!(!is_armed(
        &turret_no_gun,
        rules.object("TURRETNOGUN").expect("TURRETNOGUN")
    ));

    // `FootClass::Evaluate_Target_Threat @ 0x004D97A0` scores 0 only when the
    // candidate's existing target is an ARMED Techno; an unarmed one falls
    // through to the real distance/cost score.
    let mut entities = EntityStore::new();
    entities.insert(armed.clone());
    entities.insert(unarmed.clone());
    let mut candidate = GameEntity::test_default(9, "ARMED", "Responder", 9, 9);
    let interner = test_interner();

    candidate.attack_target = Some(crate::sim::combat::AttackTarget::new(1));
    assert_eq!(
        current_target_disposition(&candidate, 99, &entities, &rules, &interner),
        ExistingTargetDisposition::OtherArmedTarget
    );

    candidate.attack_target = Some(crate::sim::combat::AttackTarget::new(2));
    assert_eq!(
        current_target_disposition(&candidate, 99, &entities, &rules, &interner),
        ExistingTargetDisposition::NoneOrUnarmed
    );

    candidate.attack_target = Some(crate::sim::combat::AttackTarget::new(99));
    assert_eq!(
        current_target_disposition(&candidate, 99, &entities, &rules, &interner),
        ExistingTargetDisposition::RequestedAttacker
    );
}
