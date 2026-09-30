//! One live object turn: AI, locomotor tails and synchronous cell/lifecycle effects.
//!
//! The master frame owns phase order. This owner completes one object's effects
//! before the live Logic cursor advances; no lifecycle work is deferred to a
//! batch tail. Native order and coordinate evidence remain beside each seam.

use std::collections::BTreeSet;

use super::{Simulation, techno_ai};
use crate::map::entities::EntityCategory;
use crate::rules::ruleset::RuleSet;
use crate::sim::lifecycle_request::LifecycleRequest;
use crate::sim::movement::{self, parachute_descent, rocket_movement, teleport_movement};
use crate::sim::pathfinding::PathGrid;

/// Whether this Unit visit reaches FootClass's SHP body-counter cadence.
///
/// An entry-active TubeMovement owns the UnitClass AI call and returns before
/// FootClass AI. Tube state armed later during an ordinary Foot visit does not
/// retroactively suppress work already reached by that visit, so only the
/// entry snapshot belongs in this admission predicate.
pub(super) fn shp_vehicle_counter_admitted(tube_active_at_entry: bool) -> bool {
    !tube_active_at_entry
}

/// The warp's two VocClass::PlayAt calls (`0x0071962C` at the old location,
/// `0x00719710` at the new): the type's ChronoOutSound/ChronoInSound
/// (TechnoType+0x578/+0x574), else `[AudioVisual]` (Rules+0x21C/+0x218), else
/// silence.
fn teleport_warp_sounds(
    sim: &mut Simulation,
    stable_id: u64,
    departure: Option<(u16, u16)>,
    rules: &RuleSet,
) {
    let Some(entity) = sim.substrate.entities.get(stable_id) else {
        return;
    };
    let arrival = (entity.position.rx, entity.position.ry);
    let object = sim.object_type(entity.type_ref(), rules);
    let chrono_out = object
        .and_then(|object| object.chrono_out_sound.clone())
        .or_else(|| rules.general.chrono_out_sound.clone());
    let chrono_in = object
        .and_then(|object| object.chrono_in_sound.clone())
        .or_else(|| rules.general.chrono_in_sound.clone());
    for (name, (rx, ry)) in [
        (chrono_out, departure.unwrap_or(arrival)),
        (chrono_in, arrival),
    ] {
        if let Some(name) = name {
            let sound_id = sim.interner.intern(&name);
            sim.sound_events
                .push(super::SimSoundEvent::ChronoTeleport { sound_id, rx, ry });
        }
    }
}

#[cfg(test)]
#[path = "track_object_turn_tests.rs"]
mod track_object_turn_tests;

#[cfg(test)]
#[path = "forced_track_object_turn_tests.rs"]
mod forced_track_object_turn_tests;

#[cfg(test)]
#[path = "teleport_anim_object_turn_tests.rs"]
mod teleport_anim_object_turn_tests;

#[cfg(test)]
#[path = "per_cell_object_turn_tests.rs"]
mod per_cell_object_turn_tests;

#[derive(Default)]
pub(super) struct LiveObjectPassOutcome {
    pub movement: movement::MovementTickStats,
    pub destroyed_structure: bool,
    pub bridge_state_changed: bool,
    pub tube_turn_owned_ids: BTreeSet<u64>,
}

#[derive(Default)]
pub(crate) struct GroundLocomotorOutcome {
    pub(super) movement: movement::MovementTickStats,
    pub(super) bridge_state_changed: bool,
    track_owned: bool,
    /// The Hover or tube-exit arrival ran `Per_Cell_Process(2)`.
    per_cell_ran: bool,
}

impl GroundLocomotorOutcome {
    /// Whether the Process changed bridge state.
    pub(crate) fn bridge_state_changed(&self) -> bool {
        self.bridge_state_changed
    }
}

/// Re-enter the pending movement pass for the same mover of this Process (see
/// `MoverReentry`), with the Simulation's current grids and state.
fn reenter_pending_pass(
    sim: &mut Simulation,
    pending: &mut movement::movement_tick::PendingMovementPass,
    reentry: movement::movement_tick::MoverReentry,
    rules: Option<&RuleSet>,
    path_grid: Option<&PathGrid>,
    overlay_registry: Option<&crate::map::overlay_types::OverlayTypeRegistry>,
    timing: movement::MovementConfig,
) {
    let current_grid = sim.path_grid_snapshot();
    pending.reenter_mover(
        reentry,
        &mut sim.substrate.entities,
        current_grid.as_deref().or(path_grid),
        sim.zone_grid.as_ref(),
        sim.resolved_terrain.as_ref(),
        &sim.terrain_costs,
        &sim.house_alliances,
        &mut sim.substrate.occupancy,
        &mut sim.substrate.cell_occupation,
        &mut sim.substrate.raw_cell_occupation,
        &mut sim.substrate.next_occupancy_enter_order,
        &mut sim.scenario_rng,
        sim.session.tick,
        sim.session.binary_frame,
        sim.overlay_grid.as_ref(),
        overlay_registry,
        sim.playfield_bounds,
        &sim.terrain_speed_config,
        timing.close_enough,
        timing.path_delay_ticks,
        timing.blockage_path_delay_ticks,
        &mut sim.interner,
        rules,
        Some(&sim.type_handles),
        &mut sim.movement_pass_cache,
        &sim.houses,
    );
}

#[derive(Default)]
pub(super) struct ObjectTurnOutcome {
    movement: movement::MovementTickStats,
    destroyed_structure: bool,
    bridge_state_changed: bool,
    tube_owned: bool,
}

