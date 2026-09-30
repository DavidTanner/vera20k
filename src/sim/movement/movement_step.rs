//! Movement step helpers — cell transition mechanics, vehicle rotation, lepton advancement,
//! and cell boundary crossing detection.
//!
//! Contains the inner-loop logic extracted from `tick_movement_with_grids`: how a mover
//! rotates in place, advances sub-cell position, detects cell boundary crossings, and
//! performs the actual cell transition with occupancy/terrain checks.

use std::collections::BTreeSet;

use super::cell_arrival::CellArrival;
use crate::map::entities::EntityCategory;
use crate::map::resolved_terrain::ResolvedTerrainGrid;
use crate::rules::locomotor_type::LocomotorKind;
use crate::sim::components::{MovementTarget, Position};
use crate::sim::debug_event_log::DebugEventKind;
use crate::sim::movement::bump_crush;
use crate::sim::movement::locomotor::{LocomotorState, MovementLayer};
use crate::sim::movement::movement_blocked::handle_blocked_tick;
use crate::sim::movement::movement_bridge::resolve_cell_transition_bridge_state;
use crate::sim::movement::movement_occupancy::{
    BuildingEntrySkipLookup, DeferredCellCheck, detect_deferred_cell_check,
    evaluate_runtime_can_enter_cell_with_transition, naval_terrain_diag,
    runtime_can_enter_cell_args,
};
use crate::sim::occupancy::{CellOccupationGrid, OccupancyGrid};
use crate::sim::pathfinding::PathGrid;
use crate::sim::pathfinding::terrain_cost::TerrainCostGrid;
use crate::sim::rng::SimRng;
use crate::util::fixed_math::{SIM_HALF, SIM_ONE, SIM_ZERO, SimFixed, fixed_distance};
use crate::util::lepton::CELL_CENTER_LEPTON;

use super::{
    CLIFF_HEIGHT_THRESHOLD, MovementConfig, MovementTickStats, MoverSnapshot, PATH_STUCK_INIT,
    PathfindingContext,
};

pub(super) fn apply_cell_transition_remainder(
    path_runtime: &mut crate::sim::components::FootPathRuntime,
    position: &mut Position,
    dx_cell: i32,
    dy_cell: i32,
    nx: u16,
    ny: u16,
    is_infantry: bool,
    native_frame: u32,
    walk: bool,
) {
    // Infantry: clear blocking state on each cell arrival (fresh grace period).
    // Vehicles: keep both flags — once blocked, urgency escalates permanently.
    if is_infantry {
        // Walk75BE11/75BFD1 clears the latch but retains the grace timer.
        if !walk {
            path_runtime.start_blocked(native_frame, 0);
        }
        path_runtime.path_blocked = false;
    }
    if dx_cell > 0 {
        position.sub_x -= crate::util::lepton::LEPTONS_PER_CELL;
    } else if dx_cell < 0 {
        position.sub_x += crate::util::lepton::LEPTONS_PER_CELL;
    }
    if dy_cell > 0 {
        position.sub_y -= crate::util::lepton::LEPTONS_PER_CELL;
    } else if dy_cell < 0 {
        position.sub_y += crate::util::lepton::LEPTONS_PER_CELL;
    }
    position.rx = nx;
    position.ry = ny;
}

