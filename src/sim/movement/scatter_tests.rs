use super::*;
use crate::sim::components::{DriveCoord, NavTargetRef};
use crate::sim::deploy::DeployPhase;
use crate::sim::mission::{MissionId, MissionType};
use crate::sim::movement::{FacingClass, locomotor::LocomotorState};

fn no_rules() -> RuleSet {
    RuleSet::from_ini(&crate::rules::ini_parser::IniFile::from_str("")).unwrap()
}

fn with_mission(actor: &GameEntity, mission: MissionType) -> GameEntity {
    let mut actor = actor.clone();
    actor
        .mission
        .apply_test_fixture(crate::sim::mission::state::MissionTestFixture {
            current: MissionId::from_known(mission),
            queued: MissionId::NONE,
            suspended: MissionId::NONE,
            movement_bypass_latch: 0,
            handler_state: 0,
            mission_start_frame: 0,
            ai_counter: 0,
            dispatch_timer: crate::sim::mission::MissionDispatchTimer::at_frame(0),
        });
    actor
}

/// The Unit receiver's refusals before its coordinate, with both flags set so
/// neither the mission's `Scatter=` nor a NavCom decides (the corpus leaves
/// them to the flags). A refusal draws no RNG and writes nothing.
#[test]
fn unit_refusals_match_native_and_leave_orders_and_rng_untouched() {
    let rows: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tools/spatial_oracle/unit_scatter_state.json"
    ))
    .unwrap();
    let rules = no_rules();
    let flags = ScatterFlags::new(true, true);
    let mut checked = 0;
    for row in rows.as_array().unwrap() {
        let input = &row["input"];
        let flag = |key: &str| input[key].as_bool().unwrap_or(false);
        let mission =
            |key: &str, default| MissionId::from_raw(input[key].as_i64().unwrap_or(default) as i32);
        let frame = input["frame"].as_u64().unwrap_or(100) as u32;
        let now = frame.wrapping_add(input["elapsed"].as_u64().unwrap_or(0) as u32);
        let mut actor = GameEntity::test_default(1, "MTNK", "Allies", 5, 5);
        actor.category = EntityCategory::Unit;
        actor
            .mission
            .apply_test_fixture(crate::sim::mission::state::MissionTestFixture {
                current: mission("mission", 5),
                queued: mission("queued", -1),
                suspended: MissionId::NONE,
                movement_bypass_latch: 0,
                handler_state: 0,
                mission_start_frame: 0,
                ai_counter: 0,
                dispatch_timer: crate::sim::mission::MissionDispatchTimer::at_frame(0),
            });
        actor.locomotor = Some(LocomotorState::for_test_kind(if flag("teleport") {
            LocomotorKind::Teleport
        } else {
            LocomotorKind::Drive
        }));
        actor.locomotor.as_mut().unwrap().powered = !flag("power_off");
        actor.deploy_state = if flag("deployed") {
            Some(DeployPhase::Deployed)
        } else if flag("deploying") {
            Some(DeployPhase::Deploying { ticks_remaining: 5 })
        } else if flag("undeploying") {
            Some(DeployPhase::Undeploying { ticks_remaining: 5 })
        } else {
            None
        };
        if flag("nav") {
            actor.navigation.nav_com = Some(NavTargetRef::cell(8, 8));
        }
        let mut facing = FacingClass::new(0, input["rot"].as_i64().unwrap_or(5) as i32);
        if flag("turn") {
            facing.set(0x4000, frame);
        }
        actor.body_facing = facing;
        let admitted = unit_scatter_admitted(&actor, flags, &rules, now);
        assert_eq!(admitted, row["admitted"].as_bool().unwrap(), "{input}");
        if !admitted {
            // The production receiver, not only the predicate: a refusal
            // precedes the search, the setter and every write.
            let before = serde_json::to_value(&actor).unwrap();
            let mut sim = Simulation::new();
            sim.session.binary_frame = now;
            sim.substrate.entities.insert(actor);
            let rng = sim.scenario_rng.state();
            assert!(!sim.scatter_null(1, flags, &rules, None).unwrap());
            assert_eq!(
                serde_json::to_value(sim.substrate.entities.get(1).unwrap()).unwrap(),
                before,
                "{input}"
            );
            assert_eq!(sim.scenario_rng.state(), rng, "{input}");
        }
        checked += 1;
    }
    assert_eq!(checked, 69);
}

