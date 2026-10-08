//! Aircraft mission state machines — orchestrates attack runs, guard/RTB,
//! movement, and idle behavior for Fly-locomotor aircraft.
//!
//! This module implements the mission layer that sits between air movement
//! physics (air_movement.rs) and combat firing (combat/). Missions control
//! WHEN aircraft fire, HOW they approach targets, and WHAT they do after
//! completing an attack pass.
//!
//! ## Dependency rules
//! - Part of sim/ — depends on sim/components, sim/combat, sim/docking, rules/.
//! - sim/ NEVER depends on render/, ui/, audio/, net/.

mod airfield;
pub mod attack_mission;
pub mod drop_payload;
mod idle_entry;
pub(crate) use idle_entry::enter_idle_mode_for;
pub mod idle_mode;
pub(crate) mod landing_base;
mod leave_map;
#[cfg(test)]
mod leave_map_tests;
pub(crate) mod move_mission;
pub mod paradrop_mission;
mod retreat_mission;
pub mod runtime_contract;
pub(crate) mod spyplane_mission;

#[cfg(test)]
mod dock_cycle_tests;
#[cfg(test)]
mod release_tests;

use serde::{Deserialize, Serialize};

use crate::map::entities::EntityCategory;
use crate::rules::foundation::foundation_dimensions;
use crate::rules::locomotor_type::LocomotorKind;
use crate::rules::ruleset::RuleSet;
use crate::sim::mission::MissionTimer;
use crate::sim::movement::locomotor::AirMovePhase;
use crate::sim::world::Simulation;
use crate::util::fixed_math::{SIM_ZERO, SimFixed};

/// Aircraft mission — determines the high-level behavior each tick.
///
/// Replaces the original engine's MissionClass dispatch for aircraft.
/// Each variant carries its own sub-state for the state machine.
#[derive(Debug, Clone, Hash, Serialize, Deserialize)]
pub enum AircraftMission {
    /// Idle on the ground or hovering — waiting for orders.
    /// BalloonHover aircraft idle at cruise altitude.
    Idle,

    /// Flying toward a destination (player Move command).
    Move {
        /// 0=init, 1=set_course, 2=in_flight, 3=arrived, 4=course_correction
        sub_state: u8,
    },

    /// Attacking a target — 11-state machine from gamemd.exe.
    Attack {
        /// State within the attack state machine (0-10).
        sub_state: u8,
    },

    /// Guard — idle in the air, scanning for targets, RTB when low ammo.
    Guard,

    /// Returning to base — flying toward airfield for reload.
    /// Absorbed from the old AircraftDockPhase::ReturnToBase.
    ReturnToBase {
        /// Target airfield entity stable_id.
        airfield_id: u64,
    },

    /// Docking at an airfield — descending, reloading.
    Docking {
        /// Target airfield entity stable_id.
        airfield_id: u64,
        /// 0=wait_for_dock, 1=descending, 2=reloading
        sub_state: u8,
        /// Frame-anchored gate until the next ammo point is restored (during
        /// reloading); was a per-tick `u32` countdown.
        reload_timer: MissionTimer,
    },

    /// Parked on helipad pad — freshly built, waiting for player command.
    /// Dock slot is reserved. Aircraft is Landed, altitude 0.
    /// Exits via Move/Attack command (releases dock, triggers takeoff).
    DockedIdle {
        /// Airfield entity stable_id this aircraft is docked at.
        airfield_id: u64,
    },
}

impl AircraftMission {
    /// Whether this mission is an active attack (any Attack sub-state).
    pub fn is_attacking(&self) -> bool {
        matches!(self, AircraftMission::Attack { .. })
    }

    /// Whether this aircraft is parked on a helipad waiting for orders.
    pub fn is_docked_idle(&self) -> bool {
        matches!(self, AircraftMission::DockedIdle { .. })
    }
}

/// Batch driver for fixtures that dispatch every aircraft without the live
/// object pass. Production dispatches each aircraft in its own LogicVector
/// slot through [`dispatch_aircraft_mission`].
#[cfg(test)]
pub fn tick_aircraft_missions(
    sim: &mut Simulation,
    rules: &RuleSet,
) -> std::collections::BTreeSet<u64> {
    let order = sim.substrate.logic.as_slice().to_vec();
    order
        .into_iter()
        // This batch fixture has no resident overlay table.
        .filter(|&id| dispatch_aircraft_mission(sim, rules, id, None))
        .collect()
}