/// What one object's locomotor Process did this turn.
#[derive(Default)]
struct LocomotorProcess {
    /// FootClass::AI admitted the Process (`0x004DA806..0x004DA877`).
    admitted: bool,
    movement: movement::MovementTickStats,
    bridge_state_changed: bool,
    track_owned: bool,
    /// The Process ran `Per_Cell_Process(2)`: a Hover, tube-exit, touchdown
    /// or warp arrival.
    per_cell_ran: bool,
    /// The Process UnInit its owner: an aircraft impact or a dead missile.
    ended: bool,
}

impl LocomotorProcess {
    fn from_ground(ground: GroundLocomotorOutcome) -> Self {
        Self {
            admitted: true,
            movement: ground.movement,
            bridge_state_changed: ground.bridge_state_changed,
            track_owned: ground.track_owned,
            per_cell_ran: ground.per_cell_ran,
            ended: false,
        }
    }

    fn admitted() -> Self {
        Self {
            admitted: true,
            ..Self::default()
        }
    }
}

impl Simulation {
    /// FootClass::AI's one call of the active locomotor's Process
    /// (`0x004DA877`, `ILocomotion` vtable `+0x40`).
    ///
    /// - Admission (`0x004DA806..0x004DA86E`): a locomotor (Foot `+0x674`),
    ///   not sinking (`+0x3CD`), not falling (`+0x8D`) and not in limbo
    ///   (`+0x81`). The DirectRocker link test (`+0x2A8` against TechnoType
    ///   `+0x692`) is dormant: no retail warhead sets `DirectRocker=`, and
    ///   VERA keeps no link.
    /// - The active class picks the one Process: the ground corridor for
    ///   Drive, Ship, Walk and Hover, the air pass for Fly and Jumpjet, and
    ///   Teleport's and Rocket's own.
    fn process_active_locomotor(
        &mut self,
        stable_id: u64,
        rules: Option<&RuleSet>,
        path_grid: Option<&PathGrid>,
        overlay_registry: Option<&crate::map::overlay_types::OverlayTypeRegistry>,
        cell_before_movement: Option<(u16, u16)>,
    ) -> Result<LocomotorProcess, super::FrameAdvanceError> {
        use crate::rules::locomotor_type::LocomotorKind;
        let (admitted, sinking) =
            self.substrate
                .entities
                .get(stable_id)
                .map_or((None, false), |entity| {
                    let sinking = entity.sinking.is_active();
                    let admitted = entity.locomotor.as_ref().and_then(|locomotor| {
                        (!sinking && !entity.is_falling_down() && !entity.lifecycle.in_limbo)
                            .then(|| locomotor.active_kind())
                    });
                    (admitted, sinking)
                });
        let ground = matches!(
            admitted,
            Some(
                LocomotorKind::Drive
                    | LocomotorKind::Ship
                    | LocomotorKind::Walk
                    | LocomotorKind::Hover
            )
        );
        // The vehicle plane is a projection each object turn reconciles once
        // at Process entry: the ground corridor in `prepare_movement_pass`,
        // any other turn here. A sinking object keeps its projection.
        if !ground
            && !sinking
            && let Some(entity) = self.substrate.entities.get(stable_id)
        {
            self.substrate.cell_occupation.reconcile_entity(entity);
        }
        let Some(kind) = admitted else {
            return Ok(LocomotorProcess::default());
        };
        match kind {
            LocomotorKind::Drive
            | LocomotorKind::Ship
            | LocomotorKind::Walk
            | LocomotorKind::Hover => self
                .process_ground_locomotor_one(stable_id, rules, path_grid, overlay_registry)
                .map(LocomotorProcess::from_ground),
            LocomotorKind::Fly | LocomotorKind::Jumpjet => {
                self.process_air_locomotor(stable_id, rules, overlay_registry)
            }
            LocomotorKind::Teleport => self.process_teleport_locomotor(
                stable_id,
                rules,
                overlay_registry,
                cell_before_movement,
            ),
            LocomotorKind::Rocket => Ok(self.process_rocket_locomotor(stable_id)),
        }
    }

    /// Fly and Jumpjet Process: the air pass inside the Fly cell-list
    /// transaction, then a touchdown's `Per_Cell_Process(2)` or an impact.
    fn process_air_locomotor(
        &mut self,
        stable_id: u64,
        rules: Option<&RuleSet>,
        overlay_registry: Option<&crate::map::overlay_types::OverlayTypeRegistry>,
    ) -> Result<LocomotorProcess, super::FrameAdvanceError> {
        let mut process = LocomotorProcess::admitted();
        self.complete_pending_class_order(stable_id, rules);
        let air = self.tick_air_movement_with_cell_lists_one(stable_id, rules);
        if air.touched_down {
            process.bridge_state_changed |= self.per_cell_process(
                stable_id,
                movement::PerCellReason::Arrival,
                rules,
                overlay_registry,
            )?;
            process.per_cell_ran = true;
        }
        if !air.impact {
            return Ok(process);
        }
        let Some(entity) = self.substrate.entities.get(stable_id) else {
            return Ok(process);
        };
        if entity.category == EntityCategory::Infantry {
            // The Infantry notice keeps the infantryman: its class AI runs on
            // to the sequencer and locomotion actions.
            if let Some(rules) = rules {
                self.aircraft_tracker_remove(stable_id);
                self.infantry_crash_impact(stable_id, rules);
            }
            return Ok(process);
        }
        // The impact UnInits the object; `FootClass::AI` returns on the
        // cleared Object+90 (`0x004DA87E`) and the class AI after it
        // (`AircraftClass::AI 0x00414DAA`).
        let jumpjet = entity.locomotor.as_ref().is_some_and(|locomotor| {
            locomotor.active_kind() == crate::rules::locomotor_type::LocomotorKind::Jumpjet
        });
        match rules {
            Some(rules) if jumpjet => {
                self.jumpjet_crash_impact(stable_id, rules, overlay_registry);
            }
            Some(rules) => self.fly_crash_impact(stable_id, rules, overlay_registry),
            None => self.uninit(stable_id),
        }
        process.ended = true;
        Ok(process)
    }

