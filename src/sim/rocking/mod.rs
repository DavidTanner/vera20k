//! Body-rocking simulation.
//!
//! Implements the spring-damper that drives `RockingState::angle_*` toward zero
//! each tick. Drive/Ship slope interpolation is owned separately by locomotor
//! runtime payload state.
//!
//! # The crash spin is live; the rocker is still dead (GSI-08.14)
//!
//! `FootClass::Crash @ 0x004DEBB0` writes a crashing object's spin rates
//! (`sim::world::crash`), [`tick`]'s crashing branch
//! ([`rocking_system::advance_crash_spin`], `RockingUpdate 0x0070B63D`)
//! integrates them, and the unit renderer draws the angles through Fly
//! Draw_Matrix's crashing arm (`render::vxl_raster`); Jumpjet crash tilt also
//! consumes these angles. Everything below is about the ordinary damped
//! ground rocker, which still has no producer or Drive body-drawing consumer:
//!
//! - **No producer.** [`apply_rocker_impulse`] has no caller outside this
//!   module's tests. gamemd reaches `TechnoClass::ApplyRocker` (vtable `+0x3D8`
//!   → `0x0070B280`) from exactly four places, established this session by a
//!   `CALL [reg+0x3D8]` census over all 1.16M instructions in the image:
//!   `BulletClass::DetonateAtCoord @ 0x004699A1` (`DirectRocker=`, zero stock
//!   authors), `Apply_area_damage @ 0x00489DFF`/`0x00489E3E` (`Rocker=`), and
//!   `ParasiteClass::AI @ 0x0062A21C` (a parasite bite
//!   on a non-Infantry victim, force `1.5`; its lateral-sign RNG draw at
//!   `0x0062A17F` is taken in `combat/parasite.rs`).
//!   `FootClass::ReceiveEMP @ 0x004DECF0` writes the velocities directly
//!   instead. **Firing never rocks the body** — there is no
//!   `ApplyRocker` site in `Fire_At`, and none in any crush or deploy path.
//! - **No ground-body consumer.** The presentation consumers of
//!   `GameEntity::rocking` select Fly/Jumpjet crash arms, not ordinary Drive
//!   Draw_Matrix. A ground impulse alone would not tilt its vehicle body, and
//!   the self-destruct hook the production caller passes is
//!   [`NoopSelfDestruct`].
//!
//! - Trigger: a `Rocker=yes` warhead, such as stock `V3WH`, detonating near a
//!   vehicle. Ordinary stock `105mm`/`AP` shots do not enter either warhead
//!   rocker arm: the original Warhead constructor and `ReadINI 0x0075D3A0`
//!   retain both `Rocker` and `DirectRocker` as false for physical AP.
//!   Evidence: `tools/procedural_drawing_oracle/action_lines_attack_prerequisites.json`.
//! - Player effect: vehicles do not lurch when shells land beside them. The
//!   whole "shot lands next to the Rhino and the Rhino rocks" reading is
//!   missing.
//! - Frequency: engagements involving the applicable warheads or parasites.
//! - Downstream risk: wiring the producer alone would add per-entity
//!   deterministic state and move the pinned replay hash while changing nothing
//!   a player can see, so the producer and the renderer belong in one slice. See
//!   the verified DRIFT list on [`apply_rocker_impulse`] for what the impulse
//!   maths must also change when that slice lands.
//!
//! ## Dependency rules
//! - Part of sim/ — depends only on sim/components, sim/entity_store, map,
//!   rules, util/fixed_math. Never on render/, ui/, audio/, net/.

pub mod impulse;
pub mod rocking_system;
pub mod self_destruct;

#[cfg(test)]
#[path = "rocking_tests.rs"]
mod rocking_tests;

pub use impulse::apply_rocker_impulse;
pub use rocking_system::tick;
pub use self_destruct::{NoopSelfDestruct, SelfDestructHook, check_and_fire};

use crate::util::fixed_math::SimFixed;

/// Body rocking state for voxel-bodied units.
///
/// Tracks spring-damped roll/pitch angles driven by weapon impacts and EMP
/// wobble. Drive/Ship slope interpolation is locomotor-owned state and is
/// intentionally independent from this optional component.
///
/// Optional component on `GameEntity` — present on vehicles, ships, and
/// voxel-bodied buildings; `None` for infantry and SHP-bodied buildings, and
/// for aircraft until `FootClass::Crash` sets their spin rates.
#[derive(Debug, Clone, Copy, Default, serde::Serialize, serde::Deserialize)]
pub struct RockingState {
    /// Roll angle, rad. Positive sign matches AngleRotatedSideways convention.
    angle_sideways: SimFixed,
    /// Pitch angle, rad. Positive sign matches AngleRotatedForwards convention.
    angle_forwards: SimFixed,
    /// Roll angular velocity, rad/tick.
    vel_sideways: SimFixed,
    /// Pitch angular velocity, rad/tick.
    vel_forwards: SimFixed,
    /// If true, integrate without damping (EMP wobble, naval continuous rocking).
    is_ship_rocking: bool,
}

impl RockingState {
    /// Tilt-renderer deadband — both angles below this snap to zero and the unit
    /// renders via the static atlas path.
    pub const DEADBAND: SimFixed = SimFixed::lit("0.00002");

    /// Roll angle, rad.
    pub fn angle_sideways(&self) -> SimFixed {
        self.angle_sideways
    }

    /// Pitch angle, rad.
    pub fn angle_forwards(&self) -> SimFixed {
        self.angle_forwards
    }

    /// Roll angular velocity, rad/tick.
    pub fn vel_sideways(&self) -> SimFixed {
        self.vel_sideways
    }

    /// Pitch angular velocity, rad/tick.
    pub fn vel_forwards(&self) -> SimFixed {
        self.vel_forwards
    }

    /// Integrates without damping.
    pub fn is_ship_rocking(&self) -> bool {
        self.is_ship_rocking
    }

    /// `FootClass::Crash @ 0x004DEBB0`'s spin rates on a crashing object,
    /// installing the state when it has none.
    pub(crate) fn spin_for_crash(
        rocking: &mut Option<Self>,
        vel_sideways: SimFixed,
        vel_forwards: SimFixed,
    ) {
        let rocking = rocking.get_or_insert_with(Self::default);
        rocking.vel_sideways = vel_sideways;
        rocking.vel_forwards = vel_forwards;
    }

    /// Install a fixture's sideways angle.
    #[cfg(test)]
    pub(crate) fn set_angle_sideways_for_test(&mut self, angle: SimFixed) {
        self.angle_sideways = angle;
    }

    /// Install a fixture's forwards angle.
    #[cfg(test)]
    pub(crate) fn set_angle_forwards_for_test(&mut self, angle: SimFixed) {
        self.angle_forwards = angle;
    }

    /// Install a fixture's sideways rate.
    #[cfg(test)]
    pub(crate) fn set_vel_sideways_for_test(&mut self, vel: SimFixed) {
        self.vel_sideways = vel;
    }

    /// Returns true when the body-rocking transform is neutral.
    #[cfg(test)]
    pub fn is_neutral(&self) -> bool {
        !self.is_ship_rocking
            && self.angle_sideways.abs() <= Self::DEADBAND
            && self.angle_forwards.abs() <= Self::DEADBAND
    }
}