/// `MissionClass::AI @ 0x005B3060` for the aircraft missions ported as
/// native handlers: an alive aircraft whose mission timer is due runs its
/// current mission's handler (jump table `0x005B34E8`) and restarts the
/// timer with the frames it returns. These are Retreat (`0x00415A50`,
/// vtable `+0x230`), Unload (`0x004151E0`, `+0x23C`), the paradrop plane's
/// two (`0x004158E0`, `0x00415960`) and the Spy Plane's two (`+0x26C`,
/// `+0x270`). Mission_Move and Mission_Attack hold their state in
/// `AircraftMission` and run in [`dispatch_aircraft_mission`], beside VERA's
/// aircraft state machine for the other missions.
pub(crate) fn dispatch_native_mission(
    sim: &mut Simulation,
    id: u64,
    rules: &RuleSet,
    overlay_registry: Option<&crate::rules::overlay_types::OverlayTypeRegistry>,
) {
    use crate::sim::mission::MissionType;
    let now = sim.session.binary_frame;
    let Some(entity) = sim.substrate.entities.get(id) else {
        return;
    };
    if entity.dying
        || entity.category != EntityCategory::Aircraft
        || !entity.mission.dispatch_timer().due(now)
    {
        return;
    }
    let delay = match entity.mission.current().known() {
        Some(MissionType::Retreat) => retreat_mission::retreat(sim, id, rules),
        Some(MissionType::Unload) => {
            crate::sim::transport_unload::mission_unload(sim, id, rules, overlay_registry)
        }
        Some(MissionType::ParadropApproach) => paradrop_mission::approach(sim, id, rules),
        Some(MissionType::ParadropOverfly) => {
            paradrop_mission::overfly(sim, id, rules, overlay_registry)
        }
        Some(MissionType::SpyplaneApproach) => spyplane_mission::approach(sim, id, rules),
        Some(MissionType::SpyplaneOverfly) => spyplane_mission::overfly(sim, id, rules),
        _ => return,
    };
    if let Some(entity) = sim.substrate.entities.get_mut(id) {
        entity.mission.write_dispatch_epilogue(now as i32, delay);
    }
}

/// Whether `entity` flies a paradrop or Spy Plane mission, whose native
/// handlers steer it ([`paradrop_mission`], [`spyplane_mission`]): VERA's
/// pre-combat pursuit stage, a ground stand-in that would halt the plane
/// within its weapon's `Range=` of its Target cell, leaves it alone.
pub(crate) fn native_missions_steer(entity: &crate::sim::game_entity::GameEntity) -> bool {
    use crate::sim::mission::MissionType;
    matches!(
        entity.mission.current().known(),
        Some(
            MissionType::ParadropApproach
                | MissionType::ParadropOverfly
                | MissionType::SpyplaneApproach
                | MissionType::SpyplaneOverfly
        )
    )
}

/// `ObjectClass::Distance_To @ 0x005F6440` from the aircraft; a NULL target
/// answers 0 (`0x005F644B..0x005F6456`).
fn target_distance(
    sim: &Simulation,
    id: u64,
    target: Option<crate::sim::combat::TargetKind>,
) -> i32 {
    let entities = &sim.substrate.entities;
    target
        .zip(entities.get(id))
        .and_then(|(target, plane)| {
            crate::sim::combat::object_distance_to(plane, &target, entities)
        })
        .unwrap_or(0)
}

/// Whether the aircraft has no NavCom (`+0x5A4`).
fn nav_com_absent(sim: &Simulation, id: u64) -> bool {
    sim.substrate
        .entities
        .get(id)
        .is_some_and(|plane| plane.navigation.nav_com.is_none())
}

/// `Queue_Mission(mission, 0)` (vt+0x1E8 = `0x0041BA90`).
fn queue_mission(sim: &mut Simulation, id: u64, mission: crate::sim::mission::MissionType) {
    if let Some(plane) = sim.substrate.entities.get_mut(id) {
        crate::sim::mission::authority::queue_entity_mission_deferred(
            plane,
            crate::sim::mission::MissionId::from_known(mission),
        );
    }
}

