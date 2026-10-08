//! `AircraftClass::Enter_Idle_Mode @ 0x004176F0` (vt+0x484), the one owner of
//! an aircraft's idle mode. Mission_Move (state 0 without a NavCom, state 3
//! once stopped) and Mission_Attack's state 10 call it with `(0, 1)`, as do
//! Mission_Unload once the hold is empty and VERA's own Idle state.
//!
//! - The head (`0x004176F8`): a suspended selector (vt+0x1FC, `0x005B3A10`)
//!   is restored (vt+0x1F8) and nothing else happens; a restored Patrol
//!   restarts at Mission+0xBC 0 with `+0x6D2` clear. Then Retreat, the two
//!   Paradrop and the two Spy Plane missions keep running unless an
//!   Airstrike (`+0x294`) owns the aircraft (jump table `0x00417BAC`), and a
//!   queued ParadropApproach or SpyplaneApproach commences at once
//!   (`0x00417764`). All three return 0.
//! - `FootClass::Enter_Idle_Mode @ 0x004D82B0`, whose answer is the call's.
//! - The pick (`0x00417787`): Area Guard for a computer house's unteamed armed
//!   aircraft, else Guard.
//! - The landed arm (`0x00417A38`), for an aircraft on the ground layer
//!   (vt+0x78), at or below its landing altitude (vt+0x1C8 against
//!   IFlyControl+0xC) or `MissileSpawn=` (Type `+0xD68`), whatever its layer
//!   and height. With `+0x3D4` set (`GameEntity::is_mission_only`, which
//!   Unlimbo sets on every non-Selectable missile), passengers (`+0x118`)
//!   unload, or guard in a team; a team member keeps the pick; any other
//!   aircraft drops its Target and its destination, in that order, and
//!   retreats. Without `+0x3D4` it drops the destination, then the Target,
//!   and keeps the pick.
//! - The tail (`0x00417AD4`): an armed aircraft without Ammo and out of radio
//!   contact hunts a dock; radio contact (`0x0065AE30`) makes it Enter; the
//!   mission is queued (vt+0x1E8) and commenced when ready (vt+0x200,
//!   vt+0x1EC).
//!
//! VERA's tree (`aircraft::idle_mode`) stands in for the two parts not
//! ported, through [`IdleEntry::Vera`]:
//!
//! RESIDUAL: the airborne arm (`0x00417802`): re-engaging, a dock in radio
//! contact, Find_Nearest_Friendly_Airfield (`0x0041A160`) and an AirportBound
//! aircraft's Crash. The tree also stands in for the head and the Foot base
//! before it, and its choice is VERA's aircraft state, never a queued
//! mission. Trigger: every aircraft that enters idle mode in flight (a
//! Harrier after its run, a Kirov at its destination). Effect: a suspended
//! mission is not restored, Retreat, Paradrop and Spy Plane missions do not
//! hold, a queued waypoint is not taken, and the current mission stays the
//! one that ended. Frequency: every combat aircraft's sortie. Downstream:
//! readers of an aircraft's current mission see the ended one.
//!
//! RESIDUAL: the tail's dock hunt (vt+0x528 with the type's `Dock=`, Enter,
//! or an AirportBound aircraft's Crash), which the tree replaces after the
//! landed arm's clears. Trigger: an armed aircraft entering idle mode on the
//! ground with Ammo 0 and out of radio contact. Effect: its choice is the
//! tree's, and no mission is queued. Frequency: rare; a rearming aircraft
//! waits on its pad in VERA's docking states.
//!
//! The picked missions' aircraft handlers are not ported except Unload and
//! Retreat: a `Fly` aircraft that commenced Guard, Area Guard or Enter holds
//! VERA's Guard state, the tree's own choice for it, and one that commenced
//! Unload or Retreat holds no VERA state, so their native handlers alone run.
//! A missile's Rocket keeps only the native handlers' states
//! (`aircraft::commence_handler_state`), so one that picks Guard, Area Guard
//! or Enter (in a team or in radio contact; no retail missile) runs no
//! handler.
//!
//! Evidence: tools/spatial_oracle/aircraft_idle_retreat.py runs the original
//! 0x004176F0 up to its airborne arm: every combination of what the landed
//! arm reads for a `MissileSpawn=` aircraft and for one on the ground or at
//! its landing altitude, armed and unarmed, the head's cases, the dock hunt's
//! gate, and the airborne arm's entry. `idle_entry_tests` replays every row
//! through [`enter_idle_mode`].
//!
//! Not kept: Mission_Attack's state 10 clears `+0x6D5` before the call and
//! sets it after (`0x00418C9B`, `0x00418D00`). Its one reader, Aircraft
//! vt+0x430 (`0x0041B9E0`), gates TechnoClass::Enter_Idle_Mode's
//! planning-path arrival (`0x00709A6B` -> `0x006385C0`), which needs a
//! waypoint-planning token (`0x00705D20`) VERA never has.

