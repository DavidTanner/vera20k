//! Movement path management — path computation, repath-after-block, and bridge pathing support.
//!
//! Wraps the A* pathfinder for use by the movement tick: computes initial paths,
//! retries after blockages through the zone-aware search, and determines whether
//! an entity's locomotor supports layered bridge pathing.

use std::collections::BTreeSet;

use crate::map::resolved_terrain::ResolvedTerrainGrid;
use crate::rules::locomotor_type::{LocomotorKind, MovementZone, SpeedType};
use crate::sim::find_nearby_cell::{
    NearbyAnchorGate, NearbyFootprint, NearbyQuery, PassabilityArgs, RADIUS_HARD_CAP,
    find_nearby_passable_cell,
};
use crate::sim::movement::locomotor::{LocomotorState, MovementLayer};
use crate::sim::pathfinding::LayeredEntityBlockMap;
#[path = "path_finishing.rs"]
mod path_finishing;
use crate::sim::pathfinding::terrain_cost::TerrainCostGrid;
use crate::sim::pathfinding::zone_map::{ZONE_INVALID, ZoneGrid};
use crate::sim::pathfinding::zone_search;
use crate::sim::pathfinding::{
    MAX_PATH_SEGMENT_STEPS, PathGrid, SearchMarkerOverlay, truncate_layered_path,
};

use super::{MoverPathFacts, PathfindingContext};

#[cfg(test)]
pub(crate) fn reset_path_search_used_zone_grid_marker() {
    PATH_SEARCH_USED_ZONE_GRID.with(|used| used.set(false));
}

#[cfg(test)]
pub(crate) fn path_search_used_zone_grid_marker() -> bool {
    PATH_SEARCH_USED_ZONE_GRID.with(|used| used.get())
}

