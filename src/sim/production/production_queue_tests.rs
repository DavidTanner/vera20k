//! Production queue tests — verifies build queue ordering, credit deduction, prerequisite
//! checks, multi-factory speed bonus, and queue pause/resume behavior.

use super::{
    BuildQueueState, ProductionCategory, build_options_for_owner, cancel_by_type_for_owner,
    credits_for_owner, enqueue_by_type, queue_view_for_owner, suspend_production, tick_production,
};
use crate::rules::ini_parser::IniFile;
use crate::rules::locomotor_type::SpeedType;
use crate::rules::ruleset::RuleSet;
use crate::sim::intern::InternedId;
use crate::sim::pathfinding::PathGrid;
use crate::sim::pathfinding::terrain_cost::TerrainCostGrid;
use crate::sim::rng::SimRng;
use crate::sim::timer::CdTimer;
use crate::sim::world::{SimSoundEvent, Simulation};

// Re-use test helpers from the main production_tests module.
// P5d: build state is now constructed by ARMING the registry (`arm_build_via`), not by
// inserting `BuildQueueItem`s into the retired `queues_by_owner`.
use super::tests::{
    arm_build_via, basic_infantry_rules, basic_multi_queue_rules, build_catalog_rules,
    naval_production_rules, production_modifier_rules, spawn_structure, water_terrain,
};

fn factory_constructor_rules() -> RuleSet {
    RuleSet::from_ini(&IniFile::from_str(
        "[InfantryTypes]\n0=E1\n1=E2\n\
         [VehicleTypes]\n0=MTNK\n\
         [AircraftTypes]\n\
         [BuildingTypes]\n0=GAPILE\n1=GAWEAP\n\
         [E1]\nCost=200\nStrength=100\nSpeed=4\nTechLevel=1\nOwner=Americans\n\
         [E2]\nCost=300\nStrength=125\nSpeed=4\nTechLevel=1\nOwner=Americans\n\
         [MTNK]\nCost=700\nStrength=300\nSpeed=6\nTechLevel=1\nOwner=Americans\n\
         [GAPILE]\nFactory=InfantryType\n\
         [GAWEAP]\nFactory=UnitType\n",
    ))
    .expect("factory constructor rules")
}

#[test]
fn factory_constructor_start_cancel_and_promotion_own_scenario_words() {
    let seed = 0xFAC7_0001;
    let rules = factory_constructor_rules();
    let mut sim = Simulation::with_seed(seed);
    sim.intern_rule_type_ids(&rules);
    sim.resolve_type_handles(&rules);
    let owner = sim.interner.intern("Americans");
    sim.houses.insert(
        owner,
        crate::sim::house_state::HouseState::new(owner, 0, None, true, 50_000, 10),
    );
    spawn_structure(&mut sim, 1, "Americans", "GAPILE", 10, 10);
    spawn_structure(&mut sim, 2, "Americans", "GAWEAP", 14, 10);

    let mut expected = SimRng::new(seed);
    let mtnk_word = (expected.next_u32() & 0xFFFF) as u16;
    assert!(enqueue_by_type(&mut sim, &rules, "Americans", "MTNK"));
    let vehicle = sim
        .production
        .factory_shadow
        .view(owner, ProductionCategory::Vehicle)
        .and_then(|view| view.object.cloned())
        .expect("vehicle constructed at StartProduction");
    let vehicle_id = vehicle.entity_id.expect("Factory+0x58 identity");
    let vehicle_entity = sim.substrate.entities.get(vehicle_id).unwrap();
    assert!(vehicle_entity.lifecycle.in_limbo);
    assert!(!vehicle_entity.lifecycle.cell_marked);
    assert!(!vehicle_entity.in_logic_vector);
    assert!(!vehicle_entity.in_playfield);
    assert_eq!(vehicle_entity.techno_ctor_random_word, mtnk_word);

    let e1_word = (expected.next_u32() & 0xFFFF) as u16;
    assert!(enqueue_by_type(&mut sim, &rules, "Americans", "E1"));
    let infantry = sim
        .production
        .factory_shadow
        .view(owner, ProductionCategory::Infantry)
        .and_then(|view| view.object.cloned())
        .expect("infantry constructed at StartProduction");
    let e1_id = infantry.entity_id.expect("Factory+0x58 identity");
    let e1_entity = sim.substrate.entities.get(e1_id).unwrap();
    assert!(e1_entity.lifecycle.in_limbo);
    assert!(!e1_entity.lifecycle.cell_marked);
    assert!(!e1_entity.in_logic_vector);
    assert!(!e1_entity.in_playfield);
    assert_eq!(e1_entity.techno_ctor_random_word, e1_word);

    assert!(enqueue_by_type(&mut sim, &rules, "Americans", "E2"));
    assert_eq!(
        sim.scenario_rng.logical_state(),
        expected.logical_state(),
        "a queued tail has not reached StartProduction and must not construct"
    );

    let e2_word = (expected.next_u32() & 0xFFFF) as u16;
    assert!(cancel_by_type_for_owner(
        &mut sim,
        &rules,
        "Americans",
        "E1",
        false
    ));
    assert!(sim.substrate.entities.get(e1_id).is_none());
    let promoted = sim
        .production
        .factory_shadow
        .view(owner, ProductionCategory::Infantry)
        .and_then(|view| view.object.cloned())
        .expect("E2 promoted through StartNextQueued");
    let e2_id = promoted.entity_id.expect("promoted Factory+0x58 identity");
    assert_eq!(promoted.type_id, sim.interner.get("E2").unwrap());
    assert_eq!(
        sim.substrate
            .entities
            .get(e2_id)
            .unwrap()
            .techno_ctor_random_word,
        e2_word
    );
    assert!(sim.substrate.entities.get(vehicle_id).is_some());
    assert_eq!(sim.scenario_rng.logical_state(), expected.logical_state());
}

