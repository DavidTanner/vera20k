//! Production lifecycle scenarios: registry decisions must finish their held
//! object graph, accounting and successor work before returning to the caller.

use super::tests::spawn_structure;
use super::{
    ProductionCategory, cancel_by_type_for_owner, dispatch_production_changes_for_tests,
    enqueue_by_type,
};
use crate::rules::{ini_parser::IniFile, ruleset::RuleSet};
use crate::sim::{intern::InternedId, rng::SimRng, world::Simulation};

pub(super) fn manager_rules() -> RuleSet {
    RuleSet::from_ini(&IniFile::from_str(
        "[Countries]\n0=Americans\n1=Russians\n[InfantryTypes]\n0=E1\n1=SLAV\n\
         [VehicleTypes]\n0=MTNK\n1=SMIN\n\
         [AircraftTypes]\n0=HORN\n1=ORCA\n\
         [BuildingTypes]\n0=GAPILE\n1=GAWEAP\n2=GACNST\n3=YAREFN\n4=TECH\n5=GAAIRC\n6=GAPOWR\n\
         [E1]\nName=GI\nCost=200\nStrength=100\nArmor=flak\nSpeed=4\nSight=5\nTechLevel=1\nOwner=Americans\nLocomotor={4A582744-9839-11d1-B709-00A024DDAFD1}\n\
         [SLAV]\nStrength=125\nSpeed=4\nStorage=4\nLocomotor={4A582744-9839-11d1-B709-00A024DDAFD1}\n\
         [MTNK]\nName=Tank\nCost=700\nStrength=300\nArmor=heavy\nSpeed=6\nSight=6\nTechLevel=1\nOwner=Americans\nSpawns=HORN\nSpawnsNumber=3\nSpawnRegenRate=600\nSpawnReloadRate=25\nLocomotor={4A582741-9839-11d1-B709-00A024DDAFD1}\n\
         [SMIN]\nCost=900\nStrength=2000\nSpeed=3\nTechLevel=1\nOwner=Americans\nPrerequisite=TECH\nEnslaves=SLAV\nSlavesNumber=2\nSlaveRegenRate=500\nSlaveReloadRate=25\nLocomotor={4A582741-9839-11d1-B709-00A024DDAFD1}\n\
         [HORN]\nStrength=75\nSpeed=14\nAmmo=1\n\
         [ORCA]\nCost=1000\nStrength=200\nSpeed=8\nTechLevel=1\nOwner=Americans\n\
         [GAPILE]\nOwner=Americans\nFactory=InfantryType\n\
         [GAWEAP]\nOwner=Americans\nFactory=UnitType\n\
         [GACNST]\nOwner=Americans\nFactory=BuildingType\nConstructionYard=yes\n\
         [YAREFN]\nCost=1000\nStrength=2000\nFoundation=1x1\nTechLevel=1\nOwner=Americans\nEnslaves=SLAV\nSlavesNumber=2\nSlaveRegenRate=500\nSlaveReloadRate=25\n\
         [GAPOWR]\nCost=800\nStrength=500\nFoundation=1x1\nTechLevel=1\nOwner=Americans\n\
         [TECH]\nStrength=500\n\
         [GAAIRC]\nOwner=Americans\nFactory=AircraftType\nHelipad=yes\n",
    )).expect("factory manager fixture")
}

fn world(seed: u64) -> (Simulation, RuleSet, InternedId) {
    let rules = manager_rules();
    let mut sim = Simulation::with_seed(seed);
    sim.intern_rule_type_ids(&rules);
    sim.resolve_type_handles(&rules);
    let owner = sim.interner.intern("Americans");
    sim.houses.insert(
        owner,
        crate::sim::house_state::HouseState::new(owner, 0, None, true, 50_000, 10),
    );
    spawn_structure(&mut sim, 1, "Americans", "GAWEAP", 10, 10);
    spawn_structure(&mut sim, 2, "Americans", "GAPILE", 14, 10);
    spawn_structure(&mut sim, 3, "Americans", "GACNST", 18, 10);
    spawn_structure(&mut sim, 4, "Americans", "TECH", 22, 10);
    // Raw structure fixture admission bypasses lifecycle accounting.
    sim.houses
        .get_mut(&owner)
        .unwrap()
        .tracking
        .set_buildings_for_test(4);
    (sim, rules, owner)
}

pub(super) fn held_id(sim: &Simulation, owner: InternedId, category: ProductionCategory) -> u64 {
    sim.production
        .factories
        .view(owner, category)
        .unwrap()
        .object
        .unwrap()
        .entity_id
        .unwrap()
}

pub(super) fn children(sim: &Simulation, parent: u64) -> Vec<u64> {
    let ids: Vec<_> = if let Some(manager) = sim
        .substrate
        .entities
        .get(parent)
        .unwrap()
        .spawn_manager
        .as_ref()
    {
        manager
            .slots
            .iter()
            .map(|slot| slot.spawn.unwrap())
            .collect()
    } else {
        sim.substrate
            .entities
            .get(parent)
            .unwrap()
            .slave_manager
            .as_ref()
            .map(|manager| manager.slaves().collect())
            .unwrap_or_default()
    };
    for &id in &ids {
        let child = sim
            .substrate
            .entities
            .get(id)
            .expect("held manager child remains represented");
        assert!(child.lifecycle.in_limbo && !child.lifecycle.cell_marked);
        assert!(child.spawn_owner_id == Some(parent) || child.slave.owner() == Some(parent));
    }
    ids
}