    /// Teleport Process (`0x007192F0`): the warp and its arrival.
    fn process_teleport_locomotor(
        &mut self,
        stable_id: u64,
        rules: Option<&RuleSet>,
        overlay_registry: Option<&crate::map::overlay_types::OverlayTypeRegistry>,
        cell_before_movement: Option<(u16, u16)>,
    ) -> Result<LocomotorProcess, super::FrameAdvanceError> {
        self.complete_pending_class_order(stable_id, rules);
        let sim = self;
        let one = [stable_id];
        let mut process = LocomotorProcess::admitted();
        let teleport_armed = sim
            .substrate
            .entities
            .get(stable_id)
            .and_then(|entity| entity.teleport_state.as_ref())
            .is_some_and(|state| state.phase == teleport_movement::TeleportPhase::Relocate);
        // Teleport Process 0x007197AF: already on the destination, no warp.
        let teleport_reached = sim.substrate.entities.get(stable_id).is_some_and(|entity| {
            teleport_movement::warp_destination_reached(entity, sim.resolved_terrain.as_ref())
        });
        let teleport_relocating = teleport_armed && !teleport_reached;
        // Teleport Process 0x007195BF..0x007195CF: the warp step ejects a
        // parasite (ExitUnit, no suppression) before the relocation.
        if teleport_relocating
            && let Some(rules) = rules
            && let Some(eater) = sim
                .substrate
                .entities
                .get(stable_id)
                .and_then(|entity| entity.parasite_eating_me)
        {
            sim.parasite_exit_unit(eater, rules);
        }
        // `0x007195D4` Mark(UP) and `0x007196B8` Mark(DOWN) bracket the
        // relocation: the cell lists, the raw occupation and the vehicle plane
        // leave the old cell and join the new one on its OnBridge layer.
        if teleport_relocating {
            sim.foot_mark_remove(stable_id, rules, overlay_registry);
        }
        if let Some(rules) = rules {
            let warp_out_type = sim.interner.intern(&rules.general.warp_out.name);
            let mut warp_spawns = Vec::new();
            let mut teleport_visuals = teleport_movement::TeleportVisuals {
                anim_spawns: &mut warp_spawns,
                warp_out_type,
            };
            teleport_movement::tick_teleport_movement(
                &mut sim.substrate.entities,
                &one,
                sim.session.tick,
                sim.resolved_terrain.as_ref(),
                Some(&mut teleport_visuals),
            );
            if teleport_relocating {
                sim.foot_mark_put(stable_id, Some(rules), overlay_registry);
            }
            // RESIDUAL: the destination WarpOut is built here with the source
            // one; native builds it after the arrival's Per_Cell_Process(2),
            // Stop_Moving, crate pickup and NULL assign (`0x00719742`). Only
            // an arrival step that draws RNG (a crush's death animation)
            // could see the order.
            for descriptor in warp_spawns {
                let type_name = descriptor.type_name;
                if let Err(error) = sim.spawn_anim_object(rules, descriptor) {
                    // An art type that never bound draws nothing natively
                    // either; see `spawn_combat_explosion_anim`.
                    log::debug!(
                        "teleport warp [{}] did not construct: {error}",
                        sim.interner.resolve(type_name)
                    );
                }
            }
        } else {
            teleport_movement::tick_teleport_movement(
                &mut sim.substrate.entities,
                &one,
                sim.session.tick,
                sim.resolved_terrain.as_ref(),
                None,
            );
            if teleport_relocating {
                sim.foot_mark_put(stable_id, None, overlay_registry);
            }
        }
        // Teleport Process's warp step (`0x007192F0`), after the relocation.
        // Its `vt+0x480(NULL, 1)` is the owner's class setter: the Unit one,
        // or the represented NavCom clear for an infantryman.
        let unit_teleport = sim
            .substrate
            .entities
            .get(stable_id)
            .is_some_and(|entity| entity.category == EntityCategory::Unit);
        let null_destination = |sim: &mut Simulation| {
            if unit_teleport {
                sim.set_unit_null_destination(stable_id, rules);
            } else {
                sim.assign_null_destination(stable_id, rules);
            }
        };
        if teleport_reached {
            // 0x007197B4: vt+0x480(NULL, 1) before Stop_Moving.
            null_destination(sim);
        }
        if teleport_relocating {
            if let Some(rules) = rules {
                teleport_warp_sounds(sim, stable_id, cell_before_movement, rules);
            }
            // Relocation 0x0071971C calls vt+0x18C(2), including a same-cell
            // relocation; ordinary Fly motion has no such call.
            process.bridge_state_changed |= sim.per_cell_process(
                stable_id,
                movement::PerCellReason::Arrival,
                rules,
                overlay_registry,
            )?;
            process.per_cell_ran = true;
            // 0x00719725 Stop_Moving: the tick retired the request.
            // 0x0071972E CellClass::PickupCrate (`0x00481A00`): the crate
            // receiver every mover still lacks (`movement::track_fresh`
            // residuals).
            // 0x0071973C: vt+0x480(NULL, 1). A Teleporter still in radio
            // contact gets a Drive here, which the FootClass::AI tail ends
            // again.
            null_destination(sim);
        }
        Ok(process)
    }