/// `HouseClass::CanBuild @ 0x004F7870` caps a type's TechLevel at the
/// house's own (`HouseClass+0x1D4`), which the match options set.
#[test]
fn a_house_builds_only_up_to_its_own_tech_level() {
    let mut sim = Simulation::new();
    let rules = build_catalog_rules();
    sim.intern_rule_type_ids(&rules);
    sim.resolve_type_handles(&rules);
    let americans = sim.interner.intern("Americans");
    spawn_structure(&mut sim, 1, "Americans", "GAPILE", 10, 10);
    let e1 = sim.interner.intern("E1");
    // An UnbuildableTechLevel option is hidden from the sidebar list.
    for (house_tech_level, listed_enabled) in [(0, None), (1, Some(true))] {
        sim.houses.insert(
            americans,
            crate::sim::house_state::HouseState::new(
                americans,
                0,
                None,
                true,
                50_000,
                house_tech_level,
            ),
        );
        let options = build_options_for_owner(&sim, &rules, "Americans");
        assert_eq!(
            options
                .iter()
                .find(|option| option.type_id == e1)
                .map(|option| option.enabled),
            listed_enabled,
            "house TechLevel {house_tech_level}"
        );
    }
}

#[test]
fn build_catalog_exposes_sidebar_categories_and_required_houses() {
    let mut sim = Simulation::new();
    let rules = build_catalog_rules();
    // Pre-intern all rule type IDs so build_options_for_owner can resolve them.
    sim.intern_rule_type_ids(&rules);
    sim.resolve_type_handles(&rules);

    for (side, name) in [(0, "Americans"), (0, "Alliance")] {
        let id = sim.interner.intern(name);
        sim.houses.insert(
            id,
            crate::sim::house_state::HouseState::new(id, side, None, true, 50_000, 10),
        );
    }
    spawn_structure(&mut sim, 1, "Americans", "GAPILE", 10, 10);
    spawn_structure(&mut sim, 2, "Americans", "GAWEAP", 12, 10);
    spawn_structure(&mut sim, 3, "Americans", "GAAIRC", 14, 10);
    spawn_structure(&mut sim, 4, "Americans", "GACNST", 16, 10);
    spawn_structure(&mut sim, 5, "Alliance", "GAPILE", 20, 10);
    spawn_structure(&mut sim, 6, "Alliance", "GAWEAP", 22, 10);
    spawn_structure(&mut sim, 7, "Alliance", "GAAIRC", 24, 10);
    spawn_structure(&mut sim, 8, "Alliance", "GACNST", 26, 10);

    let americans = build_options_for_owner(&sim, &rules, "Americans");
    let alliance = build_options_for_owner(&sim, &rules, "Alliance");

    // Americans should see all items (they satisfy RequiredHouses for GTUR).
    assert_eq!(
        americans
            .iter()
            .map(|opt| opt.queue_category)
            .collect::<Vec<_>>(),
        vec![
            ProductionCategory::Building,
            ProductionCategory::Building,
            ProductionCategory::Defense,
            ProductionCategory::Infantry,
            ProductionCategory::Vehicle,
            ProductionCategory::Vehicle,
            ProductionCategory::Aircraft,
        ]
    );
    assert!(
        americans
            .iter()
            .filter(|opt| {
                matches!(
                    opt.queue_category,
                    ProductionCategory::Infantry
                        | ProductionCategory::Vehicle
                        | ProductionCategory::Aircraft
                )
            })
            .all(|opt| opt.enabled)
    );

    // Alliance should NOT see GTUR — it has RequiredHouses=Americans,
    // so it's hidden (not just greyed out) per RA2 behavior.
    assert!(
        alliance
            .iter()
            .find(|opt| opt.type_id == sim.interner.intern("GTUR"))
            .is_none(),
        "GTUR should be hidden for Alliance (RequiredHouses=Americans)"
    );

    let americans_yard = americans
        .iter()
        .find(|opt| opt.type_id == sim.interner.intern("GACNST"))
        .expect("construction yard should be listed");
    assert!(americans_yard.enabled);
    assert_eq!(americans_yard.reason, None);

    let americans_turret = americans
        .iter()
        .find(|opt| opt.type_id == sim.interner.intern("GTUR"))
        .expect("defense should be listed for Americans");
    assert!(americans_turret.enabled);
    assert_eq!(americans_turret.reason, None);
}

#[test]
fn named_skirmish_owner_uses_country_for_build_permissions() {
    let mut sim = Simulation::new();
    let rules = build_catalog_rules();
    sim.intern_rule_type_ids(&rules);
    sim.resolve_type_handles(&rules);

    let owner_id = sim.interner.intern("Commander");
    let country_id = sim.interner.intern("Americans");
    sim.houses.insert(
        owner_id,
        crate::sim::house_state::HouseState::new(
            owner_id,
            0,
            Some(country_id),
            true,
            super::production_types::STARTING_CREDITS,
            10,
        ),
    );
    spawn_structure(&mut sim, 1, "Commander", "GACNST", 10, 10);

    let options = build_options_for_owner(&sim, &rules, "Commander");
    let yard = options
        .iter()
        .find(|opt| opt.type_id == sim.interner.intern("GACNST"))
        .expect("country-matched owner should see Allied construction options");
    assert!(yard.enabled);

    let turret = options
        .iter()
        .find(|opt| opt.type_id == sim.interner.intern("GTUR"))
        .expect("RequiredHouses=Americans should match the house country");
    assert!(turret.enabled);
}