/// One aircraft's mission dispatch inside its own LogicVector slot.
///
/// `FootClass::AI @ 0x004DA530` runs TechnoClass AI, and with it the mission
/// dispatch, before locomotor Process (`+0x40`). Mission_Attack's Scenario RNG
/// draws (state1 Rate jitter, FindFireLocation) and NavCom reservations
/// therefore interleave with the other objects' AI in Logic order, ahead of
/// this aircraft's own Fly Process. Returns whether the combat phase must
/// run a Mission_Attack strike visit (states 4..9) for this aircraft this
/// frame. RESIDUAL: that visit runs in VERA's combat phase after the live
/// pass, like every other attacker's FireAt, so its draws do not interleave.
pub(crate) fn dispatch_aircraft_mission(
    sim: &mut Simulation,
    rules: &RuleSet,
    id: u64,
    registry: Option<&crate::rules::overlay_types::OverlayTypeRegistry>,
) -> bool {
    let Some(e) = sim.substrate.entities.get(id) else {
        return false;
    };
    // A Dying aircraft corpse must not run its mission (move, fire,
    // paradrop, reveal fog) for the tick before the end-of-tick drain.
    if e.dying {
        return false;
    }
    let Some(mission) = e.aircraft_mission.clone() else {
        return false;
    };
    let native_move = native_move(e, &mission);
    if (mission.is_attacking() || native_move)
        && !e.mission.dispatch_timer().due(sim.session.binary_frame)
    {
        return false;
    }
    // A missile's Rocket runs only the native handlers; VERA's states are
    // Fly's.
    let kind = e.locomotor.as_ref().map(|l| l.kind);
    if !(kind == Some(LocomotorKind::Fly)
        || native_handler_current(e) && kind == Some(LocomotorKind::Rocket))
    {
        return false;
    }
    match mission_step(sim, rules, id, &mission, registry) {
        Some(m) => apply_mission_mutation(sim, rules, m, registry),
        None => false,
    }
}

/// `AircraftMission::Move` holds Mission_Move's state only while the native
/// mission (`Mission+0xAC`) is Move. VERA's Attack Move stand-in also flies
/// in `AircraftMission::Move`; the original dispatches mission 29 to
/// Mission_Sleep (`0x005B34C4`).
fn native_move(entity: &crate::sim::game_entity::GameEntity, mission: &AircraftMission) -> bool {
    matches!(mission, AircraftMission::Move { .. }) && native_handler_current(entity)
}

/// The aircraft's current mission (`Mission+0xAC`) runs one of the handlers
/// ported natively, whose Mission+0xBC `AircraftMission` holds:
/// Mission_Attack (`0x00417FE0`) or Mission_Move (`0x004166E0`).
fn native_handler_current(entity: &crate::sim::game_entity::GameEntity) -> bool {
    use crate::sim::mission::{MissionId, MissionType};
    let current = entity.mission.current();
    match entity.aircraft_mission {
        Some(AircraftMission::Attack { .. }) => {
            current == MissionId::from_known(MissionType::Attack)
        }
        Some(AircraftMission::Move { .. }) => current == MissionId::from_known(MissionType::Move),
        _ => false,
    }
}

/// Commence's Mission+0xBC reset (`MissionClass::Commence` zeroes it with
/// the promote) for the handlers ported natively: a commenced Move or Attack
/// starts Mission_Move or Mission_Attack at state 0. Called by every Commence
/// (`mission::authority::commence_entity_mission`). A missile's Rocket runs
/// only native handlers, so it holds the commenced mission's state whatever
/// it held before, and none for another mission (Retreat from its idle
/// mode): a stale Attack state would answer for the current mission
/// (`Simulation::foot_null_destination` reads it). An aircraft on one of
/// VERA's own states (its idle tree's choices, return to base, docking)
/// keeps it: those hold no Mission+0xBC, and the paths that queue a Move over
/// them write its state themselves ([`queue_move_state`]).
pub(crate) fn commence_handler_state(entity: &mut crate::sim::game_entity::GameEntity) {
    use crate::sim::mission::MissionType;
    let rocket = entity
        .locomotor
        .as_ref()
        .is_some_and(|locomotor| locomotor.kind == LocomotorKind::Rocket);
    if !rocket
        && !matches!(
            entity.aircraft_mission,
            Some(AircraftMission::Attack { .. } | AircraftMission::Move { .. })
        )
    {
        return;
    }
    entity.aircraft_mission = match entity.mission.current().known() {
        Some(MissionType::Move) => Some(AircraftMission::Move { sub_state: 0 }),
        Some(MissionType::Attack) => Some(AircraftMission::Attack { sub_state: 0 }),
        _ if rocket => None,
        _ => return,
    };
}