pub(super) fn configure_motion_after_transition(
    target: &mut MovementTarget,
    locomotor: &Option<LocomotorState>,
    category: EntityCategory,
    position: &Position,
) {
    let current_cell = (position.rx, position.ry);
    let current_sub = (position.sub_x, position.sub_y);
    target.next_index += 1;
    if target.next_index < target.path.len() {
        let next = target.path[target.next_index];
        let ndx = next.0 as i32 - current_cell.0 as i32;
        let ndy = next.1 as i32 - current_cell.1 as i32;

        if category == EntityCategory::Infantry {
            // Infantry: direction from current sub-cell toward next cell's subcell position.
            // Use the allocated subcell offset to maintain visual spread during movement,
            // matching the WalkLocomotionClass which walks to FindSubCellDest result.
            let (sc_x, sc_y) = locomotor
                .as_ref()
                .and_then(|l| l.subcell_dest)
                .unwrap_or((CELL_CENTER_LEPTON, CELL_CENTER_LEPTON));
            let dest_x = SimFixed::from_num(ndx * 256) + sc_x;
            let dest_y = SimFixed::from_num(ndy * 256) + sc_y;
            let dx = dest_x - current_sub.0;
            let dy = dest_y - current_sub.1;
            target.move_dir_x = dx;
            target.move_dir_y = dy;
            target.move_dir_len = fixed_distance(dx, dy);
        } else {
            let (d_x, d_y, d_len) = crate::util::lepton::cell_delta_to_lepton_dir(ndx, ndy);
            target.move_dir_x = d_x;
            target.move_dir_y = d_y;
            target.move_dir_len = d_len;
        }
    } else if let Some(loco) = locomotor {
        if let Some((dest_x, dest_y)) = loco.subcell_dest {
            let dx = dest_x - current_sub.0;
            let dy = dest_y - current_sub.1;
            target.move_dir_x = dx;
            target.move_dir_y = dy;
            let len: SimFixed = fixed_distance(dx, dy);
            target.move_dir_len = if len > SIM_HALF { len } else { SIM_ONE };
        }
    }
}

/// Result of vehicle rotation — tells the caller whether to skip this tick.
pub(super) enum RotationResult {
    /// Still rotating in place — caller should `continue` (skip lepton advancement).
    StillRotating,
    /// Rotation complete or not needed — proceed with movement.
    ReadyToMove,
}

/// Handle vehicle in-place rotation before movement begins.
///
/// A vehicle holds position while its hull turns: gamemd's Drive/Ship Process
/// returns through its tail while `FacingClass::Is_Rotating` answers true on
/// the body (`0x004B0788`), whoever started the turn. `desired` is a turn this
/// visit issues first — the fresh arm's `Do_Turn` (`0x004B343B`), one
/// `FacingClass::Set` at the body's constructor rate (duration
/// `abs(delta) / rate` native frames; a non-positive rate is instant). Infantry
/// are excluded by the caller.
///
/// Takes individual fields to avoid borrow conflicts with `entity.movement_target`.
pub(super) fn handle_vehicle_rotation(
    body_facing: &mut super::facing_class::FacingClass,
    desired: Option<u16>,
    native_frame: u32,
) -> RotationResult {
    if let Some(desired) = desired {
        body_facing.set(desired, native_frame);
    }
    if !body_facing.is_rotating(native_frame) {
        return RotationResult::ReadyToMove;
    }
    // Still rotating in place — the hull turns but the mover does not advance.
    RotationResult::StillRotating
}

#[cfg(test)]
#[path = "movement_step_tests.rs"]
mod tests;

/// Walk75BD70 completes a retained subcell head within 17 world leptons.
pub(super) fn completed_walk_head(
    position: &Position,
    locomotor: &Option<LocomotorState>,
) -> Option<crate::sim::components::DriveCoord> {
    let loco = locomotor
        .as_ref()
        .filter(|l| l.kind == LocomotorKind::Walk)?;
    let head = loco.step_head()?;
    let [x, y] = super::ground_pose::position_world_xy(position);
    (fixed_distance(
        SimFixed::from_num(head.x.wrapping_sub(x)),
        SimFixed::from_num(head.y.wrapping_sub(y)),
    ) < SimFixed::from_num(17))
    .then_some(head)
}

