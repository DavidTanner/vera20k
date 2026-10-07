//! `AircraftClass::Enter_Idle_Mode @ 0x004176F0` (vt+0x484) for an unarmed
//! `MissileSpawn=` aircraft: every retail missile (V3ROCKET, DMISL, CMISL)
//! has no weapon. Mission_Move (state 0 without a NavCom, state 3 once
//! stopped) and Mission_Attack's state 10 call it with `(0, 1)`.
//!
//! - The head (`0x004176F8`): a suspended selector (vt+0x1FC, `0x005B3A10`)
//!   is restored (vt+0x1F8) and nothing else happens; a restored Patrol
//!   restarts at Mission+0xBC 0 with `+0x6D2` clear. Then Retreat, the two
//!   Paradrop and the two Spy Plane missions keep running unless an
//!   Airstrike (`+0x294`) owns the aircraft (jump table `0x00417BAC`), and a
//!   queued ParadropApproach or SpyplaneApproach commences at once
//!   (`0x00417764`). All three return 0.
//! - `FootClass::Enter_Idle_Mode @ 0x004D82B0`, whose answer is the call's.
//! - The landed arm (`0x00417A38`), which `MissileSpawn=` (Type `+0xD68`)
//!   selects whatever the layer and height: with `+0x3D4` set
//!   (`GameEntity::is_mission_only`, which Unlimbo sets on every
//!   non-Selectable missile), passengers (`+0x118`) unload, or guard in a
//!   team; a team member guards; any other aircraft drops its Target and
//!   its destination, in that order, and retreats. Without `+0x3D4` it drops
//!   the destination, then the Target, and guards.
//! - The tail (`0x00417AD4`): radio contact (`0x0065AE30`) makes it Enter;
//!   the mission is queued (vt+0x1E8) and commenced when ready (vt+0x200,
//!   vt+0x1EC).
//!
//! A computer house's unteamed armed aircraft would pick Area Guard over
//! Guard (`0x00417787`), and an armed one with no ammo would hunt a dock in
//! the tail; an unarmed aircraft reaches neither, which is why the port is
//! limited to unarmed ones. The airborne arm (`0x00417802`) is never taken
//! by a `MissileSpawn=` aircraft.
//!
//! Evidence: tools/spatial_oracle/aircraft_idle_retreat.py runs the original
//! 0x004176F0 over every combination of what this arm reads, the head's
//! cases, and the Ammo, layer and height values it ignores;
//! `idle_entry_tests` replays every row through [`enter_idle_mode`].
//!
//! Not kept: Mission_Attack's state 10 clears `+0x6D5` before the call and
//! sets it after (`0x00418C9B`, `0x00418D00`). Its one reader, Aircraft
//! vt+0x430 (`0x0041B9E0`), gates TechnoClass::Enter_Idle_Mode's
//! planning-path arrival (`0x00709A6B` -> `0x006385C0`), which needs a
//! waypoint-planning token (`0x00705D20`) VERA never has.

use crate::map::overlay_types::OverlayTypeRegistry;
use crate::rules::ruleset::RuleSet;
use crate::sim::mission::{MissionId, MissionType};
use crate::sim::world::Simulation;

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

/// The call on an unarmed `MissileSpawn=` aircraft. Returns its answer, the
/// Foot base's, or 0 where the head returns.
fn enter_idle_mode(facts: &IdleFacts, host: &mut impl IdleHost) -> bool {
    use MissionType::*;
    if facts.suspended {
        if host.restore().known() == Some(Patrol) {
            host.restart_patrol();
        }
        return false;
    }
    let current = facts.current.known();
    if matches!(
        current,
        Some(Retreat | ParadropApproach | ParadropOverfly | SpyplaneApproach | SpyplaneOverfly)
    ) && !facts.airstrike
    {
        return false;
    }
    if matches!(
        facts.queued.known(),
        Some(ParadropApproach | SpyplaneApproach)
    ) {
        host.commence();
        return false;
    }
    let answer = host.foot_enter_idle();
    // Is_Armed (vt+0x2AC) is false, so the pick at 0x00417787 is Guard.
    let mission = if !facts.mission_only {
        host.clear_destination();
        host.clear_target();
        Guard
    } else if facts.passengers {
        if facts.team { Guard } else { Unload }
    } else if facts.team {
        Guard
    } else {
        host.clear_target();
        host.clear_destination();
        Retreat
    };
    let mission = if host.in_radio_contact() {
        Enter
    } else {
        mission
    };
    host.queue(mission);
    if host.ready() {
        host.commence();
    }
    answer
}

/// [`enter_idle_mode`] for aircraft `id`, or `None` when it is not an
/// unarmed `MissileSpawn=` aircraft, whose idle mode is VERA's tree
/// (`aircraft::enter_idle_mode`).
pub(super) fn enter_idle_mode_native(
    sim: &mut Simulation,
    id: u64,
    rules: &RuleSet,
    registry: Option<&OverlayTypeRegistry>,
) -> Option<bool> {
    let entity = sim.substrate.entities.get(id)?;
    let object = rules.object(sim.interner.resolve(entity.type_ref()))?;
    if !object.missile_spawn || crate::sim::combat::combat_weapon::is_armed(entity, object) {
        return None;
    }
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
    };
    let mut host = WorldIdle {
        sim,
        id,
        rules,
        registry,
    };
    Some(enter_idle_mode(&facts, &mut host))
}

struct WorldIdle<'a> {
    sim: &'a mut Simulation,
    id: u64,
    rules: &'a RuleSet,
    registry: Option<&'a OverlayTypeRegistry>,
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
