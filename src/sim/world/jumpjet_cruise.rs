//! Production host for the Jumpjet locomotor kernel ([`jumpjet_flight`]).
//!
//! Every Jumpjet runs `Process @ 0x0054AEC0` here: the Update gate
//! (`Is_Moving 0x0054AE50` or `Is_Moving_Now 0x0054D0D0`) and then the state at
//! `+0x50` — ground `0x0054B980`, ascend `0x0054BA30`, hold `0x0054BD30`,
//! translate `0x0054BFF0`, descend `0x0054C550`. That state field is the only
//! Jumpjet phase: readers take `JumpjetRuntime::phase`, and `AirMovePhase`
//! (Fly's phase) is neither written nor read for a Jumpjet. The locomotor
//! altitude is derived from the kernel's height. VERA's air adapter does not
//! drive a Jumpjet at all, which is what stops an idle one cycling takeoff and
//! landing forever.
//!
//! The cell `AltObject` air slot (`CellClass+0xE0`; `0x004135A0` queries it and
//! `0x00487D70` sets or clears it) lives in `ObjectSubstrate::air_slots`.
//! Claims and releases are collected as effects while the substrate stays
//! borrowed immutably, then applied once the frame's states have run.
//!
//! Orders reach the locomotor through `Move_To` and `Stop_Moving`
//! (`movement::jumpjet_movement`). VERA's movement adapter carries a Jumpjet's
//! orders in its goal cell, so each frame first applies the native entry the
//! goal stands for: a fresh goal is `Move_To` on the cell, and a goal dropped
//! in flight is Foot's null `Set_Destination`, whose `Stop_Moving` re-targets
//! the cell under the owner. State 4 admits a landing by the owner's own
//! `Can_Enter_Cell` (`Simulation::mover_can_enter`, asked at `0x0054C66D` with
//! no direction, height or source cell), and its refused landing runs
//! `Stop_Moving` once the frame is committed. A Unit owner's Update Mark has
//! by then cleared the vehicle bit of the cell below, so while a parked
//! vehicle holds that cell, the re-target picks it again and the owner hovers
//! until it frees (`retail_dustbowl_night_hawk_hovers_over_a_tank_on_its_cell`).
//!
//! The host lends the world immutably to the states, so the scenario stream
//! the scatters draw from is held outside the `Simulation` for the frame.
//!
//! A crashing owner (`FootClass+0x425`) takes no orders: the kill's Stun ran
//! `Stop_Moving` on it (`Simulation::jumpjet_stun_stop`), which keeps a moving
//! wreck flying and lifts a descending one into State 1, so `Process` latches
//! it into State 5 wherever the kill found it, and the object turn finishes
//! its impact (`Simulation::jumpjet_crash_impact`). A touchdown clears the
//! latch (`0x0054CA12`).
//!
//! Residuals:
//! - The landing State 4 admits raises only the locomotor latch, not the
//!   owner's cell occupation bit (`+0xF0` at `0x0054C731`: a Unit's
//!   `0x007441B0` sets `0x20`; the orders' `+0xF4`, `0x00744210`, clears it),
//!   so a second Jumpjet can pick a cell another is landing in.
//! - `Process`'s owner `+0x90` dispatch gate, `RulesClass+0x48`'s deploy
//!   facing, owner `+0x134` and the `JumpJetTurn=` hold facing are
//!   unmodelled.
//!
//! A Health-0
//! wreck whose kill found no cell to re-target (its search failed at the map's
//! edge) can land without its crash and stay; native's handling of that owner
//! is untraced.
//!
//! ## Dependency rules
//! - Part of sim/ — depends on map/, rules/ and sim/ only.

use super::Simulation;
use crate::map::cell_index::NativeCellIdentity;
use crate::map::entities::EntityCategory;
use crate::map::overlay_types::OverlayTypeRegistry;
use crate::map::resolved_terrain::ResolvedTerrainGrid;
use crate::map::retail_trig::{AtanTable, TrigTable, required_atan_table, required_math_tables};
use crate::rules::locomotor_type::LocomotorKind;
use crate::rules::ruleset::RuleSet;
use crate::sim::movement::air_movement::AirMovementTickStats;
use crate::sim::movement::ground_pose::{
    foot_set_location, ground_surface_z_at, position_world_xy,
};
use crate::sim::movement::infantry_entry::InfantryEntryArgs;
use crate::sim::movement::jumpjet_flight::{
    self, FlightOwnerKind, JumpjetFlightHost, STATE_ASCEND, STATE_DESCEND, STATE_HOLD,
    STATE_TRANSLATE,
};
use crate::sim::movement::jumpjet_movement::JumpjetRuntime;
use crate::sim::movement::locomotor::MovementLayer;
use crate::sim::occupancy::RawCellKey;
use crate::sim::rng::SimRng;
use crate::util::fixed_math::SimFixed;
use crate::util::lepton::{
    GROUND_LEVEL_HEIGHT_LEPTONS, ground_height_leptons, lepton_to_cell_packed,
};

struct CruiseHost<'a> {
    /// The world as the frame began; the host's own fields carry what the
    /// frame changes.
    sim: &'a Simulation,
    frame: u32,
    trig: &'a TrigTable,
    atan: &'a AtanTable,
    rules: Option<&'a RuleSet>,
    registry: Option<&'a OverlayTypeRegistry>,
    kind: FlightOwnerKind,
    location: [i32; 3],
    on_bridge: bool,
    balloon_hover: bool,
    has_target: bool,
    piggyback_active: bool,
    simple_deployer: bool,
    deploy_to_land: bool,
    /// The owner's body (`+0x388`) as this frame reads it.
    body_facing: u16,
    /// The flight's `Set_Current` on the body, when it made one.
    snapped_body_facing: Option<u16>,
    /// The owner's Mark calls and grounded resets in the order the frame made
    /// them, each at the location it made them from. The commit replays them
    /// through the owner's Mark once the world is writable again.
    owner_ops: Vec<OwnerOp>,
    /// A Mark(REMOVE) has not been followed by a Mark(PUT) yet, so the
    /// owner's cell no longer lists it.
    owner_lifted: bool,
    /// The owner, as the air slots identify it.
    stable_id: u64,
    /// `ScenarioClass+0x218`, drawn only when a scatter actually happens so the
    /// RNG cursor advances exactly where the original's does. It is held
    /// outside `sim` for the frame.
    rng: &'a mut SimRng,
    /// Deferred `+0xE0` writes over `sim`'s air slots: `None` releases the
    /// cell, `Some(owner)` claims it. Applied after the kernel returns, which
    /// keeps the world borrowed immutably while the states run.
    slot_ops: Vec<((u16, u16), Option<u64>)>,
    /// The neighbour `Set_Destination` (vtable `+0x480`) was handed.
    scatter_to: Option<(i16, i16)>,
    /// State 4 refused the landing and called `Stop_Moving`.
    stop_requested: bool,
    /// Locomotor `+0x90`, the landing-admitted latch, in and out.
    landing_latched: bool,
    /// Touchdown ran, so the owner must be put back on the ground.
    touched_down: bool,
    /// Owner `+0x425`.
    crashing: bool,
    /// Owner `+0xB4` or `GetCurrentMission` is Enter (7).
    mission_enter: bool,
    /// `MapClass+0xF4/+0xF8`; no map size admits no cell.
    map_size: Option<(i32, i32)>,
    /// State 5 moved the owner through its Mark/display transaction.
    crash_relocated: bool,
    /// State 5 reached the ground: the owner's impact notice is due.
    impact: bool,
    /// The last `SetSpeedFraction` (owner vtable `+0x544`) of the frame, as the
    /// native double's bits.
    speed_fraction: Option<u64>,
    /// The crash latch engaged this frame.
    crash_latched: bool,
}