#[cfg(test)]
thread_local! {
    static PATH_SEARCH_USED_ZONE_GRID: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// **VERA-internal, gamemd equivalent UNCHECKED**: the locomotor whitelist and
/// the zero-size grid sentinel below. `FootClass::Find_Path` @ `0x004D3920`
/// gates on `vtable+0x2CC`, not on locomotor kind.
///
/// **Hover was added 2026-08-27 (matrix row T1-01).** The exclusion was the
/// whole of that defect: a Hover mover never reached the layered builder, so
/// `build_flat_fallback_layers` stamped `Ground` on every node of its path
/// including the deck ones, and the crossing loop's terrain test then read the
/// deck cell's raw ground-walkability — the riverbed under the span — and
/// refused. The order was never dropped, so a Robot Tank ordered across a high
/// bridge accelerated into the abutment and was snapped back to cell centre
/// about thirteen times a second, indefinitely. The bridge legality check
/// itself passed every time; it was never consulted about the right plane.
/// Measured on BayOPigs.mmx: seven refusals, every one `layer_walkable`, zero
/// `bridge_traversal`.
///
/// Stock YR gives the Hover CLSID to `[LCRF]`, `[ROBO]`, `[SAPC]` and `[YHVR]`
/// — the Robot Tank and all three amphibious transports. (`ROBO` also carries
/// `TooBigToFitUnderBridge=true`, but that flag gates nothing in movement:
/// it is draw-pipeline-only (see `ObjectType::too_big_to_fit_under_bridge`)
/// and `Can_Enter_Cell` never reads it. It is not why a Robot Tank
/// could not use a span, and it did not stop one driving under one either.)
///
/// **What the one native gate on this path actually does.** VERIFIED 2026-08-27
/// by direct reads, with one stated premise rather than a read:
///   - *Premise (not a Ghidra read):* the movers this predicate admits are all
///     `[VehicleTypes]` — `ROBO`, `LCRF`, `SAPC`, `YHVR` alongside `MTNK`,
///     checked in `ini/rulesmd.ini` — hence `UnitClass` instances. That is what
///     makes the vtable read below the same slot the `CALL [EDX + 0x2CC]`
///     dispatches through for them: the read is at a fixed vtable, the call is
///     through the mover's own. Named because reading an offset is not the same
///     as proving a slot, and conflating the two has produced a wrong label in
///     this project before.
///   - `UnitClass`'s vtable is `0x007F5C70` — installed by
///     `UnitClass::Constructor` @ `0x0073543A`, and by its destructor and
///     `Load`. Identity rests on that existing Ghidra label; **UNCHECKED**
///     against RTTI/COL, which is the reliable route here.
///   - `0x007F5C70 + 0x2CC` = `0x007F5F3C`, which holds `0x004D3810`. Slot read
///     directly; not inferred from a callee offset, which is the trap that has
///     produced a wrong label in this project before.
///   - `FootClass::Find_Path` calls that slot at `0x004D397F`
///     (`CALL dword ptr [EDX + 0x2CC]`), and on a false return takes the
///     clear-and-return-0 exit at `0x004D3989`.
///   - `0x004D3810` (`CanReachDestination`) reads the type class through
///     `vtable+0x84`, takes `TechnoTypeClass+0x5B4` (**MovementZone**), returns
///     1 at once when that is `-1`, and otherwise tail-calls
///     `MapClass::Can_Reach_Zone` with it.
///
/// So the gate is a zone **reachability abort** — should the search run at all —
/// keyed on MovementZone, with no locomotor term. **Scoped claim:** at this
/// gate, nothing selects a pathing plane by locomotor kind. Whether anything
/// elsewhere in the binary does is UNCHECKED, and this comment does not assert
/// a binary-wide negative.
///
/// That is enough, because the change here **deletes** a VERA gate rather than
/// adding one: removing a restriction needs the absence of a verified native
/// gate demanding it, not an affirmative native proof. The fix's positive
/// evidence is production — two ordinary undisabled crossings on two retail
/// maps with different span axes.
///
/// Do NOT reason "same vtable slot, therefore same answer": a shared virtual
/// that reads type data answers differently per type, and this one does exactly
/// that. `ROBO` is `MovementZone=AmphibiousDestroyer` and `MTNK` is `Normal`, so
/// `0x004D3810` genuinely can separate them — it just separates them by zone,
/// not by locomotor, and never by pathing plane. An earlier draft of this
/// comment argued from `[VehicleTypes]` membership to a shared slot value to a
/// shared answer; the middle step is true and the last does not follow.
///
/// `Can_Enter_Cell` @ `0x0073F0A0`, `CheckBridgeTraversal` @ `0x004D9C60` and
/// the `ILocomotion+0x1C` slot are **UNCHECKED here** — reported as corroboration
/// that the downstream legality path is locomotor-agnostic, not re-read at this
/// callsite, and not the gate. Do not cite them as if they resolved `+0x2CC`.
///
/// The remaining `Drive | Walk | Hover` list stays VERA-internal. The other
/// five kinds — Ship, Fly, Teleport, Jumpjet and Rocket — are excluded because
/// admitting them is a separate question with its own blast radius, not because
/// the gate above excludes them; it excludes nothing by kind.
pub(super) fn supports_layered_bridge_pathing(
    loco: &LocomotorState,
    grid: &PathGrid,
    on_bridge: bool,
) -> bool {
    if grid.width() == 0 || grid.height() == 0 {
        return false;
    }
    matches!(
        loco.kind,
        LocomotorKind::Drive | LocomotorKind::Walk | LocomotorKind::Hover
    ) || on_bridge
}

fn is_bridge_layer_walkable(grid: Option<&PathGrid>, cell: (u16, u16)) -> bool {
    grid.is_some_and(|g| g.is_walkable_on_layer(cell.0, cell.1, MovementLayer::Bridge))
}

/// **VERA-internal, gamemd has no equivalent.** A goal reachable only on the
/// bridge layer makes `find_move_path` drop the order outright.
/// `FootClass::Find_Path` @ `0x004D3920` has exactly **one** clear-and-return-0
/// exit — `vtable+0x2CC` at `0x004D3989`, whose tail is
/// `POP/POP/POP; ADD ESP,0x1F9C; RET 0xC` at `0x004D3993`. Its other two
/// `MOV [EBP+0x5E0],-1` sites are not failures: `0x004D393A` is the entry
/// `prefix == 0` clear, and `0x004D3E22` clears on the preserved-prefix length
/// and then falls straight into `FootClass::Run_AStar` at `0x004D3E4B`. Nothing
/// in that head gates the search on a bridge layer — Find_Path does read
/// `Cell+0x140 & 0x100` in its *failure* tail, when it decides scatter and
/// team-removal, but that is after the search, not instead of it.
///
/// Trigger: a move ordered onto a cell whose ground plane is impassable but
/// whose bridge deck is walkable. Player effect: VERA refuses the order where
/// retail runs the search. Frequency: clicks on a bridge deck over water or a
/// cliff — a few times a match on any map with a high bridge. Downstream risk:
/// removing it re-opens the layered-goal case the bridge-parity boundary at
/// the top of this module records, so it must land with that work.
fn is_bridge_only_goal(grid: &PathGrid, goal: (u16, u16)) -> bool {
    !grid.is_walkable(goal.0, goal.1) && is_bridge_layer_walkable(Some(grid), goal)
}

pub(super) fn is_move_goal_walkable(
    grid: &PathGrid,
    goal: (u16, u16),
    movement_zone: Option<MovementZone>,
    resolved_terrain: Option<&ResolvedTerrainGrid>,
) -> bool {
    if movement_zone.is_some_and(|mz| mz.is_water_mover()) {
        return crate::sim::pathfinding::is_cell_passable_for_mover(
            grid,
            goal.0,
            goal.1,
            movement_zone,
            resolved_terrain,
        );
    }
    grid.is_any_layer_walkable(goal.0, goal.1)
}

fn nearest_move_goal(
    grid: &PathGrid,
    goal: (u16, u16),
    max_radius: u16,
    blocked_cells: Option<&BTreeSet<(u16, u16)>>,
    movement_zone: Option<MovementZone>,
    resolved_terrain: Option<&ResolvedTerrainGrid>,
) -> Option<(u16, u16)> {
    if !movement_zone.is_some_and(|mz| mz.is_water_mover()) {
        return grid.nearest_walkable_any_layer(goal.0, goal.1, max_radius, blocked_cells, None);
    }

    let check = |x: u16, y: u16| {
        is_move_goal_walkable(grid, (x, y), movement_zone, resolved_terrain)
            && blocked_cells.map_or(true, |blocks| !blocks.contains(&(x, y)))
    };
    if check(goal.0, goal.1) {
        return Some(goal);
    }
    for radius in 1..=max_radius {
        let r = radius as i32;
        for d in -r..=r {
            let candidates = [
                (goal.0 as i32 + d, goal.1 as i32 - r),
                (goal.0 as i32 + d, goal.1 as i32 + r),
                (goal.0 as i32 - r, goal.1 as i32 + d),
                (goal.0 as i32 + r, goal.1 as i32 + d),
            ];
            for (x, y) in candidates {
                if x < 0 || y < 0 || x >= grid.width() as i32 || y >= grid.height() as i32 {
                    continue;
                }
                let candidate = (x as u16, y as u16);
                if check(candidate.0, candidate.1) {
                    return Some(candidate);
                }
            }
        }
    }
    None
}

pub(super) fn resolve_requested_move_goal(
    grid: &PathGrid,
    goal: (u16, u16),
    blocked_cells: Option<&BTreeSet<(u16, u16)>>,
    movement_zone: Option<MovementZone>,
    resolved_terrain: Option<&ResolvedTerrainGrid>,
    max_radius: u16,
) -> Option<(u16, u16)> {
    if is_move_goal_walkable(grid, goal, movement_zone, resolved_terrain)
        && blocked_cells.map_or(true, |blocks| !blocks.contains(&goal))
    {
        return Some(goal);
    }

    nearest_move_goal(
        grid,
        goal,
        max_radius,
        blocked_cells,
        movement_zone,
        resolved_terrain,
    )
}

/// Which zone layer a destination cell is resolved on.
///
/// Gamemd passes the destination cell's own bridge bit into `Can_Reach_Zone` and
/// into the nearby-passable-cell search from the ground-click destination
/// resolver, so a click on a bridge deck is answered on the bridge layer.
fn goal_zone_layer(
    resolved_terrain: Option<&ResolvedTerrainGrid>,
    goal: (u16, u16),
) -> MovementLayer {
    let on_bridge = resolved_terrain
        .and_then(|terrain| terrain.cell(goal.0, goal.1))
        .is_some_and(|cell| cell.bridge_facts.has_structural_bridge());
    if on_bridge {
        MovementLayer::Bridge
    } else {
        MovementLayer::Ground
    }
}

/// Gamemd's order-time reachability gate and zone-constrained destination
/// substitution.
///
/// The ground-click destination resolver runs `Can_Reach_Zone(myCell, clicked,
/// mzRow, myLayer, clickedBridgeBit, 0)` before the mission is installed:
///
/// * success — the clicked cell is used verbatim;
/// * failure — **the order is still accepted**. The resolver takes the mover's
///   own zone id and runs the nearby-passable-cell search seeded at the clicked
///   cell, requiring that zone, so the unit drives to the near bank of the river
///   / the near side of the cliff instead of standing still. Only a search that
///   finds no candidate at all drops the order (the engine's null-cell branch).
///
/// Returns `Some(goal)` unchanged when there is no zone data — that is the
/// engine's `mzRow == -1` short-circuit, which returns "reachable".
///
/// Selection mode: the search is given the clicked cell as its distance
/// reference, so the substitute is the nearest surviving candidate to the click.
/// The engine's alternative frame-counter selection is reached only when the
/// reference coord is the null cell; which of the two this call site takes is
/// UNCHECKED, and nearest-to-click is the deterministic choice that needs no
/// frame input threaded through every move-order caller.
pub(super) fn resolve_reachable_move_goal(
    grid: &PathGrid,
    zone_grid: Option<&ZoneGrid>,
    resolved_terrain: Option<&ResolvedTerrainGrid>,
    start: (u16, u16),
    start_layer: MovementLayer,
    goal: (u16, u16),
    zone_mz: MovementZone,
    speed_type: SpeedType,
) -> Option<(u16, u16)> {
    let Some(zone_grid) = zone_grid else {
        return Some(goal);
    };
    let goal_layer = goal_zone_layer(resolved_terrain, goal);
    if start == goal || zone_grid.can_reach(zone_mz, start, start_layer, goal, goal_layer) {
        return Some(goal);
    }

    let Some(zone_map) = zone_grid.map_for(zone_mz) else {
        return Some(goal);
    };
    let reduced_zone = zone_map.zone_at(start.0, start.1, start_layer);
    if reduced_zone == ZONE_INVALID {
        // The mover itself has no zone — a unit standing on a footprint cell, a
        // factory exit or an unmapped bridge cell. There is no zone to require,
        // so the order passes through unchanged rather than being refused.
        // VERA-internal; the engine labels such a node rather than leaving it
        // undefined, so the gamemd equivalent here is UNCHECKED.
        return Some(goal);
    }

    // Engine radius cap: min(map width + map height, 32).
    let radius_cap = grid
        .width()
        .saturating_add(grid.height())
        .min(RADIUS_HARD_CAP);
    let query = NearbyQuery {
        native_cells: None,
        raw_occupation: None,
        passability: PassabilityArgs {
            speed_type,
            // The candidates' zones come from GetZoneID (`0x0056D230`) when
            // terrain is present (cell_rect), so the required zone must too.
            required_zone_id: match resolved_terrain {
                Some(terrain) => zone_grid.get_zone_id_native(
                    terrain,
                    start,
                    zone_mz,
                    start_layer == MovementLayer::Bridge,
                ),
                None => Some(u32::from(reduced_zone)),
            },
            movement_zone: zone_mz,
            bridge_aware_zone: goal_layer == MovementLayer::Bridge,
        },
        footprint: NearbyFootprint::SINGLE,
        anchor_gate: NearbyAnchorGate::UnverifiedCompatibilityBypass,
        allow_bridge_cells: true,
        check_height: false,
        check_occupancy: false,
        radius_cap,
        target_cell: Some((goal.0 as i32, goal.1 as i32)),
        path_grid: Some(grid),
        resolved_terrain,
        overlay_grid: None,
        occupancy: None,
        entities: None,
        zone_grid: Some(zone_grid),
        playfield_bounds: None,
    };
    // `target_cell` is set, so the frame counter is not consulted for selection.
    find_nearby_passable_cell((goal.0 as i32, goal.1 as i32), &query, 0)
}

pub(super) fn find_move_path(
    ctx: PathfindingContext<'_>,
    layered_pathing: bool,
    start: (u16, u16),
    start_layer: MovementLayer,
    goal: (u16, u16),
    terrain_costs: Option<&TerrainCostGrid>,
    entity_blocks: Option<&BTreeSet<(u16, u16)>>,
    ground_blocks: Option<&BTreeSet<(u16, u16)>>,
    bridge_blocks: Option<&BTreeSet<(u16, u16)>>,
    zone_mz: MovementZone,
    movement_zone: Option<MovementZone>,
    entity_block_map: Option<&LayeredEntityBlockMap>,
    facts: MoverPathFacts,
    allow_zone_hierarchy: bool,
) -> Option<(Vec<(u16, u16)>, Vec<MovementLayer>)> {
    find_move_path_with_marker(
        ctx,
        layered_pathing,
        start,
        start_layer,
        goal,
        terrain_costs,
        entity_blocks,
        ground_blocks,
        bridge_blocks,
        zone_mz,
        movement_zone,
        entity_block_map,
        None,
        facts,
        allow_zone_hierarchy,
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn find_move_path_with_marker(
    ctx: PathfindingContext<'_>,
    layered_pathing: bool,
    start: (u16, u16),
    start_layer: MovementLayer,
    goal: (u16, u16),
    terrain_costs: Option<&TerrainCostGrid>,
    entity_blocks: Option<&BTreeSet<(u16, u16)>>,
    ground_blocks: Option<&BTreeSet<(u16, u16)>>,
    bridge_blocks: Option<&BTreeSet<(u16, u16)>>,
    zone_mz: MovementZone,
    movement_zone: Option<MovementZone>,
    entity_block_map: Option<&LayeredEntityBlockMap>,
    marker_overlay: Option<&SearchMarkerOverlay>,
    facts: MoverPathFacts,
    allow_zone_hierarchy: bool,
) -> Option<(Vec<(u16, u16)>, Vec<MovementLayer>)> {
    find_move_path_with_marker_detailed(
        ctx,
        layered_pathing,
        start,
        start_layer,
        goal,
        terrain_costs,
        entity_blocks,
        ground_blocks,
        bridge_blocks,
        zone_mz,
        movement_zone,
        entity_block_map,
        marker_overlay,
        facts,
        allow_zone_hierarchy,
        None,
    )
    .ok()
}

/// Internal request failures retain the layer where the return originated.
/// Option callers preserve their established API through the facade below.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum MovePathFailure {
    MissingGrid,
    BridgeOnlyGoal,
    Search(zone_search::PathSearchFailure),
}

#[allow(clippy::too_many_arguments)]
pub(super) fn find_move_path_with_marker_detailed(
    ctx: PathfindingContext<'_>,
    layered_pathing: bool,
    start: (u16, u16),
    start_layer: MovementLayer,
    goal: (u16, u16),
    terrain_costs: Option<&TerrainCostGrid>,
    entity_blocks: Option<&BTreeSet<(u16, u16)>>,
    ground_blocks: Option<&BTreeSet<(u16, u16)>>,
    bridge_blocks: Option<&BTreeSet<(u16, u16)>>,
    zone_mz: MovementZone,
    movement_zone: Option<MovementZone>,
    entity_block_map: Option<&LayeredEntityBlockMap>,
    marker_overlay: Option<&SearchMarkerOverlay>,
    facts: MoverPathFacts,
    allow_zone_hierarchy: bool,
    foot_entry: Option<&dyn crate::sim::pathfinding::SearchFootEntry>,
) -> Result<(Vec<(u16, u16)>, Vec<MovementLayer>), MovePathFailure> {
    let grid = ctx.path_grid.ok_or(MovePathFailure::MissingGrid)?;
    let zone_grid = ctx.zone_grid;
    #[cfg(test)]
    if zone_grid.is_some() {
        PATH_SEARCH_USED_ZONE_GRID.with(|used| used.set(true));
    }
    let resolved_terrain = ctx.resolved_terrain;
    let merged_entity_blocks = entity_blocks.cloned().unwrap_or_default();
    let entity_blocks = (!merged_entity_blocks.is_empty()).then_some(&merged_entity_blocks);
    // Build the Foot `+0x1AC` cost-class producer here, at the search boundary:
    // the tables are per pass and on `ctx`, the rest is per mover and on
    // `facts`. Before this the search passed `wall_cost: None` at every site,
    // so `astar_search` answered 7 for a wall - "does not expand" - and a mover
    // whose only route crossed a wall line got no path at all. Native prices it
    // instead: `AStar_main_loop @ 0x00429A90` calls the slot, the wall arm
    // answers 4 for an allied wall and 5 for any other (`0x0073F4EB`,
    // `0x0073F50E`), and `AStar_compute_edge_cost @ 0x00429830` multiplies the
    // step by 60 and 20 from the table at `0x0081870C`. So the route exists and
    // is merely expensive, and the runtime arm already wired in
    // `movement_step` attacks the wall when the mover reaches it.
    //
    // `None` where the mover has no owner keeps the old search exactly: the
    // wall arm needs a house to compare, and an unowned mover answers 7.
    // `interner.is_some()` is part of the gate, not an afterthought: the arm
    // fails closed without it and would answer 7 for every refused neighbour,
    // while still paying a full `evaluate_can_enter_cell` per call - two cost
    // grid reads, bridge lookups, overlay and registry probes. A* refuses far
    // more neighbours than it accepts, so at 20k movers that is pure waste.
    // Review caught it: the two Drive contexts set the tables but leave the
    // interner `None`, so every blocked repath was paying it for a guaranteed 7.
    let wall_classifier = ctx
        .wall_tables
        .filter(|tables| tables.interner.is_some())
        .zip(facts.owner)
        .map(
            |(tables, owner)| crate::sim::pathfinding::cell_entry::WallSearchCostClassifier {
                wall: crate::sim::pathfinding::cell_entry::WallArmContext {
                    overlay_grid: tables.overlay_grid,
                    overlay_registry: tables.overlay_registry,
                    alliances: tables.alliances,
                    interner: tables.interner,
                    mover_owner: Some(owner),
                    is_armed: facts.is_armed,
                    warhead_wall: facts.warhead_wall,
                    warhead_wood: facts.warhead_wood,
                },
                path_grid: Some(grid),
                resolved_terrain,
                terrain_costs,
                movement_zone,
                speed_type: facts.speed_type,
                is_infantry: facts.is_infantry,
                mover_is_crusher: facts.mover_is_crusher,
            },
        );
    let wall_cost = wall_classifier
        .as_ref()
        .map(|c| c as &dyn crate::sim::pathfinding::SearchCellCostClassifier);
    // A slave's deposit Cells answer through its own `Can_Enter_Cell` arm,
    // ahead of the wall arm (`sim::slave_deposit`).
    let slave_classifier = (facts.slave_deposit_cells != [None, None]).then_some(
        crate::sim::pathfinding::cell_entry::SlaveDepositSearchClassifier {
            cells: facts.slave_deposit_cells,
            inner: wall_cost,
            path_grid: Some(grid),
            resolved_terrain,
            terrain_costs,
            movement_zone,
            speed_type: facts.speed_type,
        },
    );
    let wall_cost = slave_classifier
        .as_ref()
        .map(|c| c as &dyn crate::sim::pathfinding::SearchCellCostClassifier)
        .or(wall_cost);
    if layered_pathing {
        let layered_result = zone_search::find_layered_path_zoned_marker_detailed(
            grid,
            ground_blocks,
            bridge_blocks,
            start,
            start_layer,
            goal,
            zone_grid,
            zone_mz,
            terrain_costs,
            movement_zone,
            resolved_terrain,
            entity_block_map,
            marker_overlay,
            ctx.blocker_neighbor_counts,
            crate::sim::pathfinding::MoverSearchFacts {
                urgency: facts.urgency,
                mover_is_crusher: facts.mover_is_crusher,
                is_infantry: facts.is_infantry,
                wall_cost,
                foot_entry,
            },
            allow_zone_hierarchy,
            ctx.playfield_bounds,
        );
        let path = layered_result.map_err(MovePathFailure::Search)?;

        log::trace!(
            "find_move_path: layered A* succeeded ({:?}→{:?}), {} steps",
            start,
            goal,
            path.len(),
        );
        let (coords, layers) = path_finishing::finish(
            path,
            ctx,
            grid,
            foot_entry,
            terrain_costs,
            movement_zone,
            facts,
            ground_blocks,
            bridge_blocks,
            entity_block_map,
            marker_overlay,
        )
        .map_err(|cause| {
            MovePathFailure::Search(zone_search::PathSearchFailure::CellEntryUnavailable(cause))
        })?;
        let (coords, layers) = truncate_layered_path(coords, layers, MAX_PATH_SEGMENT_STEPS);
        return Ok((coords, layers));
    }

    if is_bridge_only_goal(grid, goal) {
        return Err(MovePathFailure::BridgeOnlyGoal);
    }

    let path = zone_search::find_path_zoned_marker_detailed(
        grid,
        start,
        goal,
        terrain_costs,
        entity_blocks,
        zone_grid,
        zone_mz,
        movement_zone,
        resolved_terrain,
        entity_block_map,
        marker_overlay,
        ctx.blocker_neighbor_counts,
        crate::sim::pathfinding::MoverSearchFacts {
            urgency: facts.urgency,
            mover_is_crusher: facts.mover_is_crusher,
            is_infantry: facts.is_infantry,
            wall_cost,
            foot_entry,
        },
        allow_zone_hierarchy,
        ctx.playfield_bounds,
    )
    .map_err(MovePathFailure::Search)?;

    let (path, path_layers) = path_finishing::finish(
        path,
        ctx,
        grid,
        foot_entry,
        terrain_costs,
        movement_zone,
        facts,
        entity_blocks,
        entity_blocks,
        entity_block_map,
        marker_overlay,
    )
    .map_err(|cause| {
        MovePathFailure::Search(zone_search::PathSearchFailure::CellEntryUnavailable(cause))
    })?;
    Ok(truncate_layered_path(
        path,
        path_layers,
        MAX_PATH_SEGMENT_STEPS,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::resolved_terrain::{
        ResolvedTerrainCell, ResolvedTerrainGrid, YR_CELL_LAND_TUNNEL,
    };
    use crate::map::tube_facts::{TubeFact, TubeId};
    use crate::sim::pathfinding::passability::LandType;
    use crate::sim::pathfinding::zone_map::ZoneGrid;

    fn make_resolved_cell(rx: u16, ry: u16) -> ResolvedTerrainCell {
        ResolvedTerrainCell {
            ..crate::map::resolved_terrain::test_flat_cell(rx, ry)
        }
    }

    #[test]
    fn path_failure_detail_distinguishes_missing_grid_from_executed_search() {
        use crate::sim::pathfinding::zone_search::PathSearchFailure;
        fn search(
            grid: Option<&PathGrid>,
        ) -> Result<(Vec<(u16, u16)>, Vec<MovementLayer>), MovePathFailure> {
            find_move_path_with_marker_detailed(
                PathfindingContext {
                    wall_tables: None,
                    path_grid: grid,
                    zone_grid: None,
                    resolved_terrain: None,
                    playfield_bounds: None,
                    blocker_neighbor_counts: None,
                },
                false,
                (0, 1),
                MovementLayer::Ground,
                (4, 1),
                None,
                None,
                None,
                None,
                MovementZone::Normal,
                Some(MovementZone::Normal),
                None,
                None,
                super::MoverPathFacts::without_wall_arm(0, false, true),
                false,
                None,
            )
        }
        assert_eq!(search(None), Err(MovePathFailure::MissingGrid));
        let mut grid = PathGrid::test_all_passable(5, 3);
        let (path, layers) = search(Some(&grid)).expect("open cell search");
        assert_eq!(path.first(), Some(&(0, 1)));
        assert_eq!(path.last(), Some(&(4, 1)));
        assert_eq!(layers, vec![MovementLayer::Ground; path.len()]);
        for y in 0..3 {
            grid.set_blocked(2, y, true);
        }
        assert_eq!(
            search(Some(&grid)),
            Err(MovePathFailure::Search(
                PathSearchFailure::CellSearchExhausted
            ))
        );
    }

    #[test]
    fn water_mover_goal_redirect_stays_on_water_cells() {
        let mut cells = Vec::new();
        for ry in 0..3 {
            for rx in 0..3 {
                let is_water = matches!((rx, ry), (0, 1) | (1, 1) | (2, 1));
                cells.push(ResolvedTerrainCell {
                    land_type: if is_water {
                        LandType::Water.as_index()
                    } else {
                        LandType::Clear.as_index()
                    },
                    is_water,
                    ..make_resolved_cell(rx, ry)
                });
            }
        }
        let terrain = ResolvedTerrainGrid::from_cells(3, 3, cells);
        let grid = PathGrid::from_resolved_terrain(&terrain);
        let mut blocked = BTreeSet::new();
        blocked.insert((1, 1));

        let redirected = resolve_requested_move_goal(
            &grid,
            (1, 1),
            Some(&blocked),
            Some(MovementZone::Water),
            Some(&terrain),
            2,
        )
        .expect("water mover should find an alternate water goal");

        assert!(
            matches!(redirected, (0, 1) | (2, 1)),
            "water mover redirect should stay on water, got {:?}",
            redirected
        );
    }

    #[test]
    fn explicit_tube_path_survives_zone_precheck_and_smoothing() {
        let mut cells: Vec<_> = (0..5).map(|x| make_resolved_cell(x, 0)).collect();
        cells[0].yr_cell_land_type = YR_CELL_LAND_TUNNEL;
        cells[0].tube_index = Some(TubeId(0));
        for cell in &mut cells[1..4] {
            cell.ground_walk_blocked = true;
            cell.base_ground_walk_blocked = true;
        }
        let terrain = ResolvedTerrainGrid::from_cells_with_tubes(
            5,
            1,
            cells,
            vec![TubeFact::explicit((0, 0), (4, 0), 2, vec![2, 2, 2, 2])],
        );
        let grid = PathGrid::from_resolved_terrain(&terrain);
        let zone_grid = ZoneGrid::build_with_terrain(&grid, &terrain, &[], 5, 1);

        let (path, layers) = find_move_path(
            PathfindingContext {
                wall_tables: None,
                path_grid: Some(&grid),
                zone_grid: Some(&zone_grid),
                resolved_terrain: Some(&terrain),
                playfield_bounds: None,
                blocker_neighbor_counts: None,
            },
            false,
            (0, 0),
            MovementLayer::Ground,
            (4, 0),
            None,
            None,
            None,
            None,
            MovementZone::Normal,
            Some(MovementZone::Normal),
            None,
            super::MoverPathFacts::without_wall_arm(0, false, false),
            true,
        )
        .expect("movement path should use explicit tube despite disconnected zones");

        assert_eq!(path, vec![(0, 0), (4, 0)]);
        assert_eq!(layers, vec![MovementLayer::Ground, MovementLayer::Ground]);
    }

    #[test]
    fn move_path_marker_overlay_survives_path_smoothing() {
        let grid = PathGrid::test_all_passable(5, 3);
        let mut marker_overlay = SearchMarkerOverlay::new();
        marker_overlay.toggle((1, 1));
        marker_overlay.toggle((2, 1));
        marker_overlay.toggle((3, 1));

        let (path, layers) = find_move_path_with_marker(
            PathfindingContext {
                wall_tables: None,
                path_grid: Some(&grid),
                zone_grid: None,
                resolved_terrain: None,
                playfield_bounds: None,
                blocker_neighbor_counts: None,
            },
            false,
            (0, 1),
            MovementLayer::Ground,
            (4, 1),
            None,
            None,
            None,
            None,
            MovementZone::Normal,
            Some(MovementZone::Normal),
            None,
            Some(&marker_overlay),
            super::MoverPathFacts::without_wall_arm(0, false, false),
            true,
        )
        .expect("marker overlay should still allow a path");

        assert_eq!(path.first().copied(), Some((0, 1)));
        assert_eq!(path.last().copied(), Some((4, 1)));
        assert!(
            !path
                .iter()
                .any(|cell| matches!(cell, (1, 1) | (2, 1) | (3, 1))),
            "path smoothing must not collapse the marker-avoiding route back through {:?}",
            path
        );
        assert_eq!(path.len(), layers.len());
    }

    #[test]
    fn gsi_04_12_layered_failure_does_not_retry_flat_ground_path() {
        let mut grid = PathGrid::test_all_passable(3, 1);
        // Start on a non-transition bridge deck. Native layered A* cannot
        // descend directly to the ground-only neighbor, while a separate flat
        // search from the same coordinate would incorrectly find (0..=2, 0).
        grid.set_cell_for_test(0, 0, 0, true, false);
        assert!(grid.is_walkable(2, 0), "goal must remain ground-walkable");

        let flat = find_move_path(
            PathfindingContext {
                wall_tables: None,
                path_grid: Some(&grid),
                zone_grid: None,
                resolved_terrain: None,
                playfield_bounds: None,
                blocker_neighbor_counts: None,
            },
            false,
            (0, 0),
            MovementLayer::Ground,
            (2, 0),
            None,
            None,
            None,
            None,
            MovementZone::Normal,
            Some(MovementZone::Normal),
            None,
            super::MoverPathFacts::without_wall_arm(0, false, false),
            true,
        )
        .expect("fixture must prove the removed ground-only retry could succeed");
        assert_eq!(flat.0.last().copied(), Some((2, 0)));

        let layered = find_move_path(
            PathfindingContext {
                wall_tables: None,
                path_grid: Some(&grid),
                zone_grid: None,
                resolved_terrain: None,
                playfield_bounds: None,
                blocker_neighbor_counts: None,
            },
            true,
            (0, 0),
            MovementLayer::Bridge,
            (2, 0),
            None,
            None,
            None,
            None,
            MovementZone::Normal,
            Some(MovementZone::Normal),
            None,
            super::MoverPathFacts::without_wall_arm(0, false, false),
            true,
        );

        assert!(
            layered.is_none(),
            "a failed layered search must remain failed instead of launching a second flat A*"
        );
    }

    /// A wall line the mover may shoot is **routed through at cost**, not
    /// reported unreachable.
    ///
    /// This is the search half of ledger row I9b, and it is the test the first
    /// version of that work did not have. `astar_search` consults the Foot
    /// `+0x1AC` cost-class producer only after its own `neighbor_passable` has
    /// refused a cell; a `Wall=yes` overlay is `overlay_blocks`, so every wall
    /// cell takes that path. With no producer the search answers 7 - "does not
    /// expand" - and a mover whose only route crosses the line gets **no path
    /// at all**. Native prices it instead: `AStar_main_loop 0x00429A90` calls
    /// the slot, the arm answers 5 for an enemy wall (`0x0073F50E`), and
    /// `AStar_compute_edge_cost 0x00429830` multiplies the step by 20 from the
    /// table at `0x0081870C`.
    ///
    /// The two halves of this test differ **only** in the mover's own facts, so
    /// it fails if `find_move_path_with_marker_detailed` goes back to passing
    /// `wall_cost: None`: both halves would then answer `None`.
    #[test]
    fn a_shootable_wall_line_is_routed_through_instead_of_refusing_the_order() {
        use crate::map::resolved_terrain::{ResolvedTerrainCell, ResolvedTerrainGrid, zone_class};
        use crate::rules::ini_parser::IniFile;

        // Column x=2 is a solid, unowned `Wall=yes` line across a 5x3 board, so
        // the only route from (0,1) to (4,1) crosses it.
        let mut cells = Vec::with_capacity(15);
        for ry in 0..3u16 {
            for rx in 0..5u16 {
                let mut cell = ResolvedTerrainCell::clear_for_test(rx, ry);
                cell.speed_costs.track = Some(100);
                if rx == 2 {
                    cell.zone_type = zone_class::WALL;
                    cell.overlay_zone_type = Some(zone_class::WALL);
                    cell.overlay_blocks = true;
                }
                cells.push(cell);
            }
        }
        let terrain = ResolvedTerrainGrid::from_cells(5, 3, cells);
        let grid = PathGrid::from_resolved_terrain(&terrain);
        // A REAL cost grid, because its absence is what hid the defect this
        // test was written to prove. `Wall=yes` sets `overlay_blocks`, which
        // `TerrainCostGrid` turns into `COST_BLOCKED` for every SpeedType, so
        // the first version of this test - which passed `None` here - went green
        // while production still refused every wall a few lines further on.
        let costs = TerrainCostGrid::from_resolved_terrain(&terrain, SpeedType::Track);

        let registry = crate::rules::overlay_types::OverlayTypeRegistry::from_ini(
            &IniFile::from_str("[OverlayTypes]\n0=GAWALL\n\n[GAWALL]\nWall=yes\n"),
            None,
        );
        let mut overlays = crate::sim::overlay_grid::OverlayGrid::new(5, 3);
        for ry in 0..3u16 {
            overlays.cell_mut(2, ry).overlay_id = Some(0);
        }
        // Intern before cloning the thread-local interner.
        let mover = crate::sim::intern::test_intern("Americans");
        let interner = crate::sim::intern::test_interner();
        let alliances = crate::map::houses::HouseAllianceMap::new();

        let search = |facts: MoverPathFacts| {
            find_move_path(
                PathfindingContext {
                    wall_tables: Some(crate::sim::pathfinding::cell_entry::WallArmTables {
                        overlay_grid: Some(&overlays),
                        overlay_registry: Some(&registry),
                        alliances: Some(&alliances),
                        interner: Some(&interner),
                    }),
                    path_grid: Some(&grid),
                    zone_grid: None,
                    resolved_terrain: Some(&terrain),
                    playfield_bounds: None,
                    blocker_neighbor_counts: None,
                },
                false,
                (0, 1),
                MovementLayer::Ground,
                (4, 1),
                Some(&costs),
                None,
                None,
                None,
                MovementZone::Normal,
                Some(MovementZone::Normal),
                None,
                facts,
                true,
            )
        };

        // An armed mover whose primary warhead sets `Wall=` takes the wall arm:
        // the line is priced, so the order is admitted and the route crosses it.
        let armed = MoverPathFacts {
            urgency: 0,
            mover_is_crusher: false,
            is_infantry: false,
            speed_type: Some(SpeedType::Track),
            owner: Some(mover),
            is_armed: true,
            warhead_wall: true,
            warhead_wood: false,
            slave_deposit_cells: [None, None],
        };
        let (path, _layers) = search(armed).expect(
            "a mover that can shoot the wall must be given a route through it, not refused",
        );
        assert_eq!(path.first(), Some(&(0, 1)));
        assert_eq!(path.last(), Some(&(4, 1)));
        assert!(
            path.iter().any(|&(rx, _)| rx == 2),
            "the admitted route must cross the wall line, not detour around a 5x3 board: {path:?}"
        );

        // Same board, same search, mover facts that cannot take the arm: an
        // unarmed mover answers 7 at `0x0073F48F` and the line stays solid.
        let unarmed = MoverPathFacts {
            is_armed: false,
            warhead_wall: false,
            ..armed
        };
        assert!(
            search(unarmed).is_none(),
            "an unarmed mover must still find the wall line impassable"
        );
    }

    /// A wall line that **disconnects** the map is still refused, and that is
    /// the production gate stack — not the A* arm the test above exercises.
    ///
    /// Review caught the headline this work was published under: "a mover whose
    /// only route crossed a wall line got no path at all" is still true after
    /// pricing the wall, because the search never reaches A*.
    /// `zone_search` compares the start and goal base zone labels and returns
    /// `Err` without calling `astar_search`; a non-crushable `Wall=yes` reduces
    /// to `zone_class::WALL`, which the zone flood fill treats as impassable, so
    /// a line across the only route puts the endpoints in different zones.
    ///
    /// Native rejects on the same comparison and under the same condition:
    /// `0x0042CB2A CMP EAX,EDX` / `0x0042CB2C MOV AL,[ESP+0x4C]` /
    /// `0x0042CB30 JZ` (labels equal, continue) / `0x0042CB34 JZ 0x0042CB8B`
    /// (hierarchy unusable, fall through) / `0x0042CB39 XOR EAX,EAX; RET`. VERA
    /// guards its own rejection on `hierarchy_counts_available` and a live
    /// level-0 hierarchy, which is the same shape.
    ///
    /// So what the cost class actually buys is a wall the mover can shoot that
    /// does **not** disconnect the map — where 20x still beats the detour. This
    /// test pins the disconnecting case so that claim cannot drift again.
    #[test]
    fn a_wall_line_that_disconnects_the_map_is_refused_before_the_cost_class_runs() {
        use crate::map::resolved_terrain::{ResolvedTerrainCell, ResolvedTerrainGrid, zone_class};
        use crate::rules::ini_parser::IniFile;

        let mut cells = Vec::with_capacity(15);
        for ry in 0..3u16 {
            for rx in 0..5u16 {
                let mut cell = ResolvedTerrainCell::clear_for_test(rx, ry);
                cell.speed_costs.track = Some(100);
                if rx == 2 {
                    cell.zone_type = zone_class::WALL;
                    cell.overlay_zone_type = Some(zone_class::WALL);
                    cell.overlay_blocks = true;
                }
                cells.push(cell);
            }
        }
        let terrain = ResolvedTerrainGrid::from_cells(5, 3, cells);
        let grid = PathGrid::from_resolved_terrain(&terrain);
        let costs = TerrainCostGrid::from_resolved_terrain(&terrain, SpeedType::Track);
        let zone_grid = ZoneGrid::build_with_terrain(&grid, &terrain, &[], 5, 3);
        let counts = crate::sim::pathfinding::BlockerNeighborCounts::new(5, 3);

        let registry = crate::rules::overlay_types::OverlayTypeRegistry::from_ini(
            &IniFile::from_str("[OverlayTypes]\n0=GAWALL\n\n[GAWALL]\nWall=yes\n"),
            None,
        );
        let mut overlays = crate::sim::overlay_grid::OverlayGrid::new(5, 3);
        for ry in 0..3u16 {
            overlays.cell_mut(2, ry).overlay_id = Some(0);
        }
        let mover = crate::sim::intern::test_intern("Americans");
        let interner = crate::sim::intern::test_interner();
        let alliances = crate::map::houses::HouseAllianceMap::new();

        let armed = MoverPathFacts {
            urgency: 0,
            mover_is_crusher: false,
            is_infantry: true,
            speed_type: Some(SpeedType::Track),
            owner: Some(mover),
            is_armed: true,
            warhead_wall: true,
            warhead_wood: false,
            slave_deposit_cells: [None, None],
        };
        let path = find_move_path(
            PathfindingContext {
                wall_tables: Some(crate::sim::pathfinding::cell_entry::WallArmTables {
                    overlay_grid: Some(&overlays),
                    overlay_registry: Some(&registry),
                    alliances: Some(&alliances),
                    interner: Some(&interner),
                }),
                path_grid: Some(&grid),
                zone_grid: Some(&zone_grid),
                resolved_terrain: Some(&terrain),
                playfield_bounds: None,
                blocker_neighbor_counts: Some(&counts),
            },
            false,
            (0, 1),
            MovementLayer::Ground,
            (4, 1),
            Some(&costs),
            None,
            None,
            None,
            MovementZone::Normal,
            Some(MovementZone::Normal),
            None,
            armed,
            true,
        );
        assert!(
            path.is_none(),
            "a wall line that splits the map is refused by the zone comparison \
             before the cost class is consulted; pricing the wall does not change \
             that, and the published claim that it did was wrong: {path:?}"
        );
    }
}
