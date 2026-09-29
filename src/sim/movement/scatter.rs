//! `TechnoClass::Scatter` (vtable `+0x174`) with a null coordinate: the one
//! dispatcher over the class receivers, the Unit receiver, and the cell
//! dispatcher `CellClass::Scatter_Objects @ 0x00481670` that calls it.
//!
//! Receivers:
//! - Unit `0x00743A50`: [`Simulation::unit_scatter_null`].
//! - Infantry `0x0051D0D0`: `infantry_scatter`.
//! - Aircraft `0x0041A590`: when the current mission's MissionControl
//!   `Scatter=` is set, `Enter_Idle_Mode(0, 1)` (`0x004176F0`); it reads
//!   neither the coordinate nor the flags. RESIDUAL: not dispatched, because
//!   no aircraft idle-mode owner is callable outside the aircraft mission
//!   code. Trigger: a landed aircraft asked to scatter. Effect: it keeps its
//!   mission instead of re-entering idle mode. Frequency: rare, since a
//!   landed aircraft is seldom in a blocked cell.
//! - Every other class inherits ObjectClass `0x005F43A0` (`RET 0xC`).
//!
//! A null coordinate is the all-zero CoordStruct each caller passes
//! (`0x008B3DA8`, `0x00B1CFE8`); the receivers compare against their own zero
//! globals (`0x00B1CFE8`, `0x00A8F200`).

use crate::map::entities::EntityCategory;
use crate::map::overlay_types::OverlayTypeRegistry;
use crate::rules::locomotor_type::{LocomotorKind, MovementZone};
use crate::rules::ruleset::RuleSet;
use crate::sim::find_nearby_cell::{
    NearbyAnchorGate, NearbyFootprint, NearbyQuery, PassabilityArgs, find_nearby_passable_cell,
    map_owned_radius_cap,
};
use crate::sim::game_entity::GameEntity;
use crate::sim::world::Simulation;
use std::collections::BTreeSet;

use super::bump_crush::{
    ScatterEligibility, ScatterTechno, cell_has_elite_occupant, scatter_dispatch_allowed,
};
use super::locomotor::MovementLayer;

/// The two flags every Scatter call passes after its coordinate, under
/// Westwood's names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ScatterFlags {
    /// Second argument: overrides the current mission's MissionControl
    /// `Scatter=` (Unit `0x00743AD8..0x00743AEA`). Infantry drops it while
    /// its locomotor is moving (`0x0051D172`).
    pub(super) forced: bool,
    /// Third argument: a Unit with a NavCom scatters only with it
    /// (`0x00743B1D..0x00743B2D`). Infantry reads it in its deploy arm and
    /// its human-owner gate. `Scatter_Objects` passes its dispatch-all byte
    /// here.
    pub(super) no_kidding: bool,
}

impl ScatterFlags {
    pub(crate) const fn new(forced: bool, no_kidding: bool) -> Self {
        Self { forced, no_kidding }
    }
}

/// `Scatter(null, flags)` calls a movement-pass step makes while the pass
/// holds the world split: the tube exit's blocked cell (`0x00735F55`) and the
/// pass lane's cell-entry arms. The object turn runs them in call order once
/// the pass returns ([`Simulation::run_scatter_requests`]), still inside that
/// object's Process. Each object is asked at most once per pass.
#[derive(Debug, Default)]
pub(crate) struct ScatterRequests {
    asked: BTreeSet<u64>,
    pending: Vec<(u64, ScatterFlags)>,
}

impl ScatterRequests {
    /// Queues `Scatter(null, flags)` on `id`; false when this pass already
    /// asked it.
    pub(super) fn request(&mut self, id: u64, flags: ScatterFlags) -> bool {
        let first = self.asked.insert(id);
        if first {
            self.pending.push((id, flags));
        }
        first
    }

    /// The queued calls, in call order. The pass keeps whom it asked.
    pub(super) fn take(&mut self) -> Vec<(u64, ScatterFlags)> {
        std::mem::take(&mut self.pending)
    }
}

/// The current mission's MissionControl `Scatter=` (`0x005B3A00` indexes the
/// table by `+0xAC`). A mission outside the table reads as permitted.
pub(super) fn mission_permits_scatter(entity: &GameEntity, rules: &RuleSet) -> bool {
    entity
        .mission
        .current()
        .known()
        .and_then(|mission| rules.mission_control.entry(mission))
        .is_none_or(|entry| entry.scatter)
}