/// Output from the cell boundary crossing loop.
pub(super) struct CrossingOutput {
    /// A production Walk crossing must release the entity borrow before
    /// Mark(REMOVE), SetCoords, SetHeight and Mark(PUT). No PerCell here.
    pub walk_boundary: Option<crate::sim::components::DriveCoord>,
    pub walk_head_admitted: bool,
    /// If set, the caller must handle deferred occupancy outside the entity borrow.
    pub deferred_cell_check: Option<DeferredCellCheck>,
    /// If set, the mover was refused a second time at this cell by a wall it can
    /// shoot, and the caller must run the wall-attack Override there.
    ///
    /// Deferred rather than fired inline: this function holds decomposed `&mut`
    /// fields and has neither `&mut EntityStore` nor `&mut GameEntity`, and the
    /// Override needs to write `mission`, `attack_target` and `navigation`.
    pub deferred_wall_override: Option<(u16, u16)>,
    /// Bridge render state to apply after the loop. Predicate-driven; see movement_bridge.rs.
    pub pending_bridge_update: super::movement_bridge::BridgeStateUpdate,
    /// The resolved movement layer after all crossings.
    pub active_layer: MovementLayer,
    /// Debug events accumulated during crossing checks.
    pub debug_events: Vec<(u32, DebugEventKind)>,
    /// Whether the entity was marked as stuck and should abort.
    pub aborted_for_stuck: bool,
    pub runtime_bridge_transition: super::movement_bridge::RuntimeBridgeTransitionState,
}