impl<'a> CruiseHost<'a> {
    fn terrain(&self) -> Option<&'a ResolvedTerrainGrid> {
        self.sim.resolved_terrain.as_ref()
    }

    /// The pending-aware view of a cell's air slot.
    fn slot_holder(&self, cell: (i16, i16)) -> Option<u64> {
        if cell.0 < 0 || cell.1 < 0 {
            return None;
        }
        let key = (cell.0 as u16, cell.1 as u16);
        match self.slot_ops.iter().rev().find(|(at, _)| *at == key) {
            Some((_, pending)) => *pending,
            None => self.sim.substrate.air_slots.holder(key.0, key.1),
        }
    }

    /// Level and slope of the cell holding `xy`, through the shared dummy cell
    /// for lookups outside the map (`MapClass::Get_CellClass_At_Coord @ 0x00565730`).
    fn cell_terrain(&self, xy: [i32; 2]) -> (u8, u8) {
        let Some(terrain) = self.terrain() else {
            return (0, 0);
        };
        match terrain.native_cell_identity(self.cell_of(xy)) {
            NativeCellIdentity::Real(index) => {
                let cell = &terrain.cells()[index];
                (cell.level, cell.slope_type)
            }
            NativeCellIdentity::Dummy => {
                let dummy = terrain.shared_cell_dummy().snapshot();
                (dummy.level as u8, dummy.slope_type)
            }
        }
    }
}

impl JumpjetFlightHost for CruiseHost<'_> {
    fn binary_frame(&self) -> u32 {
        self.frame
    }

    fn trig(&self) -> &TrigTable {
        self.trig
    }

    fn atan(&self) -> &AtanTable {
        self.atan
    }

    fn owner_kind(&self) -> FlightOwnerKind {
        self.kind
    }

    fn location(&self) -> [i32; 3] {
        self.location
    }

    fn set_location(&mut self, coord: [i32; 3]) {
        self.location = coord;
    }

    fn set_z(&mut self, z: i32) {
        self.location[2] = z;
    }

    fn height_above_ground(&self) -> i32 {
        let xy = [self.location[0], self.location[1]];
        self.location[2].wrapping_sub(
            ground_surface_z_at(xy, self.on_bridge, self.terrain(), None).unwrap_or(0),
        )
    }

    fn on_bridge(&self) -> bool {
        self.on_bridge
    }

    fn grounded_reset(&mut self) {
        self.owner_ops.push(OwnerOp::GroundedReset {
            location: self.location,
        });
        self.on_bridge = false;
    }

    fn floor_height(&self, xy: [i32; 2]) -> i32 {
        ground_surface_z_at(xy, false, self.terrain(), None).unwrap_or(0)
    }

    fn cell_high_bridge(&self, xy: [i32; 2]) -> bool {
        self.terrain().is_some_and(|terrain| {
            let cell = terrain.native_cell_identity(self.cell_of(xy));
            terrain.native_cell_flags(cell) & 0x100 != 0
        })
    }

    fn cell_top_height(&self, xy: [i32; 2]) -> i32 {
        let (level, slope) = self.cell_terrain(xy);
        let centre = ground_height_leptons(level, slope, 128, 128).unwrap_or(0);
        let (cx, cy) = self.cell_of(xy);
        if cx < 0 || cy < 0 {
            return jumpjet_flight::cell_top_height(centre, None, false);
        }
        let (rx, ry) = (cx as u16, cy as u16);
        // `BuildingTypeClass::Dimension2 @ 0x00464AF0`: art `Height=` (`+0xEF4`,
        // written at `0x00461101` from the ART key at `0x0081A7A8`) times
        // `g_HeightFactor` (`0x0089DDB8`). The startup chain `0x0045AFA0..
        // 0x0045B070` sets it to 104 under the native control word
        // (`tools/spatial_oracle/height_factor.json`), the same value as the
        // cell level height.
        let building_height = self
            .sim
            .substrate
            .occupancy
            .first_building_on_layer(rx, ry, MovementLayer::Ground)
            .map(|id| {
                self.sim
                    .substrate
                    .entities
                    .get(id)
                    .zip(self.rules)
                    .and_then(|(building, rules)| {
                        rules
                            .object(self.sim.interner.resolve(building.type_ref()))
                            .map(|object| rules.building_launch_height(object))
                    })
                    .unwrap_or(0)
                    .wrapping_mul(GROUND_LEVEL_HEIGHT_LEPTONS)
            });
        let any_techno = self
            .sim
            .substrate
            .occupancy
            .get(rx, ry)
            .is_some_and(|cell| {
                cell.iter_layer(MovementLayer::Ground)
                    .any(|object| !(self.owner_lifted && object.entity_id == self.stable_id))
            });
        jumpjet_flight::cell_top_height(centre, building_height, any_techno)
    }

    fn cell_land_type(&self, xy: [i32; 2]) -> u8 {
        self.terrain().map_or(0, |terrain| {
            match terrain.native_cell_identity(self.cell_of(xy)) {
                NativeCellIdentity::Real(index) => terrain.cells()[index].yr_cell_land_type,
                NativeCellIdentity::Dummy => 0,
            }
        })
    }

    fn balloon_hover(&self) -> bool {
        self.balloon_hover
    }

    fn has_target(&self) -> bool {
        self.has_target
    }

    fn piggyback_active(&self) -> bool {
        self.piggyback_active
    }

    fn piggyback_arrival(&mut self) {}

    fn simple_deployer(&self) -> bool {
        self.simple_deployer
    }

    fn type_flag_6ad(&self) -> bool {
        self.deploy_to_land
    }

    fn set_speed_fraction(&mut self, fraction_bits: u64) {
        self.speed_fraction = Some(fraction_bits);
    }

    fn arrival_notify(&mut self) {}

    fn snap_body_facing(&mut self, facing: u16) {
        self.body_facing = facing;
        self.snapped_body_facing = Some(facing);
    }

    fn hold_target_facing(&self) -> Option<u16> {
        None
    }

    fn cell_of(&self, xy: [i32; 2]) -> (i16, i16) {
        (lepton_to_cell_packed(xy[0]), lepton_to_cell_packed(xy[1]))
    }

    fn body_facing(&self) -> u16 {
        self.body_facing
    }

    fn random_direction(&mut self) -> u32 {
        self.rng.next_range_i32_inclusive(0, 7) as u32
    }

    fn air_slot_taken_at(&mut self, cell: (i16, i16)) -> bool {
        self.slot_holder(cell)
            .is_some_and(|held| held != self.stable_id)
    }

    fn holds_air_slot_at(&self, cell: (i16, i16)) -> bool {
        self.slot_holder(cell) == Some(self.stable_id)
    }

    fn air_slot_empty_at(&self, cell: (i16, i16)) -> bool {
        self.slot_holder(cell).is_none()
    }

    fn release_owner_air_slots(&mut self) {
        // Stands in for the cached-cell (`+0x560`) release: drop every cell
        // this owner still holds, so a claim cannot orphan when it drifts.
        let held: Vec<(u16, u16)> = self
            .sim
            .substrate
            .air_slots
            .entries()
            .filter(|(_, _, owner)| *owner == self.stable_id)
            .map(|(rx, ry, _)| (rx, ry))
            .collect();
        for cell in held {
            self.slot_ops.push((cell, None));
        }
    }

    fn claim_air_slot_at(&mut self, cell: (i16, i16)) {
        if cell.0 >= 0 && cell.1 >= 0 {
            self.slot_ops
                .push(((cell.0 as u16, cell.1 as u16), Some(self.stable_id)));
        }
    }

    fn release_air_slot_at(&mut self, cell: (i16, i16)) {
        if cell.0 >= 0 && cell.1 >= 0 {
            self.slot_ops.push(((cell.0 as u16, cell.1 as u16), None));
        }
    }

    fn set_destination_cell(&mut self, cell: (i16, i16)) {
        self.scatter_to = Some(cell);
    }

    fn can_enter_cell(&self, cell: (i16, i16)) -> i32 {
        // `vt+0x1AC(cell, -1, -1, NULL, 1)` at `0x0054C66D`: the owner's own
        // answer, with no direction, no height and no source cell.
        let answer = match self.rules {
            Some(rules) => self.sim.mover_can_enter(
                self.stable_id,
                cell,
                InfantryEntryArgs::REPAIR,
                rules,
                self.registry,
            ),
            None => Err("no rules".into()),
        };
        answer.map_or_else(
            |error| {
                // RESIDUAL: native always has an answer; here only a world
                // without rules, the owner's type or map cells lacks one. Such
                // an owner lands. A refusal would run `Stop_Moving`, whose
                // failed search kills a live owner with C4
                // (`0x0054B68F..0x0054B6D3`) and whose re-target only flies it
                // back into the same refusal.
                log::warn!("Jumpjet {} landing entry: {error}", self.stable_id);
                0
            },
            i32::from,
        )
    }

    fn sub_cell_free(&self, cell: (i16, i16), sub_cell: i32, bridge: bool) -> bool {
        if cell.0 < 0 || cell.1 < 0 || !(0..8).contains(&sub_cell) {
            return true;
        }
        let key = RawCellKey::Real(cell.0 as u16, cell.1 as u16);
        let layer = if bridge {
            MovementLayer::Bridge
        } else {
            MovementLayer::Ground
        };
        self.sim.substrate.raw_cell_occupation.bits_at(key, layer) & (1u8 << sub_cell) == 0
    }

    fn mission_is_seven(&self) -> bool {
        self.mission_enter
    }

    fn stop_moving(&mut self) -> i32 {
        // `Stop_Moving` is State 4's last act and needs the world's search, so
        // it runs once the frame is committed
        // (`Simulation::jumpjet_stop_moving`), lifting the descent into State 1
        // when it re-targets.
        self.stop_requested = true;
        STATE_DESCEND
    }

    fn cell_high_bridge_at(&self, cell: (i16, i16)) -> bool {
        self.terrain().is_some_and(|terrain| {
            terrain.native_cell_flags(terrain.native_cell_identity((cell.0, cell.1))) & 0x100 != 0
        })
    }

    fn landing_latched(&self) -> bool {
        self.landing_latched
    }

    fn begin_landing(&mut self, _destination: [i32; 3]) {
        self.landing_latched = true;
    }

    fn deploy_latched(&self) -> bool {
        // Owner `+0x134` has no VERA equivalent yet.
        false
    }

    fn deploy_facing(&self) -> Option<u16> {
        // `RulesClass+0x48` is not read yet, so a simple deployer keeps its
        // heading instead of turning to the deploy facing.
        None
    }

    fn touchdown(&mut self) {
        // The AircraftTracker removal (`0x0054C9DC`) is not made: see the
        // residual on `state0_ground`.
        self.touched_down = true;
        let here = self.cell_of([self.location[0], self.location[1]]);
        if self.holds_air_slot_at(here) {
            self.release_air_slot_at(here);
        }
    }

    fn crashing(&self) -> bool {
        self.crashing
    }

    fn in_bounds(&self, cell: (i16, i16)) -> bool {
        self.map_size.is_some_and(|(width, height)| {
            crate::map::playfield::size_diamond_contains(width, height, cell)
        })
    }

    fn crash_relocate(&mut self, coord: [i32; 3]) {
        // The display resubmission follows the replayed Mark(PUT).
        self.mark(false);
        self.location = coord;
        self.mark(true);
        self.crash_relocated = true;
    }

    fn mark(&mut self, put: bool) {
        self.owner_lifted = !put;
        self.owner_ops.push(OwnerOp::Mark {
            put,
            location: self.location,
        });
    }

    fn crash_impact(&mut self) {
        // The tracker removal and the notice are the owner's; the object turn
        // runs them once the frame's states are committed.
        self.impact = true;
    }

    fn crash_latched(&mut self) {
        // The Infantry owner's Do_Action is applied once the frame's states
        // are committed; it touches nothing State 5 reads.
        self.crash_latched = true;
    }
}