fn counts(sim: &Simulation, owner: InternedId) -> (u32, u32) {
    sim.owned_object_counts(owner)
}

fn assert_gone(sim: &Simulation, parent: u64, child_ids: &[u64]) {
    assert!(!sim.substrate.entities.contains(parent));
    for &child in child_ids {
        assert!(!sim.substrate.entities.contains(child));
    }
}

pub(super) fn assert_constructor_words(sim: &Simulation, parent: u64, expected: &mut SimRng) {
    for id in std::iter::once(parent).chain(children(sim, parent)) {
        let word = (expected.next_u32() & 0xffff) as u16;
        assert_eq!(
            sim.substrate
                .entities
                .get(id)
                .unwrap()
                .techno_ctor_random_word,
            word
        );
    }
    assert_eq!(sim.scenario_rng.logical_state(), expected.logical_state());
}

#[test]
fn manager_factory_cancellation_finishes_graph_accounting_and_promotion() {
    for parent_type in ["MTNK", "SMIN"] {
        let (mut sim, rules, owner) = world(0xfac7_0010);
        let before = counts(&sim, owner);
        let mut expected = sim.scenario_rng.clone();
        assert!(enqueue_by_type(&mut sim, &rules, "Americans", parent_type));
        let parent = held_id(&sim, owner, ProductionCategory::Vehicle);
        let child_ids = children(&sim, parent);
        assert_eq!(child_ids.len(), if parent_type == "MTNK" { 3 } else { 2 });
        assert_constructor_words(&sim, parent, &mut expected);
        assert_eq!(
            counts(&sim, owner),
            (before.0, before.1 + 1 + child_ids.len() as u32)
        );
        let allocated = sim.substrate.next_stable_object_id;
        assert!(enqueue_by_type(&mut sim, &rules, "Americans", parent_type));
        assert_eq!(sim.substrate.next_stable_object_id, allocated);
        // ABANDON removes the queued copy first (`0x004FAAEE`), then the active build.
        assert!(cancel_by_type_for_owner(
            &mut sim,
            &rules,
            "Americans",
            parent_type,
            false
        ));
        assert_eq!(held_id(&sim, owner, ProductionCategory::Vehicle), parent);
        assert_eq!(children(&sim, parent), child_ids);
        assert_eq!(sim.houses[&owner].economy.credits, 50_000);
        assert_eq!(sim.scenario_rng.logical_state(), expected.logical_state());

        assert!(cancel_by_type_for_owner(
            &mut sim,
            &rules,
            "Americans",
            parent_type,
            false
        ));
        assert_gone(&sim, parent, &child_ids);
        assert_eq!(counts(&sim, owner), before);
        assert_eq!(sim.substrate.next_stable_object_id, allocated);
        assert_eq!(sim.scenario_rng.logical_state(), expected.logical_state());
        assert!(
            sim.production
                .factories
                .view(owner, ProductionCategory::Vehicle)
                .is_none()
        );

        assert!(enqueue_by_type(&mut sim, &rules, "Americans", parent_type));
        let parent = held_id(&sim, owner, ProductionCategory::Vehicle);
        let child_ids = children(&sim, parent);
        assert_constructor_words(&sim, parent, &mut expected);
        let successor_type = if parent_type == "MTNK" {
            "SMIN"
        } else {
            "MTNK"
        };
        assert!(enqueue_by_type(
            &mut sim,
            &rules,
            "Americans",
            successor_type
        ));
        assert!(cancel_by_type_for_owner(
            &mut sim,
            &rules,
            "Americans",
            parent_type,
            false
        ));
        assert_gone(&sim, parent, &child_ids);
        let successor = held_id(&sim, owner, ProductionCategory::Vehicle);
        assert!(successor > parent);
        assert_constructor_words(&sim, successor, &mut expected);
        assert_eq!(
            counts(&sim, owner),
            (
                before.0,
                before.1 + 1 + children(&sim, successor).len() as u32
            )
        );
        assert_eq!(
            sim.production
                .factories
                .view(owner, ProductionCategory::Vehicle)
                .unwrap()
                .progress,
            0
        );
    }
}