    /// Rocket Process (`0x006622C0`): the flight step, an arrival's
    /// detonation request and a dead missile's explosion.
    fn process_rocket_locomotor(&mut self, stable_id: u64) -> LocomotorProcess {
        let mut process = LocomotorProcess::admitted();
        let arrivals = rocket_movement::tick_rocket_movement(
            &mut self.substrate.entities,
            &[stable_id],
            self.session.tick,
        );
        let arrived = !arrivals.is_empty();
        self.pending_rocket_detonations.extend(arrivals);
        // `0x00662FA1..0x00662FD0`: after the flight step the Process moves
        // its owner's AircraftTracker entry (`0x004138C0`) to the new cell.
        self.sync_air_spatial_membership(stable_id);
        // `0x00662FD5..0x00662FE1`: a missile left with no Health explodes
        // where it is and is UnInit, so `FootClass::AI` and
        // `AircraftClass::AI` stop here.
        let died = !arrived
            && self
                .substrate
                .entities
                .get(stable_id)
                .is_some_and(|entity| entity.health.current <= 0 && entity.rocket_state.is_some());
        if died {
            crate::sim::spawn_manager::detonate_dead_missile(self, stable_id);
            process.ended = true;
        }
        process
    }

    /// ObjectClass::AI's fall step for one object (`0x005F3F11..0x005F3FA4`).
    /// Answers whether the fall grounded this frame.
    fn advance_fall_one(&mut self, stable_id: u64, rules: &RuleSet) -> bool {
        let falling = |sim: &Simulation| {
            sim.substrate
                .entities
                .get(stable_id)
                .is_some_and(crate::sim::game_entity::GameEntity::is_falling_down)
        };
        if !falling(self) {
            return false;
        }
        parachute_descent::tick_parachute_descent_in_order(
            &mut self.substrate.entities,
            &[stable_id],
            rules.general.parachute_max_fall_rate,
            self.session.tick,
        );
        !falling(self)
    }

    /// A destination Rust deferred (`pending_arrival_clear`), finished before
    /// a Teleport, Fly or Jumpjet Process, where natively its class setter
    /// already ran. The one producer these locomotors meet is a mission
    /// Restore, whose `Assign_Destination(saved, 1)` (`FootClass::Restore_Mission`
    /// `0x004D8F8C..0x004D8F99`) `mission::authority` represents by NavCom
    /// alone. Drive and Ship finish theirs in `complete_pending_track_order`;
    /// Walk and Hover keep the ground corridor's rebuild.
    /// - NavCom's cell, else the first queued cell, goes to the class setter
    ///   as a fresh destination: the Teleport route
    ///   ([`Self::teleport_destination`]) or the air setter
    ///   ([`Self::issue_air_cell_destination`]). NavCom is cleared first, as
    ///   the setter found it, so the Unit setter's unchanged-NavCom return
    ///   cannot swallow it.
    /// - With neither, the NULL clear.
    ///
    /// This keeps the retired rebuild's choice of cell. An object NavCom (an
    /// Enter or follow target) falls to the queue or the clear, as it did.
    fn complete_pending_class_order(&mut self, stable_id: u64, rules: Option<&RuleSet>) {
        use crate::rules::locomotor_type::LocomotorKind;
        use crate::sim::components::NavTargetRef;
        let Some(rules) = rules else {
            return;
        };
        let Some(entity) = self.substrate.entities.get_mut(stable_id) else {
            return;
        };
        if !entity.navigation.pending_arrival_clear {
            return;
        }
        entity.navigation.pending_arrival_clear = false;
        let navigation = &mut entity.navigation;
        let cell = match navigation.nav_com {
            Some(NavTargetRef::Cell { rx, ry }) => Some((rx, ry)),
            _ => match navigation.nav_queue.first().copied() {
                Some(NavTargetRef::Cell { rx, ry }) => {
                    navigation.nav_queue.remove(0);
                    Some((rx, ry))
                }
                _ => None,
            },
        };
        let Some(cell) = cell else {
            movement::set_destination_internal_null(entity);
            return;
        };
        entity.navigation.nav_com = None;
        entity.navigation.nav_com_aux = None;
        let kind = entity
            .locomotor
            .as_ref()
            .map(|locomotor| locomotor.active_kind());
        match kind {
            Some(LocomotorKind::Teleport) => {
                self.teleport_destination(stable_id, cell, Some(rules));
            }
            Some(LocomotorKind::Fly | LocomotorKind::Jumpjet) => {
                if let Some(info) = self.resolve_move_info(stable_id, Some(rules)) {
                    self.issue_air_cell_destination(stable_id, cell, info.speed, Some(rules));
                }
            }
            _ => {}
        }
    }
}

impl Simulation {
    /// Component-based movement fixtures enter the same Process corridor as
    /// live object turns. This exposes no alternate physics or callback loop.
    #[cfg(test)]
    pub(crate) fn process_ground_locomotor_for_test(
        &mut self,
        id: u64,
        rules: Option<&RuleSet>,
        grid: Option<&PathGrid>,
        registry: Option<&crate::map::overlay_types::OverlayTypeRegistry>,
    ) -> Result<movement::MovementTickStats, super::FrameAdvanceError> {
        self.install_fixture_path_grid(grid);
        self.process_ground_locomotor_one(id, rules, grid, registry)
            .map(|outcome| outcome.movement)
    }

    /// The ordinary ground locomotor Process corridor, without Object/Techno AI.
    /// Infantry Scatter51D478 calls the active locomotor synchronously; its
    /// PerCell and boundary receivers must finish before Scatter returns.
    pub(crate) fn process_ground_locomotor_one(
        &mut self,
        stable_id: u64,
        rules: Option<&RuleSet>,
        path_grid: Option<&PathGrid>,
        overlay_registry: Option<&crate::map::overlay_types::OverlayTypeRegistry>,
    ) -> Result<GroundLocomotorOutcome, super::FrameAdvanceError> {
        let timing = movement::MovementConfig::from_rules(self.session.binary_frame, rules);
        self.process_ground_locomotor_with_config(
            stable_id,
            rules,
            path_grid,
            overlay_registry,
            timing,
        )
    }