/// `UnitClass::Scatter` refusals before the coordinate
/// (`0x00743A5C..0x00743BAA`), in native order:
/// - `+0x28C` (`0x006F3280`): effective Sleep, Sticky or Unload, or
///   `IsTrain=` (unparsed; no retail type sets it);
/// - an active Teleport locomotor (the active class, not the installed slot:
///   a Chrono Miner's temporary Drive is eligible);
/// - the mission's `Scatter=` unless forced;
/// - a rotating body (`+0x388`);
/// - a NavCom unless no-kidding;
/// - the deploy bytes `+0x6E0..+0x6E2` (`deploy_state`);
/// - an unpowered locomotor (`ILocomotion+0x60`).
///
/// None draws RNG or writes state. Evidence for the flag-free prefix:
/// tools/spatial_oracle/unit_scatter_state.{py,json,meta.json}.
pub(super) fn unit_scatter_admitted(
    unit: &GameEntity,
    flags: ScatterFlags,
    rules: &RuleSet,
    binary_frame: u32,
) -> bool {
    if matches!(unit.mission.effective().raw(), 0 | 6 | 16) {
        return false;
    }
    let Some(locomotor) = unit.locomotor.as_ref() else {
        return false;
    };
    locomotor.active_kind() != LocomotorKind::Teleport
        && (flags.forced || mission_permits_scatter(unit, rules))
        && !unit.body_facing.is_rotating(binary_frame)
        && (flags.no_kidding || unit.navigation.nav_com.is_none())
        && unit.deploy_state.is_none()
        && locomotor.is_powered()
}

impl Simulation {
    /// `TechnoClass::Scatter` (vtable `+0x174`) with a null coordinate.
    /// Answers whether an Infantry receiver's immediate Process changed bridge
    /// state. Missing map state leaves the receiver where it stands; malformed
    /// live class state is an error.
    pub(crate) fn scatter_null(
        &mut self,
        id: u64,
        flags: ScatterFlags,
        rules: &RuleSet,
        registry: Option<&OverlayTypeRegistry>,
    ) -> Result<bool, String> {
        let Some(entity) = self.substrate.entities.get(id) else {
            return Ok(false);
        };
        match entity.category {
            EntityCategory::Unit => {
                self.unit_scatter_null(id, flags, rules);
                Ok(false)
            }
            EntityCategory::Infantry => self.infantry_scatter_null(id, flags, rules, registry),
            _ => Ok(false),
        }
    }

    /// `UnitClass::Scatter @ 0x00743A50` with a null coordinate
    /// (`0x00743BE0..0x00743C9B`): after [`unit_scatter_admitted`], the
    /// nearby passable cell and `SetDestination(cell, 1)` (`0x00744063..
    /// 0x00744070`). This arm draws no RNG, queues no mission and runs no
    /// Process.
    fn unit_scatter_null(&mut self, id: u64, flags: ScatterFlags, rules: &RuleSet) {
        let Some(unit) = self.substrate.entities.get(id) else {
            return;
        };
        if !unit_scatter_admitted(unit, flags, rules, self.session.binary_frame) {
            return;
        }
        if let Some(cell) = self.scatter_nearby_cell(id, rules) {
            self.set_unit_cell_destination(id, cell, rules);
        }
    }