/// The `Enter_Idle_Mode(0, 1)` (`vt+0x484`) a Mission_Move or Mission_Attack
/// visit made.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum IdleEntry {
    NotCalled,
    /// The original, already applied (`idle_entry`): `AircraftMission` is as
    /// its Commence and the idle mode left it, so the dispatch writes no
    /// visit state.
    Native,
    /// VERA's tree ([`idle_stand_in`]), which the dispatch applies after the
    /// visit's state.
    Vera,
}

/// vt+0x484 from outside the aircraft dispatch (Mission_Unload's empty
/// hold): VERA's tree, where it stands in, applies at once.
pub(crate) fn enter_idle_mode_now(
    sim: &mut Simulation,
    rules: &RuleSet,
    id: u64,
    registry: Option<&crate::rules::overlay_types::OverlayTypeRegistry>,
) {
    if enter_idle_mode_for(sim, id, rules, registry) != IdleEntry::Vera {
        return;
    }
    let Some(mission) = sim
        .substrate
        .entities
        .get(id)
        .and_then(|entity| entity.aircraft_mission.clone())
    else {
        return;
    };
    let mut m = MissionMutation::new(id, mission);
    if idle_stand_in(sim, rules, id, &mut m).is_some() {
        apply_mission_mutation(sim, rules, m, registry);
    }
}

/// The Move an order or a spawn manager queues for an aircraft (`vt+0x1E8
/// Queue_Mission(Move, 0)`). Mission_Move starts at the Commence
/// ([`commence_handler_state`]); until then the current handler runs on, as
/// natively: Mission_Attack, which can hold the queue while its release
/// latch (`+0x6D2`) refuses `ReadyToCommence`, or Mission_Move itself, which
/// a queued Move leaves alone (`0x005B35E0`).
///
/// RESIDUAL: an aircraft on one of VERA's own states takes Mission_Move's
/// state 0 with the order, because those states do not stand for the native
/// handler that would run until the Commence; VERA's idle tree, return to
/// base and docking would act on it in between (an idle pass sends an
/// AirportBound aircraft home). Trigger: a Move for an aircraft that is
/// idle, guarding, docked or returning. Effect: the native current
/// mission's last visits before the Commence do not run.
pub(crate) fn queue_move_state(entity: &mut crate::sim::game_entity::GameEntity) {
    if !native_handler_current(entity) {
        entity.aircraft_mission = Some(AircraftMission::Move { sub_state: 0 });
    }
}

/// One mission handler's decision, applied by [`apply_mission_mutation`].
struct MissionMutation {
    id: u64,
    new_mission: AircraftMission,
    ammo_delta: i32,
    fire_at: Option<crate::sim::combat::TargetKind>,
    move_to: Option<(u16, u16)>,
    /// `Assign_Destination(target, 1)` through the aircraft's destination
    /// owner (NavCom and the Fly MoveTo), ahead of any `move_to`.
    assign_destination: Option<crate::sim::components::NavTargetRef>,
    self_destruct: bool,
    /// Fly BeginLanding4CFA70 through the world owner, after the mission write.
    begin_landing: bool,
}

impl MissionMutation {
    /// No decision yet: `mission` stays.
    fn new(id: u64, mission: AircraftMission) -> Self {
        Self {
            id,
            new_mission: mission,
            ammo_delta: 0,
            fire_at: None,
            move_to: None,
            assign_destination: None,
            self_destruct: false,
            begin_landing: false,
        }
    }
}

