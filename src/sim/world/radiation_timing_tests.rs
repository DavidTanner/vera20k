//! Production ordering of deployed DESO self-fire and Foot radiation visits.
//!
//! Foot4DA530 applies radiation after Techno AI, before locomotion; a Bullet
//! appended by later Guard521320 cannot retroactively damage earlier Foots.

use super::{FrameEffects, Simulation, TickLane};
use crate::rules::ruleset::RuleSet;
use crate::sim::combat::TargetKind;
use crate::sim::combat::combat_weapon::WeaponSlot;
use crate::sim::mission::state::MissionTestFixture;
use crate::sim::mission::{MissionDispatchTimer, MissionId, MissionType};

fn advance(sim: &mut Simulation, rules: &RuleSet) -> super::SimFrameOutput {
    let output = sim
        .advance_app_frame(
            Some(rules),
            None,
            33,
            TickLane::Ordinary,
            None,
            FrameEffects::default(),
        )
        .expect("ordinary radiation frame");
    assert!(output.tick.frame_committed);
    output
}

/// The deployed Doing27 boundary is supplied after real object admission;
/// Guard, Direct FireAt, Bullet admission/AI, field creation and subsequent
/// Foot turns all execute their ordinary production owners. Retail weapon
/// and radiation values are unchanged.
#[test]
fn stock_deso_first_site_does_not_pulse_an_earlier_foot_until_frame32() {
    let Some(fixture) = crate::rules::retail_ini_fixture::retail_battle_rules() else {
        return;
    };
    let rules = fixture.rules;
    let deso_type = rules.object("DESO").unwrap();
    assert!(deso_type.deployer && deso_type.deploy_fire && deso_type.immune_to_radiation);
    assert_eq!(deso_type.secondary(), Some("RadEruptionWeapon"));
    let weapon = rules.weapon("RadEruptionWeapon").unwrap();
    assert_eq!(weapon.rad_level, 500);
    assert!(weapon.area_fire);
    assert_eq!(weapon.projectile.as_deref(), Some("InvisibleLow"));
    assert!(rules.projectile("InvisibleLow").unwrap().inviso);
    assert_eq!(rules.radiation.application_delay, 16);

    // Frame17 cannot run the global modulo16 pulse, so its otherwise identical
    // real self-shot measures the immediate area damage without inventing an
    // AreaDamage value or disabling the stock weapon's damage.
    let (control, control_victim, initial_health) = create_first_site(&rules, 17);
    let health = control
        .substrate
        .entities
        .get(control_victim)
        .unwrap()
        .health
        .current();
    assert!(
        health < initial_health,
        "the real self-shot's direct damage is retained"
    );
    let (mut sim, victim, same_initial_health) = create_first_site(&rules, 16);
    assert_eq!(same_initial_health, initial_health);
    assert_eq!(
        sim.substrate.entities.get(victim).unwrap().health.current(),
        health,
        "the earlier Foot already completed frame16 before the first site existed"
    );

    while sim.session.binary_frame < 32 {
        advance(&mut sim, &rules);
        assert_eq!(
            sim.substrate.entities.get(victim).unwrap().health.current(),
            health,
            "no periodic pulse between frame16 and frame32"
        );
    }
    advance(&mut sim, &rules);
    assert!(
        sim.substrate
            .entities
            .get(victim)
            .is_none_or(|actor| actor.health.current() < health),
        "the existing site first damages this Foot on frame32"
    );
}

/// Legal initialized/deployed boundary, followed only by real production
/// Guard/self-shot/Bullet work. Both controls differ solely in global frame.
fn create_first_site(rules: &RuleSet, creation_frame: u32) -> (Simulation, u64, i32) {
    let mut sim = crate::sim::arena_fixture::rival_arena(rules, 31);
    let victim = sim
        .spawn_object("E1", "Americans", 11, 10, 0, rules)
        .unwrap();
    let deso = sim
        .spawn_object("DESO", "Americans", 10, 10, 0, rules)
        .unwrap();
    let order = sim.substrate.logic.as_slice();
    assert!(
        order.iter().position(|id| *id == victim).unwrap()
            < order.iter().position(|id| *id == deso).unwrap()
    );
    let initial_health = sim.substrate.entities.get(victim).unwrap().health.current();
    assert!(!rules.object("E1").unwrap().immune_to_radiation);
    let actor = sim.substrate.entities.get_mut(deso).unwrap();
    actor.mission_leaf.set_infantry_doing_verified(27).unwrap();
    actor.mission.apply_test_fixture(MissionTestFixture {
        current: MissionId::from_known(MissionType::Guard),
        suspended: MissionId::NONE,
        queued: MissionId::NONE,
        movement_bypass_latch: 0,
        handler_state: 0,
        mission_start_frame: creation_frame,
        ai_counter: 0,
        dispatch_timer: MissionDispatchTimer::at_frame(creation_frame),
    });
    sim.session.binary_frame = creation_frame;
    assert!(sim.radiation.is_empty());

    let output = advance(&mut sim, rules);
    assert!(
        output.fire_events.iter().any(|fire| {
            fire.attacker_id == deso
                && fire.weapon_slot == WeaponSlot::Secondary
                && sim.interner.resolve(fire.weapon_id) == "RadEruptionWeapon"
                && fire.target == TargetKind::Cell(10, 10)
        }),
        "deployed Guard fires the stock self-shot on frame{creation_frame}"
    );
    assert!(
        sim.radiation.site_at((10, 10)).is_some(),
        "the appended Bullet creates the first site in frame{creation_frame}"
    );
    assert!(
        sim.radiation
            .damaging_level((11, 10), rules.radiation.level_max)
            > 0
    );
    (sim, victim, initial_health)
}