/// Process cell boundary crossings — the inner loop that checks whether
/// sub_x/sub_y have crossed cell boundaries, validates terrain walkability,
/// cliff height, and occupancy, then performs cell transitions with lepton
/// remainder carry-over.
///
/// Takes individual entity fields to avoid borrow conflicts with
/// `entity.movement_target` (which the caller holds as `ref mut target`).
#[allow(clippy::too_many_arguments)]
pub(super) fn process_cell_crossings(
    target: &mut MovementTarget,
    path_runtime: &mut crate::sim::components::FootPathRuntime,
    position: &mut Position,
    body_facing: &super::FacingClass,
    locomotor: &mut Option<LocomotorState>,
    sub_cell: &mut Option<u8>,
    category: EntityCategory,
    entity_id: u64,
    mut active_layer: MovementLayer,
    snap: &MoverSnapshot,
    path_grid: Option<&PathGrid>,
    resolved_terrain: Option<&ResolvedTerrainGrid>,
    entity_cost_grid: Option<&TerrainCostGrid>,
    mover_entity_blocks: Option<&BTreeSet<(u16, u16)>>,
    mover_entity_block_map: Option<&crate::sim::pathfinding::LayeredEntityBlockMap>,
    live_building_entry_skips: &impl BuildingEntrySkipLookup,
    occupancy: &mut OccupancyGrid,
    cell_occupation: &mut CellOccupationGrid,
    stats: &mut MovementTickStats,
    finished_entities: &mut Vec<u64>,
    rng: &mut SimRng,
    // Borrowed for this call only. The wall arm needs it to resolve house names
    // for the ally test, and a per-call borrow is what keeps it off
    // `PathfindingContext`, which outlives the pass and would collide with the
    // `&mut StringInterner` the pass still needs.
    interner: &crate::sim::intern::StringInterner,
    ctx: PathfindingContext<'_>,
    mcfg: MovementConfig,
    sim_tick: u64,
    marker_context: Option<super::path_markers::BridgeMarkerContext<'_>>,
    suspend_walk_boundary: bool,
    walk_head_admission: bool,
) -> CrossingOutput {
    let walk = locomotor
        .as_ref()
        .is_some_and(|l| l.kind == LocomotorKind::Walk);
    let mut walk_head_admitted = false;
    let mut walk_boundary = None;
    let mut debug_events: Vec<(u32, DebugEventKind)> = Vec::new();
    let mut deferred_cell_check: Option<DeferredCellCheck> = None;
    let mut deferred_wall_override: Option<(u16, u16)> = None;
    let mut runtime_bridge_transition = snap.runtime_bridge_transition;
    let mut pending_bridge_update: super::movement_bridge::BridgeStateUpdate =
        super::movement_bridge::BridgeStateUpdate::Unchanged;
    let mut projected_on_bridge_state = snap.on_bridge;
    let mut aborted_for_stuck: bool = false;

    loop {
        if target.next_index >= target.path.len() {
            break;
        }
        let old_rx = position.rx;
        let old_ry = position.ry;
        let committed_walk = locomotor
            .as_ref()
            .is_some_and(|l| l.kind == LocomotorKind::Walk && l.step_head().is_some());
        let (nx, ny): (u16, u16) = if committed_walk {
            let [x, y] = super::ground_pose::position_world_xy(position);
            ((x / 256) as u16, (y / 256) as u16)
        } else {
            target.path[target.next_index]
        };
        let dx_cell: i32 = nx as i32 - position.rx as i32;
        let dy_cell: i32 = ny as i32 - position.ry as i32;
        if dx_cell == 0
            && dy_cell == 0
            && locomotor
                .as_ref()
                .is_some_and(|l| l.kind == LocomotorKind::Walk)
        {
            break;
        }

        // Check if sub_x/sub_y have crossed cell boundaries on each axis.
        let crossed_x: bool = match dx_cell.signum() {
            1 => position.sub_x >= crate::util::lepton::LEPTONS_PER_CELL,
            -1 => position.sub_x <= SIM_ZERO,
            _ => true, // No X movement needed for this step.
        };
        let crossed_y: bool = match dy_cell.signum() {
            1 => position.sub_y >= crate::util::lepton::LEPTONS_PER_CELL,
            -1 => position.sub_y <= SIM_ZERO,
            _ => true,
        };
        let crossing = if walk_head_admission {
            true
        } else if committed_walk {
            //75C0F3..75C117 compares both actual cell coordinates. A
            //diagonal step can enter a side cell before its other axis crosses.
            let [x, y] = super::ground_pose::position_world_xy(position);
            ((x / 256) as u16, (y / 256) as u16) != (old_rx, old_ry)
        } else {
            crossed_x && crossed_y
        };
        if !crossing {
            break;
        }

        // The wall memo names the cell where a wall refused this mover on its
        // previous attempt. The moment it attempts a different cell - because a
        // repath found a detour - that refusal is stale, and a memo that outlived
        // its cell would let the Override fire on a FIRST refusal later, skipping
        // the repath native requires before its second evaluation.
        if target
            .wall_refusal_cell
            .is_some_and(|cell| cell != (nx, ny))
        {
            target.wall_refusal_cell = None;
        }

        let next_layer = target.layer_at(target.next_index);
        //75AECD..75AEF8 jumps past pathfind/admission when a head exists.
        //Only the no-head branch reaches CanEnter75B690 before75BC1A.
        if !committed_walk {
            let runtime_entry = evaluate_runtime_can_enter_cell_with_transition(
                path_grid,
                next_layer,
                &mut runtime_bridge_transition,
                projected_on_bridge_state,
                runtime_can_enter_cell_args(
                    path_grid,
                    (position.rx, position.ry),
                    (nx, ny),
                    projected_on_bridge_state,
                    position.z,
                ),
            );
            let layer_context = runtime_entry.layers;
            let mut layer_grid_ok: Option<bool> = None;
            let mut layer_terrain_ok: Option<bool> = None;

            if !runtime_entry.bridge_traversal_allowed {
                position.sub_x = crate::util::lepton::CELL_CENTER_LEPTON;
                position.sub_y = crate::util::lepton::CELL_CENTER_LEPTON;
                // VERA centre recovery is not a native locomotor step; keep its
                // committed old-cell pose coherent without sampling the rejected XY.
                super::ground_pose::set_height(
                    position,
                    projected_on_bridge_state,
                    0,
                    resolved_terrain,
                    path_grid,
                );
                path_runtime.start_movement(mcfg.binary_frame, 0);
                let evts = handle_blocked_tick(
                    target,
                    path_runtime,
                    body_facing.current(mcfg.binary_frame),
                    &snap.locomotor,
                    entity_id,
                    (position.rx, position.ry),
                    active_layer,
                    snap.on_bridge,
                    stats,
                    finished_entities,
                    &mut aborted_for_stuck,
                    ctx,
                    entity_cost_grid,
                    mover_entity_blocks,
                    mover_entity_block_map,
                    mcfg,
                    sim_tick,
                    PATH_STUCK_INIT,
                    super::MoverPathFacts::from_snapshot(snap, 0),
                    snap.allow_zone_hierarchy,
                    true,
                    true,
                    marker_context,
                    occupancy,
                );
                debug_events.extend(evts);
                break;
            }

            // --- Terrain walkability check (static map data) ---
            // Set when the Ground arm refuses because of a wall this mover could
            // shoot (`Can_Enter_Cell` 4 or 5 rather than 7). Recorded here and
            // consumed by the refusal block; ledger row I9b.
            let mut ground_wall_class: Option<u8> = None;
            let layer_walkable = match layer_context.terrain_layer {
                MovementLayer::Ground => {
                    // Water movers (ships) bypass PathGrid — water cells are
                    // marked non-walkable for land units but ships need them.
                    // Use passability matrix directly, same as the pathfinder.
                    let cost_grid = entity_cost_grid;
                    // Same predicate the search ran. The original reaches its cell
                    // gate through a single per-class slot, so an infantryman's
                    // sub-cell view of terrain objects has to hold here too —
                    // otherwise A* plans through a tree cell the step-in refuses
                    // and the mover block/repath-loops onto the identical route.
                    // Result-preserving: the bool form flattens the wall
                    // arm's 4 and 5 into the same refusal as a hard 7, and the
                    // Override needs that distinction. With no wall tables the
                    // context is `None` and this is the previous predicate
                    // exactly - see `evaluate_cell_entry_for_category_on_layer`.
                    let entry = match path_grid {
                        Some(grid) => {
                            crate::sim::pathfinding::evaluate_cell_entry_for_category_on_layer(
                                grid,
                                nx,
                                ny,
                                MovementLayer::Ground,
                                Some(snap.movement_zone),
                                snap.speed_type,
                                resolved_terrain,
                                cost_grid,
                                snap.slave_deposit_cells.contains(&Some((nx, ny))),
                                category == EntityCategory::Infantry,
                                snap.crush_capability().wall_arm_crusher(),
                                ctx.wall_tables.map(|tables| {
                                    crate::sim::pathfinding::cell_entry::WallArmContext {
                                        overlay_grid: tables.overlay_grid,
                                        overlay_registry: tables.overlay_registry,
                                        alliances: tables.alliances,
                                        interner: Some(interner),
                                        mover_owner: Some(snap.owner),
                                        is_armed: snap.is_armed,
                                        warhead_wall: snap.warhead_wall,
                                        warhead_wood: snap.warhead_wood,
                                    }
                                }),
                            )
                        }
                        None => crate::sim::pathfinding::cell_entry::CanEnterCellResult::Clear,
                    };
                    if let crate::sim::pathfinding::cell_entry::CanEnterCellResult::WallBlocked {
                        cost_class,
                    } = entry
                    {
                        ground_wall_class = Some(cost_class);
                    }
                    let grid_ok: bool = entry.is_clear();
                    let terrain_ok: bool = true;
                    layer_grid_ok = Some(grid_ok);
                    layer_terrain_ok = Some(terrain_ok);
                    grid_ok && terrain_ok
                }
                MovementLayer::Bridge => path_grid.is_some_and(|grid| {
                    crate::sim::pathfinding::is_cell_passable_for_mover_on_layer_with_speed(
                        grid,
                        nx,
                        ny,
                        MovementLayer::Bridge,
                        Some(snap.movement_zone),
                        snap.speed_type,
                        resolved_terrain,
                        entity_cost_grid,
                        false,
                    )
                }),
                MovementLayer::Air | MovementLayer::Underground => false,
            };
            if !layer_walkable {
                if snap.movement_zone.is_water_mover() {
                    log::info!(
                        "NAVAL transition blocked: entity={} cur=({},{}) next=({},{}) layer={:?} grid_ok={:?} terrain_ok={:?} blocked_delay={} path_blocked={} {}",
                        entity_id,
                        position.rx,
                        position.ry,
                        nx,
                        ny,
                        next_layer,
                        layer_grid_ok,
                        layer_terrain_ok,
                        path_runtime
                            .blocked_timer
                            .remaining(mcfg.binary_frame as i32),
                        path_runtime.path_blocked,
                        naval_terrain_diag(resolved_terrain, (nx, ny)),
                    );
                }
                // Undo lepton advancement — entity stays at cell center.
                position.sub_x = crate::util::lepton::CELL_CENTER_LEPTON;
                position.sub_y = crate::util::lepton::CELL_CENTER_LEPTON;
                // VERA centre recovery is not a native locomotor step; keep its
                // committed old-cell pose coherent without sampling the rejected XY.
                super::ground_pose::set_height(
                    position,
                    projected_on_bridge_state,
                    0,
                    resolved_terrain,
                    path_grid,
                );

                // Wall arm (ledger I9b). `Can_Enter_Cell` answered 4 or 5: a wall
                // this mover is armed against and whose warhead admits it. Native
                // does not Override on the first refusal - it drops the path and
                // retries within the same call (`0x004B3ADB` -> `0x004B4552`,
                // `arg2 = 0`), and the Override at `0x004B3BE9` fires only when
                // the repathed first step is refused 4/5 again. So the first
                // refusal repaths exactly as any other block does, and only a
                // repeat at the same cell attacks.
                match ground_wall_class {
                    Some(_) if target.wall_refusal_cell == Some((nx, ny)) => {
                        // Second refusal at the same cell: attack it. No
                        // `handle_blocked_tick` - that would scatter, re-arm the
                        // blockage timer and start another repath, which is the
                        // loop this arm exists to break.
                        deferred_wall_override = Some((nx, ny));
                        target.wall_refusal_cell = None;
                        break;
                    }
                    Some(_) => {
                        target.wall_refusal_cell = Some((nx, ny));
                    }
                    None => {
                        // An ordinary block clears the memo, so two unrelated
                        // refusals can never be read as a repeat.
                        target.wall_refusal_cell = None;
                    }
                }

                // Terrain-blocked (building/cliff) — the path is stale.
                // Force immediate repath by clearing movement_delay.
                path_runtime.start_movement(mcfg.binary_frame, 0);
                let evts = handle_blocked_tick(
                    target,
                    path_runtime,
                    body_facing.current(mcfg.binary_frame),
                    &snap.locomotor,
                    entity_id,
                    (position.rx, position.ry),
                    active_layer,
                    snap.on_bridge,
                    stats,
                    finished_entities,
                    &mut aborted_for_stuck,
                    ctx,
                    entity_cost_grid,
                    mover_entity_blocks,
                    mover_entity_block_map,
                    mcfg,
                    sim_tick,
                    PATH_STUCK_INIT,
                    super::MoverPathFacts::from_snapshot(snap, 0),
                    snap.allow_zone_hierarchy,
                    true, // terrain block: skip code-2 grace period
                    true,
                    marker_context,
                    occupancy,
                );
                debug_events.extend(evts);
                break;
            }

            // --- Cliff detection ---
            // Original engine: if height difference >= 3 levels and not a
            // bridge ramp, treat as cliff. Catches stale paths after terrain
            // changes, bump/scatter toward cliff edges, etc.
            if let Some(pg) = path_grid {
                if let Some(next_cell) = pg.cell(nx, ny) {
                    let next_level = next_cell.effective_cell_z_for_layer(next_layer);
                    let diff = (position.z as i16 - next_level as i16).unsigned_abs();
                    // `is_elevated_bridge_cell` keys on the walkable permission bit, so a
                    // structural deck cell whose permission is clear — a damaged span, or
                    // one carrying a terrain object — reads as ordinary terrain here. A
                    // mover on the deck now carries `ground + 4` (the native height model,
                    // `ObjectClass::SetHeight` 0x005F5FA0), so against that cell's
                    // terrain level the difference is exactly the deck delta and the mover
                    // is stopped mid-span as if it had walked off a cliff. Reading the
                    // structural flag as well keeps the deck a deck regardless of the
                    // permission bit.
                    //
                    // VERA-internal, gamemd equivalent UNCHECKED: `CLIFF_HEIGHT_THRESHOLD`
                    // itself has no identified native owner (`sim::movement::mod.rs`), so
                    // this widens a VERA-only gate rather than porting a native one.
                    let is_bridge_ramp = next_cell.is_bridge_transition_cell()
                        || next_cell.is_elevated_bridge_cell()
                        || next_cell.has_structural_bridge();
                    if diff >= CLIFF_HEIGHT_THRESHOLD && !is_bridge_ramp {
                        position.sub_x = crate::util::lepton::CELL_CENTER_LEPTON;
                        position.sub_y = crate::util::lepton::CELL_CENTER_LEPTON;
                        // VERA centre recovery is not a native locomotor step; keep its
                        // committed old-cell pose coherent without sampling the rejected XY.
                        super::ground_pose::set_height(
                            position,
                            projected_on_bridge_state,
                            0,
                            resolved_terrain,
                            path_grid,
                        );
                        path_runtime.start_movement(mcfg.binary_frame, 0);
                        let evts = handle_blocked_tick(
                            target,
                            path_runtime,
                            body_facing.current(mcfg.binary_frame),
                            &snap.locomotor,
                            entity_id,
                            (position.rx, position.ry),
                            active_layer,
                            snap.on_bridge,
                            stats,
                            finished_entities,
                            &mut aborted_for_stuck,
                            ctx,
                            entity_cost_grid,
                            mover_entity_blocks,
                            mover_entity_block_map,
                            mcfg,
                            sim_tick,
                            PATH_STUCK_INIT,
                            super::MoverPathFacts::from_snapshot(snap, 0),
                            snap.allow_zone_hierarchy,
                            true, // cliff block: skip code-2 grace period
                            true,
                            marker_context,
                            occupancy,
                        );
                        debug_events.extend(evts);
                        break;
                    }
                }
            }

            // --- Occupancy check (entity-aware: sub-cell, crush, bump) ---
            // Occupancy check: vehicles defer to crush/bump/attack handler,
            // infantry defer to sub-cell/attack handler. Both break out of the
            // loop to release the mutable entity borrow for blocker lookups.
            let current_object_list_layer = if projected_on_bridge_state {
                MovementLayer::Bridge
            } else {
                MovementLayer::Ground
            };
            if let Some(check) = detect_deferred_cell_check(
                snap.category,
                entity_id,
                layer_context,
                (nx, ny),
                (position.rx, position.ry),
                current_object_list_layer,
                occupancy,
                cell_occupation,
                live_building_entry_skips,
            ) {
                deferred_cell_check = Some(check);
                break;
            }
        }
        if walk_head_admission {
            walk_head_admitted = true;
            break;
        }

        if suspend_walk_boundary
            && locomotor
                .as_ref()
                .is_some_and(|l| l.kind == LocomotorKind::Walk)
        {
            //75C117: the provisional polar coordinate is selected, but old
            //XYZ must remain visible to RemoveContent and its Recalc callback.
            walk_boundary = Some(super::ground_pose::position_world_coord(position));
            break;
        }

        // --- Cell transition: carry over lepton remainder ---
        // Only adjust the axes that actually crossed a boundary.
        // Do NOT snap the perpendicular axis to center — that causes
        // a visible position jump when transitioning from diagonal
        // to cardinal movement (e.g., sub_x=51 → 128 = ~9px snap).
        apply_cell_transition_remainder(
            path_runtime,
            position,
            dx_cell,
            dy_cell,
            nx,
            ny,
            category == EntityCategory::Infantry,
            mcfg.binary_frame,
            walk,
        );
        // GATE A2 verified order: the object-list layer is selected by the
        // occupant's OnBridge byte sampled at each call site. Capture the OLD
        // (pre-transition) layer BEFORE evaluating the bridge transition, so the
        // old-cell removal walks the old layer and the new-cell insertion the new
        // layer (the two halves may differ when stepping on/off the deck).
        let old_occupancy_layer = if projected_on_bridge_state {
            MovementLayer::Bridge
        } else {
            MovementLayer::Ground
        };
        let bridge_update = resolve_cell_transition_bridge_state(
            position,
            path_grid,
            (old_rx, old_ry),
            (nx, ny),
            projected_on_bridge_state,
        );
        projected_on_bridge_state =
            super::movement_bridge::projected_on_bridge(projected_on_bridge_state, bridge_update);
        if locomotor
            .as_ref()
            .is_some_and(|loco| loco.kind == LocomotorKind::Walk)
        {
            super::ground_pose::set_height(
                position,
                projected_on_bridge_state,
                0,
                resolved_terrain,
                path_grid,
            );
        }
        if !matches!(
            bridge_update,
            super::movement_bridge::BridgeStateUpdate::Unchanged
        ) {
            pending_bridge_update = bridge_update;
        }
        let new_occupancy_layer = if projected_on_bridge_state {
            MovementLayer::Bridge
        } else {
            MovementLayer::Ground
        };
        if committed_walk {
            //Headless adapter: current-coordinate list projection only.
            //Production uses the world runner above, including raw/Recalc.
            *sub_cell = Some(crate::sim::cell_kernel::infantry_preferred_spot(
                crate::sim::cell_kernel::CellQueryPoint {
                    x: position.sub_x.to_num::<i32>(),
                    y: position.sub_y.to_num::<i32>(),
                },
            ));
            occupancy.move_entity_layered(
                old_rx,
                old_ry,
                nx,
                ny,
                entity_id,
                old_occupancy_layer,
                new_occupancy_layer,
                *sub_cell,
                crate::sim::occupancy::CellListInsertion::from_category(category),
            );
            stats.moved_steps = stats.moved_steps.saturating_add(1);
        } else {
            CellArrival {
                entity_id,
                category,
                from: (old_rx, old_ry),
                to: (nx, ny),
                old_list_layer: old_occupancy_layer,
                new_list_layer: new_occupancy_layer,
                position,
                locomotor,
                sub_cell,
                occupancy,
                stats,
                priority: snap.sub_cell_priority_mission && snap.nav_com_cell == Some((nx, ny)),
            }
            .ordinary(next_layer);
        }
        active_layer = next_layer;

        if locomotor
            .as_ref()
            .is_some_and(|l| l.kind == LocomotorKind::Walk)
        {
            // Walk75C12E relinks a boundary crossing but retains Head_To and
            // the current path entry until its <17 completion corridor.
            break;
        }
        if let Some(loco) = locomotor.as_mut() {
            loco.set_step_head(None);
        }

        configure_motion_after_transition(target, locomotor, category, position);

        // Pre-allocate subcell in the NEXT path cell for infantry direction targeting.
        // FindSubCellDest reserves a subcell in the destination cell before walking,
        // so each infantry targets its own subcell position based on the destination
        // cell's occupancy rather than carrying the current cell's.
        if category == EntityCategory::Infantry && target.next_index < target.path.len() {
            let next_cell = target.path[target.next_index];
            // Missions Enter / Capture / Eaten / Area Guard / Patrol whose
            // NavCom sits in the cell being reserved place unconditionally,
            // skipping the occupancy, blocker and garrison gates and taking no
            // random draw — matching the original engine's priority branch.
            let pre_priority =
                snap.sub_cell_priority_mission && snap.nav_com_cell == Some(next_cell);
            let pre_slot = if pre_priority {
                Some(bump_crush::priority_sub_cell(
                    position.sub_x,
                    position.sub_y,
                ))
            } else {
                bump_crush::allocate_sub_cell_with_preference(
                    occupancy.get(next_cell.0, next_cell.1),
                    active_layer,
                    None,
                    position.sub_x,
                    position.sub_y,
                    rng,
                )
            };
            if let Some(pre_sub) = pre_slot {
                let (sc_x, sc_y) = crate::util::lepton::subcell_lepton_offset(Some(pre_sub));
                if let Some(loco) = locomotor {
                    loco.subcell_dest = Some((sc_x, sc_y));
                }
                // Recompute direction toward the destination cell's subcell.
                let ndx = next_cell.0 as i32 - nx as i32;
                let ndy = next_cell.1 as i32 - ny as i32;
                let dest_x = SimFixed::from_num(ndx * 256) + sc_x;
                let dest_y = SimFixed::from_num(ndy * 256) + sc_y;
                let dx = dest_x - position.sub_x;
                let dy = dest_y - position.sub_y;
                target.move_dir_x = dx;
                target.move_dir_y = dy;
                target.move_dir_len = fixed_distance(dx, dy);
            }
        }
    }

    CrossingOutput {
        deferred_wall_override,
        walk_boundary,
        walk_head_admitted,
        deferred_cell_check,
        pending_bridge_update,
        active_layer,
        debug_events,
        aborted_for_stuck,
        runtime_bridge_transition,
    }
}