use super::{AircraftMission, IdleEntry};
use crate::rules::locomotor_type::LocomotorKind;
use crate::rules::overlay_types::OverlayTypeRegistry;
use crate::rules::ruleset::RuleSet;
use crate::sim::mission::{MissionId, MissionType};
use crate::sim::world::Simulation;
use crate::sim::world::display_layers::DisplayLayer;

#[cfg(test)]
#[path = "idle_entry_tests.rs"]
mod tests;

/// What the call reads besides its live queries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct IdleFacts {
    /// vt+0x1FC (`0x005B3A10`): a suspended selector (`+0xB0 != -1`).
    suspended: bool,
    /// The current mission `+0xAC`.
    current: MissionId,
    /// The queued mission `+0xB4`.
    queued: MissionId,
    /// `+0x294`, an AirstrikeClass.
    airstrike: bool,
    /// `+0x3D4`.
    mission_only: bool,
    /// `+0x118`, the first passenger.
    passengers: bool,
    /// `+0x5D4`, the team.
    team: bool,
    /// `HouseClass::IsControlledByHuman @ 0x0050B730` on the owner.
    human: bool,
    /// `TechnoClass::Is_Armed @ 0x00701120` (vt+0x2AC).
    armed: bool,
    /// Ammo `+0x2FC`.
    ammo: i32,
    /// The landed arm's test (`0x004177BA..0x004177FC`): the ground layer,
    /// the height at or below the landing altitude, or `MissileSpawn=`.
    landed: bool,
}

/// What the call does, each where the original does it.
trait IdleHost {
    /// vt+0x1F8 Restore (`0x004D8F80`); answers the current mission after it.
    fn restore(&mut self) -> MissionId;
    /// Mission+0xBC = 0 and `+0x6D2` = 0 (`0x00417719`).
    fn restart_patrol(&mut self);
    /// vt+0x1EC Commence (`0x0041B870`).
    fn commence(&mut self);
    /// `FootClass::Enter_Idle_Mode @ 0x004D82B0` with the caller's `(0, 1)`.
    fn foot_enter_idle(&mut self) -> bool;
    /// vt+0x3C8 `Assign_Target(NULL)` (`0x006FCDB0`).
    fn clear_target(&mut self);
    /// vt+0x480 `Assign_Destination(NULL, 1)` (`0x0041AA80`).
    fn clear_destination(&mut self);
    /// `RadioClass::In_Radio_Contact @ 0x0065AE30`.
    fn in_radio_contact(&mut self) -> bool;
    /// vt+0x1E8 `Queue_Mission(mission, 0)` (`0x0041BA90`).
    fn queue(&mut self, mission: MissionType);
    /// vt+0x200 `Ready_To_Commence` (`0x0041B5E0`).
    fn ready(&mut self) -> bool;
}

/// The call: its answer (the Foot base's, or 0 where the head returns), or
/// `None` where VERA's tree stands in (module RESIDUALs), before the head
/// for an aircraft in flight and after the landed arm's clears for the dock
/// hunt.
fn enter_idle_mode(facts: &IdleFacts, host: &mut impl IdleHost) -> Option<bool> {
    use MissionType::{
        AreaGuard, Enter, Guard, ParadropApproach, ParadropOverfly, Patrol, Retreat,
        SpyplaneApproach, SpyplaneOverfly, Unload,
    };
    if !facts.landed {
        return None;
    }
    if facts.suspended {
        if host.restore().known() == Some(Patrol) {
            host.restart_patrol();
        }
        return Some(false);
    }
    let current = facts.current.known();
    if matches!(
        current,
        Some(Retreat | ParadropApproach | ParadropOverfly | SpyplaneApproach | SpyplaneOverfly)
    ) && !facts.airstrike
    {
        return Some(false);
    }
    if matches!(
        facts.queued.known(),
        Some(ParadropApproach | SpyplaneApproach)
    ) {
        host.commence();
        return Some(false);
    }
    let answer = host.foot_enter_idle();
    // `0x00417787`, read again by the arm without `+0x3D4` (`0x00417AA3`).
    let pick = if !facts.human && !facts.team && facts.armed {
        AreaGuard
    } else {
        Guard
    };
    let mission = if !facts.mission_only {
        host.clear_destination();
        host.clear_target();
        pick
    } else if facts.passengers {
        if facts.team { Guard } else { Unload }
    } else if facts.team {
        pick
    } else {
        host.clear_target();
        host.clear_destination();
        Retreat
    };
    if facts.ammo == 0 && facts.armed && !host.in_radio_contact() {
        return None;
    }
    let mission = if host.in_radio_contact() {
        Enter
    } else {
        mission
    };
    host.queue(mission);
    if host.ready() {
        host.commence();
    }
    Some(answer)
}