    /// `FootClass::Find_Nearby_Passable_Cell` as both null-coordinate Scatter
    /// arms call it (Unit `0x00743C6B`, Infantry `0x0051D41D`, identical
    /// arguments): from the navigation cell (`vt+0x4C`, `0x004DBDF0`), with the
    /// type's SpeedType, zone -1, Normal, the Foot's bridge byte (`+0x8C`), a
    /// 1x1 rectangle, flags (0,1,0,1) and a zeroed target cell. `None` when
    /// the search finds nothing or the map has no native Size.
    pub(super) fn scatter_nearby_cell(&self, id: u64, rules: &RuleSet) -> Option<(u16, u16)> {
        let entity = self.substrate.entities.get(id)?;
        let object = self.object_type(entity.type_ref(), rules)?;
        let on_bridge = entity.on_bridge;
        let seed = self.foot_navigation_coordinate(id).ok()?;
        let seed = (
            i32::from((seed.x / 256) as i16),
            i32::from((seed.y / 256) as i16),
        );
        let size = self
            .playfield_bounds
            .zip(self.playfield_size_height)
            .map(|(bounds, height)| (bounds.base, height))
            .or_else(|| self.bridge_state.as_ref()?.native_zone_source_size())?;
        let grid = self.path_grid_snapshot();
        find_nearby_passable_cell(
            seed,
            &NearbyQuery {
                native_cells: None,
                raw_occupation: Some(&self.substrate.raw_cell_occupation),
                passability: PassabilityArgs {
                    speed_type: object.speed_type,
                    required_zone_id: None,
                    movement_zone: MovementZone::Normal,
                    bridge_aware_zone: on_bridge,
                },
                footprint: NearbyFootprint::SINGLE,
                anchor_gate: NearbyAnchorGate::NativeHeightAware,
                allow_bridge_cells: true,
                check_height: true,
                check_occupancy: false,
                radius_cap: map_owned_radius_cap(size.0, size.1),
                target_cell: None,
                path_grid: grid.as_deref(),
                resolved_terrain: self.resolved_terrain.as_ref(),
                overlay_grid: self.overlay_grid.as_ref(),
                occupancy: Some(&self.substrate.occupancy),
                entities: Some(&self.substrate.entities),
                zone_grid: self.zone_grid.as_ref(),
                playfield_bounds: self.playfield_bounds,
            },
            self.session.binary_frame,
        )
    }

    /// Runs the Scatter calls a movement pass queued ([`ScatterRequests`]).
    /// Without rules nothing can admit a receiver, so none runs. Answers
    /// whether an Infantry receiver's immediate Process changed bridge state.
    pub(crate) fn run_scatter_requests(
        &mut self,
        requests: Vec<(u64, ScatterFlags)>,
        rules: Option<&RuleSet>,
        registry: Option<&OverlayTypeRegistry>,
    ) -> Result<bool, String> {
        let Some(rules) = rules else {
            return Ok(false);
        };
        let mut bridge_state_changed = false;
        for (id, flags) in requests {
            bridge_state_changed |= self.scatter_null(id, flags, rules, registry)?;
        }
        Ok(bridge_state_changed)
    }

    /// `CellClass::Scatter_Objects @ 0x00481670` with a null coordinate on
    /// `layer`'s list (`+0xE4` ground, `+0xE8` deck). The list is
    /// snapshotted first. Without dispatch-all (`flags.no_kidding`) it is
    /// pre-scanned for an elite occupant. Each occupant [`scatter_dispatch_allowed`]
    /// admits then takes `Scatter(null, flags)` (`vt+0x174`) in list order.
    /// Evidence: tools/spatial_oracle/cell_scatter.{py,json,meta.json}.
    pub(crate) fn scatter_cell_objects(
        &mut self,
        cell: (u16, u16),
        layer: MovementLayer,
        flags: ScatterFlags,
        rules: &RuleSet,
        registry: Option<&OverlayTypeRegistry>,
    ) -> Result<bool, String> {
        let occupants = self
            .substrate
            .occupancy
            .get(cell.0, cell.1)
            .map_or_else(Vec::new, |occ| occ.snapshot_layer(layer));
        let eligibility = ScatterEligibility::from_rules(Some(rules));
        let elite_in_cell = !flags.no_kidding
            && cell_has_elite_occupant(&occupants, u64::MAX, &self.substrate.entities);
        let mut bridge_state_changed = false;
        for id in occupants {
            let admitted = self.substrate.entities.get(id).is_some_and(|occupant| {
                scatter_dispatch_allowed(
                    eligibility,
                    flags.no_kidding,
                    elite_in_cell,
                    Some(ScatterTechno::from_entity(
                        occupant,
                        Some(rules),
                        &self.houses,
                        &self.interner,
                    )),
                )
            });
            if admitted {
                bridge_state_changed |= self.scatter_null(id, flags, rules, registry)?;
            }
        }
        Ok(bridge_state_changed)
    }
}

#[cfg(test)]
#[path = "scatter_tests.rs"]
mod tests;
