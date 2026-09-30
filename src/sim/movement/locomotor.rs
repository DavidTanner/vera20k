//! Locomotor component and access to each movement mechanism's runtime.
//!
//! `MovementTarget` owns destination/path data. Mechanism payloads own native
//! controller state; shared fields retain movement and presentation adapters.
//! Current world Z is authoritative in Object coordinates. `altitude` is its
//! bounded cache, while Fly's integer target lives in its own payload.

use crate::rules::locomotor_type::{LocomotorKind, MovementZone, SpeedType};
use crate::rules::object_type::ObjectType;
use crate::sim::movement::locomotion::LocomotorSlot;
use crate::sim::movement::locomotion::piggyback::{
    self, LocomotorRuntimePayload, StashedLocomotor,
};
use crate::sim::movement::slope_transition::SlopeTransitionState;
use crate::util::fixed_math::{SIM_ZERO, SimFixed};

/// Which spatial layer the unit currently occupies.
///
/// Affects occupancy checks, rendering, and targeting. Ground units block
/// ground cells; air units occupy the air layer and can fly over obstacles.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub enum MovementLayer {
    /// Standard ground surface.
    Ground,
    /// Elevated bridge deck above the ground/water layer.
    Bridge,
    /// Airborne (aircraft, jumpjets at altitude).
    Air,
    /// Burrowed underground (tunnel units).
    Underground,
}

/// Derived view of Fly height versus target for legacy aircraft missions.
/// This is neither serialized controller state nor native takeoff/landing flags.
///
/// Fly units cycle through TakingOff → Cruising → Descending → Landed. A
/// Jumpjet does not use it: its phase is the native state field,
/// `JumpjetRuntime::phase`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum AirMovePhase {
    /// On the ground, not yet airborne.
    Landed,
    /// Ascending from ground to cruise/hover altitude.
    Ascending,
    /// At cruise altitude, moving toward destination.
    Cruising,
    /// Descending from cruise altitude back to ground.
    Descending,
}

/// Runtime locomotor state attached to each movable ECS entity.
///
/// Created from `ObjectType` at spawn time. The movement system reads this
/// to decide how to process the entity's `MovementTarget` each tick. It is
/// one complete locomotor object: a piggyback suspends the whole object in
/// the new active object's `piggyback` slot and restores it untouched.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct LocomotorState {
    /// Which locomotor class is currently active.
    pub kind: LocomotorKind,
    /// The locomotor class this unit was built with — the installed slot.
    ///
    /// Natively a unit holds exactly one locomotor interface, created once in
    /// its class constructor from the type's `Locomotor=` CLSID; there is no
    /// second slot and no re-selection. `kind` is the class *currently driving*
    /// the unit, which differs from this only while a piggyback stash is active.
    pub slot: LocomotorSlot,
    /// Whether this locomotor is powered.
    ///
    /// Natively a plain flag on the locomotor instance, set by `Power_On` /
    /// `Power_Off` and read by `Is_Powered`; both setters also re-dispatch to
    /// another slot, which has no verified effect and is not modelled. Defaults
    /// to on — an unpowered locomotor is a state something must actively put a
    /// unit into.
    pub powered: bool,
    /// The suspended locomotor object, when a piggyback displaced it.
    ///
    /// For CMIN drive phases, `kind` becomes Drive and this stores the complete
    /// primary Teleport object until the active Drive locomotor is ok to end.
    #[serde(default)]
    pub piggyback: Option<StashedLocomotor>,
    /// Class-local state of this locomotor object.
    pub runtime_payload: LocomotorRuntimePayload,
    /// Which spatial layer the unit currently occupies.
    pub layer: MovementLayer,
    /// Bounded altitude cache for movement/presentation adapters. Fly's exact
    /// current height comes from Object Z and terrain; its target is in FlyRuntime.
    pub altitude: SimFixed,
    /// Stay airborne after reaching destination (BalloonHover=yes).
    pub balloon_hover: bool,
    /// Can attack while hovering in place (HoverAttack=yes).
    pub hover_attack: bool,
    /// Which terrain type this unit traverses (from rules.ini SpeedType=).
    /// Used to select the correct TerrainCostGrid for cost-aware pathfinding.
    pub speed_type: SpeedType,
    /// Pathfinder movement zone — determines crush capability and special routing.
    /// Cached from ObjectType at spawn to avoid per-tick RuleSet lookups.
    pub movement_zone: MovementZone,
    /// Within-cell walk destination for infantry. Set when a sub-cell is allocated
    /// during cell entry. The locomotor walks the infantry toward this point after
    /// the path is exhausted.
    pub subcell_dest: Option<(SimFixed, SimFixed)>,
    /// Hover throttle `[0, 1]` — the persisted speed fraction of the hover
    /// locomotor's SpeedUpdate model (see `sim/movement/hover.rs`). Lives on the
    /// locomotor (not `MovementTarget`) so it survives path recomputes: a hover
    /// unit re-pathed mid-route keeps its momentum instead of re-spinning up.
    /// Zero at spawn (units start from rest) and reset to zero on full stop.
    /// Unused by non-Hover locomotors.
    #[serde(default)]
    pub hover_throttle: SimFixed,

    /// Hover speed *request* — the unramped throttle target, distinct from
    /// `hover_throttle` (the ramped value that lags it).
    ///
    /// Persisted solely so the Mission readiness producer can read it: the
    /// native readiness slot reads the request, not the ramp, and the ramp lags
    /// by up to ~27 ticks on the brake side, which is the direction that would
    /// wrongly report "moving". Takes only three values (0, 0.5, 1).
    /// Unused by non-Hover locomotors.
    #[serde(default)]
    pub hover_speed_request: SimFixed,
    /// Hover vertical-spring state — the velocity-like bob offset of the
    /// damped-spring altitude controller (see `hover::hover_vertical_tick`).
    /// Pairs with `altitude`, which for hover units holds the visible float
    /// height above ground. Unused by non-Hover locomotors.
    #[serde(default)]
    pub hover_bob_offset: SimFixed,
}