#[test]
#[ignore = "WIP: MCV deploy build-option unlock not yet landed"]
fn deployed_mcv_unlocks_building_options_for_named_skirmish_owner() {
    let mut sim = Simulation::new();
    let ini = IniFile::from_str(
        "[InfantryTypes]\n\
         [VehicleTypes]\n\
         0=AMCV\n\
         [AircraftTypes]\n\
         [BuildingTypes]\n\
         0=GACNST\n\
         1=GAPOWR\n\
         [AMCV]\n\
         Strength=450\n\
         Armor=heavy\n\
         Speed=5\n\
         TechLevel=1\n\
         Owner=Americans\n\
         DeploysInto=GACNST\n\
         [GACNST]\n\
         Name=Construction Yard\n\
         Cost=3000\n\
         Strength=1000\n\
         Armor=wood\n\
         TechLevel=1\n\
         Owner=Americans\n\
         Foundation=4x3\n\
         Factory=BuildingType\n\
         [GAPOWR]\n\
         Name=Power Plant\n\
         Cost=800\n\
         Strength=750\n\
         Armor=wood\n\
         TechLevel=1\n\
         Owner=Americans\n\
         Foundation=2x2\n\
         Prerequisite=GACNST\n",
    );
    let rules = RuleSet::from_ini(&ini).expect("rules should parse");
    sim.intern_rule_type_ids(&rules);
    sim.resolve_type_handles(&rules);

    let owner_id = sim.interner.intern("Commander");
    let country_id = sim.interner.intern("Americans");
    sim.houses.insert(
        owner_id,
        crate::sim::house_state::HouseState::new(
            owner_id,
            0,
            Some(country_id),
            true,
            super::production_types::STARTING_CREDITS,
            10,
        ),
    );
    let mcv = sim
        .spawn_object("AMCV", "Commander", 20, 22, 64, &rules)
        .expect("MCV should spawn");
    assert!(sim.deploy_mcv(mcv, &rules, None));

    for _ in 0..30 {
        sim.advance_tick(&[], Some(&rules), None, None, 33);
    }

    let options = build_options_for_owner(&sim, &rules, "Commander");
    let power = options
        .iter()
        .find(|opt| opt.type_id == sim.interner.intern("GAPOWR"))
        .expect("deployed yard should unlock first building");
    assert!(power.enabled);
    assert_eq!(power.reason, None);
}

/// `Time_To_Build` reads the owner's power (House `+0x53A4`/`+0x53A8`) and the
/// owner's factory count for the object's class (`0x00500910`).
#[test]
fn build_time_inputs_read_owner_power_and_matching_factories() {
    let mut sim = Simulation::new();
    let rules = production_modifier_rules();

    spawn_structure(&mut sim, 1, "Americans", "GAPILE", 10, 10);
    spawn_structure(&mut sim, 2, "Americans", "GAPILE", 12, 10);
    spawn_structure(&mut sim, 3, "Americans", "GAWEAP", 14, 10);
    spawn_structure(&mut sim, 4, "Soviet", "NAHAND", 20, 20);
    spawn_structure(&mut sim, 5, "Soviet", "GAPOWR", 22, 20);
    crate::sim::power_system::tick_power_states(
        &mut sim.power_states,
        &mut sim.substrate.entities,
        &rules,
        &sim.interner,
    );

    let americans = sim.interner.intern("Americans");
    let soviet = sim.interner.intern("Soviet");
    let e1 = rules.object("E1").expect("E1");
    let mtnk = rules.object("MTNK").expect("MTNK");
    let inputs = |owner, category, obj| {
        super::factory::time_to_build_inputs(&sim, &rules, owner, category, obj)
    };

    let americans_infantry = inputs(americans, ProductionCategory::Infantry, e1);
    assert_eq!(
        (
            americans_infantry.power_output,
            americans_infantry.power_drain
        ),
        (0, 60)
    );
    assert_eq!(americans_infantry.factory_count, 2);
    assert_eq!(
        inputs(americans, ProductionCategory::Vehicle, mtnk).factory_count,
        1
    );
    let soviet_infantry = inputs(soviet, ProductionCategory::Infantry, e1);
    assert_eq!(
        (soviet_infantry.power_output, soviet_infantry.power_drain),
        (200, 20)
    );
    assert_eq!(soviet_infantry.factory_count, 1);
    assert!(!soviet_infantry.wall);

    // A factory counts from its Unlimbo (`0x00440D13`), so one still playing
    // its build-up animation counts.
    sim.substrate
        .entities
        .get_mut(2)
        .unwrap()
        .install_building_up(
            crate::sim::components::BuildingUp::completing_in_ticks(30, 0),
            0,
        );
    assert_eq!(
        super::factory::time_to_build_inputs(
            &sim,
            &rules,
            americans,
            ProductionCategory::Infantry,
            e1
        )
        .factory_count,
        2
    );
}

/// A `Wall=yes` building takes `WallBuildSpeedCoefficient=` (Time_To_Build's
/// branch on BuildingType `+0x1571`, `0x006F4929..0x006F493D`); every building
/// factory counts.
#[test]
fn wall_build_time_inputs_carry_the_wall_coefficient() {
    let mut sim = Simulation::new();
    let ini = IniFile::from_str(
        "[General]\n\
             BuildSpeed=1.0\n\
             MultipleFactory=0.8\n\
             WallBuildSpeedCoefficient=0.5\n\
             [InfantryTypes]\n\
             [VehicleTypes]\n\
             [AircraftTypes]\n\
             [BuildingTypes]\n\
             0=GACNST\n\
             1=NACNST\n\
             2=GAWALL\n\
             [GACNST]\n\
             Factory=BuildingType\n\
             Owner=Americans\n\
             [NACNST]\n\
             Factory=BuildingType\n\
             Owner=Americans\n\
             [GAWALL]\n\
             Name=Wall\n\
             Cost=1000\n\
             Strength=100\n\
             Armor=wood\n\
             TechLevel=1\n\
             Owner=Americans\n\
             Wall=yes\n",
    );
    let rules = RuleSet::from_ini(&ini).expect("wall rules should parse");

    spawn_structure(&mut sim, 1, "Americans", "GACNST", 10, 10);
    spawn_structure(&mut sim, 2, "Americans", "NACNST", 12, 10);

    let americans = sim.interner.intern("Americans");
    let wall = rules.object("GAWALL").expect("wall should exist");
    let inputs = super::factory::time_to_build_inputs(
        &sim,
        &rules,
        americans,
        ProductionCategory::Building,
        wall,
    );
    assert!(inputs.wall);
    assert_eq!(inputs.factory_count, 2);
    assert_eq!(
        inputs.wall_coefficient,
        crate::util::native_x87::NativeF64Bits::HALF
    );
    let tower = rules.object("GACNST").expect("GACNST");
    assert!(
        !super::factory::time_to_build_inputs(
            &sim,
            &rules,
            americans,
            ProductionCategory::Building,
            tower,
        )
        .wall
    );
}