/// A Slave Miner leaving its war factory starts its hunt
/// (`UnitClass::PerCellProcess @ 0x0073A9CA..0x0073A9D9`, before the rally
/// arm) instead of driving to the factory's rally point, which a tank from the
/// same factory takes.
#[test]
fn a_produced_slave_miner_hunts_instead_of_taking_the_rally_point() {
    for (unit_type, hunts) in [("SMIN", true), ("MTNK", false)] {
        let (mut sim, rules, owner) = world(0xfac7_0020);
        sim.substrate
            .entities
            .get_mut(1)
            .unwrap()
            .set_archive_target(Some(crate::sim::combat::TargetKind::Cell(30, 30)));
        assert!(enqueue_by_type(&mut sim, &rules, "Americans", unit_type));
        let produced = held_id(&sim, owner, ProductionCategory::Vehicle);
        assert!(
            sim.production
                .factories
                .test_arm_ready(owner, ProductionCategory::Vehicle)
        );
        let grid = crate::sim::pathfinding::PathGrid::new(64, 64);
        sim.install_fixture_path_grid(Some(&grid));
        assert!(dispatch_production_changes_for_tests(
            &mut sim, &rules, None
        ));
        let entity = sim.substrate.entities.get(produced).unwrap();
        assert!(!entity.lifecycle.in_limbo, "{unit_type} delivered");
        assert_eq!(
            entity.movement_target.is_some(),
            !hunts,
            "{unit_type}: the rally move"
        );
        if hunts {
            assert_eq!(
                entity.slave_manager.as_ref().unwrap().state(),
                crate::sim::slave_manager::ManagerState::Scanning,
                "the hunt starts at the factory exit"
            );
        }
    }
}

/// Each factory keeps its own rally point (its ArchiveTarget, `+0x218`), and
/// `BuildingClass::ExitObject_Main @ 0x00443C60` hands the leaving object its
/// own factory's: an infantryman walks to the barracks' rally although the
/// owner's last rally click was on the war factory.
#[test]
fn produced_infantry_retains_its_barracks_rally_until_exit_handoff() {
    let (mut sim, rules, owner) = world(0xfac7_0021);
    super::tests::install_infantry_delivery_fixture_map(&mut sim);
    for (factory, rally) in [(2, (30, 30)), (1, (40, 12))] {
        let command = crate::sim::command::Command::SetRally {
            rx: rally.0,
            ry: rally.1,
            producer_ids: vec![factory],
        };
        assert!(sim.apply_command("Americans", &command, Some(&rules)));
    }
    assert!(enqueue_by_type(&mut sim, &rules, "Americans", "E1"));
    let produced = held_id(&sim, owner, ProductionCategory::Infantry);
    assert!(
        sim.production
            .factories
            .test_arm_ready(owner, ProductionCategory::Infantry)
    );
    let grid = crate::sim::pathfinding::PathGrid::new(64, 64);
    sim.install_fixture_path_grid(Some(&grid));
    assert!(dispatch_production_changes_for_tests(
        &mut sim, &rules, None
    ));
    let entity = sim.substrate.entities.get(produced).unwrap();
    assert!(!entity.lifecycle.in_limbo, "E1 delivered");
    // Nonzero rally: TechnoUnlimbo's InfIdle establishes Nav, then Exit
    // saves it back to Archive444CE7 before assigning the exit444D11.
    // A stock NavNULL prior follows the separate skip arm. FootEnterIdle
    // consumes this retained Archive later (520F92 -> 51CBA0 -> 4D82B0).
    assert_eq!(entity.rally_cell(), Some((30, 30)));
    assert!(
        entity.navigation.nav_com.is_some(),
        "the initial exit is paid first"
    );
    assert_ne!(
        entity.navigation.nav_com,
        Some(crate::sim::components::NavTargetRef::cell(30, 30))
    );
    assert!(entity.has_live_contact_with(2));
    assert_eq!(entity.dock_entered_with, Some(2));
}

/// `TechnoClass::ChangeOwner @ 0x007014A0` clears the ArchiveTarget
/// (`0x0070151A`), so a captured factory loses its rally point.
#[test]
fn a_captured_factory_loses_its_rally_point() {
    let (mut sim, rules, _) = world(0xfac7_0022);
    let command = crate::sim::command::Command::SetRally {
        rx: 40,
        ry: 12,
        producer_ids: vec![1],
    };
    assert!(sim.apply_command("Americans", &command, Some(&rules)));
    assert_eq!(
        sim.substrate.entities.get(1).unwrap().rally_cell(),
        Some((40, 12))
    );
    let russians = sim.interner.intern("Russians");
    sim.change_owner_with_rules(1, russians, &rules, None);
    assert_eq!(sim.substrate.entities.get(1).unwrap().rally_cell(), None);
}