impl LocomotorState {
    /// Create a LocomotorState from an ObjectType's parsed rules.ini data.
    ///
    /// Class runtime starts from its native constructor; Fly requests resolve
    /// FlightLevel when takeoff is admitted, not during construction.
    pub fn from_object_type(obj: &ObjectType, binary_frame: u32) -> Self {
        let kind: LocomotorKind = obj.locomotor;

        let layer: MovementLayer = match kind {
            LocomotorKind::Drive
            | LocomotorKind::Walk
            | LocomotorKind::Hover
            | LocomotorKind::Ship
            | LocomotorKind::Teleport => MovementLayer::Ground,
            // Air family — use Air layer with altitude state
            LocomotorKind::Fly | LocomotorKind::Jumpjet | LocomotorKind::Rocket => {
                MovementLayer::Air
            }
        };

        Self {
            kind,
            slot: LocomotorSlot::new(kind),
            powered: true,
            piggyback: None,
            runtime_payload: {
                // `Link_To_Object @ 0x0054AD30` copies the type's jumpjet block and
                // builds the locomotor facing at its `JumpjetTurnRate=`.
                let mut payload = LocomotorRuntimePayload::for_kind(kind, binary_frame);
                if let LocomotorRuntimePayload::Jumpjet(runtime) = &mut payload {
                    runtime.link(&obj.jumpjet_params);
                }
                if let LocomotorRuntimePayload::Fly(runtime) = &mut payload {
                    runtime.link(
                        obj.category == crate::rules::object_type::ObjectCategory::Aircraft
                            && obj.airport_bound,
                    );
                }
                payload
            },
            layer,
            altitude: SIM_ZERO,

            balloon_hover: obj.balloon_hover,
            hover_attack: obj.hover_attack,
            speed_type: obj.speed_type,
            movement_zone: obj.movement_zone,
            subcell_dest: None,
            hover_throttle: SIM_ZERO,
            hover_speed_request: SIM_ZERO,
            hover_bob_offset: SIM_ZERO,
        }
    }

