//! The airfield loop through the live frame: a spent AirportBound aircraft
//! on Mission_Guard (`0x0041A5C0`, a human house's) or Mission_AreaGuard
//! (`0x0041A940`, a computer house's) hunts its dock and commences
//! Mission_Enter (`0x00419C80`), which lands it on the pad its radio contact
//! names; the airfield's Mission_Repair (`0x0044C836`) reloads it and
//! releases it to Guard on the pad, still in radio contact. A Target sends
//! it off its pad again (Mission_Guard's Target arm, `0x0041A822`).

use crate::rules::{ini_parser::IniFile, ruleset::RuleSet};
use crate::sim::mission::{MissionId, MissionType};
use crate::sim::movement::air_movement;
use crate::sim::world::Simulation;

const RULES: &str = "[General]\nFlightLevel=1500\nReloadRate=.3\n\
[InfantryTypes]\n0=E1\n[VehicleTypes]\n0=VICTIM\n[AircraftTypes]\n0=ORCA\n\
[BuildingTypes]\n0=GAAIRC\n\
[ORCA]\nStrength=150\nSpeed=14\nAmmo=1\nLandable=yes\nAirportBound=yes\nFighter=yes\n\
Primary=TestGun\nDock=GAAIRC\nLocomotor={4A582746-9839-11D1-B709-00A024DDAFD1}\n\
[GAAIRC]\nStrength=1000\nFoundation=2x2\nHelipad=yes\nUnitReload=yes\nNumberOfDocks=4\n\
[TestGun]\nDamage=10\nRange=6\nROF=60\nProjectile=TestShot\nWarhead=TestWH\n\
[TestShot]\nROT=0\n[TestWH]\nVerses=100%\n\
[VICTIM]\nStrength=1000\nLocomotor={4A582741-9839-11D1-B709-00A024DDAFD1}\n\
[E1]\nStrength=100\n";

fn mission(sim: &Simulation, id: u64) -> Option<MissionType> {
    sim.substrate.entities.get(id)?.mission.current().known()
}

/// What a frame shows of the loop.
#[derive(Debug, Default)]
struct Milestones {
    entered: bool,
    linked: bool,
    landed_linked: bool,
    reloaded: bool,
    guarding_frames: u32,
}

fn run_cycle(sim: &mut Simulation, rules: &RuleSet, orca: u64, airfield: u64) -> Milestones {
    let mut seen = Milestones::default();
    for _ in 0..4000 {
        let _ = sim.advance_tick(&[], Some(rules), None, None, 33);
        let entity = sim.substrate.entities.get(orca).expect("aircraft survives");
        let linked = entity.radio_contacts.contains(airfield)
            && sim
                .substrate
                .entities
                .get(airfield)
                .is_some_and(|af| af.radio_contacts.contains(orca));
        let height = air_movement::current_fly_height(entity, sim.resolved_terrain.as_ref());
        seen.entered |= mission(sim, orca) == Some(MissionType::Enter);
        seen.linked |= linked;
        seen.landed_linked |= linked && height == 0;
        let full = entity
            .aircraft_ammo
            .as_ref()
            .is_some_and(|ammo| ammo.current == ammo.max);
        seen.reloaded |= seen.landed_linked && full;
        if seen.reloaded
            && mission(sim, orca) == Some(MissionType::Guard)
            && height == 0
            && linked
            && !air_movement::fly_moving(entity)
        {
            seen.guarding_frames += 1;
            if seen.guarding_frames == 300 {
                break;
            }
        }
    }
    seen
}

/// An Orca of a human or a computer house, landed and out of Ammo.
fn spent_orca(rules: &RuleSet, human: bool) -> (Simulation, u64, u64) {
    let mut sim = Simulation::new();
    let owner = sim.interner.intern("Americans");
    sim.houses.insert(
        owner,
        crate::sim::house_state::HouseState::new(owner, 0, None, human, 0, 10),
    );
    let airfield = sim
        .spawn_object_at_height("GAAIRC", "Americans", 10, 10, 0, 0, rules)
        .expect("airfield");
    let orca = sim
        .spawn_object_at_height("ORCA", "Americans", 24, 12, 0, 0, rules)
        .expect("aircraft");
    // Unlimbo's idle mode commenced its pick on the ground: Area Guard for
    // a computer house's armed aircraft, else Guard (`0x00417787`).
    let pick = if human {
        MissionType::Guard
    } else {
        MissionType::AreaGuard
    };
    assert_eq!(mission(&sim, orca), Some(pick));
    let entity = sim.substrate.entities.get_mut(orca).unwrap();
    entity.aircraft_ammo.as_mut().unwrap().current = 0;
    (sim, orca, airfield)
}

#[test]
fn a_spent_aircraft_enters_its_dock_reloads_and_guards_on_the_pad() {
    let rules = RuleSet::from_ini(&IniFile::from_str(RULES)).expect("rules");
    for human in [true, false] {
        let (mut sim, orca, airfield) = spent_orca(&rules, human);
        let seen = run_cycle(&mut sim, &rules, orca, airfield);
        assert!(seen.entered, "it Enters its dock (human {human}): {seen:?}");
        assert!(seen.landed_linked, "it lands in radio contact: {seen:?}");
        assert!(seen.reloaded, "the airfield reloads it: {seen:?}");
        assert_eq!(
            seen.guarding_frames, 300,
            "reloaded, it guards on its pad in contact (human {human}): {seen:?}"
        );
    }
}

#[test]
fn a_guarding_aircraft_with_a_target_leaves_its_pad_to_attack() {
    let rules = RuleSet::from_ini(&IniFile::from_str(RULES)).expect("rules");
    let (mut sim, orca, airfield) = spent_orca(&rules, true);
    let victim = sim
        .spawn_object("VICTIM", "Russians", 40, 30, 0, &rules)
        .expect("target");
    let seen = run_cycle(&mut sim, &rules, orca, airfield);
    assert_eq!(seen.guarding_frames, 300, "the aircraft reloads and guards");

    sim.substrate.entities.get_mut(orca).unwrap().attack_target =
        Some(crate::sim::combat::AttackTarget::new(victim));
    let mut relaunched = false;
    for _ in 0..600 {
        let _ = sim.advance_tick(&[], Some(&rules), None, None, 33);
        let entity = sim.substrate.entities.get(orca).expect("aircraft survives");
        if entity.mission.current() == MissionId::from_known(MissionType::Attack)
            && air_movement::fly_moving(entity)
        {
            relaunched = true;
            break;
        }
    }
    assert!(relaunched, "a guarding aircraft holding a Target attacks");
}