#[test]
fn ready_manager_cancel_refunds_disposes_and_constructs_one_successor() {
    let (mut sim, rules, owner) = world(0xfac7_0011);
    let before = counts(&sim, owner);
    assert!(enqueue_by_type(&mut sim, &rules, "Americans", "YAREFN"));
    let parent = held_id(&sim, owner, ProductionCategory::Building);
    let child_ids = children(&sim, parent);
    assert_eq!(child_ids.len(), 2);
    assert!(
        sim.production
            .factories
            .test_arm_ready(owner, ProductionCategory::Building)
    );
    super::publish_production_changes(&mut sim, &rules);
    assert_eq!(sim.production.ready_by_owner[&owner].len(), 1);
    let mut expected = sim.scenario_rng.clone();
    let credits = sim.houses[&owner].economy.credits;
    // A PRODUCE of the type waiting finished takes Begin_Production's resume
    // branch (0x004FA5A8..0x004FA5C4), which the build start refuses at stage 54
    // (0x004C9ECD): nothing is queued or charged.
    assert!(!enqueue_by_type(&mut sim, &rules, "Americans", "YAREFN"));
    assert_eq!(held_id(&sim, owner, ProductionCategory::Building), parent);
    assert!(
        sim.production
            .factories
            .view(owner, ProductionCategory::Building)
            .unwrap()
            .queue
            .is_empty()
    );
    assert_eq!(sim.houses[&owner].economy.credits, credits);
    // A different queued type does not intercept cancellation of the ready head.
    assert!(enqueue_by_type(&mut sim, &rules, "Americans", "GAPOWR"));
    assert!(cancel_by_type_for_owner(
        &mut sim,
        &rules,
        "Americans",
        "YAREFN",
        false
    ));
    assert_gone(&sim, parent, &child_ids);
    assert_eq!(sim.houses[&owner].economy.credits, credits + 1000);
    assert_eq!(counts(&sim, owner), (before.0 + 1, before.1));
    assert!(
        sim.production
            .ready_by_owner
            .get(&owner)
            .is_none_or(|ready| ready.is_empty())
    );
    let successor = held_id(&sim, owner, ProductionCategory::Building);
    assert!(successor > parent);
    assert_eq!(
        sim.interner
            .resolve(sim.substrate.entities.get(successor).unwrap().type_ref),
        "GAPOWR"
    );
    assert_constructor_words(&sim, successor, &mut expected);
    assert_eq!(
        sim.production
            .factories
            .view(owner, ProductionCategory::Building)
            .unwrap()
            .progress,
        0
    );
}

/// The promoted build starts at the revalidation frame (StartNextQueued runs
/// Begin_Production, whose build start arms the timer), so it first steps one rate later.
#[test]
fn prerequisite_revalidation_disposes_manager_and_promoted_build_steps_a_rate_later() {
    let (mut sim, rules, owner) = world(0xfac7_0012);
    assert!(enqueue_by_type(&mut sim, &rules, "Americans", "SMIN"));
    assert!(enqueue_by_type(&mut sim, &rules, "Americans", "MTNK"));
    let parent = held_id(&sim, owner, ProductionCategory::Vehicle);
    let child_ids = children(&sim, parent);
    for _ in 0..40 {
        sim.advance_tick(&[], Some(&rules), None, None, 67);
    }
    let spent = {
        let factory = sim
            .production
            .factories
            .test_factory_mut(owner, ProductionCategory::Vehicle)
            .unwrap();
        assert!(factory.progress > 0 && factory.progress < 54);
        let balance = factory.balance;
        sim.cost_of(owner, rules.object("SMIN").unwrap(), &rules) - balance
    };
    let before = sim.houses[&owner].economy.credits;
    let mut expected = sim.scenario_rng.clone();
    // The producing factory remains; only the active type loses its prerequisite.
    sim.substrate.entities.remove(4);
    let promotion_frame = sim.session.binary_frame;
    sim.advance_tick(&[], Some(&rules), None, None, 67);
    assert_gone(&sim, parent, &child_ids);
    assert_eq!(sim.houses[&owner].economy.credits, before + spent);
    let successor = held_id(&sim, owner, ProductionCategory::Vehicle);
    assert_eq!(
        sim.interner
            .resolve(sim.substrate.entities.get(successor).unwrap().type_ref),
        "MTNK"
    );
    assert_constructor_words(&sim, successor, &mut expected);
    assert_eq!(
        sim.production
            .factories
            .view(owner, ProductionCategory::Vehicle)
            .unwrap()
            .progress,
        0
    );
    let (rate, timer) = {
        let factory = sim
            .production
            .factories
            .test_factory_mut(owner, ProductionCategory::Vehicle)
            .unwrap();
        (factory.step_rate_frames, factory.step_timer)
    };
    assert_eq!(timer.start_frame(), promotion_frame as i32);
    assert!(rate > 1);
    let progress = |sim: &Simulation| {
        sim.production
            .factories
            .view(owner, ProductionCategory::Vehicle)
            .unwrap()
            .progress
    };
    for _ in 1..rate {
        sim.advance_tick(&[], Some(&rules), None, None, 67);
        assert_eq!(progress(&sim), 0);
    }
    assert_eq!(sim.houses[&owner].economy.credits, before + spent);
    sim.advance_tick(&[], Some(&rules), None, None, 67);
    assert_eq!(progress(&sim), 1);
    assert!(sim.houses[&owner].economy.credits < before + spent);
}