/// An owner call the kernel made over the frozen world, replayed on commit.
#[derive(Clone, Copy)]
enum OwnerOp {
    /// `Mark` (vtable `+0x124`) from `location`.
    Mark { put: bool, location: [i32; 3] },
    /// Update's grounded reset (`0x0054D407..0x0054D438`): the owner's raw
    /// REMOVE receiver (vtable `+0xF4`) on `location`, then `+0x8C = 0`.
    GroundedReset { location: [i32; 3] },
}

/// Every Jumpjet the native locomotor owns - all of them, airborne or landed.
/// `Process 0x0054AEC0` itself decides whether a frame does anything, so an
/// idle landed or idle holding owner is advanced by nothing.
fn jumpjet_locomotor(entity: &crate::sim::game_entity::GameEntity) -> bool {
    entity.locomotor.as_ref().is_some_and(|locomotor| {
        locomotor.kind == LocomotorKind::Jumpjet && locomotor.jumpjet_runtime().is_some()
    })
}

/// What the kernel asked the world to do, collected while the substrate was
/// still borrowed immutably.
struct HostEffects {
    moving: bool,
    /// The destination the frame flew toward, which is what the locomotor keeps
    /// at `+0x40` — not the owner's new position.
    destination: [i32; 3],
    /// The state the frame began in, so the caller can see a cruise end.
    entry_state: i32,
    /// The flight's `Set_Current` on the body, when it made one.
    body_facing: Option<u16>,
    owner_ops: Vec<OwnerOp>,
    height: i32,
    slot_ops: Vec<((u16, u16), Option<u64>)>,
    scatter_to: Option<(i16, i16)>,
    stop_requested: bool,
    landing_latched: bool,
    touched_down: bool,
    crash_relocated: bool,
    impact: bool,
    speed_fraction: Option<u64>,
    crash_latched: bool,
}