    /// A freshly constructed `kind` object linked to the same Foot, as every
    /// native BEGIN site installs one: the Unit setter allocates
    /// (`0x0041C250`), constructs (Drive `0x004AF540` over the
    /// `LocomotionClass` constructor `0x0055A6C0`, which raises Powered) and
    /// links it (`0x007426C9`) before BEGIN (`0x0074276F`). It shares only
    /// the type's data and the installed slot with this object. The only
    /// production BEGIN installs a Drive; a Jumpjet or Fly temporary would
    /// also need its type's link block, which this constructor has no type
    /// to read.
    pub(crate) fn fresh_linked(
        &self,
        kind: LocomotorKind,
        layer: MovementLayer,
        binary_frame: u32,
    ) -> Self {
        Self {
            kind,
            slot: self.slot,
            powered: true,
            piggyback: None,
            runtime_payload: LocomotorRuntimePayload::for_kind(kind, binary_frame),
            layer,
            altitude: SIM_ZERO,
            balloon_hover: self.balloon_hover,
            hover_attack: self.hover_attack,
            speed_type: self.speed_type,
            movement_zone: self.movement_zone,
            subcell_dest: None,
            hover_throttle: SIM_ZERO,
            hover_speed_request: SIM_ZERO,
            hover_bob_offset: SIM_ZERO,
        }
    }

    #[cfg(test)]
    pub(crate) fn for_test_kind(kind: LocomotorKind) -> Self {
        Self::for_test_kind_at_frame(kind, 0)
    }

    #[cfg(test)]
    pub(crate) fn for_test_kind_at_frame(kind: LocomotorKind, binary_frame: u32) -> Self {
        let layer = match kind {
            LocomotorKind::Fly | LocomotorKind::Jumpjet | LocomotorKind::Rocket => {
                MovementLayer::Air
            }
            _ => MovementLayer::Ground,
        };

        Self {
            kind,
            slot: LocomotorSlot::new(kind),
            powered: true,
            piggyback: None,
            runtime_payload: LocomotorRuntimePayload::for_kind(kind, binary_frame),
            layer,
            altitude: SIM_ZERO,

            balloon_hover: false,
            hover_attack: false,
            speed_type: SpeedType::Track,
            movement_zone: MovementZone::Normal,
            subcell_dest: None,
            hover_throttle: SIM_ZERO,
            hover_speed_request: SIM_ZERO,
            hover_bob_offset: SIM_ZERO,
        }
    }

    /// Whether this locomotor is in the ground family (Drive/Walk/Hover/Ship).
    #[cfg(test)]
    pub fn is_ground_mover(&self) -> bool {
        matches!(
            self.kind,
            LocomotorKind::Drive | LocomotorKind::Walk | LocomotorKind::Hover | LocomotorKind::Ship
        )
    }

    /// Whether this locomotor is an air mover (Fly/Jumpjet/Rocket).
    #[cfg(test)]
    pub fn is_air_mover(&self) -> bool {
        matches!(
            self.kind,
            LocomotorKind::Fly | LocomotorKind::Jumpjet | LocomotorKind::Rocket
        )
    }

    /// Whether this unit is currently airborne (altitude > 0).
    pub fn is_airborne(&self) -> bool {
        self.altitude > SIM_ZERO
    }

    pub(crate) fn fly_runtime(&self) -> Option<&super::fly_height::FlyRuntime> {
        match (self.kind, &self.runtime_payload) {
            (LocomotorKind::Fly, LocomotorRuntimePayload::Fly(state)) => Some(state),
            _ => None,
        }
    }

    pub(crate) fn fly_runtime_mut(&mut self) -> Option<&mut super::fly_height::FlyRuntime> {
        match (self.kind, &mut self.runtime_payload) {
            (LocomotorKind::Fly, LocomotorRuntimePayload::Fly(state)) => Some(state),
            _ => None,
        }
    }

    pub(crate) fn fly_target_height(&self) -> i32 {
        self.fly_runtime().map_or(0, |state| state.target_height())
    }

    pub(crate) fn set_fly_target_height(&mut self, height: i32) {
        if let Some(state) = self.fly_runtime_mut() {
            state.set_target_height(height);
        }
    }

    pub(crate) fn begin_fly_takeoff(&mut self, flight_level: i32) {
        if let Some(state) = self.fly_runtime_mut() {
            state.begin_takeoff(flight_level);
        }
    }

    pub(crate) fn begin_fly_landing(&mut self) {
        if let Some(state) = self.fly_runtime_mut() {
            state.begin_landing();
        }
    }

    #[cfg(test)]
    pub(crate) fn air_phase(&self) -> AirMovePhase {
        self.fly_runtime().map_or(AirMovePhase::Landed, |state| {
            state.mission_phase(self.altitude.to_num::<i32>())
        })
    }