#[test]
fn missing_barracks_retains_completed_infantry_and_queued_successor() {
    let (mut sim, rules, owner) = world(0xfac7_0013);
    assert!(enqueue_by_type(&mut sim, &rules, "Americans", "E1"));
    assert!(enqueue_by_type(&mut sim, &rules, "Americans", "E1"));
    let held = held_id(&sim, owner, ProductionCategory::Infantry);
    assert!(
        sim.production
            .factories
            .test_arm_ready(owner, ProductionCategory::Infantry)
    );
    // Supply an absent producer without invoking destruction's separate
    // AbandonProduction owner. HousePlace4FB520 returns before ExitObject.
    for structure in 1..=4 {
        sim.substrate.entities.remove(structure);
    }
    sim.houses
        .get_mut(&owner)
        .unwrap()
        .tracking
        .set_buildings_for_test(0);
    let credits = sim.houses[&owner].economy.credits;
    let rng = (
        sim.main_rng.logical_state(),
        sim.scenario_rng.logical_state(),
        sim.mapgen_rng.logical_state(),
    );
    let allocated = sim.substrate.next_stable_object_id;
    assert!(!dispatch_production_changes_for_tests(
        &mut sim, &rules, None
    ));
    assert_eq!(held_id(&sim, owner, ProductionCategory::Infantry), held);
    let factory = sim
        .production
        .factories
        .view(owner, ProductionCategory::Infantry)
        .unwrap();
    assert!(factory.ready);
    assert_eq!(factory.progress, super::PRODUCTION_STEPS);
    assert_eq!(factory.queue.len(), 1);
    let object = sim.substrate.entities.get(held).unwrap();
    assert!(object.lifecycle.in_limbo && !object.lifecycle.cell_marked);
    assert_eq!(sim.houses[&owner].economy.credits, credits);
    assert_eq!(sim.substrate.next_stable_object_id, allocated);
    assert_eq!(
        (
            sim.main_rng.logical_state(),
            sim.scenario_rng.logical_state(),
            sim.mapgen_rng.logical_state()
        ),
        rng
    );
}

#[test]
fn active_cancel_without_house_does_not_create_refund_account() {
    for all in [false, true] {
        let (mut sim, rules, owner) = world(0xfac7_0014);
        assert!(enqueue_by_type(&mut sim, &rules, "Americans", "SMIN"));
        let parent = held_id(&sim, owner, ProductionCategory::Vehicle);
        let child_ids = children(&sim, parent);
        sim.houses.remove(&owner);
        let rng = sim.scenario_rng.logical_state();
        let cancelled = cancel_by_type_for_owner(&mut sim, &rules, "Americans", "SMIN", all);
        assert!(cancelled);
        assert_gone(&sim, parent, &child_ids);
        assert!(!sim.houses.contains_key(&owner));
        assert_eq!(sim.scenario_rng.logical_state(), rng);
    }
}

#[test]
fn factory_loss_revalidation_disposes_parent_and_children_before_returning() {
    for parent_type in ["MTNK", "SMIN"] {
        let (mut sim, rules, owner) = world(0xfac7_0016);
        assert!(enqueue_by_type(&mut sim, &rules, "Americans", parent_type));
        assert!(enqueue_by_type(&mut sim, &rules, "Americans", parent_type));
        let parent = held_id(&sim, owner, ProductionCategory::Vehicle);
        let child_ids = children(&sim, parent);
        for _ in 0..40 {
            sim.advance_tick(&[], Some(&rules), None, None, 67);
        }
        let spent = {
            let factory = sim
                .production
                .factories
                .test_factory_mut(owner, ProductionCategory::Vehicle)
                .unwrap();
            assert!(factory.progress > 0 && factory.progress < 54);
            let balance = factory.balance;
            sim.cost_of(owner, rules.object(parent_type).unwrap(), &rules) - balance
        };
        sim.substrate.entities.remove(1);
        let before = sim.houses[&owner].economy.credits;
        let allocated = sim.substrate.next_stable_object_id;
        let rng = sim.scenario_rng.logical_state();
        sim.advance_tick(&[], Some(&rules), None, None, 67);
        assert_gone(&sim, parent, &child_ids);
        assert_eq!(sim.houses[&owner].economy.credits, before + spent);
        assert_eq!(sim.owned_object_counts(owner).1, 0);
        assert_eq!(sim.substrate.next_stable_object_id, allocated);
        assert_eq!(sim.scenario_rng.logical_state(), rng);
        assert!(
            sim.production
                .factories
                .view(owner, ProductionCategory::Vehicle)
                .is_none()
        );
    }
}

#[test]
fn revalidation_without_house_disposes_held_graph_without_creating_account() {
    let (mut sim, rules, owner) = world(0xfac7_0017);
    assert!(enqueue_by_type(&mut sim, &rules, "Americans", "SMIN"));
    let parent = held_id(&sim, owner, ProductionCategory::Vehicle);
    let child_ids = children(&sim, parent);
    sim.substrate.entities.remove(1);
    sim.houses.remove(&owner);
    let rng = sim.scenario_rng.logical_state();
    super::revalidate_and_step_factories(&mut sim, &rules);
    assert_gone(&sim, parent, &child_ids);
    assert!(!sim.houses.contains_key(&owner));
    assert_eq!(sim.scenario_rng.logical_state(), rng);
}

