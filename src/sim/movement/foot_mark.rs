//! `FootClass::Mark` (vt+0x124 @ `0x004D3780`), the one Rust port for the
//! Infantry, Unit and Aircraft vtables (`0x007EB058`, `0x007F5C70`,
//! `0x007E22A4`). Buildings have their own Mark (`0x0043F180`).
//!
//! Mark(UP) and Mark(DOWN) run `TechnoClass::Mark` (`0x006F4A70`), whose
//! `ObjectClass::Mark` (`0x005F5850`) refuses a Limbo object (+0x81) and an
//! object already in the requested state, then writes Cell.Marked (+0x74)
//! before anything else. Only an object whose layer (vt+0x78, the locomotor's
//! `In_Which_Layer`) is Ground reaches `MapClass::Pick_Up` (`0x005687F0`) or
//! `Place_Down` (`0x005683C0`). For the Foot occupy list `{(0,0)}` they
//! address the Cell of the stored Location (vt+0x1B8 = `0x0041BEA0`) when it
//! is a real Cell, unlink it from or prepend it to the OnBridge list
//! (`CellClass::RemoveContent` `0x0047EA90` / `AddContent` `0x0047E8A0`), and
//! run `CellClass::Recalc(-1)` (`0x0047D2B0`) on that Cell. Remove/AddContent
//! skip Infantry's raw receiver; the others write theirs only while the Foot
//! occupation enable (+0x6B6, vt+0xC0 = `0x0041C070`) holds.
//!
//! RESIDUALS, none of which a ported consumer reads:
//! - `TechnoClass::Mark` sends radio 0xD to the first contact of a tethered
//!   object (+0x418, `0x006F4A81..0x006F4A91`). Trigger: Mark while tethered
//!   (a unit docking at a pad or refinery). Effect and frequency: unmapped.
//! - AddContent's discovery (`0x0047E953..0x0047E9DC`, `DiscoveredBy(Player)`
//!   `0x006F4960`, which can raise Tag event 4) is not run; the observed PUT
//!   below marks where it sits. Trigger: every Mark(DOWN) on a visible Cell.
//!   Effect: an unseen object is not discovered by its first Mark. Risk: tag
//!   actions on discovery.
//! - AddContent skips the insert when the list's second object is already this
//!   one (`0x0047E903..0x0047E906`). Mark's +0x74 gate keeps an unmarked
//!   object out of its lists, and the one direct Place_Down follows its own
//!   Pick_Up, so VERA's lists never reach that state.
//! - Recalc runs only when the caller passes an overlay registry. Reveal and
//!   Limbo pass none, as before this owner existed.
//!
//! Readers of list membership follow the lists themselves: list walks
//! (`Simulation::cell_object_list_location`) and the vehicle plane
//! (`CellOccupationGrid::reconcile_entity`). The A* caches
//! (`block_index::contribution`, `bump_crush::blocker_plane_source`) still
//! take a Jumpjet's Air path layer as unlisted (RESIDUAL). Trigger: a landed
//! or low Jumpjet in a route. Effect: A* plans through a cell whose
//! `Can_Enter_Cell` refuses, so the mover waits or repaths at the boundary.
//! Frequency: occasional. Risk: detours differ from native A*, which asks
//! `Can_Enter_Cell` per neighbour.

use super::{ground_pose, locomotor::MovementLayer};
use crate::map::entities::EntityCategory;
use crate::map::overlay_types::OverlayTypeRegistry;
use crate::map::resolved_terrain::ResolvedTerrainGrid;
use crate::rules::ruleset::RuleSet;
use crate::sim::components::DriveCoord;
use crate::sim::occupancy::{CellListInsertion, OBJECT_OCCUPATION_BIT};
use crate::sim::pathfinding::PathGrid;
#[cfg(test)]
use crate::sim::world::LifecycleTestEvent;
use crate::sim::world::Simulation;

/// The raw occupation plane an object receiver writes at the stored
/// Location `coord` (Unit `0x007441B0`/`0x00744210`, Infantry
/// `0x005217C0`/`0x00521850`, Object `0x005F60A0`/`0x005F6120`): its Cell,
/// and whether the deck plane is selected. The deck needs Z at or above the
/// Cell's ground (`0x00578080`) plus the 416-lepton deck and, when
/// `needs_structural`, the Cell's structural bridge bit (`+0x140 & 0x100`).
/// Every PUT and the Object clear test that bit; the Unit and Infantry
/// clears test height alone.
pub(super) fn raw_occupation_plane(
    coord: DriveCoord,
    needs_structural: bool,
    terrain: Option<&ResolvedTerrainGrid>,
    path_grid: Option<&PathGrid>,
) -> ((u16, u16), MovementLayer) {
    let at = ((coord.x / 256) as u16, (coord.y / 256) as u16);
    let ground = ground_pose::ground_surface_z_at([coord.x, coord.y], false, terrain, path_grid)
        .unwrap_or(coord.z);
    let deck = coord.z >= ground.wrapping_add(crate::util::lepton::BRIDGE_DECK_HEIGHT_LEPTONS)
        && (!needs_structural
            || terrain
                .and_then(|terrain| terrain.cell(at.0, at.1))
                .is_some_and(|cell| cell.bridge_facts.has_structural_bridge()));
    (
        at,
        if deck {
            MovementLayer::Bridge
        } else {
            MovementLayer::Ground
        },
    )
}