/// Without the flags the mission's `Scatter=` (`0x00743AD8..0x00743AEA`) and
/// a NavCom (`0x00743B1D..0x00743B2D`) refuse.
#[test]
fn unit_flags_override_mission_scatter_and_nav_com() {
    let rules = RuleSet::from_ini(&crate::rules::ini_parser::IniFile::from_str(
        "[Guard]\nScatter=no\n",
    ))
    .unwrap();
    let mut actor = GameEntity::test_default(1, "MTNK", "Allies", 5, 5);
    actor.category = EntityCategory::Unit;
    actor.locomotor = Some(LocomotorState::for_test_kind(LocomotorKind::Drive));
    let guard = with_mission(&actor, MissionType::Guard);
    let hunt = with_mission(&actor, MissionType::Hunt);
    assert!(!unit_scatter_admitted(
        &guard,
        ScatterFlags::new(false, false),
        &rules,
        100
    ));
    assert!(unit_scatter_admitted(
        &guard,
        ScatterFlags::new(true, false),
        &rules,
        100
    ));
    assert!(unit_scatter_admitted(
        &hunt,
        ScatterFlags::new(false, false),
        &rules,
        100
    ));
    let mut nav = hunt.clone();
    nav.navigation.nav_com = Some(NavTargetRef::cell(8, 8));
    assert!(!unit_scatter_admitted(
        &nav,
        ScatterFlags::new(true, false),
        &rules,
        100
    ));
    assert!(unit_scatter_admitted(
        &nav,
        ScatterFlags::new(false, true),
        &rules,
        100
    ));
}

#[test]
fn unit_checks_active_locomotor_and_body_rotation_not_stashed_slot_or_turret() {
    let rules = no_rules();
    let flags = ScatterFlags::new(true, true);
    let mut actor = GameEntity::test_default(1, "CMIN", "Allies", 5, 5);
    actor.category = EntityCategory::Unit;
    let mut actor = with_mission(&actor, MissionType::Move);
    actor.locomotor = Some(LocomotorState::for_test_kind(LocomotorKind::Teleport));
    assert!(!unit_scatter_admitted(&actor, flags, &rules, 100));
    assert!(crate::sim::movement::locomotor_owner::begin_drive_for_teleporter(&mut actor, 100));
    actor.turret_rotation_latch = true;
    actor.navigation.nav_com = Some(NavTargetRef::cell(8, 8));
    actor.drive_locomotion = Some(crate::sim::components::DriveLocomotionRuntime {
        head_to: Some(DriveCoord::cell(6, 5, 0)),
        ..Default::default()
    });
    assert!(
        unit_scatter_admitted(&actor, flags, &rules, 100),
        "no moving or turret gate, and no-kidding passes the NavCom"
    );
}

/// A pass asks each object once, keeps call order across takes, and never
/// asks again an object it already asked.
#[test]
fn scatter_requests_keep_call_order_and_ask_each_object_once() {
    let mut requests = ScatterRequests::default();
    assert!(requests.request(4, ScatterFlags::new(true, true)));
    assert!(requests.request(2, ScatterFlags::new(true, false)));
    assert!(!requests.request(4, ScatterFlags::new(true, false)));
    assert_eq!(
        requests.take(),
        vec![
            (4, ScatterFlags::new(true, true)),
            (2, ScatterFlags::new(true, false))
        ]
    );
    assert!(!requests.request(2, ScatterFlags::new(true, true)));
    assert!(requests.take().is_empty());
}

/// `UnitClass::Scatter` never asks its locomotor whether it moves, so a
/// driving vehicle is admitted like a parked one.
#[test]
fn unit_receiver_does_not_read_its_own_motion() {
    let mut actor = GameEntity::test_default(1, "MTNK", "Allies", 5, 5);
    actor.category = EntityCategory::Unit;
    let mut actor = with_mission(&actor, MissionType::Move);
    actor.locomotor = Some(LocomotorState::for_test_kind(LocomotorKind::Drive));
    let head = DriveCoord::cell(6, 5, 0);
    actor.foot_speed.applied_fraction = crate::util::fixed_math::SIM_ONE;
    actor.drive_locomotion = Some(crate::sim::components::DriveLocomotionRuntime {
        destination: Some(head),
        head_to: Some(head),
        ..Default::default()
    });
    assert_eq!(super::super::motion_query::is_moving(&actor), Some(true));
    assert!(unit_scatter_admitted(
        &actor,
        ScatterFlags::new(true, true),
        &no_rules(),
        100
    ));
}

/// Every class without its own receiver inherits `ObjectClass::Scatter`
/// (`0x005F43A0`, `RET 0xC`): a building takes the call and nothing happens.
#[test]
fn building_receiver_is_the_object_no_op() {
    let mut building = GameEntity::test_default(1, "GAREFN", "Allies", 5, 5);
    building.category = EntityCategory::Structure;
    let before = serde_json::to_value(&building).unwrap();
    let mut sim = Simulation::new();
    sim.substrate.entities.insert(building);
    let rng = sim.scenario_rng.state();
    assert!(
        !sim.scatter_null(1, ScatterFlags::new(true, true), &no_rules(), None)
            .unwrap()
    );
    assert_eq!(
        serde_json::to_value(sim.substrate.entities.get(1).unwrap()).unwrap(),
        before
    );
    assert_eq!(sim.scenario_rng.state(), rng);
}