#[test]
fn naval_unit_rally_uses_water_pathing_after_spawn() {
    let mut sim = Simulation::new();
    let rules = naval_production_rules();
    let terrain = water_terrain(32, 32);
    let grid = PathGrid::from_resolved_terrain(&terrain);
    sim.install_fixture_path_grid(Some(&grid));
    sim.resolved_terrain = Some(terrain.clone());
    sim.playfield_bounds = Some(crate::sim::cell_rect::PlayfieldBounds {
        base: 0,
        off_fc: -100,
        off_100: -100,
        off_104: 200,
        off_108: 200,
    });
    sim.terrain_costs.insert(
        SpeedType::Float,
        TerrainCostGrid::from_resolved_terrain(&terrain, SpeedType::Float),
    );
    spawn_structure(&mut sim, 1, "Americans", "GAYARD", 20, 20);
    sim.substrate
        .entities
        .get_mut(1)
        .unwrap()
        .set_archive_target(Some(crate::sim::combat::TargetKind::Cell(26, 21)));
    let americans_key = sim.interner.intern("AMERICANS");
    let americans_display = sim.interner.intern("Americans");
    sim.houses.insert(
        americans_key,
        crate::sim::house_state::HouseState::new(
            americans_display,
            0,
            None,
            true,
            super::production_types::STARTING_CREDITS,
            10,
        ),
    );
    // Arm the native Ship-slot factory directly in the registry, then force it
    // ready so `tick_production` spawns the destroyer this tick.
    arm_build_via(
        &mut sim,
        &rules,
        "Americans",
        "DEST",
        ProductionCategory::Ship,
        0,
    );
    let held_id = sim
        .production
        .factory_shadow
        .view(americans_display, ProductionCategory::Ship)
        .and_then(|view| view.object.and_then(|object| object.entity_id))
        .expect("ship exists in limbo from StartProduction");
    let rng_before_delivery = sim.scenario_rng.logical_state();
    assert!(
        sim.production
            .factory_shadow
            .test_arm_ready(americans_display, ProductionCategory::Ship)
    );

    let spawned = tick_production(&mut sim, &rules);
    assert!(spawned, "completed naval production should spawn the unit");
    assert_eq!(sim.scenario_rng.logical_state(), rng_before_delivery);

    let ship = sim
        .substrate
        .entities
        .values()
        .find(|e| {
            sim.interner
                .resolve(e.type_ref)
                .eq_ignore_ascii_case("DEST")
        })
        .expect("spawned destroyer");
    assert_eq!(
        ship.stable_id, held_id,
        "delivery reuses Factory+0x58 identity"
    );
    assert_eq!(
        ship.navigation.nav_com,
        Some(crate::sim::components::NavTargetRef::cell(26, 21)),
        "the selected producer owns the represented rally destination"
    );
    assert_eq!(
        ship.mission.queued(),
        crate::sim::mission::MissionId::from_known(crate::sim::mission::MissionType::Move),
        "the producer rally queues Move before the immediate-path adapter"
    );
    assert!(
        ship.movement_target.is_some(),
        "the clear water fixture should also build its optional immediate A* path"
    );
}

#[test]
fn build_options_dedupe_house_specific_sidebar_clone() {
    let mut sim = Simulation::new();
    let ini = IniFile::from_str(
        "[InfantryTypes]\n\
         [VehicleTypes]\n\
         [AircraftTypes]\n\
         [BuildingTypes]\n\
         0=GACNST\n\
         1=GAAIRC\n\
         2=AMRADR\n\
         [GACNST]\n\
         Name=Construction Yard\n\
         Cost=3000\n\
         Strength=1000\n\
         Armor=wood\n\
         TechLevel=1\n\
         Owner=Americans,Alliance\n\
         Image=GACNST\n\
         Factory=BuildingType\n\
         [GAAIRC]\n\
         Name=Airforce Command\n\
         Cost=1000\n\
         Strength=600\n\
         Armor=wood\n\
         TechLevel=1\n\
         Owner=Americans,Alliance,British,French,Germans,Koreans\n\
         Image=GAAIRC\n\
         BuildCat=Tech\n\
         [AMRADR]\n\
         Name=Airforce Command\n\
         Cost=1000\n\
         Strength=600\n\
         Armor=wood\n\
         TechLevel=1\n\
         Owner=Americans,Alliance,British,French,Germans,Koreans\n\
         RequiredHouses=Americans\n\
         Image=GAAIRC\n\
         BuildCat=Tech\n",
    );
    let rules = RuleSet::from_ini(&ini).expect("rules should parse");
    sim.intern_rule_type_ids(&rules);
    sim.resolve_type_handles(&rules);

    let americans_id = sim.interner.intern("Americans");
    sim.houses.insert(
        americans_id,
        crate::sim::house_state::HouseState::new(americans_id, 0, None, true, 50_000, 10),
    );
    spawn_structure(&mut sim, 1, "Americans", "GACNST", 10, 10);

    let americans = build_options_for_owner(&sim, &rules, "Americans");
    let airforce: Vec<_> = americans
        .iter()
        .filter(|opt| opt.display_name == "Airforce Command")
        .collect();

    assert_eq!(
        airforce.len(),
        1,
        "sidebar should only show one Airforce Command"
    );
    assert_eq!(airforce[0].type_id, sim.interner.intern("AMRADR"));
}