impl Simulation {
    /// Mark(UP). +0x74 is cleared (`0x005F5913`) before the layer query, so
    /// a Jumpjet answers as an unmarked object. The list unlink and the raw
    /// clear use the Cell and OnBridge the object holds now.
    pub(crate) fn foot_mark_remove(
        &mut self,
        id: u64,
        rules: Option<&RuleSet>,
        registry: Option<&OverlayTypeRegistry>,
    ) -> bool {
        let Some(entity) = self.substrate.entities.get_mut(id) else {
            return false;
        };
        if entity.lifecycle.in_limbo || !entity.lifecycle.cell_marked {
            return false;
        }
        entity.lifecycle.cell_marked = false;
        self.foot_pick_up(id, rules, registry);
        true
    }

    /// `MapClass::Pick_Up` (`0x005687F0`) of a Foot: at the Cell and on the
    /// OnBridge list [`Self::foot_mark_cell`] picks, RemoveContent
    /// (`0x0047EA90`) unlinks it and runs its raw receiver tail, then Recalc.
    /// Mark(UP) runs it after clearing +0x74; the Aircraft Unload ejection
    /// calls it directly (`0x0041552E`).
    pub(crate) fn foot_pick_up(
        &mut self,
        id: u64,
        rules: Option<&RuleSet>,
        registry: Option<&OverlayTypeRegistry>,
    ) {
        let Some((cell, layer)) = self.foot_mark_cell(id, rules) else {
            return;
        };
        self.substrate
            .occupancy
            .remove_on_layer(cell.0, cell.1, id, layer);
        #[cfg(test)]
        self.trace_lifecycle_for_test(LifecycleTestEvent::RawOccupationListUnlinked);
        if self.foot_mark_raw(id, false) {
            #[cfg(test)]
            self.trace_lifecycle_for_test(LifecycleTestEvent::RawOccupationCleared);
        }
        self.recalculate_track_cell(cell, rules, registry);
    }

    pub(crate) fn foot_mark_put(
        &mut self,
        id: u64,
        rules: Option<&RuleSet>,
        registry: Option<&OverlayTypeRegistry>,
    ) -> bool {
        self.foot_mark_put_observed(id, rules, registry, &mut |_, _| {})
    }

    /// Mark(DOWN). `receive` runs after the list link, where AddContent's
    /// discovery sits.
    pub(super) fn foot_mark_put_observed(
        &mut self,
        id: u64,
        rules: Option<&RuleSet>,
        registry: Option<&OverlayTypeRegistry>,
        receive: &mut impl FnMut(&mut Simulation, u64),
    ) -> bool {
        let Some(entity) = self.substrate.entities.get_mut(id) else {
            return false;
        };
        if entity.lifecycle.in_limbo || entity.lifecycle.cell_marked {
            return false;
        }
        entity.lifecycle.cell_marked = true;
        self.foot_place_down(id, rules, registry, receive);
        #[cfg(test)]
        self.trace_lifecycle_for_test(LifecycleTestEvent::CellMarked);
        true
    }

    /// `MapClass::Place_Down` (`0x005683C0`) of a Foot: AddContent
    /// (`0x0047E8A0`) prepends it to the list [`Self::foot_mark_cell`] picks,
    /// `receive` runs where its discovery sits, then the raw receiver tail and
    /// Recalc. Both the Foot enable and the Location are read again after
    /// `receive`; Recalc still addresses the Cell it linked. Mark(DOWN) runs it
    /// after setting +0x74; the Aircraft Unload ejection calls it directly
    /// (`0x00415565`).
    pub(crate) fn foot_place_down(
        &mut self,
        id: u64,
        rules: Option<&RuleSet>,
        registry: Option<&OverlayTypeRegistry>,
        receive: &mut impl FnMut(&mut Simulation, u64),
    ) {
        let Some((cell, layer)) = self.foot_mark_cell(id, rules) else {
            return;
        };
        let (sub_cell, insertion) = {
            let entity = self.substrate.entities.get(id).expect("placed Foot");
            (
                entity.sub_cell,
                CellListInsertion::from_category(entity.category),
            )
        };
        self.substrate
            .occupancy
            .add(cell.0, cell.1, id, layer, sub_cell, insertion);
        #[cfg(test)]
        self.trace_lifecycle_for_test(LifecycleTestEvent::RawOccupationListLinked);
        receive(self, id);
        if self.foot_mark_raw(id, true) {
            #[cfg(test)]
            self.trace_lifecycle_for_test(LifecycleTestEvent::RawOccupationMarked);
        }
        self.recalculate_track_cell(cell, rules, registry);
    }