#[test]
fn missing_aircraft_producer_retains_completed_aircraft_and_queued_successor() {
    let (mut sim, rules, owner) = world(0xfac7_0018);
    spawn_structure(&mut sim, 5, "Americans", "GAAIRC", 26, 10);
    assert!(enqueue_by_type(&mut sim, &rules, "Americans", "ORCA"));
    assert!(enqueue_by_type(&mut sim, &rules, "Americans", "ORCA"));
    let held = held_id(&sim, owner, ProductionCategory::Aircraft);
    assert!(
        sim.production
            .factories
            .test_arm_ready(owner, ProductionCategory::Aircraft)
    );
    // This is FindFactory's absent producer branch, not a present, full pad.
    sim.substrate.entities.remove(5);
    let credits = sim.houses[&owner].economy.credits;
    let rng = (
        sim.main_rng.logical_state(),
        sim.scenario_rng.logical_state(),
        sim.mapgen_rng.logical_state(),
    );
    let allocated = sim.substrate.next_stable_object_id;
    assert!(!dispatch_production_changes_for_tests(
        &mut sim, &rules, None
    ));
    assert_eq!(held_id(&sim, owner, ProductionCategory::Aircraft), held);
    let factory = sim
        .production
        .factories
        .view(owner, ProductionCategory::Aircraft)
        .unwrap();
    assert!(factory.ready);
    assert_eq!(factory.progress, super::PRODUCTION_STEPS);
    assert_eq!(factory.queue.len(), 1);
    assert!(sim.substrate.entities.get(held).unwrap().lifecycle.in_limbo);
    assert_eq!(sim.houses[&owner].economy.credits, credits);
    assert_eq!(sim.substrate.next_stable_object_id, allocated);
    assert_eq!(
        (
            sim.main_rng.logical_state(),
            sim.scenario_rng.logical_state(),
            sim.mapgen_rng.logical_state()
        ),
        rng
    );
}

/// A finished object held for delivery goes with its factory. At a building's
/// kill `BuildingClass::Detach_All(1) @ 0x0044EBF0` abandons its own factory
/// and, for a Construction Yard, every production no other factory can build,
/// finished or not (AbandonProduction `0x004C9FF0` refunds what was paid and
/// deletes the object). With the only Construction Yard gone, the ready
/// building is refunded, deleted and untracked, and leaves the ready list.
#[test]
fn a_ready_building_goes_with_the_last_construction_yard() {
    let (mut sim, rules, owner) = world(0xfac7_0019);
    assert!(enqueue_by_type(&mut sim, &rules, "Americans", "GAPOWR"));
    let held = held_id(&sim, owner, ProductionCategory::Building);
    assert!(
        sim.production
            .factories
            .test_arm_ready(owner, ProductionCategory::Building)
    );
    super::publish_production_changes(&mut sim, &rules);
    assert_eq!(sim.production.ready_by_owner[&owner].len(), 1);
    let tracked = sim.houses[&owner].tracking.buildings();
    let credits = sim.houses[&owner].economy.credits;

    // With a Construction Yard standing, the ready building waits.
    super::revalidate_and_step_factories(&mut sim, &rules);
    assert!(sim.substrate.entities.contains(held));

    sim.substrate.entities.remove(3);
    super::revalidate_and_step_factories(&mut sim, &rules);
    assert!(!sim.substrate.entities.contains(held));
    assert_eq!(sim.houses[&owner].tracking.buildings(), tracked - 1);
    assert_eq!(sim.houses[&owner].economy.credits, credits + 800);
    assert!(
        sim.production
            .ready_by_owner
            .get(&owner)
            .is_none_or(|ready| ready.is_empty())
    );
    assert!(
        sim.production
            .factories
            .view(owner, ProductionCategory::Building)
            .is_none()
    );
}

/// The same for a finished vehicle held at its factory: with the last War
/// Factory gone, the tank and its spawns are deleted, untracked and refunded.
#[test]
fn a_held_vehicle_goes_with_the_last_war_factory() {
    let (mut sim, rules, owner) = world(0xfac7_001a);
    assert!(enqueue_by_type(&mut sim, &rules, "Americans", "MTNK"));
    let held = held_id(&sim, owner, ProductionCategory::Vehicle);
    let child_ids = children(&sim, held);
    assert!(
        sim.production
            .factories
            .test_arm_ready(owner, ProductionCategory::Vehicle)
    );
    assert_eq!(sim.houses[&owner].tracking.units_for_test(), 1);
    let credits = sim.houses[&owner].economy.credits;

    sim.substrate.entities.remove(1);
    super::revalidate_and_step_factories(&mut sim, &rules);
    assert_gone(&sim, held, &child_ids);
    assert_eq!(sim.houses[&owner].tracking.units_for_test(), 0);
    assert_eq!(sim.houses[&owner].economy.credits, credits + 700);
}