#[test]
fn tick_production_advances_each_owner_queue() {
    let mut sim = Simulation::new();
    let rules = basic_infantry_rules();

    spawn_structure(&mut sim, 1, "Americans", "GAPILE", 10, 10);
    spawn_structure(&mut sim, 2, "Soviet", "NAHAND", 20, 20);

    let americans_id = sim.interner.intern("Americans");
    let soviet_id = sim.interner.intern("Soviet");
    // P5d: arm both factories directly in the registry, then force both to the completed-and-
    // held state so `tick_production` delivers/spawns from that state this tick.
    arm_build_via(
        &mut sim,
        &rules,
        "Americans",
        "E1",
        ProductionCategory::Infantry,
        0,
    );
    arm_build_via(
        &mut sim,
        &rules,
        "Soviet",
        "E1",
        ProductionCategory::Infantry,
        1,
    );

    assert!(
        sim.production
            .factory_shadow
            .test_arm_ready(americans_id, ProductionCategory::Infantry)
    );
    assert!(
        sim.production
            .factory_shadow
            .test_arm_ready(soviet_id, ProductionCategory::Infantry)
    );

    let spawned = tick_production(&mut sim, &rules);
    assert!(spawned, "At least one queue completion should spawn");
    assert!(
        sim.production.factory_shadow.is_empty(),
        "Completed owner factories should be pruned"
    );

    let americans = sim
        .substrate
        .entities
        .values()
        .filter(|e| {
            sim.interner
                .resolve(e.owner)
                .eq_ignore_ascii_case("Americans")
                && sim.interner.resolve(e.type_ref).eq_ignore_ascii_case("E1")
        })
        .count();
    let soviet = sim
        .substrate
        .entities
        .values()
        .filter(|e| {
            sim.interner.resolve(e.owner).eq_ignore_ascii_case("Soviet")
                && sim.interner.resolve(e.type_ref).eq_ignore_ascii_case("E1")
        })
        .count();
    assert_eq!(americans, 1);
    assert_eq!(soviet, 1);
}

#[test]
fn tick_production_advances_multiple_queue_categories_for_same_owner() {
    let mut sim = Simulation::new();
    let rules = basic_multi_queue_rules();

    spawn_structure(&mut sim, 1, "Americans", "GAPILE", 10, 10);
    spawn_structure(&mut sim, 2, "Americans", "GAWEAP", 14, 10);

    let americans_id = sim.interner.intern("Americans");
    // P5d: arm both category factories directly in the registry, then force both to the
    // completed-and-held state so `tick_production` delivers them this tick.
    arm_build_via(
        &mut sim,
        &rules,
        "Americans",
        "E1",
        ProductionCategory::Infantry,
        0,
    );
    arm_build_via(
        &mut sim,
        &rules,
        "Americans",
        "MTNK",
        ProductionCategory::Vehicle,
        1,
    );

    assert!(
        sim.production
            .factory_shadow
            .test_arm_ready(americans_id, ProductionCategory::Infantry)
    );
    assert!(
        sim.production
            .factory_shadow
            .test_arm_ready(americans_id, ProductionCategory::Vehicle)
    );

    let spawned = tick_production(&mut sim, &rules);
    assert!(spawned);
    assert!(
        sim.production.factory_shadow.is_empty(),
        "all completed category factories should be pruned"
    );

    let infantry = sim
        .substrate
        .entities
        .values()
        .filter(|e| {
            sim.interner
                .resolve(e.owner)
                .eq_ignore_ascii_case("Americans")
                && sim.interner.resolve(e.type_ref).eq_ignore_ascii_case("E1")
        })
        .count();
    let vehicles = sim
        .substrate
        .entities
        .values()
        .filter(|e| {
            sim.interner
                .resolve(e.owner)
                .eq_ignore_ascii_case("Americans")
                && sim
                    .interner
                    .resolve(e.type_ref)
                    .eq_ignore_ascii_case("MTNK")
        })
        .count();
    assert_eq!(infantry, 1);
    assert_eq!(vehicles, 1);
}

#[test]
fn blocked_vehicle_delivery_keeps_completed_item_and_holds_next_queue_item() {
    let mut sim = Simulation::new();
    let rules = super::lifecycle_tests::manager_rules();
    let terrain = water_terrain(32, 32);
    let grid = PathGrid::from_resolved_terrain(&terrain);
    sim.install_fixture_path_grid(Some(&grid));
    sim.resolved_terrain = Some(terrain);

    spawn_structure(&mut sim, 1, "Americans", "GAWEAP", 10, 10);
    *super::credits_entry_for_owner(&mut sim, "Americans") = 1000;

    let americans_id = sim.interner.intern("Americans");
    // P5d: arm the front MTNK (active) then a second MTNK at a higher stamp (FIFO tail).
    arm_build_via(
        &mut sim,
        &rules,
        "Americans",
        "MTNK",
        ProductionCategory::Vehicle,
        1,
    );
    arm_build_via(
        &mut sim,
        &rules,
        "Americans",
        "MTNK",
        ProductionCategory::Vehicle,
        2,
    );

    // Force the active (front) vehicle to ready so `tick_production` attempts delivery
    // (which the water grid blocks).
    assert!(
        sim.production
            .factory_shadow
            .test_arm_ready(americans_id, ProductionCategory::Vehicle)
    );

    let held_id_before =
        super::lifecycle_tests::held_id(&sim, americans_id, ProductionCategory::Vehicle);
    let children_before = super::lifecycle_tests::children(&sim, held_id_before);
    assert_eq!(children_before.len(), 3);
    let owned_before = sim.owned_object_counts(americans_id).1;
    let rng_before = sim.scenario_rng.clone();
    let allocated_before = sim.substrate.next_stable_object_id;
    let spawned = tick_production(&mut sim, &rules);
    assert!(
        !spawned,
        "blocked completed vehicle should not spawn or advance"
    );
    assert_eq!(
        credits_for_owner(&sim, "Americans"),
        1000,
        "blocked vehicle delivery is not a failed production refund"
    );

    // The completed-held active build + its untouched FIFO tail must both persist.
    let view = sim
        .production
        .factory_shadow
        .view(americans_id, ProductionCategory::Vehicle)
        .expect("vehicle factory should remain");
    // Active (head) is the completed-held MTNK; tail still holds the one queued MTNK
    // (head + tail == the old queue.len() of 2).
    assert!(
        view.object.is_some(),
        "completed-held active build must persist"
    );
    assert_eq!(view.queue.len(), 1, "one queued tail item must remain");
    // Head state is Done; its derived remaining base frames is 0 (progress == 54).
    let active_steps_left = super::factory::PRODUCTION_STEPS
        .saturating_sub(view.progress.min(super::factory::PRODUCTION_STEPS));
    assert_eq!(
        active_steps_left, 0,
        "head is complete -> 0 remaining base frames"
    );
    // The projected sidebar view shows the head as Done and the tail as Queued and
    // not started.
    let projected = queue_view_for_owner(&sim, &rules, "Americans");
    assert_eq!(projected.len(), 2);
    assert_eq!(projected[0].state, BuildQueueState::Done);
    assert_eq!(projected[1].state, BuildQueueState::Queued);
    assert_eq!(
        projected[1].progress, 0,
        "next queued item must not start while completed vehicle is pending"
    );
    let held_id = view
        .object
        .and_then(|object| object.entity_id)
        .expect("blocked delivery retains the StartProduction identity");
    let held = sim.substrate.entities.get(held_id).unwrap();
    assert!(held.lifecycle.in_limbo && !held.lifecycle.cell_marked);
    assert_eq!(sim.interner.resolve(held.type_ref), "MTNK");
    for _ in 0..3 {
        assert!(!tick_production(&mut sim, &rules));
    }
    assert_eq!(
        super::lifecycle_tests::held_id(&sim, americans_id, ProductionCategory::Vehicle),
        held_id_before
    );
    assert_eq!(
        super::lifecycle_tests::children(&sim, held_id_before),
        children_before
    );
    assert_eq!(sim.scenario_rng.logical_state(), rng_before.logical_state());
    assert_eq!(sim.substrate.next_stable_object_id, allocated_before);
    assert_eq!(sim.owned_object_counts(americans_id).1, owned_before + 0);
}