impl Simulation {
    /// One `Process @ 0x0054AEC0` frame for a Jumpjet: the Update gate, then
    /// the state at `+0x50`. Answers `None` for any other locomotor, so the air
    /// adapter keeps every other flier.
    pub(crate) fn tick_jumpjet_cruise_one(
        &mut self,
        stable_id: u64,
        rules: Option<&RuleSet>,
        registry: Option<&OverlayTypeRegistry>,
    ) -> Option<AirMovementTickStats> {
        let frame = self.session.binary_frame;
        if !self
            .substrate
            .entities
            .get(stable_id)
            .is_some_and(jumpjet_locomotor)
        {
            return None;
        }
        self.apply_jumpjet_adapter_order(stable_id, rules, registry);
        // The host lends the whole world immutably, so the scenario stream it
        // draws scatters from is held outside it for the frame.
        let mut rng = std::mem::replace(&mut self.scenario_rng, SimRng::vacant());
        let run = self.run_jumpjet_process(stable_id, rules, registry, &mut rng);
        self.scenario_rng = rng;
        let (state, flight, location, effects) = run?;

        for (cell, owner) in &effects.slot_ops {
            match owner {
                Some(owner) => {
                    self.substrate.air_slots.claim(cell.0, cell.1, *owner);
                }
                None => self.substrate.air_slots.release(cell.0, cell.1),
            }
        }

        // Update's Mark bracket and grounded reset and State 5's Mark pair, in
        // the frame's order, each from the location it was made at. Mark
        // leaves the AircraftTracker alone; the impact removes a wreck from it.
        for op in effects.owner_ops {
            match op {
                OwnerOp::Mark { put, location } => {
                    foot_set_location(
                        &mut self.substrate.entities,
                        stable_id,
                        crate::sim::components::DriveCoord {
                            x: location[0],
                            y: location[1],
                            z: location[2],
                        },
                        rules,
                        &self.interner,
                    );
                    if put {
                        self.foot_mark_put(stable_id, rules, registry);
                    } else {
                        self.foot_mark_remove(stable_id, rules, registry);
                    }
                }
                OwnerOp::GroundedReset { location } => {
                    self.object_raw_receiver_at(
                        stable_id,
                        crate::sim::components::DriveCoord {
                            x: location[0],
                            y: location[1],
                            z: location[2],
                        },
                        false,
                    );
                    self.substrate.entities.get_mut(stable_id)?.on_bridge = false;
                }
            }
        }
        foot_set_location(
            &mut self.substrate.entities,
            stable_id,
            crate::sim::components::DriveCoord {
                x: location[0],
                y: location[1],
                z: location[2],
            },
            rules,
            &self.interner,
        );
        let entity = self.substrate.entities.get_mut(stable_id)?;
        // `FootClass::SetSpeedFraction @ 0x004D3710` on the owner, which the
        // Infantry fire error (`0x0051C9B8`), its locomotion action AI and
        // GetCurrentSpeed's shot lead read.
        if let Some(bits) = effects.speed_fraction {
            entity.foot_speed.set_speed_fraction_native_bits(bits);
        }
        if let Some(facing) = effects.body_facing {
            entity.body_facing.snap(facing, frame);
        }
        if effects.crash_relocated {
            // The display resubmission after State 5's Mark(PUT).
            self.submit_entity_display(stable_id, rules, None);
        }

        // Jumpjet54C8F0 calls PerCell(2) only at accepted touchdown, before
        // clearing the destination; cruise coordinate changes do not. The
        // object turn runs it from `touched_down`, after this commit.
        // RESIDUAL: so it follows the destination clear, the crash reset and
        // the Fly cell-list re-add rather than preceding them. No ported
        // per-cell step reads those.
        let entity = self.substrate.entities.get_mut(stable_id)?;
        let mut moving = effects.moving;
        // `Set_Destination` after a scatter re-aims the owner at the neighbour.
        if let Some(cell) = effects.scatter_to
            && cell.0 >= 0
            && cell.1 >= 0
        {
            let target = (cell.0 as u16, cell.1 as u16);
            entity.movement_target = Some(crate::sim::components::MovementTarget {
                path: vec![target],
                path_layers: vec![MovementLayer::Air],
                next_index: 0,
                final_goal: Some(target),
                ..Default::default()
            });
            moving = true;
        }
        // A cruise that leaves State 3 for the hold or the descent has reached
        // the ordered cell: the order is done, and what follows needs no goal.
        let ended_cruise =
            effects.entry_state == STATE_TRANSLATE && matches!(state, STATE_HOLD | STATE_DESCEND);
        if effects.touched_down || ended_cruise {
            entity.movement_target = None;
        }
        if effects.touched_down {
            moving = false;
            // `0x0054CA12`: touchdown ends a crash the latch never caught.
            entity.crashing = false;
        }
        let arrived = effects.touched_down || ended_cruise;

        let locomotor = entity.locomotor.as_mut()?;
        locomotor.altitude = SimFixed::from_num(effects.height.max(0));
        if let Some(runtime) = locomotor.jumpjet_runtime_mut() {
            runtime.flight = flight;
            runtime.phase = state;
            runtime.moving = moving;
            runtime.landing_latched = effects.landing_latched && !effects.touched_down;
            runtime.destination = if moving {
                crate::sim::components::DriveCoord {
                    x: effects.destination[0],
                    y: effects.destination[1],
                    z: effects.destination[2],
                }
            } else {
                JumpjetRuntime::NULL
            };
        }
        if effects.stop_requested {
            // State 4 refused the landing and called `Stop_Moving` as its last
            // act, so it runs on the committed frame. Its re-target is the
            // order the adapter follows from here.
            let speed = self.jumpjet_order_speed(stable_id, rules);
            self.jumpjet_stop_moving(stable_id, rules, registry);
            self.publish_jumpjet_destination(stable_id, speed);
        }
        // The crash latch's AirDeathStart for an Infantry owner (`0x0054B02C`).
        if effects.crash_latched
            && let Some(rules) = rules
            && self
                .substrate
                .entities
                .get(stable_id)
                .is_some_and(|entity| entity.category == EntityCategory::Infantry)
            && let Err(cause) = self.infantry_do_action(
                stable_id,
                crate::sim::movement::infantry_action::DO_AIR_DEATH_START,
                false,
                rules,
            )
        {
            log::debug!("infantry {stable_id} AirDeathStart: {cause}");
        }
        Some(AirMovementTickStats {
            arrivals: u32::from(arrived),
            impact: effects.impact,
            touched_down: effects.touched_down,
        })
    }

    /// The kernel's frame (`jumpjet_flight::process`) over the world as the
    /// frame began, drawing from `rng` for its scatters. What the frame changes
    /// comes back for the caller to commit. `None` for an owner without a
    /// Jumpjet runtime.
    fn run_jumpjet_process(
        &self,
        stable_id: u64,
        rules: Option<&RuleSet>,
        registry: Option<&OverlayTypeRegistry>,
        rng: &mut SimRng,
    ) -> Option<(i32, jumpjet_flight::JumpjetFlight, [i32; 3], HostEffects)> {
        let frame = self.session.binary_frame;
        let terrain = self.resolved_terrain.as_ref();
        let entity = self.substrate.entities.get(stable_id)?;
        let locomotor = entity.locomotor.as_ref()?;
        let runtime = locomotor.jumpjet_runtime()?;
        let moving = runtime.moving;
        let state = runtime.phase;
        let destination = [
            runtime.destination.x,
            runtime.destination.y,
            runtime.destination.z,
        ];

        let xy = position_world_xy(&entity.position);
        let z = crate::sim::movement::ground_pose::object_world_z_leptons(entity, terrain);
        let object = rules.and_then(|rules| rules.object(self.interner.resolve(entity.type_ref())));
        let (trig, _) = required_math_tables();
        let mut host = CruiseHost {
            sim: self,
            frame,
            trig,
            atan: required_atan_table(),
            rules,
            registry,
            kind: FlightOwnerKind::of(entity.category),
            location: [xy[0], xy[1], z],
            on_bridge: entity.on_bridge,
            balloon_hover: locomotor.balloon_hover,
            has_target: entity.attack_target.is_some(),
            piggyback_active: entity.foot_locomotor_swap_active,
            simple_deployer: object.is_some_and(|object| object.is_simple_deployer),
            deploy_to_land: object.is_some_and(|object| object.deploy_to_land),
            body_facing: entity.body_facing_current(frame),
            snapped_body_facing: None,
            owner_ops: Vec::new(),
            owner_lifted: false,
            stable_id,
            rng,
            slot_ops: Vec::new(),
            scatter_to: None,
            stop_requested: false,
            landing_latched: runtime.landing_latched,
            touched_down: false,
            crashing: entity.crashing,
            mission_enter: crate::sim::movement::jumpjet_movement::owner_mission_is_enter(entity),
            map_size: self.map_size_diamond(),
            crash_relocated: false,
            impact: false,
            speed_fraction: None,
            crash_latched: false,
        };

        let params = runtime.params;
        let mut flight = runtime.flight;
        let entry_state = state;
        let state =
            jumpjet_flight::process(moving, state, destination, &params, &mut flight, &mut host);
        let height = host.height_above_ground();
        let effects = HostEffects {
            moving,
            destination,
            entry_state,
            body_facing: host.snapped_body_facing,
            owner_ops: host.owner_ops,
            height,
            slot_ops: host.slot_ops,
            scatter_to: host.scatter_to,
            stop_requested: host.stop_requested,
            landing_latched: host.landing_latched,
            touched_down: host.touched_down,
            crash_relocated: host.crash_relocated,
            impact: host.impact,
            speed_fraction: host.speed_fraction,
            crash_latched: host.crash_latched,
        };
        Some((state, flight, host.location, effects))
    }