    #[cfg(test)]
    pub(crate) fn process_ground_locomotor_with_config_for_test(
        &mut self,
        stable_id: u64,
        rules: Option<&RuleSet>,
        path_grid: Option<&PathGrid>,
        overlay_registry: Option<&crate::map::overlay_types::OverlayTypeRegistry>,
        timing: movement::MovementConfig,
    ) -> Result<movement::MovementTickStats, super::FrameAdvanceError> {
        self.process_ground_locomotor_with_config(
            stable_id,
            rules,
            path_grid,
            overlay_registry,
            timing,
        )
        .map(|outcome| outcome.movement)
    }

    fn process_ground_locomotor_with_config(
        &mut self,
        stable_id: u64,
        rules: Option<&RuleSet>,
        path_grid: Option<&PathGrid>,
        overlay_registry: Option<&crate::map::overlay_types::OverlayTypeRegistry>,
        timing: movement::MovementConfig,
    ) -> Result<GroundLocomotorOutcome, super::FrameAdvanceError> {
        let sim = self;
        let one = [stable_id];
        let mut outcome = GroundLocomotorOutcome::default();
        // Grid-less component fixtures keep the pass's empty-arrival cleanup;
        // the pass searches with the Simulation grid or the caller's.
        if path_grid.is_some() || sim.path_grid.is_some() {
            sim.complete_pending_track_order(stable_id, rules);
        }
        let movement_before = sim.substrate.entities.get(stable_id).map(|entity| {
            (
                (entity.position.rx, entity.position.ry),
                entity.movement_target.is_some(),
                entity.low_bridge_tube_state.is_some(),
                entity.locomotor.as_ref().map(|loco| loco.active_kind()),
            )
        });
        // Drive4B050B..0557 / Ship69FC1B..FC67 samples the containing
        // cell slope before any active-track, destination or turn return.
        // Entry-active Tube owns its whole visit and does not call Process.
        if let Some(entity) = sim.substrate.entities.get_mut(stable_id)
            && entity.low_bridge_tube_state.is_none()
            && let Some(slope) = sim.resolved_terrain.as_ref().and_then(|terrain| {
                terrain
                    .cell(entity.position.rx, entity.position.ry)
                    .map(|cell| cell.slope_type)
            })
        {
            movement::slope_transition::sample_process_entry(
                entity,
                slope,
                sim.session.binary_frame,
            );
        }
        if !sim.process_track_turn(stable_id, rules, overlay_registry) {
            return Ok(outcome);
        }
        let mut pending_movement = {
            let current_grid = sim.path_grid_snapshot();
            movement::movement_tick::begin_movement_with_grids_scoped(
                &mut sim.substrate.entities,
                Some(&one),
                current_grid.as_deref().or(path_grid),
                &sim.terrain_costs,
                &sim.house_alliances,
                &mut sim.substrate.occupancy,
                &mut sim.substrate.cell_occupation,
                &mut sim.substrate.raw_cell_occupation,
                &mut sim.substrate.next_occupancy_enter_order,
                &mut sim.scenario_rng,
                sim.session.tick,
                sim.session.binary_frame,
                sim.zone_grid.as_ref(),
                sim.resolved_terrain.as_ref(),
                sim.overlay_grid.as_ref(),
                overlay_registry,
                sim.playfield_bounds,
                &sim.terrain_speed_config,
                timing.close_enough,
                timing.path_delay_ticks,
                timing.blockage_path_delay_ticks,
                &mut sim.interner,
                rules,
                Some(&sim.type_handles),
                &mut sim.movement_pass_cache,
                &sim.houses,
            )
            .map_err(|cause| super::FrameAdvanceError {
                tick: sim.session.tick,
                binary_frame: sim.session.binary_frame,
                entity_id: stable_id,
                cause,
            })?
        };
        // Process_Movement's no-queue request and the Process_Track after it.
        // When the active-track Process_Track(0) ends its track, the same
        // Process continues into Process_Movement and Process_Track(1)
        // (Drive 0x4B0583..0x4B0667 / Ship 0x69FC93..0x69FD0E;
        // `movement::track_continuation`).
        let frame_error = |sim: &Simulation, cause| super::FrameAdvanceError {
            tick: sim.session.tick,
            binary_frame: sim.session.binary_frame,
            entity_id: stable_id,
            cause,
        };
        let mut retry = false;
        loop {
            // The Scatter calls a step queued while the pass held the world
            // (tube exit, pass-lane cell entry), in call order.
            outcome.bridge_state_changed |= sim
                .run_scatter_requests(
                    pending_movement.take_scatter_requests(),
                    rules,
                    overlay_registry,
                )
                .map_err(|cause| frame_error(sim, cause))?;
            if let Some(request) = pending_movement.take_foot_path_request() {
                let held = Some(pending_movement.held_block_sets());
                let outcome = sim
                    .run_foot_path_request(&request, held, rules, overlay_registry)
                    .map_err(|cause| frame_error(sim, cause))?;
                if outcome == movement::FootPathOutcome::Resume {
                    reenter_pending_pass(
                        sim,
                        &mut pending_movement,
                        movement::movement_tick::MoverReentry::FootPath(Box::new(request)),
                        rules,
                        path_grid,
                        overlay_registry,
                        timing,
                    );
                }
            }
            if let Some(request) = pending_movement.take_walk_admission_request() {
                let retry = sim
                    .run_walk_admission_request(
                        request,
                        Some(pending_movement.held_block_sets()),
                        rules,
                        overlay_registry,
                    )
                    .map_err(|cause| frame_error(sim, cause))?;
                if let Some(request) = retry {
                    //75B716/75BABB/75BC04: one recursive Process(0),
                    //with the already prepared mover visit retained.
                    pending_movement.request_foot_path(request);
                    continue;
                }
            }
            // Drive4B0A79 / Ship6A0142 and the track-end continuation
            // 4B0647 / 69FCEE: Process_Movement(&out, 1, 0); Process_Track
            // follows unless the out byte is set or the Foot died
            // (4B0A7E..4B0AAA, 4B064C..4B0667).
            if let Some((id, family)) = pending_movement.take_track_movement() {
                let rules = rules.ok_or_else(|| {
                    frame_error(sim, "Drive/Ship Process_Movement requires rules".into())
                })?;
                let out = sim
                    .run_track_process_movement(
                        id,
                        family,
                        movement::ProcessMovementArgs::OUTER,
                        Some(pending_movement.held_block_sets()),
                        rules,
                        overlay_registry,
                    )
                    .map_err(|cause| frame_error(sim, cause))?;
                if !out
                    && sim
                        .substrate
                        .entities
                        .get(id)
                        .is_some_and(|entity| entity.lifecycle.object_alive)
                {
                    pending_movement.record_native_track(
                        movement::track_process::TrackInvocation::after_process_movement(
                            id, family,
                        ),
                    );
                }
            }
            let Some(invocation) = pending_movement
                .take_native_track()
                // Ordinary Process reloads Object+90 after the fresh receiver.
                .filter(|invocation| {
                    sim.substrate
                        .entities
                        .get(invocation.entity_id)
                        .is_some_and(|entity| entity.lifecycle.object_alive)
                })
            else {
                break;
            };
            let invocation = movement::track_process::TrackInvocation {
                retry,
                ..invocation
            };
            let pass = sim
                .run_track_process(invocation, rules, overlay_registry)
                .map_err(|cause| frame_error(sim, cause))?;
            pending_movement.record_track_movement(pass.moved);
            outcome.track_owned = true;
            if retry || !invocation.active_gate || pass.aborted {
                break;
            }
            if !sim
                .begin_track_end_continuation(invocation.entity_id, invocation.family, rules)
                .map_err(|cause| frame_error(sim, cause))?
            {
                break;
            }
            reenter_pending_pass(
                sim,
                &mut pending_movement,
                movement::movement_tick::MoverReentry::AfterTrackEnd(invocation.entity_id),
                rules,
                path_grid,
                overlay_registry,
                timing,
            );
            retry = true;
        }
        outcome.bridge_state_changed |= sim
            .run_scatter_requests(
                pending_movement.take_scatter_requests(),
                rules,
                overlay_registry,
            )
            .map_err(|cause| frame_error(sim, cause))?;
        if let Some((id, head)) = pending_movement.take_walk_per_cell() {
            outcome.bridge_state_changed |=
                sim.run_completed_walk_step(id, head, rules, overlay_registry)?;
            pending_movement.retain_walk_completion(id, &sim.substrate.entities);
        }
        if let Some((id, coord)) = pending_movement.take_walk_boundary() {
            sim.run_walk_boundary(id, coord, rules, overlay_registry);
            pending_movement.record_track_movement(1);
        }

        outcome
            .movement
            .merge(movement::movement_tick::finish_movement_pass(
                pending_movement,
                &mut sim.substrate.entities,
                &sim.houses,
                &sim.house_alliances,
                &mut sim.substrate.cell_occupation,
                sim.session.binary_frame,
                sim.resolved_terrain.as_ref(),
                sim.path_grid.as_deref().or(path_grid),
                &mut sim.interner,
                rules,
                &mut sim.sound_events,
                &mut sim.pending_lifecycle_requests,
                &mut sim.movement_pass_cache,
            ));
        if let Some((old_cell, had_target, tube_active, kind)) = movement_before {
            let per_cell = sim.substrate.entities.get(stable_id).is_some_and(|entity| {
                // Tube exits Unit73603F / Infantry51BA9B and Hover arrival5146CA / cell-entry
                // 515A1C call vt+0x18C(2). The existing Hover integrator still
                // approximates native crossing timing; its accepted entries
                // use this owner.
                (tube_active && entity.low_bridge_tube_state.is_none())
                    || (kind == Some(crate::rules::locomotor_type::LocomotorKind::Hover)
                        && (old_cell != (entity.position.rx, entity.position.ry)
                            || (had_target && entity.movement_target.is_none())))
            });
            if per_cell {
                outcome.bridge_state_changed |= sim.per_cell_process(
                    stable_id,
                    movement::PerCellReason::Arrival,
                    rules,
                    overlay_registry,
                )?;
                outcome.per_cell_ran = true;
            }
        }
        Ok(outcome)
    }
    pub(super) fn advance_live_object_pass(
        &mut self,
        rules: Option<&RuleSet>,
        path_grid: Option<&PathGrid>,
        overlay_registry: Option<&crate::map::overlay_types::OverlayTypeRegistry>,
    ) -> Result<LiveObjectPassOutcome, super::FrameAdvanceError> {
        let miner_config = rules.map(crate::sim::miner::MinerConfig::from_rules);
        let terrain_spawner_cells = self
            .production
            .terrain_spawners
            .keys()
            .copied()
            .collect::<BTreeSet<_>>();
        let object_ctx = techno_ai::ObjectAiCtx {
            path_grid,
            overlay_registry,
            terrain_spawner_cells: Some(&terrain_spawner_cells),
            miner_config: miner_config.as_ref(),
        };

        let mut outcome = LiveObjectPassOutcome::default();
        self.try_for_each_live_object::<super::FrameAdvanceError>(|sim, stable_id| {
            let turn = sim.advance_live_object_turn(stable_id, rules, object_ctx)?;
            outcome.movement.merge(turn.movement);
            outcome.destroyed_structure |= turn.destroyed_structure;
            outcome.bridge_state_changed |= turn.bridge_state_changed;
            if turn.tube_owned {
                outcome.tube_turn_owned_ids.insert(stable_id);
            }
            Ok(())
        })?;
        Ok(outcome)
    }