/// The field is produced by stock Guard/Bullet first. Only the already-paid
/// Walk step is supplied, as in the shared Walk boundary fixtures: no path
/// search result or radiation value is replaced.
fn paid_walk_across(
    sim: &mut Simulation,
    rules: &RuleSet,
    id: u64,
    from: (u16, u16),
    to: (u16, u16),
) -> crate::sim::components::DriveCoord {
    use crate::sim::components::{DriveCoord, NavTargetRef};
    let east = to.0 > from.0;
    let current = DriveCoord {
        x: i32::from(from.0) * 256 + if east { 255 } else { 1 },
        y: i32::from(from.1) * 256 + 128,
        z: 0,
    };
    sim.foot_set_location_marked(id, current, Some(rules), None, FrameEffects::default());
    let head = DriveCoord {
        x: i32::from(to.0) * 256 + 128,
        y: current.y,
        z: 0,
    };
    let owner = sim.substrate.entities.get(id).unwrap().owner();
    crate::sim::movement::walk_head::raw_at(
        &mut sim.substrate.raw_cell_occupation,
        owner,
        head,
        true,
        sim.resolved_terrain.as_ref(),
        None,
    );
    let actor = sim.substrate.entities.get_mut(id).unwrap();
    actor.mission_leaf.set_infantry_doing_verified(2).unwrap();
    actor
        .mission
        .set_current_for_test(MissionId::from_known(MissionType::Move), 0);
    actor
        .navigation
        .set_nav_com_for_test(Some(NavTargetRef::cell(to.0, to.1)));
    let loco = actor.locomotor.as_mut().unwrap();
    loco.set_walk_destination(Some(head));
    loco.set_step_head(Some(head));
    loco.begin_walk_motion();
    current
}

fn field_edge(sim: &Simulation, rules: &RuleSet) -> ((u16, u16), (u16, u16)) {
    let inside_x = (11..31)
        .rev()
        .find(|x| {
            sim.radiation
                .damaging_level((*x, 10), rules.radiation.level_max)
                > 0
        })
        .unwrap();
    let inside = (inside_x, 10);
    let outside = (inside_x + 1, 10);
    assert_eq!(
        sim.radiation
            .damaging_level(outside, rules.radiation.level_max),
        0
    );
    (inside, outside)
}

#[test]
fn stock_radiation_samples_the_cell_before_entering_or_leaving_it() {
    let Some(fixture) = crate::rules::retail_ini_fixture::retail_battle_rules() else {
        return;
    };
    let rules = fixture.rules;
    for entering in [true, false] {
        let (mut sim, victim, _) = create_first_site(&rules, 17);
        let (inside, outside) = field_edge(&sim, &rules);
        let (from, to) = if entering {
            (outside, inside)
        } else {
            (inside, outside)
        };
        paid_walk_across(&mut sim, &rules, victim, from, to);
        let health = sim.substrate.entities.get(victim).unwrap().health.current();
        sim.session.binary_frame = 32;
        advance(&mut sim, &rules);
        let actor = sim.substrate.entities.get(victim).unwrap();
        assert_eq!(
            (actor.position.rx(), actor.position.ry()),
            to,
            "the admitted paid step must actually cross the field edge"
        );
        if entering {
            assert_eq!(
                actor.health.current(),
                health,
                "entry occurs after Foot's cell sample"
            );
        } else {
            assert!(
                actor.health.current() < health,
                "leaving cannot evade the pre-movement pulse"
            );
        }
    }
}