    /// Cell orders reach a Jumpjet through Foot's setter
    /// ([`Simulation::jumpjet_cell_destination`]). An object order on the
    /// ground route, a scatter's re-aim and the orders that drop a goal still
    /// hand it a goal cell rather than calling Foot's `Set_Destination`, so
    /// before `Process` the two native entries are applied from it:
    /// - a goal the locomotor is not flying to is a fresh
    ///   `Set_Destination(cell)`: Foot stores the NavCom and hands `Move_To`
    ///   the cell's `GetCoords` (`0x004D94B0`), and the goal then names the
    ///   cell `Move_To` chose;
    /// - a goal dropped while the owner climbs or cruises with its NavCom
    ///   still set (an Attack order and the attack approach drop only the
    ///   goal) is a null `Set_Destination`, whose Foot arm runs `Stop_Moving`
    ///   ([`Simulation::jumpjet_null_destination`]) and clears the NavCom, so
    ///   it is applied once.
    ///
    /// A wreck takes no orders.
    fn apply_jumpjet_adapter_order(
        &mut self,
        id: u64,
        rules: Option<&RuleSet>,
        registry: Option<&OverlayTypeRegistry>,
    ) {
        enum Order {
            MoveTo((u16, u16)),
            Stop,
        }
        let order = {
            let Some(entity) = self.substrate.entities.get(id) else {
                return;
            };
            let Some(runtime) = entity
                .locomotor
                .as_ref()
                .and_then(|locomotor| locomotor.jumpjet_runtime())
            else {
                return;
            };
            if entity.crashing {
                return;
            }
            match entity.movement_target.as_ref().and_then(|t| t.final_goal) {
                Some(goal) => {
                    let flying_to = (
                        lepton_to_cell_packed(runtime.destination.x),
                        lepton_to_cell_packed(runtime.destination.y),
                    );
                    if runtime.moving
                        && runtime.destination != JumpjetRuntime::NULL
                        && flying_to == (goal.0 as i16, goal.1 as i16)
                    {
                        return;
                    }
                    Order::MoveTo(goal)
                }
                None if runtime.moving
                    && matches!(runtime.phase, STATE_ASCEND | STATE_TRANSLATE)
                    && entity.navigation.nav_com.is_some() =>
                {
                    Order::Stop
                }
                None => return,
            }
        };
        match order {
            Order::MoveTo(goal) => {
                let speed = self.jumpjet_order_speed(id, rules);
                let request = crate::sim::movement::target_cell_coord(
                    goal.0,
                    goal.1,
                    self.resolved_terrain.as_ref(),
                );
                if let Some(entity) = self.substrate.entities.get_mut(id) {
                    entity.navigation.nav_com =
                        Some(crate::sim::components::NavTargetRef::cell(goal.0, goal.1));
                    entity.navigation.nav_com_aux = None;
                }
                self.jumpjet_move_to(id, request, rules);
                let moving = self.substrate.entities.get(id).is_some_and(|entity| {
                    entity
                        .locomotor
                        .as_ref()
                        .and_then(|locomotor| locomotor.jumpjet_runtime())
                        .is_some_and(|runtime| runtime.moving)
                });
                if moving {
                    self.publish_jumpjet_destination(id, speed);
                } else if let Some(entity) = self.substrate.entities.get_mut(id) {
                    // A refused Move_To leaves the owner where it was; the goal
                    // retires instead of being retried every frame.
                    entity.movement_target = None;
                }
            }
            Order::Stop => {
                self.jumpjet_null_destination(id, rules, registry);
                if let Some(entity) = self.substrate.entities.get_mut(id) {
                    crate::sim::movement::DestinationTiming::from_rules(
                        self.session.binary_frame,
                        rules,
                    )
                    .accept(entity);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::jumpjet_params::JumpjetParams;
    use crate::sim::components::MovementTarget;
    use crate::sim::game_entity::GameEntity;
    use crate::sim::movement::ground_pose::position_world_coord;
    use crate::sim::movement::locomotor::LocomotorState;
    use serde_json::Value;

    fn retail_tables_present() -> bool {
        let (trig, _) = required_math_tables();
        if !trig.matches_retail() || !required_atan_table().matches_retail() {
            // With RA2_DIR set, a mismatched table is a failure, not a skip.
            assert!(
                std::env::var_os("RA2_DIR").is_none(),
                "RA2_DIR is set but the retail sine or atan table does not match"
            );
            eprintln!("skipped: set RA2_DIR to the retail install to run this");
            return false;
        }
        true
    }

    fn native_row(name: &str) -> Value {
        let rows: Value = serde_json::from_str(include_str!(
            "../../../tools/spatial_oracle/jumpjet_flight.json"
        ))
        .expect("corpus parses");
        rows.as_array()
            .expect("rows")
            .iter()
            .find(|row| row["name"] == name)
            .cloned()
            .expect("named row")
    }

    /// A Unit Jumpjet with the corpus BASE type block, fresh from `link`
    /// (state 0, locomotor facing `0x4000`), hovering at 500 at cell (10,10)
    /// with a move to cell (16,10). The terrain is the oracle fixture: level 0
    /// and flat, except cell (9,10) at level 2 with slope 1.
    fn hovering_jumpjet(body_facing: u8) -> Simulation {
        let mut sim = Simulation::new();
        super::super::lifecycle_tests::install_common_raw_terrain(&mut sim, 24, 16, 0, None);
        // A playfield holding the corpus corridor (x 6..20, y 9..11), for the
        // orders' searches and `In_Bounds`.
        sim.playfield_bounds = Some(crate::sim::cell_rect::PlayfieldBounds {
            base: 12,
            off_fc: 0,
            off_100: 0,
            off_104: 16,
            off_108: 16,
        });
        sim.playfield_size_height = Some(16);
        {
            let cell = sim
                .resolved_terrain
                .as_mut()
                .and_then(|terrain| terrain.cell_mut(9, 10))
                .expect("fixture cell");
            cell.level = 2;
            cell.slope_type = 1;
        }
        let mut entity = GameEntity::test_default(1, "JUMPJETUNIT", "Americans", 10, 10);
        entity.category = EntityCategory::Unit;
        entity.body_facing.snap(u16::from(body_facing) << 8, 0);
        entity.position.sub_x = SimFixed::from_num(128);
        entity.position.sub_y = SimFixed::from_num(128);
        let mut locomotor = LocomotorState::for_test_kind(LocomotorKind::Jumpjet);
        locomotor
            .jumpjet_runtime_mut()
            .expect("jumpjet runtime")
            .link(&JumpjetParams {
                turn_rate: 4,
                speed: SimFixed::from_num(14),
                climb: 5.0,
                crash: 5.0,
                height: 500,
                accel: 2.0,
                wobbles: 0.15,
                deviation: 40,
                no_wobbles: false,
            });
        {
            let runtime = locomotor.jumpjet_runtime_mut().expect("jumpjet runtime");
            // The cruise rows begin mid-flight, which is what `Move_To` leaves
            // behind: State 3 with the moving byte set, and State 0's seed
            // already applied (locomotor facing snapped to the body, both speed
            // doubles and the bob zero, `+0x80` at `JumpjetHeight=`).
            runtime.phase = STATE_TRANSLATE;
            runtime.moving = true;
            runtime.flight.facing.snap(u16::from(body_facing) << 8, 0);
            runtime.flight.target_height = 500;
        }
        locomotor.altitude = SimFixed::from_num(500);

        entity.locomotor = Some(locomotor);
        entity.movement_target = Some(MovementTarget {
            path: vec![(16, 10)],
            path_layers: vec![MovementLayer::Air],
            next_index: 0,
            speed: SimFixed::from_num(14),
            final_goal: Some((16, 10)),
            ..Default::default()
        });
        sim.substrate.entities.insert(entity);
        sim
    }

    /// Park a Jumpjet with no order in a given native state and run the tick.
    fn idle_for(state: i32, balloon_hover: bool, frames: u32) -> Simulation {
        let mut sim = hovering_jumpjet(0x40);
        {
            let entity = sim.substrate.entities.get_mut(1).expect("jumpjet");
            entity.movement_target = None;
            let locomotor = entity.locomotor.as_mut().expect("locomotor");
            locomotor.balloon_hover = balloon_hover;
            locomotor.altitude = SimFixed::from_num(if state == jumpjet_flight::STATE_GROUND {
                0
            } else {
                500
            });
            let runtime = locomotor.jumpjet_runtime_mut().expect("runtime");
            runtime.phase = state;
            runtime.moving = false;
            runtime.destination = JumpjetRuntime::NULL;
        }
        for frame in 0..frames {
            sim.session.binary_frame = 1001 + frame;
            sim.tick_air_movement_with_cell_lists_one(1, None, None);
        }
        sim
    }

    /// A Jumpjet unit lifting off leaves its takeoff cell once it climbs past
    /// twice the level height: Update takes it off the map for its body and
    /// puts it back (`0x0054D12C` / `0x0054D6A6`), and that Mark(PUT) finds it
    /// in the Air layer (`0x0054B8D0`), so no ground list holds it and its
    /// 0x20 is gone (`0x00744210`).
    #[test]
    fn a_lifting_jumpjet_unit_leaves_its_takeoff_cell() {
        let mut sim = hovering_jumpjet(0x40);
        {
            let entity = sim.substrate.entities.get_mut(1).expect("jumpjet");
            entity.foot_occupation_enabled = true;
            entity.position.exact_z_leptons = Some(0);
            let locomotor = entity.locomotor.as_mut().expect("locomotor");
            locomotor.altitude = SimFixed::from_num(0);
            let runtime = locomotor.jumpjet_runtime_mut().expect("runtime");
            runtime.phase = jumpjet_flight::STATE_GROUND;
            runtime.moving = false;
            runtime.destination = JumpjetRuntime::NULL;
        }
        sim.add_entity_occupancy(1);
        assert!(sim.substrate.occupancy.contains_entity(10, 10, 1));
        assert_eq!(sim.substrate.raw_cell_occupation.ground_bits(10, 10), 0x20);
        for frame in 0..80 {
            sim.session.binary_frame = 1001 + frame;
            sim.tick_air_movement_with_cell_lists_one(1, None, None);
        }
        let entity = sim.substrate.entities.get(1).expect("jumpjet");
        assert!(entity.position.exact_z_leptons.unwrap() >= 208);
        let here = (entity.position.rx, entity.position.ry);
        for (rx, ry) in [(10, 10), here] {
            assert!(!sim.substrate.occupancy.contains_entity(rx, ry, 1));
            assert_eq!(sim.substrate.raw_cell_occupation.ground_bits(rx, ry), 0);
        }
    }

    /// Before the locomotor owned this tick, VERA's air adapter cycled an idle
    /// non-balloon Jumpjet Ascending -> Hovering -> Descending -> Landed every
    /// 102 frames. `Process 0x0054AEC0` runs Update only while the moving byte
    /// is set or the state is not ground/hold, and State 0 leaves the ground
    /// only when moving, so an idle landed owner does nothing at all.
    #[test]
    fn an_idle_landed_jumpjet_never_leaves_the_ground() {
        let sim = idle_for(jumpjet_flight::STATE_GROUND, false, 400);
        let entity = sim.substrate.entities.get(1).expect("jumpjet");
        let locomotor = entity.locomotor.as_ref().expect("locomotor");
        assert_eq!(locomotor.altitude, SimFixed::from_num(0));
        assert_eq!(
            locomotor.jumpjet_runtime().map(|runtime| runtime.phase),
            Some(jumpjet_flight::STATE_GROUND)
        );
        assert_eq!((entity.position.rx, entity.position.ry), (10, 10));
    }

    /// Re-opening a cruise must not leave a claim behind on the cell the owner
    /// drifted out of.
    ///
    /// Native State 2 releases the owner's cached cell (`+0x560`) at
    /// `0x0054BF75..0x0054BFA8` before releasing the current one. The drift is
    /// real: State 1 sets the target speed before promoting to the hold, so the
    /// owner leaves the cell it claimed — the native corpus shows a claim on
    /// one cell and a release on another a frame later. VERA keeps no cached
    /// cell, so it drops every slot the owner still holds. Without this the
    /// orphaned claim would block that cell against every later hoverer and
    /// grow the hashed, snapshotted grid without bound.
    ///
    /// The corpus cannot witness this: its declared substitution pins `+0x560`,
    /// so the original skips the cached-cell release there. Hence a regression
    /// test rather than a parity row.
    #[test]
    fn re_opening_a_cruise_drops_a_drifted_slot() {
        let mut sim = hovering_jumpjet(0x40);
        // The owner holds (13,10) but sits at (10,10) — the drift case.
        assert!(sim.substrate.air_slots.claim(13, 10, 1));
        {
            let locomotor = sim
                .substrate
                .entities
                .get_mut(1)
                .and_then(|entity| entity.locomotor.as_mut())
                .expect("locomotor");
            let runtime = locomotor.jumpjet_runtime_mut().expect("runtime");
            runtime.phase = STATE_HOLD;
            runtime.moving = true;
        }
        sim.session.binary_frame = 1001;
        sim.tick_air_movement_with_cell_lists_one(1, None, None);

        assert_eq!(
            sim.substrate.air_slots.holder(13, 10),
            None,
            "the claim on the cell the owner drifted out of must not leak"
        );
        assert_eq!(
            sim.substrate
                .entities
                .get(1)
                .and_then(|entity| entity.locomotor.as_ref())
                .and_then(|locomotor| locomotor.jumpjet_runtime())
                .map(|runtime| runtime.phase),
            Some(STATE_TRANSLATE),
            "a destination elsewhere re-opens the cruise"
        );
    }

    /// `Is_Moving_Now 0x0054D0D0` answers false for the hold too, so a parked
    /// balloon with no order neither bobs nor drifts.
    #[test]
    fn an_idle_hold_without_a_move_is_inert() {
        let sim = idle_for(STATE_HOLD, true, 400);
        let entity = sim.substrate.entities.get(1).expect("jumpjet");
        let locomotor = entity.locomotor.as_ref().expect("locomotor");
        assert_eq!(
            locomotor.jumpjet_runtime().map(|runtime| runtime.phase),
            Some(STATE_HOLD)
        );
        assert_eq!((entity.position.rx, entity.position.ry), (10, 10));
    }

    /// Fly a native row through the production air tick, comparing the owner
    /// position and exact Z every frame, until arrival hands it to descent.
    fn fly_native_row(name: &str, body_facing: u8) {
        let row = native_row(name);
        let frames = row["output"]["frames"].as_array().expect("frames");
        let mut sim = hovering_jumpjet(body_facing);
        for (index, expected) in frames.iter().enumerate() {
            sim.session.binary_frame = 1001 + index as u32;
            let stats = sim.tick_air_movement_with_cell_lists_one(1, None, None);
            let entity = sim.substrate.entities.get(1).expect("jumpjet");
            let coord = position_world_coord(&entity.position);
            let native: Vec<i64> = expected["coord"]
                .as_array()
                .expect("coord")
                .iter()
                .map(|value| value.as_i64().expect("int"))
                .collect();
            assert_eq!(
                vec![i64::from(coord.x), i64::from(coord.y), i64::from(coord.z)],
                native,
                "{name}: frame {index}"
            );
            assert_eq!(
                u32::from(entity.body_facing_byte(sim.session.binary_frame)),
                (expected["body_facing"].as_u64().expect("body facing") >> 8) as u32,
                "{name}: body facing, frame {index}"
            );
            // `SetSpeedFraction` reaches Foot `+0x578`: the frame's last native
            // fraction, clamped to [0, 1] and truncated to `SimFixed`.
            if let Some(bits) = expected["speed_fractions"]
                .as_array()
                .and_then(|fractions| fractions.last())
            {
                let value = f64::from_bits(bits.as_u64().expect("fraction bits"));
                let truncated = if value.is_nan() || value <= 0.0 {
                    0
                } else if value >= 1.0 {
                    1 << 16
                } else {
                    (value * 65536.0).floor() as i32
                };
                assert_eq!(
                    entity.foot_speed.applied_fraction().to_bits(),
                    truncated,
                    "{name}: speed fraction, frame {index}"
                );
            }
            let last = index + 1 == frames.len();
            assert_eq!(stats.arrivals, u32::from(last), "{name}: frame {index}");
        }
        let entity = sim.substrate.entities.get(1).expect("jumpjet");
        assert!(entity.movement_target.is_none());
        let locomotor = entity.locomotor.as_ref().expect("locomotor");
        assert_eq!(
            locomotor.jumpjet_runtime().map(|runtime| runtime.phase),
            Some(STATE_DESCEND)
        );
    }

    /// Production parity for the native `east_cruise` row, owner already facing
    /// east.
    #[test]
    fn production_cruise_flies_the_native_east_cruise_frames() {
        if retail_tables_present() {
            fly_native_row("east_cruise", 0x40);
        }
    }

    /// Production parity for `east_from_west_facing`: the takeoff seed snaps the
    /// locomotor facing to the body facing (west), so the cruise turns around
    /// from there instead of starting from the linked `0x4000`.
    #[test]
    fn production_cruise_turns_from_the_body_facing() {
        if retail_tables_present() {
            fly_native_row("east_from_west_facing", 0xC0);
        }
    }

    /// An order dropped mid-cruise (an Attack order, the attack approach at
    /// range) is Foot's null `Set_Destination`: `Stop_Moving` re-targets the
    /// cell under the owner and keeps the moving byte, so it flies on to that
    /// cell's centre and lands there, and the NavCom its `Move_To` wrote is
    /// cleared again. It is applied once: the next frame finds no NavCom.
    #[test]
    fn a_dropped_order_stops_at_the_cell_under_the_owner() {
        let mut sim = hovering_jumpjet(0x40);
        for frame in 0..20 {
            sim.session.binary_frame = 1001 + frame;
            sim.tick_air_movement_with_cell_lists_one(1, None, None);
        }
        let runtime = |sim: &Simulation| {
            sim.substrate
                .entities
                .get(1)
                .and_then(|entity| entity.locomotor.as_ref())
                .and_then(|locomotor| locomotor.jumpjet_runtime())
                .cloned()
                .expect("runtime")
        };
        let flying = runtime(&sim);
        assert_eq!(flying.phase, STATE_TRANSLATE);
        assert_eq!(
            flying.destination,
            crate::sim::components::DriveCoord {
                x: 16 * 256 + 128,
                y: 10 * 256 + 128,
                z: 0
            },
            "the order's Move_To placed the owner at the ordered cell's floor"
        );
        let entity = sim.substrate.entities.get_mut(1).expect("jumpjet");
        assert!(entity.navigation.nav_com.is_some());
        let here = (entity.position.rx, entity.position.ry);
        assert!(here.0 < 16, "still on its way");
        entity.movement_target = None;

        sim.session.binary_frame = 1021;
        sim.tick_air_movement_with_cell_lists_one(1, None, None);
        let stopped = runtime(&sim);
        assert!(stopped.moving, "Stop_Moving keeps the moving byte");
        assert_eq!(
            stopped.destination,
            crate::sim::components::DriveCoord {
                x: i32::from(here.0) * 256 + 128,
                y: i32::from(here.1) * 256 + 128,
                z: 0
            }
        );
        let entity = sim.substrate.entities.get(1).expect("jumpjet");
        assert!(entity.navigation.nav_com.is_none());
        assert!(entity.movement_target.is_none());

        for frame in 0..400 {
            sim.session.binary_frame = 1022 + frame;
            sim.tick_air_movement_with_cell_lists_one(1, None, None);
        }
        let landed = runtime(&sim);
        assert_eq!(landed.phase, jumpjet_flight::STATE_GROUND);
        assert!(!landed.moving);
        let entity = sim.substrate.entities.get(1).expect("jumpjet");
        assert_eq!((entity.position.rx, entity.position.ry), here);
        assert_eq!(entity.position.exact_z_leptons, Some(0));
    }

    /// State 4 admits a landing through the owner's own `Can_Enter_Cell`
    /// (`vt+0x1AC` at `0x0054C66D`) on the destination's cell, with no
    /// direction, no height and no source cell. A Unit landing on the cell
    /// centre passes the sub-cell gate (`0x0054C6BE`). Then 0 and 1 admit and
    /// set the landing latch; 2 refuses until the latch is set, and anything
    /// above 2 refuses, latched or not (`0x0054C6FD..0x0054C731`). A refusal
    /// runs `Stop_Moving`, which withdraws the latch and lifts the descent
    /// back into State 1.
    #[test]
    fn a_descent_lands_only_where_the_owners_can_enter_cell_admits() {
        use crate::sim::movement::fresh_oracle_seam::{self, FreshCallRecord};
        let rules =
            RuleSet::from_ini(&crate::rules::ini_parser::IniFile::from_str("")).expect("rules");
        for (code, latched, admitted) in [
            (0, false, true),
            (1, false, true),
            (2, false, false),
            (2, true, true),
            (3, false, false),
            (3, true, false),
            (7, false, false),
        ] {
            let mut sim = hovering_jumpjet(0x40);
            // The owner's type is read through the world's interner once
            // rules are present.
            sim.interner = crate::sim::intern::test_interner();
            {
                let entity = sim.substrate.entities.get_mut(1).expect("jumpjet");
                entity.movement_target = None;
                let runtime = entity
                    .locomotor
                    .as_mut()
                    .and_then(|locomotor| locomotor.jumpjet_runtime_mut())
                    .expect("runtime");
                runtime.phase = STATE_DESCEND;
                runtime.moving = true;
                runtime.landing_latched = latched;
                runtime.destination = crate::sim::components::DriveCoord {
                    x: 10 * 256 + 128,
                    y: 10 * 256 + 128,
                    z: 0,
                };
            }
            sim.session.binary_frame = 1001;
            fresh_oracle_seam::install(vec![code], Vec::new());
            sim.tick_air_movement_with_cell_lists_one(1, Some(&rules), None);
            let (records, unused) = fresh_oracle_seam::finish();
            assert_eq!(
                records,
                [FreshCallRecord::CanEnter {
                    cell: (10, 10),
                    direction: -1,
                    height: -1,
                    code,
                }],
                "code {code}, latched {latched}"
            );
            assert_eq!(unused, 0, "code {code}, latched {latched}");
            let runtime = sim
                .substrate
                .entities
                .get(1)
                .and_then(|entity| entity.locomotor.as_ref())
                .and_then(|locomotor| locomotor.jumpjet_runtime())
                .expect("runtime");
            assert_eq!(runtime.landing_latched, admitted, "code {code}");
            let phase = if admitted {
                STATE_DESCEND
            } else {
                STATE_ASCEND
            };
            assert_eq!(runtime.phase, phase, "code {code}");
        }
    }

    /// On retail Dustbowl, a tank is placed on a Night Hawk's ordered cell
    /// while the Night Hawk descends onto it. The Night Hawk's own
    /// `Can_Enter_Cell` refuses the landing (`0x0054C66D`), so it never lands
    /// on the tank. It does not land beside it either:
    /// - Every Update outside the hold and the cruise takes the owner off the
    ///   map (`0x0054D0FF..0x0054D12C`). `ObjectClass::Mark` clears `+0x74`
    ///   (`0x005F5913`) before the layer query, which then answers Ground
    ///   (`0x0054B8D0`).
    /// - So `MapClass::Pick_Up`'s `RemoveContent` runs the Unit receiver
    ///   (`0x0047EB62..0x0047EB89`, `0x00744210`) on the cell below. The Night
    ///   Hawk is not listed there, but the receiver clears the tank's `0x20`.
    /// - `Stop_Moving`'s search (`0x0054B5DF`) reads only that plane
    ///   (`0x004834A0`), so it takes the tank's own cell.
    ///
    /// The Night Hawk hovers over the tank until it drives off, then lands
    /// there. Established by instruction reading. Before #931 ported Update's
    /// Mark bracket, the bit survived and the Night Hawk landed beside the tank.
    #[test]
    #[ignore = "requires a retail RA2/YR install (RA2_DIR or config.toml)"]
    fn retail_dustbowl_night_hawk_hovers_over_a_tank_on_its_cell() {
        use super::super::jumpjet_infantry_tests::{retail_dustbowl_rocketeer, retail_frame};
        use crate::headless_scenario::HeadlessScenario;
        use crate::sim::command::{Command, CommandEnvelope};

        // Open level ground from (x - 4, y - 1) to (x + 16, y + 1); the
        // helper's Rocketeer parks at (x, y).
        let (mut scenario, _, x, y) = retail_dustbowl_rocketeer();
        let spawn = |scenario: &mut HeadlessScenario, name: &str, cell: (u16, u16)| {
            let crate::sim::runtime::SimRuntime {
                simulation: sim,
                resources,
            } = &mut scenario.runtime;
            let id = sim
                .spawn_object_with_overlay_registry(
                    name,
                    "Americans",
                    cell.0,
                    cell.1,
                    64,
                    &resources.rules,
                    &resources.overlay_registry,
                )
                .expect("spawns on open ground");
            sim.resolve_type_handles(&resources.rules);
            id
        };
        let cell_of = |scenario: &HeadlessScenario, id: u64| {
            let entity = scenario
                .runtime
                .simulation
                .substrate
                .entities
                .get(id)
                .expect("live");
            (entity.position.rx, entity.position.ry)
        };
        let phase = |scenario: &HeadlessScenario, id: u64| {
            scenario
                .runtime
                .simulation
                .substrate
                .entities
                .get(id)
                .and_then(|entity| entity.locomotor.as_ref())
                .and_then(|locomotor| locomotor.jumpjet_runtime())
                .expect("Jumpjet")
                .phase
        };

        let hawk = spawn(&mut scenario, "SHAD", (x + 2, y));
        let landing = (x + 6, y);
        let sim = &scenario.runtime.simulation;
        let mut orders = vec![CommandEnvelope::new(
            sim.interner.get("Americans").expect("house"),
            sim.session.tick + 1,
            Command::Move {
                entity_id: hawk,
                target_rx: landing.0,
                target_ry: landing.1,
                queue: false,
            },
        )];
        let mut frames = 0;
        while phase(&scenario, hawk) != STATE_DESCEND {
            retail_frame(&mut scenario, std::mem::take(&mut orders));
            frames += 1;
            assert!(frames < 200, "the Night Hawk reaches its descent");
        }
        assert_eq!(cell_of(&scenario, hawk), landing);
        let tank = spawn(&mut scenario, "MTNK", landing);
        for _ in 0..200 {
            retail_frame(&mut scenario, Vec::new());
            assert_ne!(phase(&scenario, hawk), jumpjet_flight::STATE_GROUND);
        }
        assert_eq!(cell_of(&scenario, hawk), landing);
        assert_eq!(cell_of(&scenario, tank), landing);
        let sim = &scenario.runtime.simulation;
        assert!(
            sim.substrate
                .occupancy
                .contains_entity(landing.0, landing.1, tank)
        );
        assert_eq!(
            sim.substrate
                .raw_cell_occupation
                .ground_bits(landing.0, landing.1)
                & 0x20,
            0,
            "the Night Hawk's Mark cleared the tank's bit"
        );

        let mut orders = vec![CommandEnvelope::new(
            sim.interner.get("Americans").expect("house"),
            sim.session.tick + 1,
            Command::Move {
                entity_id: tank,
                target_rx: x + 12,
                target_ry: y,
                queue: false,
            },
        )];
        let mut frames = 0;
        while phase(&scenario, hawk) != jumpjet_flight::STATE_GROUND {
            retail_frame(&mut scenario, std::mem::take(&mut orders));
            frames += 1;
            assert!(frames < 400, "the Night Hawk lands once the tank leaves");
        }
        assert_eq!(cell_of(&scenario, hawk), landing);
        assert_ne!(cell_of(&scenario, tank), landing);
    }

    /// `g_HeightFactor`, read from the native startup chain, is the multiplier
    /// the cell top height applies to a building's art `Height=`.
    #[test]
    fn building_height_factor_matches_the_native_startup_value() {
        let native: Value = serde_json::from_str(include_str!(
            "../../../tools/spatial_oracle/height_factor.json"
        ))
        .expect("height factor parses");
        assert_eq!(
            native["height_factor"].as_i64(),
            Some(i64::from(GROUND_LEVEL_HEIGHT_LEPTONS))
        );
    }
}