#[test]
fn pending_vehicle_delivery_success_consumes_completed_item_and_starts_next_item() {
    let mut sim = Simulation::new();
    let rules = super::lifecycle_tests::manager_rules();
    let mut terrain = water_terrain(32, 32);
    let blocked_grid = PathGrid::from_resolved_terrain(&terrain);
    sim.install_fixture_path_grid(Some(&blocked_grid));
    sim.playfield_bounds = Some(crate::sim::cell_rect::PlayfieldBounds {
        base: 0,
        off_fc: -32,
        off_100: -1,
        off_104: 64,
        off_108: 33,
    });
    sim.resolved_terrain = Some(terrain.clone());

    spawn_structure(&mut sim, 1, "Americans", "GAWEAP", 10, 10);

    let americans_id = sim.interner.intern("Americans");
    *super::credits_entry_for_owner(&mut sim, "Americans") = 50_000;
    // P5d: arm the front MTNK (active) then a second MTNK at a higher stamp (FIFO tail).
    arm_build_via(
        &mut sim,
        &rules,
        "Americans",
        "MTNK",
        ProductionCategory::Vehicle,
        1,
    );
    arm_build_via(
        &mut sim,
        &rules,
        "Americans",
        "MTNK",
        ProductionCategory::Vehicle,
        2,
    );

    // Force the front vehicle to ready once. The first (blocked-grid) delivery leaves the
    // registry untouched, so it stays ready for the later clear-grid delivery.
    assert!(
        sim.production
            .factory_shadow
            .test_arm_ready(americans_id, ProductionCategory::Vehicle)
    );

    let held_id_before =
        super::lifecycle_tests::held_id(&sim, americans_id, ProductionCategory::Vehicle);
    let children_before = super::lifecycle_tests::children(&sim, held_id_before);
    assert_eq!(children_before.len(), 3);
    let owned_before = sim.owned_object_counts(americans_id).1;
    let rng_before = sim.scenario_rng.clone();
    let allocated_before = sim.substrate.next_stable_object_id;
    let blocked = tick_production(&mut sim, &rules);
    assert!(!blocked, "first delivery attempt should remain pending");

    assert_eq!(
        super::lifecycle_tests::children(&sim, held_id_before),
        children_before
    );
    assert_eq!(sim.scenario_rng.logical_state(), rng_before.logical_state());
    assert_eq!(sim.substrate.next_stable_object_id, allocated_before);
    for cell in &mut terrain.cells {
        cell.is_water = false;
        cell.land_type = crate::rules::terrain_rules::LandType::Clear.as_index();
        cell.yr_cell_land_type = crate::rules::terrain_rules::LandType::Clear.as_index();
        cell.terrain_class = crate::rules::terrain_rules::TerrainClass::Clear;
        cell.speed_costs.track = Some(100);
        cell.zone_type = crate::map::resolved_terrain::zone_class::GROUND;
    }
    let clear_grid = PathGrid::from_resolved_terrain(&terrain);
    sim.resolved_terrain = Some(terrain);
    sim.path_grid = Some(std::sync::Arc::new(clear_grid));

    let spawned = tick_production(&mut sim, &rules);
    assert!(
        spawned,
        "later successful delivery should consume the pending completed vehicle"
    );

    let tanks = sim
        .substrate
        .entities
        .values()
        .filter(|entity| {
            sim.interner
                .resolve(entity.type_ref)
                .eq_ignore_ascii_case("MTNK")
        })
        .count();
    assert_eq!(
        tanks, 2,
        "one delivered tank and one freshly promoted limbo tank exist"
    );

    // The delivered active build is cleared and the tail MTNK is promoted into the active
    // slot (one item left, now the active Building head with an empty tail).
    let view = sim
        .production
        .factory_shadow
        .view(americans_id, ProductionCategory::Vehicle)
        .expect("next queued item should have started");
    assert!(view.object.is_some(), "promoted MTNK is the active build");
    let promoted_id = view.object.unwrap().entity_id.unwrap();
    assert!(
        sim.substrate
            .entities
            .get(promoted_id)
            .is_some_and(|entity| entity.lifecycle.in_limbo)
    );
    assert_eq!(
        sim.substrate
            .entities
            .values()
            .filter(|entity| {
                sim.interner
                    .resolve(entity.type_ref)
                    .eq_ignore_ascii_case("MTNK")
                    && !entity.lifecycle.in_limbo
            })
            .count(),
        1
    );
    assert!(view.queue.is_empty(), "the FIFO tail is now empty");
    // StartNextQueued runs Begin_Production (0x004CA60A), whose build start arms the promoted
    // build's rate and timer at this frame; it has not stepped yet.
    assert_eq!(view.progress, 0);
    let promoted = sim
        .production
        .factory_shadow
        .iter_insertion_ordered()
        .into_iter()
        .find(|f| f.owner == americans_id && f.category == ProductionCategory::Vehicle)
        .expect("promoted factory");
    assert!(promoted.step_rate_frames > 0);
    assert_eq!(
        promoted.step_timer.start_frame(),
        sim.session.binary_frame as i32,
        "successful delivery starts the next item at this frame"
    );
    // The projected sidebar view shows the single promoted item as Building.
    let projected = queue_view_for_owner(&sim, &rules, "Americans");
    assert_eq!(projected.len(), 1);
    assert_eq!(projected[0].state, BuildQueueState::Building);
    assert!(
        !sim.substrate
            .entities
            .get(held_id_before)
            .unwrap()
            .lifecycle
            .in_limbo
    );
    assert_eq!(
        super::lifecycle_tests::children(&sim, held_id_before),
        children_before
    );
    let promoted_children = super::lifecycle_tests::children(&sim, promoted_id);
    assert_eq!(promoted_children.len(), 3);
    assert!(
        promoted_children
            .iter()
            .all(|id| !children_before.contains(id))
    );
    let mut expected = rng_before;
    for id in std::iter::once(promoted_id).chain(promoted_children) {
        assert_eq!(
            sim.substrate
                .entities
                .get(id)
                .unwrap()
                .techno_ctor_random_word,
            (expected.next_u32() & 0xffff) as u16
        );
    }
    assert_eq!(sim.scenario_rng.logical_state(), expected.logical_state());
    assert_eq!(sim.owned_object_counts(americans_id).1, owned_before + 4);
}