#[test]
fn lethal_stock_radiation_stops_the_current_foot_before_its_paid_step() {
    let Some(fixture) = crate::rules::retail_ini_fixture::retail_battle_rules() else {
        return;
    };
    let rules = fixture.rules;
    for fatal in [false, true] {
        let (mut sim, victim, _) = create_first_site(&rules, 17);
        let (inside, outside) = field_edge(&sim, &rules);
        let before = paid_walk_across(&mut sim, &rules, victim, inside, outside);
        if fatal {
            sim.substrate
                .entities
                .get_mut(victim)
                .unwrap()
                .health
                .set_for_test(1);
        }
        sim.session.binary_frame = 32;
        let output = advance(&mut sim, &rules);
        if fatal {
            if let Some(actor) = sim.substrate.entities.get(victim) {
                assert!(actor.health.current() <= 0);
                assert_eq!(
                    crate::sim::movement::ground_pose::object_get_coords(
                        actor,
                        sim.resolved_terrain.as_ref()
                    ),
                    before,
                    "fatal receiver must return before locomotor Process"
                );
            }
            assert!(
                !output
                    .fire_events
                    .iter()
                    .any(|fire| fire.attacker_id == victim)
            );
        } else {
            let actor = sim.substrate.entities.get(victim).unwrap();
            assert!(actor.health.current() > 0);
            assert_eq!(
                (actor.position.rx(), actor.position.ry()),
                outside,
                "the corresponding surviving Foot executes the same paid step"
            );
        }
    }
}

#[test]
fn an_off_map_limbo_foot_does_not_receive_a_stale_cell_pulse() {
    let Some(fixture) = crate::rules::retail_ini_fixture::retail_battle_rules() else {
        return;
    };
    let rules = fixture.rules;
    let (mut sim, _, _) = create_first_site(&rules, 17);
    let limbo = sim
        .spawn_object_limbo_at_height("E1", "Americans", 11, 10, 0, 0, &rules)
        .unwrap();
    sim.foot_set_location_marked(
        limbo,
        crate::sim::components::DriveCoord {
            x: 11 * 256 + 128,
            y: 10 * 256 + 128,
            z: 0,
        },
        Some(&rules),
        None,
        FrameEffects::default(),
    );
    let actor = sim.substrate.entities.get(limbo).unwrap();
    assert!(actor.lifecycle.in_limbo());
    assert!(!sim.substrate.logic.as_slice().contains(&limbo));
    let health = actor.health.current();
    assert!(
        sim.radiation
            .damaging_level((11, 10), rules.radiation.level_max)
            > 0
    );
    sim.session.binary_frame = 32;
    advance(&mut sim, &rules);
    assert_eq!(
        sim.substrate.entities.get(limbo).unwrap().health.current(),
        health
    );
}

#[test]
fn foot_radiation_direct_dispatch_does_not_require_cell_marking() {
    let Some(fixture) = crate::rules::retail_ini_fixture::retail_battle_rules() else {
        return;
    };
    let rules = fixture.rules;
    let (mut sim, victim, _) = create_first_site(&rules, 17);
    assert!(sim.unmark_entity_remove(victim, super::UninitContext::new(Some(&rules), None),));
    let actor = sim.substrate.entities.get(victim).unwrap();
    assert!(actor.lifecycle.object_alive() && !actor.lifecycle.in_limbo());
    assert!(!actor.lifecycle.cell_marked());
    let health = actor.health.current();
    sim.session.binary_frame = 32;
    // Supply the Foot prefix's live Mark(REMOVE) boundary. IsInAir is false
    // for an unmarked object; native4DA629 still dispatches its receiver.
    let _receipt = crate::sim::combat::world_receiver::apply_foot_radiation(
        &mut sim,
        &rules,
        victim,
        None,
        FrameEffects::default(),
    );
    assert!(sim.substrate.entities.get(victim).unwrap().health.current() < health);
}

#[test]
fn foot_radiation_dispatches_a_raw_alive_zero_health_receiver() {
    let Some(fixture) = crate::rules::retail_ini_fixture::retail_battle_rules() else {
        return;
    };
    let rules = fixture.rules;
    let (mut sim, victim, _) = create_first_site(&rules, 17);
    let actor = sim.substrate.entities.get_mut(victim).unwrap();
    actor.health.set_for_test(0);
    assert!(actor.lifecycle.object_alive() && actor.lifecycle.cell_marked());
    assert!(!actor.lifecycle.in_limbo());
    sim.session.binary_frame = 32;
    // Foot does not gate Health. Techno70202E..70203E forces its fatal
    // continuation after the Object receiver returns with Health still zero.
    let _receipt = crate::sim::combat::world_receiver::apply_foot_radiation(
        &mut sim,
        &rules,
        victim,
        None,
        FrameEffects::default(),
    );
    assert!(
        sim.substrate
            .entities
            .get(victim)
            .is_none_or(|actor| !actor.lifecycle.object_alive()),
        "direct concrete receiver must enter its zero-health fatal continuation"
    );
}

