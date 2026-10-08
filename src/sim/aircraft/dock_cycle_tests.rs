//! Production dock cycle through the live frame: an AirportBound aircraft
//! returns, publishes its airfield radio contact, lands through Fly
//! BeginLanding4CFA70/Process_Landing4CE840, reloads, and stays parked on its
//! pad in radio contact, as Mission_Guard (`0x0041A5C0`) keeps a landed
//! aircraft without a Target.

use super::AircraftMission;
use crate::rules::{ini_parser::IniFile, ruleset::RuleSet};
use crate::sim::movement::locomotor::AirMovePhase;
use crate::sim::world::Simulation;

const RULES: &str = "[General]\nFlightLevel=1500\n\
[InfantryTypes]\n[VehicleTypes]\n[AircraftTypes]\n0=ORCA\n[BuildingTypes]\n0=GAAIRC\n\
[ORCA]\nStrength=150\nSpeed=14\nAmmo=1\nLandable=yes\nAirportBound=yes\nFighter=yes\n\
Dock=GAAIRC\nLocomotor={4A582746-9839-11D1-B709-00A024DDAFD1}\n\
[GAAIRC]\nStrength=1000\nFoundation=2x2\nHelipad=yes\nUnitReload=yes\nNumberOfDocks=4\n";

#[derive(Default)]
struct Milestones {
    landed_with_contact: bool,
    reloaded: bool,
    parked_frames: u32,
}

fn run_cycle(sim: &mut Simulation, rules: &RuleSet, orca: u64, airfield: u64) -> Milestones {
    let mut seen = Milestones::default();
    for _ in 0..4000 {
        let _ = sim.advance_tick(&[], Some(rules), None, None, 33);
        let entity = sim.substrate.entities.get(orca).expect("aircraft survives");
        let phase = crate::sim::movement::air_movement::fly_mission_phase(
            entity,
            sim.resolved_terrain.as_ref(),
        );
        let docking_state = match entity.aircraft_mission {
            Some(AircraftMission::Docking { sub_state, .. }) => Some(sub_state),
            _ => None,
        };
        if phase == Some(AirMovePhase::Landed) && docking_state == Some(2) {
            seen.landed_with_contact |= entity.radio_contacts.contains(airfield)
                && sim
                    .substrate
                    .entities
                    .get(airfield)
                    .is_some_and(|af| af.radio_contacts.contains(orca));
        }
        if entity
            .aircraft_ammo
            .as_ref()
            .is_some_and(|a| a.current == a.max)
            && seen.landed_with_contact
        {
            seen.reloaded = true;
        }
        if seen.reloaded
            && matches!(
                entity.aircraft_mission,
                Some(AircraftMission::DockedIdle { .. })
            )
            && phase == Some(AirMovePhase::Landed)
            && entity.radio_contacts.contains(airfield)
        {
            seen.parked_frames += 1;
        }
        if seen.parked_frames == 300 {
            break;
        }
    }
    seen
}

#[test]
fn airport_bound_aircraft_docks_reloads_and_parks_through_world_owners() {
    let rules = RuleSet::from_ini(&IniFile::from_str(RULES)).expect("rules");
    let mut sim = Simulation::new();
    let airfield = sim
        .spawn_object_at_height("GAAIRC", "Americans", 10, 10, 0, 0, &rules)
        .expect("airfield");
    let orca = sim
        .spawn_object_at_height("ORCA", "Americans", 24, 12, 0, 0, &rules)
        .expect("aircraft");
    // A spent strike craft: Guard with no ammo returns to its airfield.
    let entity = sim.substrate.entities.get_mut(orca).unwrap();
    entity.aircraft_ammo.as_mut().unwrap().current = 0;
    entity.aircraft_mission = Some(AircraftMission::Guard);

    let seen = run_cycle(&mut sim, &rules, orca, airfield);
    assert!(
        seen.landed_with_contact,
        "AirportBound landing needs the airfield's two-sided radio contact"
    );
    assert!(seen.reloaded, "a landed aircraft reloads on its pad");
    assert_eq!(
        seen.parked_frames, 300,
        "a reloaded aircraft without orders stays on its pad in contact"
    );
}