    pub(super) fn advance_live_object_turn(
        &mut self,
        stable_id: u64,
        rules: Option<&RuleSet>,
        object_ctx: techno_ai::ObjectAiCtx<'_>,
    ) -> Result<ObjectTurnOutcome, super::FrameAdvanceError> {
        let sim = self;
        let path_grid = object_ctx.path_grid;
        let overlay_registry = object_ctx.overlay_registry;
        let mut outcome = ObjectTurnOutcome::default();
        // UnitClass::AI / InfantryClass::AI give an active TubeMovement
        // object the whole live-object turn.  Capture before the leaf:
        // successful finalization clears the payload but must still skip
        // every ordinary locomotor tail and the second mission checkpoint.
        let tube_active_at_entry = sim.substrate.entities.get(stable_id).is_some_and(|entity| {
            !entity.dying
                && matches!(
                    entity.category,
                    EntityCategory::Unit | EntityCategory::Infantry
                )
                && entity.low_bridge_tube_state.is_some()
        });
        let was_structure = sim
            .substrate
            .entities
            .get(stable_id)
            .is_some_and(|entity| entity.category == EntityCategory::Structure);
        let ai = sim.object_ai_visit_one_with_effects(stable_id, rules, object_ctx);
        outcome.bridge_state_changed |= ai.bridge_state_changed;
        if was_structure
            && sim
                .substrate
                .entities
                .get(stable_id)
                .is_none_or(|entity| entity.dying)
        {
            outcome.destroyed_structure = true;
        }
        if sim.substrate.entities.get(stable_id).is_none_or(|entity| {
            entity.dying
                || (!tube_active_at_entry
                    && matches!(
                        entity.category,
                        EntityCategory::Unit | EntityCategory::Infantry | EntityCategory::Aircraft
                    )
                    && !entity.lifecycle.object_alive)
        }) {
            // Foot4DA53E..548 reloads Object+90 after TechnoAI and returns
            // before locomotor Process when false. Health and the Rust dying
            // state are independent: even Process's slope prelude must wait
            // until this live owner admission succeeds. Entry-active Tube AI
            // bypasses Foot AI and therefore does not reach this predicate.
            // Evidence: tools/spatial_oracle/foot_enter_idle, foot_ai_reset rows.
            return Ok(outcome);
        }
        // A temporal-warped object's leaf AI returned before its locomotor
        // Process (Unit 0x007362B5..0x007362CB, Infantry and Aircraft alike;
        // `Simulation::temporal_ai_prologue`).
        if sim
            .substrate
            .entities
            .get(stable_id)
            .is_some_and(crate::sim::game_entity::GameEntity::ai_frozen)
        {
            return Ok(outcome);
        }

        // ObjectClass::AI's fall step (`0x005F3F11..0x005F3FA4`), which the
        // Foot's TechnoClass::AI call (`0x004DA539`) reaches before Process.
        //
        // RESIDUAL: VERA runs it after the whole Techno visit, not at
        // ObjectClass::AI's start. Trigger: a paradropped object landing.
        // Effect: that frame's mission dispatch still sees it falling.
        // Frequency: once per paradrop landing. Risk: the first mission
        // Commence after landing waits one frame.
        let mut per_cell_ran = false;
        if let Some(rules) = rules
            && sim.advance_fall_one(stable_id, rules)
        {
            // Object AI5F3F8D: grounded fall completion calls vt+0x18C(2)
            // before the parachute animation's wind-down.
            outcome.bridge_state_changed |= sim.per_cell_process(
                stable_id,
                movement::PerCellReason::Arrival,
                Some(rules),
                overlay_registry,
            )?;
            per_cell_ran = true;
            sim.wind_down_parachute_anim(rules, stable_id);
        }

        if !tube_active_at_entry {
            sim.refresh_high_flying_sight_before_process(stable_id, rules, path_grid);
        }

        let before_movement = sim.movement_sound_probe(stable_id);
        let cell_before_movement = sim
            .substrate
            .entities
            .get(stable_id)
            .map(|entity| (entity.position.rx, entity.position.ry));
        let walk_process_owned = sim
            .substrate
            .entities
            .get(stable_id)
            .and_then(|e| e.locomotor.as_ref())
            .is_some_and(|l| l.kind == crate::rules::locomotor_type::LocomotorKind::Walk);
        // An entry-active TubeMovement leaf runs through the ground corridor
        // in place of the Foot AI and its Process.
        let process = if tube_active_at_entry {
            let ground =
                sim.process_ground_locomotor_one(stable_id, rules, path_grid, overlay_registry)?;
            LocomotorProcess {
                admitted: false,
                ..LocomotorProcess::from_ground(ground)
            }
        } else {
            sim.process_active_locomotor(
                stable_id,
                rules,
                path_grid,
                overlay_registry,
                cell_before_movement,
            )?
        };
        let track_owned = process.track_owned;
        per_cell_ran |= process.per_cell_ran;
        outcome.movement.merge(process.movement);
        outcome.bridge_state_changed |= process.bridge_state_changed;
        // `0x004DA87A`: the Process may have converted or removed the owner.
        if process.ended
            || sim
                .substrate
                .entities
                .get(stable_id)
                .is_none_or(|e| e.dying || !e.lifecycle.object_alive)
        {
            return Ok(outcome);
        }

        // FootClass advances the SHP Unit body counter immediately after
        // this object's locomotor Process, against the still-current
        // absolute binary frame, inside the Process admission
        // (`0x004DA81A` bypasses both). The global frame commits only after
        // the complete live-object pass.
        if process.admitted && shp_vehicle_counter_admitted(tube_active_at_entry) {
            let shp_vehicle_cadence = sim.substrate.entities.get(stable_id).and_then(|entity| {
                if entity.category != EntityCategory::Unit || entity.is_voxel {
                    return None;
                }
                let object = rules?.object(sim.interner.resolve(entity.type_ref()))?;
                Some(crate::sim::animation::ShpVehicleCadence {
                    walk_rate: object.walk_rate,
                    idle_rate: object.idle_rate,
                })
            });
            if let (Some(cadence), Some(entity)) = (
                shp_vehicle_cadence,
                sim.substrate.entities.get_mut(stable_id),
            ) {
                crate::sim::animation::tick_shp_vehicle_body_frame_counter(
                    entity,
                    rules.map(|rules| {
                        crate::sim::movement::SpeedRules::new(
                            rules,
                            &sim.interner,
                            &sim.type_handles,
                        )
                    }),
                    cadence,
                    sim.session.binary_frame,
                );
            }
        }

        // A direction-8 producer also ends this object's ordinary turn as
        // soon as it arms TubeMovement.  The leaf itself starts on the
        // object's next visit; an entry-active leaf may have cleared the
        // payload above, hence the captured half of this predicate.
        let tube_owns_whole_turn = tube_active_at_entry
            || sim.substrate.entities.get(stable_id).is_some_and(|entity| {
                matches!(
                    entity.category,
                    EntityCategory::Unit | EntityCategory::Infantry
                ) && entity.low_bridge_tube_state.is_some()
            });
        if tube_owns_whole_turn {
            outcome.tube_owned = true;
            return Ok(outcome);
        }

        movement::tick_locomotor_piggyback_restore_one(&mut sim.substrate.entities, stable_id);
        // FootClass::AI tail 0x004DAEE1..0x004DAEF3, after the piggyback swap:
        // an infected Foot runs its eater's ParasiteClass AI in its own turn.
        if let Some(rules) = rules {
            sim.parasite_ai_for_victim(stable_id, rules, overlay_registry);
        }

        let cell_after_movement = sim
            .substrate
            .entities
            .get(stable_id)
            .map(|entity| (entity.position.rx, entity.position.ry));
        // A mover whose Process has no ported `Per_Cell_Process` call (Fly,
        // a cruising Jumpjet, Rocket, the legacy lane's other movers) takes
        // VERA's cell-change stand-in for the Foot body's sensor, uncloak,
        // Temporal and promote steps (`movement/per_cell.rs`).
        //
        // RESIDUAL: native Fly and Jumpjet cruise reach none of these from a
        // cell change. Trigger: such a mover changing cell. Effect: its
        // sensor deposit, cloak scan, Temporal release and playfield promote
        // run there. Risk: an aircraft with `Sensors=` or a held Temporal
        // target; the promote is what admits an aircraft arriving from off
        // the map.
        if !track_owned
            && !walk_process_owned
            && !per_cell_ran
            && cell_before_movement != cell_after_movement
        {
            if let Some(rules) = rules {
                sim.refresh_unit_sensor_at_per_cell(stable_id, rules);
                crate::sim::world::techno_ai_cloak::uncloak_on_sensor_neighbour_after_cell_entry(
                    sim, stable_id, rules,
                );
            }
            sim.temporal_release_if_warping(stable_id);
            sim.promote_entity_playfield_membership_after_move(stable_id);
        }

        let mut lifecycle_requests = std::mem::take(&mut sim.pending_lifecycle_requests);
        for request in lifecycle_requests.drain(..) {
            let LifecycleRequest::Uninit { stable_id, .. } = request;
            sim.release_move_sound(stable_id);
            if let Some(rules) = rules {
                sim.apply_lifecycle_request_with_rules(request, rules);
            } else {
                sim.apply_lifecycle_request(request);
            }
        }
        debug_assert!(lifecycle_requests.is_empty());
        sim.pending_lifecycle_requests = lifecycle_requests;

        sim.tick_move_sound_after_process(stable_id, before_movement, rules);
        if let Some(rules) = rules {
            sim.sinking_edge_sounds(stable_id, rules);
            sim.crash_edge_sounds(stable_id, rules);
            if sim.tick_ship_sinking(stable_id, rules) {
                return Ok(outcome);
            }
        }
        // `InfantryClass::AI` ends, once FootClass::AI has returned, with its
        // sequencer (`0x0051BF6A`) and the locomotion actions of 0x00520F40
        // (`0x0051BF7B`), which read the fraction and state Process just left.
        if let Some(rules) = rules
            && sim
                .substrate
                .entities
                .get(stable_id)
                .is_some_and(|entity| entity.category == EntityCategory::Infantry)
            && sim.infantry_action_turn(stable_id, rules)
        {
            // Its AirDeathFinish (or WetDie) ended in UnInit.
            return Ok(outcome);
        }
        // UnitClass::AI after FootClass::AI, before its second Ready/Commence.
        crate::sim::miner::miner_system::unit_ai_clear_harvesting(sim, stable_id);
        sim.object_ai_post_movement_promote_one(stable_id, rules);
        if let Some(rules) = rules {
            sim.aircraft_crash_smoke(stable_id, rules);
        }
        Ok(outcome)
    }
}