/// A 900-credit tank buildable from a war factory, and an Industrial Plant type
/// (`UnitsCostBonus=.75`) that is not yet on the map, for a house with no money.
fn plant_world() -> (Simulation, RuleSet, InternedId) {
    let rules = RuleSet::from_ini(&IniFile::from_str(
        "[Countries]\n0=Americans\n1=Russians\n[VehicleTypes]\n0=HTNK\n\
         [BuildingTypes]\n0=NAWEAP\n1=NAINDP\n\
         [HTNK]\nOwner=Russians\nCost=900\nStrength=400\nSpeed=5\nTechLevel=1\n\
         [NAWEAP]\nOwner=Russians\nFactory=UnitType\n\
         [NAINDP]\nStrength=1000\nFoundation=1x1\nFactoryPlant=yes\nUnitsCostBonus=.75\n",
    ))
    .expect("FactoryPlant fixture");
    let mut sim = Simulation::with_seed(0xc057_0f01);
    sim.install_resolved_terrain_for_new_map(crate::map::resolved_terrain::test_flat_ground_grid(
        32,
    ));
    sim.intern_rule_type_ids(&rules);
    sim.resolve_type_handles(&rules);
    let owner = sim.interner.intern("Russians");
    sim.houses.insert(
        owner,
        crate::sim::house_state::HouseState::new(owner, 0, None, true, 0, 10),
    );
    spawn_structure(&mut sim, 1, "Russians", "NAWEAP", 10, 10);
    (sim, rules, owner)
}

fn spawn_plant(sim: &mut Simulation, rules: &RuleSet) {
    sim.spawn_object("NAINDP", "Russians", 20, 20, 0, rules)
        .expect("the Industrial Plant unlimbos");
}

/// With no money the tank still starts (`HouseClass::CanBuild @ 0x004F7870` has no
/// money check) owing its house's Cost_Of (`0x004C9DE1`), which a live Industrial
/// Plant discounts, and the sidebar offers it at that price, not greyed.
#[test]
fn a_build_starts_without_money_owing_its_cost_of() {
    let (mut sim, rules, owner) = plant_world();
    spawn_plant(&mut sim, &rules);
    let options = super::build_options_for_owner(&sim, &rules, "Russians");
    let tank = options
        .iter()
        .find(|option| sim.interner.resolve(option.type_id) == "HTNK")
        .expect("the tank is offered");
    assert!(tank.enabled);
    assert_eq!(tank.cost, 675);
    assert!(enqueue_by_type(&mut sim, &rules, "Russians", "HTNK"));
    for _ in 0..30 {
        sim.advance_tick(&[], Some(&rules), None, None, 67);
    }
    let factory = sim
        .production
        .factories
        .test_factory_mut(owner, ProductionCategory::Vehicle)
        .unwrap();
    assert_eq!(factory.balance, 675);
    assert!(factory.on_hold && factory.progress == 0);
}

/// The cancel refund is the Cost_Of at cancel time less the Balance still owed
/// (`0x004CA029..0x004CA046`). An Industrial Plant built after the first step
/// lowers the Cost_Of below that Balance, so the cancel takes money.
#[test]
fn a_cancel_refunds_the_cost_of_at_cancel_time() {
    let (mut sim, rules, owner) = plant_world();
    assert!(enqueue_by_type(&mut sim, &rules, "Russians", "HTNK"));
    crate::sim::credit_income::add_credits(&mut sim, owner, 1000);
    let progress = |sim: &mut Simulation| {
        let factory = sim
            .production
            .factories
            .test_factory_mut(owner, ProductionCategory::Vehicle)
            .unwrap();
        (factory.progress, factory.balance)
    };
    for _ in 0..2000 {
        if progress(&mut sim).0 > 0 {
            break;
        }
        sim.advance_tick(&[], Some(&rules), None, None, 67);
    }
    // The first step pays 900 / 53.
    assert_eq!(progress(&mut sim), (1, 884));
    spawn_plant(&mut sim, &rules);
    let before = sim.houses[&owner].economy.credits;
    assert!(cancel_by_type_for_owner(
        &mut sim, &rules, "Russians", "HTNK", false
    ));
    assert_eq!(sim.houses[&owner].economy.credits, before + 675 - 884);
}