    /// Whether a piggybacked locomotor is currently displacing the installed one.
    pub fn is_overridden(&self) -> bool {
        self.piggyback.is_some()
    }

    /// Current active locomotor class.
    pub fn active_kind(&self) -> LocomotorKind {
        self.kind
    }

    /// Walk interface+24 / Hover+24 retained full XYZ head. This belongs to
    /// the active instance and survives Stop/piggyback/save without sampling
    /// a newly changed ground surface. Native producers75C240/514F70; query
    /// receivers75CA80/517210, tools/spatial_oracle/locomotor_at_coord.
    pub(crate) fn step_head(&self) -> Option<crate::sim::components::DriveCoord> {
        match (self.kind, &self.runtime_payload) {
            (LocomotorKind::Walk, LocomotorRuntimePayload::Walk(state)) => state.head,
            (LocomotorKind::Hover, LocomotorRuntimePayload::Hover(head)) => *head,
            _ => None,
        }
    }

    pub(crate) fn set_step_head(&mut self, head: Option<crate::sim::components::DriveCoord>) {
        match (self.kind, &mut self.runtime_payload) {
            (LocomotorKind::Walk, LocomotorRuntimePayload::Walk(state)) => state.head = head,
            (LocomotorKind::Hover, LocomotorRuntimePayload::Hover(stored)) => *stored = head,
            _ => {}
        }
    }

    pub(crate) fn walk_destination(&self) -> Option<crate::sim::components::DriveCoord> {
        match (self.kind, &self.runtime_payload) {
            (LocomotorKind::Walk, LocomotorRuntimePayload::Walk(state)) => state.destination,
            _ => None,
        }
    }

    pub(crate) fn jumpjet_runtime(&self) -> Option<&super::jumpjet_movement::JumpjetRuntime> {
        match (self.active_kind(), &self.runtime_payload) {
            (LocomotorKind::Jumpjet, LocomotorRuntimePayload::Jumpjet(state)) => Some(state),
            _ => None,
        }
    }

    pub(crate) fn jumpjet_runtime_mut(
        &mut self,
    ) -> Option<&mut super::jumpjet_movement::JumpjetRuntime> {
        match (self.kind, &mut self.runtime_payload) {
            (LocomotorKind::Jumpjet, LocomotorRuntimePayload::Jumpjet(state)) => Some(state),
            _ => None,
        }
    }

    pub(crate) fn walk_is_moving(&self) -> Option<bool> {
        match (self.kind, &self.runtime_payload) {
            (LocomotorKind::Walk, LocomotorRuntimePayload::Walk(state)) => Some(state.moving),
            _ => None,
        }
    }