#[test]
fn paused_category_projection_and_factory_charge_remain_independent() {
    let mut sim = Simulation::new();
    let rules = basic_multi_queue_rules();

    spawn_structure(&mut sim, 1, "Americans", "GAPILE", 10, 10);
    spawn_structure(&mut sim, 2, "Americans", "GAWEAP", 14, 10);

    let americans_id = sim.interner.intern("Americans");
    *super::credits_entry_for_owner(&mut sim, "Americans") = 50_000;
    // P5d: arm both category factories directly in the registry.
    arm_build_via(
        &mut sim,
        &rules,
        "Americans",
        "E1",
        ProductionCategory::Infantry,
        0,
    );
    arm_build_via(
        &mut sim,
        &rules,
        "Americans",
        "MTNK",
        ProductionCategory::Vehicle,
        1,
    );

    let paused = suspend_production(&mut sim, "Americans", ProductionCategory::Infantry);
    assert!(paused);

    // Run the actual frame owner, which now performs revalidation and charging.
    sim.houses
        .get_mut(&americans_id)
        .unwrap()
        .tracking
        .set_buildings_for_test(2);
    for _ in 0..40 {
        sim.advance_tick(&[], Some(&rules), None, None, 67);
    }

    // Project the registry to the sidebar view for state assertions (the per-item
    // `state`/`remaining_base_frames` mirror is retired).
    let view = queue_view_for_owner(&sim, &rules, "Americans");
    let infantry = view
        .iter()
        .find(|q| q.queue_category == ProductionCategory::Infantry)
        .expect("infantry factory should remain");
    let vehicle = view
        .iter()
        .find(|q| q.queue_category == ProductionCategory::Vehicle)
        .expect("vehicle factory should remain");

    assert_eq!(infantry.state, BuildQueueState::Paused);
    assert_eq!(vehicle.state, BuildQueueState::Building);
    let infantry = sim
        .production
        .factory_shadow
        .view(americans_id, ProductionCategory::Infantry)
        .unwrap();
    let vehicle = sim
        .production
        .factory_shadow
        .view(americans_id, ProductionCategory::Vehicle)
        .unwrap();
    assert_eq!(infantry.progress, 0);
    assert!(vehicle.progress > 0);
}

/// Canceling a finished building abandons the factory's object, refunding the cost
/// less the unpaid balance (Abandon_Production 0x004FAA10, refund at 0x004FABA6), and
/// drops the ready entry with it.
#[test]
fn cancel_by_type_removes_ready_building_and_refunds() {
    use super::cancel_by_type_for_owner;

    let mut sim = Simulation::new();
    let rules = build_catalog_rules();

    spawn_structure(&mut sim, 1, "Americans", "GACNST", 10, 10);
    // The refund goes to an existing house; a cancel never creates one.
    *super::credits_entry_for_owner(&mut sim, "Americans") = 5000;

    let americans_id = sim.interner.intern("Americans");
    arm_build_via(
        &mut sim,
        &rules,
        "Americans",
        "GAREFN",
        ProductionCategory::Building,
        0,
    );
    // The fixture finishes the build with its balance paid off.
    assert!(
        sim.production
            .factory_shadow
            .test_arm_ready(americans_id, ProductionCategory::Building)
    );
    assert!(!tick_production(&mut sim, &rules));
    assert_eq!(sim.production.ready_by_owner[&americans_id].len(), 1);

    let before_credits = credits_for_owner(&sim, "Americans");

    let cancelled = cancel_by_type_for_owner(&mut sim, &rules, "Americans", "GAREFN", false);
    assert!(cancelled, "should cancel ready building");

    // Ready queue should be empty now.
    let ready_count = sim
        .production
        .ready_by_owner
        .get(&americans_id)
        .map(|q| q.len())
        .unwrap_or(0);
    assert_eq!(ready_count, 0, "ready queue should be empty after cancel");
    assert!(
        sim.production
            .factory_shadow
            .view(americans_id, ProductionCategory::Building)
            .is_none()
    );

    // Cost should be refunded.
    let after_credits = credits_for_owner(&sim, "Americans");
    let refund = rules.object("GAREFN").map(|o| o.cost).unwrap_or(0);
    assert!(refund > 0, "GAREFN should have a cost");
    assert_eq!(after_credits, before_credits + refund);
}