/// Exit4440BC calls FreeRadio65ADC0: stock one-slot barracks return1 while
/// occupied. Human House4FB57F..4FB62A sees Building+524NULL and refunds,
/// scalar-destroys the held GI and starts the FIFO successor. Body/caller
/// evidence: basic-factory-output-research native-radio-free-65adc0.json and
/// native-house-place-4fb0e0.json. This is an owner regression with supplied
/// paid-ready priors, not a comparison of the native charging cadence.
#[test]
fn occupied_barracks_radio_refunds_discards_and_promotes_one_gi() {
    let (mut sim, rules, owner) = world(0xfac7_0023);
    super::tests::install_infantry_delivery_fixture_map(&mut sim);
    let category = ProductionCategory::Infantry;
    let producer = 2;
    let cost = sim.cost_of(owner, rules.object("E1").unwrap(), &rules);
    assert!(cost > 0);

    assert!(enqueue_by_type(&mut sim, &rules, "Americans", "E1"));
    let first = held_id(&sim, owner, category);
    // Supply the paid-completion wallet and Balance0 through their existing
    // owners. The actual constructor, publication, PLACE and HELLO2/9 tail run.
    assert_eq!(
        sim.houses.get_mut(&owner).unwrap().economy.spend(cost),
        cost
    );
    assert!(sim.production.factories.test_arm_ready(owner, category));
    assert!(dispatch_production_changes_for_tests(
        &mut sim, &rules, None
    ));
    assert!(sim.pending_command_snapshot().is_empty());
    let first_entity = sim.substrate.entities.get(first).unwrap();
    assert!(!first_entity.lifecycle.in_limbo && first_entity.in_logic_vector);
    let first_contacts = first_entity.radio_contacts.clone();
    assert_eq!(first_contacts.slot(0), Some(producer));
    assert_eq!(first_entity.dock_entered_with, Some(producer));
    let producer_entity = sim.substrate.entities.get(producer).unwrap();
    let producer_contacts = producer_entity.radio_contacts.clone();
    assert_eq!(producer_contacts.capacity(), 1);
    assert_eq!(producer_contacts.slot(0), Some(first));
    assert_eq!(producer_contacts.first_free(), None);
    assert_eq!(producer_entity.dock_entered_with, Some(first));
    assert!(
        sim.production
            .factories
            .building_factory(producer)
            .is_none()
    );

    assert!(enqueue_by_type(&mut sim, &rules, "Americans", "E1"));
    assert!(enqueue_by_type(&mut sim, &rules, "Americans", "E1"));
    let refused = held_id(&sim, owner, category);
    assert_ne!(refused, first);
    // This completed second head and post-payment wallet are supplied test
    // priors. No live GI turn is invented to keep the first real link occupied.
    assert_eq!(
        sim.houses.get_mut(&owner).unwrap().economy.spend(cost),
        cost
    );
    assert!(sim.production.factories.test_arm_ready(owner, category));
    assert_eq!(
        sim.production
            .factories
            .test_factory_mut(owner, category)
            .unwrap()
            .balance,
        0
    );
    super::publish_production_changes(&mut sim, &rules);
    let pending = sim.pending_command_snapshot();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].owner, owner);
    assert!(matches!(
        pending[0].payload,
        crate::sim::command::Command::PlaceProducedMobile {
            category: ProductionCategory::Infantry
        }
    ));
    assert_eq!(held_id(&sim, owner, category), refused);
    assert!(
        sim.substrate
            .entities
            .get(refused)
            .unwrap()
            .lifecycle
            .in_limbo
    );
    let completed = sim.production.factories.view(owner, category).unwrap();
    assert!(completed.ready);
    assert_eq!(completed.queue.len(), 1);
    let credits = sim.houses[&owner].economy.credits;
    let spent = sim.houses[&owner].economy.spent_credits;
    let refund = sim.cost_of(owner, rules.object("E1").unwrap(), &rules);
    let prior_ids: Vec<_> = sim
        .substrate
        .entities
        .iter_sorted()
        .map(|(id, _)| id)
        .collect();
    let event_start = sim.sound_events.len();
    let main = sim.main_rng.logical_state();
    let mapgen = sim.mapgen_rng.logical_state();
    let mut expected_scenario = sim.scenario_rng.clone();

    assert!(!dispatch_production_changes_for_tests(
        &mut sim, &rules, None
    ));
    assert!(sim.pending_command_snapshot().is_empty());
    assert!(
        !sim.substrate.entities.contains(refused),
        "scalar cleanup is synchronous"
    );
    assert_eq!(sim.houses[&owner].economy.credits, credits + refund);
    assert_eq!(sim.houses[&owner].economy.spent_credits, spent);
    let factory = sim.production.factories.view(owner, category).unwrap();
    let successor = factory.object.unwrap().entity_id.unwrap();
    assert!(successor > refused);
    assert_eq!(factory.progress, 0);
    assert!(!factory.ready && factory.queue.is_empty());
    let new_ids: Vec<_> = sim
        .substrate
        .entities
        .iter_sorted()
        .map(|(id, _)| id)
        .filter(|id| !prior_ids.contains(id))
        .collect();
    assert_eq!(
        new_ids,
        vec![successor],
        "exactly one FIFO successor constructs"
    );
    let successor_entity = sim.substrate.entities.get(successor).unwrap();
    assert!(successor_entity.lifecycle.in_limbo && !successor_entity.lifecycle.cell_marked);
    assert!(!successor_entity.in_logic_vector);
    let first_entity = sim.substrate.entities.get(first).unwrap();
    assert_eq!(first_entity.radio_contacts, first_contacts);
    assert_eq!(first_entity.dock_entered_with, Some(producer));
    let producer_entity = sim.substrate.entities.get(producer).unwrap();
    assert_eq!(producer_entity.radio_contacts, producer_contacts);
    assert_eq!(producer_entity.dock_entered_with, Some(first));
    assert!(
        sim.production
            .factories
            .building_factory(producer)
            .is_none()
    );
    assert!(!sim.sound_events[event_start..].iter().any(|event| matches!(
        event,
        crate::sim::world::SimSoundEvent::UnitComplete { owner: receiver, .. } if *receiver == owner
    )));
    assert_constructor_words(&sim, successor, &mut expected_scenario);
    assert_eq!(sim.main_rng.logical_state(), main);
    assert_eq!(sim.mapgen_rng.logical_state(), mapgen);
}