    /// MoveTo/Stop keep a paid head. Their IsMoving byte survives a null
    ///destination while that head exists (75AD77 /75ADE9).
    pub(crate) fn set_walk_destination(
        &mut self,
        coord: Option<crate::sim::components::DriveCoord>,
    ) {
        if let (LocomotorKind::Walk, LocomotorRuntimePayload::Walk(state)) =
            (self.kind, &mut self.runtime_payload)
        {
            state.destination = coord.filter(|c| c.x != 0 || c.y != 0 || c.z != 0);
            if state.destination.is_some() {
                state.moving = true;
            } else if state.head.is_none() {
                state.moving = false;
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn walk_animation_moving(&self) -> Option<bool> {
        match (self.kind, &self.runtime_payload) {
            (LocomotorKind::Walk, LocomotorRuntimePayload::Walk(state)) => {
                Some(state.animation_moving)
            }
            _ => None,
        }
    }

    /// Original Process75BD25, after a nonnull head is admitted and before
    /// testing its remaining distance. Does not stand for the whole Process.
    pub(crate) fn begin_walk_motion(&mut self) {
        if let (LocomotorKind::Walk, LocomotorRuntimePayload::Walk(state)) =
            (self.kind, &mut self.runtime_payload)
            && state.head.is_some()
        {
            state.animation_moving = true;
        }
    }

    /// Walk75B6A3 clears animation+36 on a refused fresh head, retaining
    /// its distinct moving byte, destination and head.
    pub(super) fn refuse_walk_animation(&mut self) {
        if let (LocomotorKind::Walk, LocomotorRuntimePayload::Walk(state)) =
            (self.kind, &mut self.runtime_payload)
        {
            state.animation_moving = false;
        }
    }

    /// Stop75ADA0 differs from null MoveTo: no-head Stop also clears +36.
    pub(crate) fn stop_walk(&mut self) {
        self.set_walk_destination(None);
        if let (LocomotorKind::Walk, LocomotorRuntimePayload::Walk(state)) =
            (self.kind, &mut self.runtime_payload)
            && state.head.is_none()
        {
            state.animation_moving = false;
        }
    }

    pub(crate) fn active_slope_transition(&self) -> Option<&SlopeTransitionState> {
        match (self.active_kind(), &self.runtime_payload) {
            (LocomotorKind::Drive, LocomotorRuntimePayload::Drive(state))
            | (LocomotorKind::Ship, LocomotorRuntimePayload::Ship(state)) => Some(state),
            _ => None,
        }
    }

    pub(crate) fn active_slope_transition_mut(&mut self) -> Option<&mut SlopeTransitionState> {
        match (self.kind, &mut self.runtime_payload) {
            (LocomotorKind::Drive, LocomotorRuntimePayload::Drive(state))
            | (LocomotorKind::Ship, LocomotorRuntimePayload::Ship(state)) => Some(state),
            _ => None,
        }
    }

    /// The unit's identity for mission-level decisions: the installed class,
    /// seen through any piggyback that is currently driving it.
    ///
    /// A Chrono Miner driving out of a war factory on a piggybacked Drive still
    /// *is* a Teleport unit; `kind` answers "what is driving right now" and this
    /// answers "what is this unit". Both are needed and must stay distinct.
    pub fn effective_kind(&self) -> LocomotorKind {
        self.slot.installed()
    }

    /// Whether the primary locomotor is currently active and no piggyback is stored.
    #[cfg(test)]
    pub fn is_primary_active(&self) -> bool {
        self.kind == self.effective_kind() && self.piggyback.is_none()
    }

    /// Activate Drive over a stashed Teleport locomotor — the Chrono Miner
    /// bridge model: the unit stays a Teleport unit, Drive temporarily drives it
    /// for destinations that need ground movement.
    pub fn begin_drive_piggyback_for_teleporter(&mut self, binary_frame: u32) -> bool {
        if self.effective_kind() != LocomotorKind::Teleport {
            return false;
        }
        if self.kind == LocomotorKind::Drive {
            // WalkLocomotionClass::BeginPiggyback rejects nested/incoherent
            // ownership; it never reconstructs a missing Teleport COM object.
            return self.piggyback.is_some();
        }
        self.begin_piggyback(LocomotorKind::Drive, MovementLayer::Ground, binary_frame)
    }

    /// Return from an active piggyback to the stashed locomotor.
    ///
    /// The installed slot is deliberately NOT written here. Natively the
    /// installed interface pointer never changes — a piggyback stashes and
    /// restores around it — and the previous write was retained only until this
    /// mechanism existed to retire it.
    pub fn restore_primary_from_piggyback(&mut self) -> bool {
        self.end_piggyback()
    }

    /// Begin a piggyback: stash the driving locomotor and install this one.
    ///
    /// Refuses, changing nothing, if a stash is already present — the native
    /// BEGIN returns `E_FAIL` in exactly that case.
    pub fn begin_piggyback(
        &mut self,
        kind: LocomotorKind,
        layer: MovementLayer,
        binary_frame: u32,
    ) -> bool {
        piggyback::begin(self, kind, layer, binary_frame) == piggyback::BeginOutcome::Installed
    }

    /// End the active piggyback, restoring the stashed locomotor.
    ///
    /// Returns whether anything was stashed.
    pub fn end_piggyback(&mut self) -> bool {
        piggyback::end(self).is_some()
    }

    /// Power this locomotor back on.
    pub fn power_on(&mut self) {
        self.powered = true;
    }

    /// Power this locomotor off. Only the Hover family has an observable
    /// response today: it stops producing lift and sinks.
    pub fn power_off(&mut self) {
        self.powered = false;
    }

    /// Whether this locomotor is powered.
    pub fn is_powered(&self) -> bool {
        self.powered
    }
}

#[cfg(test)]
#[path = "locomotor_tests.rs"]
mod locomotor_tests;