    /// The Cell Pick_Up/Place_Down address and the OnBridge list, or `None`
    /// when the layer query (`0x004D37A6`) is not Ground or the Location's
    /// Cell is not a real one (`0x00568471..0x0056848E`). `rules` only tells
    /// Air from Top, so a Ground answer does not depend on it.
    pub(crate) fn foot_mark_cell(
        &self,
        id: u64,
        rules: Option<&RuleSet>,
    ) -> Option<((u16, u16), MovementLayer)> {
        if self.entity_display_layer(id, rules)
            != Some(crate::sim::world::display_layers::DisplayLayer::GROUND)
        {
            return None;
        }
        let entity = self.substrate.entities.get(id)?;
        let cell = (entity.position.rx, entity.position.ry);
        if self.resolved_terrain.as_ref().is_some_and(|terrain| {
            terrain
                .native_fixed_cell_index(cell.0 as i16, cell.1 as i16)
                .is_none()
        }) {
            return None;
        }
        Some((
            cell,
            if entity.on_bridge {
                MovementLayer::Bridge
            } else {
                MovementLayer::Ground
            },
        ))
    }

    /// Remove/AddContent's receiver tail: Infantry returns first
    /// (`0x0047EAFE` / `0x0047E9EA`), then the Foot enable gates the object's
    /// vt+0xF4/+0xF0 on its stored Location. Whether a receiver ran.
    fn foot_mark_raw(&mut self, id: u64, put: bool) -> bool {
        let Some(entity) = self.substrate.entities.get(id) else {
            return false;
        };
        if !entity.foot_occupation_enabled || entity.category == EntityCategory::Infantry {
            return false;
        }
        let coord = ground_pose::position_world_coord(&entity.position);
        self.object_raw_receiver_at(id, coord, put)
    }

    /// The object's own raw occupation receiver on `coord`, vt+0xF0 (PUT) or
    /// vt+0xF4 (REMOVE): Unit `0x007441B0`/`0x00744210` (0x20), Infantry
    /// `0x005217C0`/`0x00521850` (its sub-cell) and, for Aircraft, ObjectClass
    /// `0x005F60A0`/`0x005F6120` (0x40). Mark reaches it through
    /// [`Self::foot_mark_raw`]; a direct call (the Jumpjet grounded reset,
    /// `0x0054D42F`) has no Foot enable gate. Whether a receiver ran.
    pub(crate) fn object_raw_receiver_at(&mut self, id: u64, coord: DriveCoord, put: bool) -> bool {
        let Some(entity) = self.substrate.entities.get(id) else {
            return false;
        };
        match entity.category {
            EntityCategory::Unit => {
                self.track_raw_mark_at(id, coord, put);
            }
            EntityCategory::Infantry => {
                let owner = entity.owner();
                super::walk_head::raw_at(
                    &mut self.substrate.raw_cell_occupation,
                    owner,
                    coord,
                    put,
                    self.resolved_terrain.as_ref(),
                    self.path_grid.as_deref(),
                );
            }
            EntityCategory::Aircraft => self.object_raw_mark_at(coord, put),
            _ => return false,
        }
        true
    }

    /// Aircraft vt+0xF0/+0xF4, the ObjectClass receivers `0x005F60A0` /
    /// `0x005F6120`: bit 0x40 on the plane [`raw_occupation_plane`] selects,
    /// testing the structural bridge bit on PUT and REMOVE alike.
    fn object_raw_mark_at(&mut self, coord: DriveCoord, put: bool) {
        let (at, layer) = raw_occupation_plane(
            coord,
            true,
            self.resolved_terrain.as_ref(),
            self.path_grid.as_deref(),
        );
        let raw = &mut self.substrate.raw_cell_occupation;
        match (put, layer) {
            (true, MovementLayer::Bridge) => raw.mark_deck(at.0, at.1, OBJECT_OCCUPATION_BIT),
            (true, _) => raw.mark_ground(at.0, at.1, OBJECT_OCCUPATION_BIT),
            (false, MovementLayer::Bridge) => raw.clear_deck(at.0, at.1, OBJECT_OCCUPATION_BIT),
            (false, _) => raw.clear_ground(at.0, at.1, OBJECT_OCCUPATION_BIT),
        }
    }
}

#[cfg(test)]
#[path = "foot_mark_tests.rs"]
mod tests;