/// A build starts without money: nothing on the PRODUCE path checks the wallet
/// (`HouseClass::Begin_Production @ 0x004FA350`), and the per-step charge in
/// `step_all` pays for the build as it goes. An empty wallet still starts the
/// build and is debited nothing.
#[test]
fn enqueue_starts_a_build_without_money_and_debits_nothing() {
    let mut sim = Simulation::new();
    let rules = basic_multi_queue_rules();
    spawn_structure(&mut sim, 1, "Americans", "GAWEAP", 10, 10); // a UnitType war factory
    *super::credits_entry_for_owner(&mut sim, "Americans") = 0;
    assert!(super::enqueue_by_type(
        &mut sim,
        &rules,
        "Americans",
        "MTNK"
    ));
    assert_eq!(credits_for_owner(&sim, "Americans"), 0);
    let americans_id = sim.interner.intern("Americans");
    let mtnk_id = sim.interner.intern("MTNK");
    let factory = sim
        .production
        .factory_shadow
        .view(americans_id, ProductionCategory::Vehicle)
        .expect("the build starts a factory");
    assert_eq!(factory.object.map(|o| o.type_id), Some(mtnk_id));
    assert_eq!(factory.progress, 0);
}

fn hold_rules() -> RuleSet {
    RuleSet::from_ini(&IniFile::from_str(
        "[General]\nMaximumQueuedObjects=2\n\
         [InfantryTypes]\n\
         [VehicleTypes]\n0=MTNK\n1=AMCV\n\
         [AircraftTypes]\n\
         [BuildingTypes]\n0=GAWEAP\n\
         [MTNK]\nCost=700\nStrength=300\nSpeed=6\nTechLevel=1\nOwner=Americans\n\
         [AMCV]\nCost=3000\nStrength=1000\nSpeed=4\nTechLevel=1\nOwner=Americans\nBuildLimit=1\n\
         [GAWEAP]\nFactory=UnitType\n",
    ))
    .expect("hold rules")
}

/// A funded American house with a war factory, for the hold and queue-cap tests.
fn hold_world() -> (Simulation, RuleSet, InternedId) {
    let rules = hold_rules();
    let mut sim = Simulation::new();
    spawn_structure(&mut sim, 1, "Americans", "GAWEAP", 10, 10);
    *super::credits_entry_for_owner(&mut sim, "Americans") = 50_000;
    let owner = sim.interner.intern("Americans");
    (sim, rules, owner)
}

/// A PRODUCE resumes a held build even at its type's build limit: the held build
/// counts toward the limit, and `CanBuild(type, 1, 1)` passes a type one of the
/// house's factories holds (`0x004F8348`). The build start (`0x004C9EA0`)
/// restarts the rate from the resume frame and queues nothing.
#[test]
fn a_held_build_at_its_build_limit_resumes_on_produce() {
    let (mut sim, rules, owner) = hold_world();
    assert!(enqueue_by_type(&mut sim, &rules, "Americans", "AMCV"));
    assert!(suspend_production(
        &mut sim,
        "Americans",
        ProductionCategory::Vehicle
    ));
    // The factory is already stopped, so a second hold is refused (0x004C9E60).
    assert!(!suspend_production(
        &mut sim,
        "Americans",
        ProductionCategory::Vehicle
    ));
    assert_eq!(
        queue_view_for_owner(&sim, &rules, "Americans")[0].state,
        BuildQueueState::Paused
    );
    let option = super::production_tech::build_option_for_owner(&sim, &rules, "Americans", "AMCV")
        .expect("the type stays listed");
    assert_eq!(
        option.reason,
        Some(super::BuildDisabledReason::AtBuildLimit)
    );

    sim.session.binary_frame = 300;
    assert!(enqueue_by_type(&mut sim, &rules, "Americans", "AMCV"));
    let factory = sim
        .production
        .factory_shadow
        .test_factory_mut(owner, ProductionCategory::Vehicle)
        .unwrap();
    assert!(!factory.manual);
    assert!(factory.step_rate_frames > 0);
    assert_eq!(
        factory.step_timer,
        CdTimer::started(300, i32::from(factory.step_rate_frames))
    );
    assert!(factory.queue.is_empty(), "a resume queues nothing");
    assert_eq!(
        queue_view_for_owner(&sim, &rules, "Americans")[0].state,
        BuildQueueState::Building
    );
    // Running, the build no longer passes the limit: gamemd's StartProduction
    // refuses the append (0x004C9CEA), and VERA refuses it before that.
    assert!(!enqueue_by_type(&mut sim, &rules, "Americans", "AMCV"));
}

/// `[General] MaximumQueuedObjects=` caps the builds waiting behind the active
/// one: StartProduction refuses the append past it (`0x004C9CDE`) and scolds the
/// house's player (`0x004C9D3B..0x004C9D5F`).
#[test]
fn a_produce_past_the_queue_cap_is_refused_with_a_scold() {
    let (mut sim, rules, owner) = hold_world();
    assert_eq!(rules.general.maximum_queued_objects, 2);
    let refusals = |sim: &Simulation| {
        sim.sound_events
            .iter()
            .filter(|event| {
                matches!(event, SimSoundEvent::ProductionRefused { owner: refused } if *refused == owner)
            })
            .count()
    };
    for _ in 0..3 {
        assert!(enqueue_by_type(&mut sim, &rules, "Americans", "MTNK"));
    }
    assert_eq!(refusals(&sim), 0);
    assert!(!enqueue_by_type(&mut sim, &rules, "Americans", "MTNK"));
    assert_eq!(refusals(&sim), 1);
    let factory = sim
        .production
        .factory_shadow
        .view(owner, ProductionCategory::Vehicle)
        .unwrap();
    assert_eq!(factory.queue.len(), 2);
}