fn mission_step(
    sim: &mut Simulation,
    rules: &RuleSet,
    id: u64,
    mission: &AircraftMission,
    registry: Option<&crate::rules::overlay_types::OverlayTypeRegistry>,
) -> Option<MissionMutation> {
    let now = sim.session.binary_frame;
    let mut m = MissionMutation::new(id, mission.clone());

    match mission {
        AircraftMission::Idle => match enter_idle_mode_for(sim, id, rules, registry) {
            IdleEntry::Native => return None,
            _ => idle_stand_in(sim, rules, id, &mut m)?,
        },

        AircraftMission::Attack { sub_state } => {
            if let Some(entity) = sim.substrate.entities.get_mut(id) {
                attack_mission::enter_attack_state(entity, *sub_state);
            }
            match *sub_state {
                0 => m.new_mission = sim.aircraft_begin_attack(id),
                1 => m.new_mission = sim.aircraft_reengage(id, rules),
                3 => m.new_mission = sim.aircraft_approach(id, rules),
                // The release, the follow-up shot and the strafe run fire, so
                // they run in the combat phase, where VERA's FireAt lives.
                4..=9 => match sim.aircraft_strike_target(id, *sub_state) {
                    Some(target) => m.fire_at = Some(target),
                    None => m.new_mission = sim.aircraft_attack_visit(id, 10, 1),
                },
                10 => {
                    let (mission, idle) = sim.aircraft_exit(id, rules, registry);
                    end_visit(sim, rules, id, mission, idle, &mut m)?;
                }
                // State 2 (`0x00418D1D`) is the epilogue alone. Mission_Attack
                // runs as the Attack mission's handler, so its epilogue reads
                // Attack's Rate.
                state => {
                    let delay =
                        sim.mission_rate_epilogue(rules, crate::sim::mission::MissionType::Attack);
                    m.new_mission = sim.aircraft_attack_visit(id, state, delay);
                }
            }

            // Fly owns height targets. Native4CF3D4..4CF4CF selects
            // destination-relative height, IsDropship approach height or
            // Type FlightLevel. Repeated attack mission visits must not
            // divide the mutable target by3. The horizontal target-selection
            // transaction remains part of the Fly migration.
            // Fly Process owns the target speed (`air_movement::
            // write_fly_target_speed`); Mission_Attack writes no speed.
        }

        AircraftMission::Guard => {
            let entity = sim.substrate.entities.get(id)?;
            let ammo = entity.aircraft_ammo.as_ref();
            let ammo_current = ammo.map_or(-1, |a| a.current);
            let ammo_max = ammo.map_or(-1, |a| a.max);
            let has_target = entity.attack_target.is_some();

            // gamemd Mission_Guard RTB decision (default ReturnFire mode):
            // an in-flight aircraft returns to rearm whenever it has spent
            // ammo (ammo < maxAmmo) and is not actively engaging a target.
            // Without the second clause a strike craft whose target dies
            // mid-sortie with ammo left would hover here indefinitely.
            // (Mode1 `ammo == 0` / Mode2 `ammo < max/2` are INI-gated
            // globals not yet mapped to keys — default mode is stock.)
            let out_of_ammo = ammo_current <= 0 && ammo_max > 0;
            let spent_and_idle = !has_target && ammo_max > 0 && ammo_current < ammo_max;

            // A spawn-manager child's base is its parent, not an airfield.
            // Stock HORNET/ASW ship with `Dock=` commented out, so their
            // rearm is driven entirely by the parent's SpawnManager
            // (state 3 → 4 → 6). Letting them pick an unrelated helipad
            // here would fight that recall.
            let spawn_child = entity.spawn_owner_id.is_some();

            if has_target && ammo_current > 0 {
                m.new_mission = AircraftMission::Attack { sub_state: 0 };
            } else if spawn_child {
                // Hold station; the parent's manager issues the recall.
            } else if out_of_ammo || spent_and_idle {
                let nearest = crate::sim::docking::aircraft_dock::find_nearest_airfield(
                    sim,
                    rules,
                    entity.owner(),
                    entity.type_ref(),
                    (entity.position.rx, entity.position.ry),
                );
                if let Some((af_id, af_rx, af_ry)) = nearest {
                    m.new_mission = AircraftMission::ReturnToBase { airfield_id: af_id };
                    m.move_to = Some((af_rx, af_ry));
                } else {
                    let type_str = sim.interner.resolve(entity.type_ref());
                    let airport_bound = rules.object(type_str).map_or(false, |o| o.airport_bound);
                    if airport_bound {
                        m.self_destruct = true;
                    }
                }
            }
        }

        AircraftMission::ReturnToBase { airfield_id } => {
            let entity = sim.substrate.entities.get(id)?;
            let af_ok = sim
                .substrate
                .entities
                .get(*airfield_id)
                .is_some_and(|af| af.health.current > 0 && !af.dying);
            if !af_ok {
                m.new_mission = AircraftMission::Idle;
                return Some(m);
            }
            let af = sim.substrate.entities.get(*airfield_id).unwrap();
            let type_str = sim.interner.resolve(af.type_ref());
            let (fw, fh) = rules
                .object(type_str)
                .map(|o| foundation_dimensions(&o.foundation))
                .unwrap_or((1, 1));
            let dock_rx = af.position.rx + fw / 2;
            let dock_ry = af.position.ry + fh / 2;

            let dx = (entity.position.rx as i32 - dock_rx as i32).abs();
            let dy = (entity.position.ry as i32 - dock_ry as i32).abs();
            let dist = dx.max(dy);

            if dist <= 2 {
                // sub_state 0 = WaitForDock.
                m.new_mission = AircraftMission::Docking {
                    airfield_id: *airfield_id,
                    sub_state: 0,
                    reload_timer: MissionTimer::default(),
                };
            } else if !crate::sim::movement::air_movement::fly_moving(entity) {
                m.move_to = Some((dock_rx, dock_ry));
            }
        }

        AircraftMission::Docking {
            airfield_id,
            sub_state,
            reload_timer,
        } => {
            let entity = sim.substrate.entities.get(id)?;
            let air_phase = crate::sim::movement::air_movement::fly_mission_phase(
                entity,
                sim.resolved_terrain.as_ref(),
            );
            let landing = entity
                .locomotor
                .as_ref()
                .and_then(|l| l.fly_runtime())
                .is_some_and(|s| s.landing());
            let arrived = crate::sim::movement::air_movement::fly_landing_arrival(entity);
            let af_type_ref = sim
                .substrate
                .entities
                .get(*airfield_id)
                .map_or(entity.type_ref(), |af| af.type_ref());
            let ammo = entity.aircraft_ammo.as_ref();
            let ammo_current = ammo.map_or(0, |a| a.current);
            let ammo_max = ammo.map_or(0, |a| a.max);
            let reload_rate = rules.general.reload_rate_ticks;

            match sub_state {
                0 => {
                    // Wait for dock slot.
                    let max_slots = rules
                        .object(sim.interner.resolve(af_type_ref))
                        .map(|o| o.dock_contact_capacity())
                        .unwrap_or(1);
                    // Native AircraftClass::IsCellOccupied reaches the
                    // unconditional Winged Cell leaf first; dock ownership
                    // and first-free pad reservation remain this wrapper's
                    // meaningful admission gates.
                    if runtime_contract::aircraft_landing_cell_leaf_clear()
                        && let Some(reserved_pad) =
                            sim.reserve_airfield_pad(*airfield_id, id, max_slots)
                    {
                        m.new_mission = AircraftMission::Docking {
                            airfield_id: *airfield_id,
                            sub_state: 1,
                            reload_timer: MissionTimer::default(),
                        };
                        // Re-target descent toward the per-pad cell so
                        // multi-pad airfields visibly spread occupants.
                        if let Some((px, py)) =
                            sim.substrate.entities.get(*airfield_id).and_then(|af| {
                                let obj = sim.object_type(af.type_ref(), rules)?;
                                let foundation = crate::rules::foundation::foundation_dimensions(
                                    &obj.foundation,
                                );
                                obj.pads.get(reserved_pad as usize).map(|pad| {
                                    crate::sim::docking::pad_geometry::pad_cell_for(
                                        (af.position.rx, af.position.ry),
                                        foundation,
                                        pad,
                                    )
                                })
                            })
                        {
                            m.move_to = Some((px, py));
                        }
                    }
                }
                1 => {
                    // Descend once the pad approach arrives (Fly
                    // Horizontal_Step4CF520's landing arm distance/speed).
                    if air_phase == Some(AirMovePhase::Landed) {
                        m.new_mission = AircraftMission::Docking {
                            airfield_id: *airfield_id,
                            sub_state: 2,
                            reload_timer: MissionTimer::armed(now, reload_rate),
                        };
                    } else if !landing && arrived {
                        m.begin_landing = true;
                    }
                }
                2 => {
                    // Reloading.
                    if reload_timer.due(now) {
                        m.ammo_delta = 1;
                        if ammo_current + 1 >= ammo_max {
                            // Fully reloaded, it stays on its pad: native
                            // Mission_Guard (`0x0041A5C0`) keeps a landed
                            // aircraft in radio contact without a Target
                            // waiting; an order releases it
                            // (`release_docked_idle`).
                            m.new_mission = AircraftMission::DockedIdle {
                                airfield_id: *airfield_id,
                            };
                        } else {
                            m.new_mission = AircraftMission::Docking {
                                airfield_id: *airfield_id,
                                sub_state: 2,
                                reload_timer: MissionTimer::armed(now, reload_rate),
                            };
                        }
                    }
                    // Otherwise the same frame-anchored timer carries over.
                }
                _ => {
                    m.new_mission = AircraftMission::Idle;
                }
            }
        }

        AircraftMission::Move { sub_state } => {
            let entity = sim.substrate.entities.get(id)?;
            if native_move(entity, mission) {
                let (mission, idle) = sim.aircraft_move(id, *sub_state, rules, registry);
                end_visit(sim, rules, id, mission, idle, &mut m)?;
            } else if !crate::sim::movement::air_movement::fly_moving(entity) {
                // VERA's Attack Move stand-in ends once its Fly no longer
                // moves: a landing at its destination.
                m.new_mission = AircraftMission::Idle;
            }
        }

        AircraftMission::DockedIdle { airfield_id } => {
            // Check if airfield still alive.
            let af_ok = sim
                .substrate
                .entities
                .get(*airfield_id)
                .is_some_and(|af| af.health.current > 0 && !af.dying);
            if !af_ok {
                // Airfield destroyed — release dock and go to Idle.
                // Idle mode will handle AirportBound self-destruct.
                sim.release_airfield_pad(id);
                m.new_mission = AircraftMission::Idle;
            }
            // Otherwise: stay parked, do nothing.
        }
    }
    Some(m)
}