/// [`enter_idle_mode`] for aircraft `id`: [`IdleEntry::Native`] when it ran,
/// [`IdleEntry::Vera`] where VERA's tree stands in.
pub(crate) fn enter_idle_mode_for(
    sim: &mut Simulation,
    id: u64,
    rules: &RuleSet,
    registry: Option<&OverlayTypeRegistry>,
) -> IdleEntry {
    let Some(entity) = sim.substrate.entities.get(id) else {
        return IdleEntry::NotCalled;
    };
    let Some(object) = rules.object(sim.interner.resolve(entity.type_ref())) else {
        return IdleEntry::NotCalled;
    };
    let landing = super::landing_base::landing_base(
        entity,
        &sim.substrate.entities,
        Some((rules, &sim.interner)),
    );
    let landed = sim.entity_display_layer(id, Some(rules)) == Some(DisplayLayer::GROUND)
        || crate::sim::movement::air_movement::current_fly_height(
            entity,
            sim.resolved_terrain.as_ref(),
        ) <= landing
        || object.missile_spawn;
    let facts = IdleFacts {
        suspended: entity.mission.suspended() != MissionId::NONE,
        current: entity.mission.current(),
        queued: entity.mission.queued(),
        airstrike: entity
            .mission_leaf
            .as_aircraft()
            .is_some_and(|leaf| leaf.airstrike_manager_present()),
        mission_only: entity.is_mission_only(),
        passengers: entity
            .passenger_role
            .cargo()
            .is_some_and(|cargo| cargo.count() != 0),
        team: sim.team_script_vm.team_for_member(id).is_some(),
        human: sim.owner_is_human(entity.owner()),
        armed: crate::sim::combat::combat_weapon::is_armed(entity, object),
        ammo: entity
            .aircraft_ammo
            .as_ref()
            .map_or(-1, |ammo| ammo.current),
        landed,
    };
    let mut host = WorldIdle {
        sim,
        id,
        rules,
        registry,
        queued: None,
    };
    if enter_idle_mode(&facts, &mut host).is_none() {
        return IdleEntry::Vera;
    }
    let queued = host.queued;
    hold_fly_state(sim, id, queued);
    IdleEntry::Native
}

/// The VERA state a `Fly` aircraft holds for the mission its idle mode
/// commenced (module doc); one whose mission was only queued keeps the state
/// of the visit that ended, as the original keeps running that handler.
fn hold_fly_state(sim: &mut Simulation, id: u64, queued: Option<MissionType>) {
    let Some(entity) = sim.substrate.entities.get_mut(id) else {
        return;
    };
    let fly = entity
        .locomotor
        .as_ref()
        .is_some_and(|locomotor| locomotor.kind == LocomotorKind::Fly);
    let Some(mission) = queued.filter(|&mission| entity.mission.current().known() == Some(mission))
    else {
        return;
    };
    if !fly {
        return;
    }
    entity.aircraft_mission = match mission {
        MissionType::Unload | MissionType::Retreat => None,
        _ => Some(AircraftMission::Guard),
    };
}

struct WorldIdle<'a> {
    sim: &'a mut Simulation,
    id: u64,
    rules: &'a RuleSet,
    registry: Option<&'a OverlayTypeRegistry>,
    /// The mission the call queued.
    queued: Option<MissionType>,
}

impl IdleHost for WorldIdle<'_> {
    fn restore(&mut self) -> MissionId {
        self.sim
            .mission_restore_represented(self.id, Some(self.rules), self.registry)
            .expect("Restore setters accept a live aircraft's archived Target and NavCom");
        self.sim
            .substrate
            .entities
            .get(self.id)
            .expect("idle-mode aircraft")
            .mission
            .current()
    }

    /// No aircraft Patrol handler is ported, so no VERA state holds its
    /// Mission+0xBC; the latch is the aircraft leaf's.
    fn restart_patrol(&mut self) {
        let entity = self.sim.substrate.entities.get_mut(self.id).unwrap();
        entity.mission_leaf.set_aircraft_action_latch(false);
    }

    fn commence(&mut self) {
        let now = self.sim.session.binary_frame;
        self.sim
            .mission_commence_exact(self.id, now)
            .expect("idle-mode aircraft");
    }

    fn foot_enter_idle(&mut self) -> bool {
        self.sim
            .foot_enter_idle_base(self.id, Some(self.rules), self.registry)
    }

    fn clear_target(&mut self) {
        self.sim
            .assign_target_represented(self.id, None, Some(self.rules))
            .expect("idle-mode aircraft");
    }

    fn clear_destination(&mut self) {
        self.sim
            .assign_aircraft_destination(self.id, None, self.rules);
    }

    fn in_radio_contact(&mut self) -> bool {
        !self
            .sim
            .substrate
            .entities
            .get(self.id)
            .unwrap()
            .radio_contacts
            .is_empty()
    }

    fn queue(&mut self, mission: MissionType) {
        self.queued = Some(mission);
        let entity = self.sim.substrate.entities.get_mut(self.id).unwrap();
        crate::sim::mission::authority::queue_entity_mission_deferred(
            entity,
            MissionId::from_known(mission),
        );
    }

    fn ready(&mut self) -> bool {
        self.sim.mission_ready_to_commence(self.id, self.rules)
    }
}