#[test]
fn a_later_foot_can_receive_a_site_created_by_an_earlier_bullet() {
    let Some(fixture) = crate::rules::retail_ini_fixture::retail_battle_rules() else {
        return;
    };
    let rules = fixture.rules;
    let (control, victim, _) = create_first_site(&rules, 17);
    let direct_only_health = control
        .substrate
        .entities
        .get(victim)
        .unwrap()
        .health
        .current();
    let mut sim = crate::sim::arena_fixture::rival_arena(&rules, 31);
    let deso = sim
        .spawn_object("DESO", "Americans", 10, 10, 0, &rules)
        .unwrap();
    sim.session.binary_frame = 16;
    // A real admitted FireAt before this Logic pass, followed by a Foot
    // admission, supplies the lawful Bullet-before-Foot order. The test does
    // not move existing Logic slots or directly construct a radiation site.
    sim.commit_fire_visit(
        crate::sim::combat::world_fire::FireVisit::Direct {
            id: deso,
            target: TargetKind::Cell(10, 10),
            weapon_index: 1,
        },
        &rules,
        None,
        FrameEffects::default(),
    );
    let bullet = *sim
        .projectiles
        .iter()
        .next()
        .expect("admitted radiation Bullet")
        .0;
    // Native55B613 advances the cursor even after a terminal Bullet removes
    // its slot. Its immediate successor is therefore skipped. Supply that
    // ordinary successor explicitly so `late` really receives a later visit.
    let spacer = sim
        .spawn_object("E1", "Americans", 25, 25, 0, &rules)
        .unwrap();
    let late = sim
        .spawn_object("E1", "Americans", 11, 10, 0, &rules)
        .unwrap();
    let order = sim.substrate.logic.as_slice();
    let bullet_index = order.iter().position(|id| *id == bullet).unwrap();
    assert_eq!(order[bullet_index + 1], spacer);
    assert_eq!(order[bullet_index + 2], late);
    assert!(sim.radiation.is_empty());
    advance(&mut sim, &rules);
    assert!(
        sim.projectiles.get(bullet).is_none(),
        "the Bullet completed and retired on frame16"
    );
    assert!(sim.radiation.site_at((10, 10)).is_some());
    let late_health = sim.substrate.entities.get(late).unwrap().health.current();
    assert!(
        late_health < direct_only_health,
        "visited later Foot sees the frame16 site: health {late_health}, direct-only {direct_only_health}"
    );
}

#[test]
fn lethal_stock_radiation_prevents_the_unit_fire_suffix() {
    let Some(fixture) = crate::rules::retail_ini_fixture::retail_battle_rules() else {
        return;
    };
    let rules = fixture.rules;
    for fatal in [false, true] {
        let (mut sim, _, _) = create_first_site(&rules, 17);
        let gunner = sim
            .spawn_object("MTNK", "Americans", 11, 13, 0, &rules)
            .unwrap();
        let target = sim
            .spawn_object("MTNK", "Russians", 11, 11, 0, &rules)
            .unwrap();
        sim.assign_target_represented(
            gunner,
            Some(TargetKind::Entity(target)),
            Some(&rules),
            None,
            FrameEffects::default(),
        )
        .unwrap();
        let actor = sim.substrate.entities.get_mut(gunner).unwrap();
        actor.mission.apply_test_fixture(MissionTestFixture {
            current: MissionId::from_known(MissionType::Attack),
            suspended: MissionId::NONE,
            queued: MissionId::NONE,
            movement_bypass_latch: 0,
            handler_state: 0,
            mission_start_frame: 32,
            ai_counter: 0,
            dispatch_timer: MissionDispatchTimer::at_frame(32),
        });
        if fatal {
            actor.health.set_for_test(1);
        }
        sim.session.binary_frame = 32;
        let output = advance(&mut sim, &rules);
        let fired = output
            .fire_events
            .iter()
            .any(|fire| fire.attacker_id == gunner);
        if fatal {
            assert!(
                sim.substrate
                    .entities
                    .get(gunner)
                    .is_none_or(|actor| actor.health.current() <= 0)
            );
            assert!(
                !fired,
                "a lethal Foot pulse returns before Unit's firing suffix"
            );
        } else {
            assert!(
                fired,
                "the matching live unit must actually reach its firing suffix"
            );
        }
    }
}