/// A Mission_Move or Mission_Attack visit's end: `mission` holds the state
/// it left, unless the original Enter_Idle_Mode already ran (`None`: the
/// dispatch then writes nothing).
fn end_visit(
    sim: &Simulation,
    rules: &RuleSet,
    id: u64,
    mission: AircraftMission,
    idle: IdleEntry,
    m: &mut MissionMutation,
) -> Option<()> {
    match idle {
        IdleEntry::Native => None,
        IdleEntry::NotCalled => {
            m.new_mission = mission;
            Some(())
        }
        IdleEntry::Vera => {
            m.new_mission = mission;
            idle_stand_in(sim, rules, id, m)
        }
    }
}

/// VERA's tree (`idle_mode`) where it stands in for `AircraftClass::
/// Enter_Idle_Mode @ 0x004176F0` (`idle_entry`'s RESIDUALs): an aircraft in
/// flight, and the dock hunt of an armed one on the ground without Ammo.
///
/// A return to an airfield also sends the aircraft there in the same call,
/// replacing whatever destination it held (state 10's edge cell): the
/// airborne arm clears it (`Assign_Destination(NULL, 1)` at `0x004179B4`)
/// and assigns the dock that answers (`0x004179D7`).
///
/// The tree only changes `AircraftMission`: it queues and commences nothing,
/// so the current mission stays (Move after Mission_Move). After
/// Mission_Move's state 0 the Rate epilogue reads Move's Rate where the
/// original reads the commenced mission's (the draw is the same), and a later
/// Move order restarts Mission_Move when the timer of its last visit expires
/// ([`queue_move_state`]), not the frame after a Commence.
fn idle_stand_in(
    sim: &Simulation,
    rules: &RuleSet,
    id: u64,
    m: &mut MissionMutation,
) -> Option<()> {
    let entity = sim.substrate.entities.get(id)?;
    let type_str = sim.interner.resolve(entity.type_ref());
    let obj = rules.object(type_str);
    // Weapon-array slot 0 (`TechnoTypeClass+0x898`) is the armed
    // test here rather than `combat_weapon::is_armed`
    // (`TechnoClass::Is_Armed @ 0x00701120`). The two agree on
    // every stock aircraft: no aircraft section authors
    // `TurretCount=`, so the single slot `GetCurrentWeapon` would
    // read is slot 0. UNCHECKED which predicate the native
    // idle/return-to-airfield path uses; zero stock frequency
    // either way.
    let has_weapon = obj.is_some_and(|o| o.primary().is_some());
    let airport_bound = obj.is_some_and(|o| o.airport_bound);
    let is_airborne = entity
        .locomotor
        .as_ref()
        .is_some_and(|l| l.altitude > SIM_ZERO);
    let ammo = entity.aircraft_ammo.as_ref();

    let nearest = crate::sim::docking::aircraft_dock::find_nearest_airfield(
        sim,
        rules,
        entity.owner(),
        entity.type_ref(),
        (entity.position.rx, entity.position.ry),
    );

    let input = idle_mode::IdleModeInput {
        ammo_current: ammo.map_or(-1, |a| a.current),
        ammo_max: ammo.map_or(-1, |a| a.max),
        has_weapon,
        has_target: attack_mission::aircraft_target_present(
            entity.attack_target.as_ref(),
            &sim.substrate.entities,
        ),
        airport_bound,
        is_airborne,
        nearest_airfield: nearest,
    };

    match idle_mode::enter_idle_mode(&input) {
        idle_mode::IdleModeResult::Mission(new_m) => {
            if let AircraftMission::ReturnToBase { airfield_id } = new_m {
                m.assign_destination =
                    Some(crate::sim::components::NavTargetRef::Building { id: airfield_id });
            }
            m.new_mission = new_m;
        }
        idle_mode::IdleModeResult::SelfDestruct => {
            m.self_destruct = true;
        }
    }
    Some(())
}

/// Apply one handler decision. Returns the Mission_Attack fire request.
fn apply_mission_mutation(
    sim: &mut Simulation,
    rules: &RuleSet,
    m: MissionMutation,
    registry: Option<&crate::rules::overlay_types::OverlayTypeRegistry>,
) -> bool {
    // No "Unit lost" here: `AircraftClass::Enter_Idle_Mode @ 0x004176F0`
    // handles the AirportBound-without-airfield case by calling the
    // `Crash` slot `+0x3DC` directly with no attacker (`0x004179FD`,
    // `0x00417B88`; body `0x004DEBB0`, which runs `RecordKill +0xE0` and the
    // trigger events but never `Death_Announcement +0x3B8`), then returns 0
    // whatever Crash answered (`0x00417A06`, `0x00417B91`). An airborne
    // aircraft Crash accepted then falls to its impact
    // like a shot-down one (`sim::world::crash`). Only a damage kill
    // (`AircraftClass::ReceiveDamage 0x004165C0`, result 4 → `+0x3B8` at
    // `0x00416613`) announces, and that runs through the combat kill loop.
    //
    // Only AircraftClass has that idle mode (vtable `+0x484` = `0x004176F0`);
    // a custom Fly Infantry or Unit retires through VERA's terminal path.
    if m.self_destruct {
        let aircraft = sim
            .substrate
            .entities
            .get(m.id)
            .is_some_and(|entity| entity.category == EntityCategory::Aircraft);
        if aircraft {
            sim.foot_crash(m.id, None, rules, registry);
        } else {
            let infantry_terminal = sim.begin_raw_infantry_death(m.id);
            if !infantry_terminal && let Some(entity) = sim.substrate.entities.get_mut(m.id) {
                entity.health.current = 0;
                entity.dying = true;
            }
        }
        if let Some(entity) = sim.substrate.entities.get_mut(m.id) {
            entity.aircraft_mission = None;
        }
        return false;
    }

    if let Some(entity) = sim.substrate.entities.get_mut(m.id) {
        entity.aircraft_mission = Some(m.new_mission.clone());

        if m.ammo_delta != 0 {
            if let Some(ref mut ammo) = entity.aircraft_ammo {
                ammo.current = (ammo.current + m.ammo_delta).max(0).min(ammo.max);
            }
        }
    }
    // The world owners follow the mission write: a refused AirportBound
    // BeginLanding enters idle mode, which must not be overwritten.
    if m.begin_landing {
        sim.begin_fly_landing(m.id, Some(rules));
    }
    if let Some(destination) = m.assign_destination {
        sim.assign_aircraft_destination(m.id, Some(destination), rules);
    }
    if let Some((rx, ry)) = m.move_to {
        // No FASTER stage here: `FlyLocomotionClass` never calls the
        // `FootClass::GetCurrentSpeed` vtable slot (`+0x538`) — see
        // `veterancy::locomotor_consults_current_speed` — so a promoted
        // aircraft flies at its plain `Speed=`.
        let speed = sim
            .substrate
            .entities
            .get(m.id)
            .map(|e| {
                crate::sim::movement::order_speed(
                    e,
                    sim.object_type(e.type_ref(), rules),
                    Some(rules),
                    &sim.houses,
                )
            })
            .unwrap_or(SimFixed::from_num(8));
        sim.issue_air_cell_destination(m.id, (rx, ry), speed, Some(rules));
    }

    // Call-local dispatch receipt. Preserve the live Target and its existing
    // timing; combat will admit once, emit the burst and commit the suffix.
    m.fire_at.is_some()
}
