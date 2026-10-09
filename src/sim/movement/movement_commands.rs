//! Move command issuing — A* pathfinding and MovementTarget attachment.
//!
//! Entry points for issuing move commands to entities. These are called from
//! `world_commands.rs`, `miner_system.rs`, and `production_queue.rs` — not
//! from the per-tick movement loop.
//!
//! ## Dependency rules
//! - Internal to sim/movement — called via re-exports in mod.rs.

use std::collections::BTreeSet;

use crate::map::resolved_terrain::ResolvedTerrainGrid;
use crate::rules::locomotor_type::LocomotorKind;
use crate::sim::components::MovementTarget;
use crate::sim::entity_store::EntityStore;
use crate::sim::pathfinding::PathGrid;
use crate::sim::pathfinding::terrain_cost::TerrainCostGrid;
use crate::sim::pathfinding::zone_map::ZoneGrid;
use crate::sim::pathfinding::{BlockerNeighborCounts, LayeredEntityBlockMap};
use crate::util::fixed_math::SimFixed;

use super::PathfindingContext;
use super::movement_path::{
    find_move_path, resolve_reachable_move_goal, resolve_requested_move_goal,
    supports_layered_bridge_pathing,
};
use crate::rules::locomotor_type::MovementZone;
use crate::sim::game_entity::GameEntity;

/// Check if an entity can accept a new movement destination.
///
/// Prevents destination changes during special states: dying, deploying,
/// undeploying and falling.
pub(crate) fn can_accept_destination(entity: &GameEntity) -> bool {
    if entity.dying {
        return false;
    }
    if entity.building_up() || entity.building_down() {
        return false;
    }
    true
}

/// A committed head comes from the active locomotor after world callbacks;
/// the physical path and raw occupation metadata cannot reconstruct it.
pub(super) fn committed_movement_head(entity: &GameEntity) -> Option<(u16, u16)> {
    let head = if entity.locomotor.as_ref()?.kind == LocomotorKind::Walk {
        entity.locomotor.as_ref()?.step_head()?
    } else {
        super::track_head::committed_track_head(entity)?
    };
    Some(((head.x / 256) as u16, (head.y / 256) as u16))
}

/// Keep only physical movement already accepted by the locomotor. The caller
/// owns destination/queue writes: explicit Attack's null setter does not clear
/// NavQueue, whereas the Unit setter's null arm (`0x007423BE`) does.
pub(crate) fn retain_committed_movement(e: &mut GameEntity) {
    if committed_movement_head(e).is_some() {
        // Walk75BD29 samples current XYZ against its paid head; a
        // Drive/Ship curve finishes at its head after Foot+5E0 retires.
        // Keep the scheduling adapter, with no second goal coordinate.
        if let Some(target) = e.movement_target.as_mut() {
            target.final_goal = None;
        }
    } else {
        e.movement_target = None;
    }
}

/// Read the movement goal for diagnostics without retaining another copy.
/// Drive/Ship display Foot's requested Cell, including a Move_To refusal
/// that leaves a different locomotor destination (track_destination native
/// warp controls). Object requests use the captured locomotor coordinate;
/// after Stop the paid head remains. Other adapters retain their existing
/// goal. This projection is for presentation; it makes no simulation decision.
pub(crate) fn movement_goal_cell(entity: &GameEntity) -> Option<(u16, u16)> {
    let adapter = entity.movement_target.as_ref()?;
    if entity.locomotor.as_ref().is_some_and(|loco| {
        matches!(
            loco.active_kind(),
            LocomotorKind::Drive | LocomotorKind::Ship
        )
    }) {
        if let Some(crate::sim::components::NavTargetRef::Cell { rx, ry }) =
            entity.navigation.nav_com
        {
            return Some((rx, ry));
        }
        return super::track_path::track_destination(entity)
            .or_else(|| super::track_head::committed_track_head(entity))
            .map(|coord| ((coord.x / 256) as u16, (coord.y / 256) as u16));
    }
    adapter.final_goal
}

/// The caller's native frame and configured Foot blocked timer duration.
/// Passing this context keeps accepted destination timers anchored even when
/// Scatter and the ordinary object turn both process one actor in a frame.
#[derive(Debug, Clone, Copy)]
pub struct DestinationTiming {
    pub binary_frame: u32,
    pub blockage_path_delay_ticks: i32,
}

impl DestinationTiming {
    pub const fn new(binary_frame: u32, blockage_path_delay_ticks: i32) -> Self {
        Self {
            binary_frame,
            blockage_path_delay_ticks,
        }
    }

    pub fn from_rules(binary_frame: u32, rules: Option<&crate::rules::ruleset::RuleSet>) -> Self {
        Self::new(
            binary_frame,
            rules.map_or(60, |r| r.general.blockage_path_delay_ticks),
        )
    }

    /// Set_Destination_Internal 0x004D94B0 tail 0x4D96C2..0x4D9707: +6B7 = 0,
    /// +668 = (frame, Rules+1768), +640 = (frame, 0) for every accepted
    /// setter. The setter never writes +64C; the no-head Process FindPath
    /// success continuation 0x75B2E2 owns that reset.
    pub(crate) fn accept(self, entity: &mut crate::sim::game_entity::GameEntity) {
        let path = &mut entity.navigation.path_runtime;
        path.start_movement(self.binary_frame, 0);
        path.start_blocked(self.binary_frame, self.blockage_path_delay_ticks);
        path.path_blocked = false;
    }
}

/// Fixture shorthand for [`issue_move_command_with_destination`] without
/// the Simulation's zones, terrain or blocker counts. Production orders go
/// through `Simulation::issue_ground_move`.
///
/// Walk, Drive and Ship destinations defer the path search to their first
/// Process (the class setters never search). Other locomotors retain their
/// command-time path adapter.
///
/// `speed` is the movement speed in cells per second (from rules.ini Speed= value).
#[cfg(test)]
pub(crate) fn issue_move_command(
    entities: &mut EntityStore,
    grid: &PathGrid,
    entity_id: u64,
    target: (u16, u16),
    speed: SimFixed,
    queue: bool,
    terrain_costs: Option<&TerrainCostGrid>,
    entity_blocks: Option<&BTreeSet<(u16, u16)>>,
    entity_block_map: Option<&LayeredEntityBlockMap>,
    timing: crate::sim::movement::DestinationTiming,
) -> bool {
    issue_move_command_with_layered(
        entities,
        grid,
        entity_id,
        target,
        speed,
        queue,
        terrain_costs,
        entity_blocks,
        None, // resolved_terrain — per-tick repath has it
        None, // zone_grid — basic entrypoint has no Simulation context
        entity_block_map,
        None,
        None,
        None,
        timing,
    )
}

#[cfg(test)]
pub(crate) fn issue_move_command_with_layered(
    entities: &mut EntityStore,
    grid: &PathGrid,
    entity_id: u64,
    target: (u16, u16),
    speed: SimFixed,
    queue: bool,
    terrain_costs: Option<&TerrainCostGrid>,
    entity_blocks: Option<&BTreeSet<(u16, u16)>>,
    resolved_terrain: Option<&ResolvedTerrainGrid>,
    zone_grid: Option<&ZoneGrid>,
    entity_block_map: Option<&LayeredEntityBlockMap>,
    blocker_neighbor_counts: Option<&BlockerNeighborCounts>,
    playfield_bounds: Option<crate::sim::cell_rect::PlayfieldBounds>,
    cell_occupation: Option<&mut crate::sim::occupancy::CellOccupationGrid>,
    timing: crate::sim::movement::DestinationTiming,
) -> bool {
    issue_move_command_with_destination(
        entities,
        grid,
        entity_id,
        target,
        speed,
        queue,
        terrain_costs,
        entity_blocks,
        resolved_terrain,
        zone_grid,
        entity_block_map,
        blocker_neighbor_counts,
        playfield_bounds,
        cell_occupation,
        None,
        timing,
    )
}

/// An object order supplies its captured coordinate independently of the A*
/// approach endpoint. Foot4D9510 / Walk75ACB0 own this accepted destination.
pub(crate) fn issue_move_command_with_destination(
    entities: &mut EntityStore,
    grid: &PathGrid,
    entity_id: u64,
    target: (u16, u16),
    speed: SimFixed,
    queue: bool,
    terrain_costs: Option<&TerrainCostGrid>,
    entity_blocks: Option<&BTreeSet<(u16, u16)>>,
    resolved_terrain: Option<&ResolvedTerrainGrid>,
    zone_grid: Option<&ZoneGrid>,
    entity_block_map: Option<&LayeredEntityBlockMap>,
    blocker_neighbor_counts: Option<&BlockerNeighborCounts>,
    playfield_bounds: Option<crate::sim::cell_rect::PlayfieldBounds>,
    cell_occupation: Option<&mut crate::sim::occupancy::CellOccupationGrid>,
    object_destination: Option<(
        crate::sim::components::NavTargetRef,
        crate::sim::components::DriveCoord,
    )>,
    timing: crate::sim::movement::DestinationTiming,
) -> bool {
    // Read the entity's current position and locomotor state.
    let Some(entity) = entities.get(entity_id) else {
        log::warn!("move command: entity {} not found", entity_id);
        return false;
    };
    if !can_accept_destination(entity) {
        return false;
    }
    if entity.locomotor.as_ref().is_some_and(|loco| {
        matches!(
            loco.active_kind(),
            LocomotorKind::Drive | LocomotorKind::Ship | LocomotorKind::Hover
        )
    }) {
        prepare_track_destination(
            entities.get_mut(entity_id).expect("resolved mover"),
            target,
            object_destination,
            speed,
            resolved_terrain,
            timing,
        );
        return true;
    }
    // The original engine dispatches its cell-entry predicate by object class,
    // so terrain-object occupation is read at sub-cell granularity for infantry
    // and whole-cell for everything else, and reads the crusher flags from the
    // mover's own type. urgency=0: an initial move command.
    let path_facts = super::MoverPathFacts::from_entity_without_wall_arm(entity, 0);
    // `AStar @ 0x0042CAD6` uses hierarchy only for a mover whose stored
    // TechnoClass+0x3D5 byte is true. Authority is explicit: resolved terrain
    // and MapClass bounds have independent lifetimes in headless fixtures and
    // during staged startup, so neither can stand in for the other.
    let allow_zone_hierarchy = playfield_bounds.is_none() || entity.in_playfield;
    let locomotor_kind = entity.locomotor.as_ref().map(|locomotor| locomotor.kind);
    // A new destination never rewinds a curve already in flight.
    // `TechnoClass::Set_Destination` @ `0x00741970` only records the target —
    // NavCom in `FootClass::Set_Destination_Internal` @ `0x004D94B0`, the
    // coordinate in Drive `Head_To_Coord` @ `0x004AFD40` — and never touches
    // the Drive track cursor, so the new path takes effect at the curve's next
    // node. Keep its retained selector, cursor and head; anchor the new path
    // at that committed head cell.
    let current_cell = (entity.position.rx, entity.position.ry);
    let (start_rx, start_ry) = committed_movement_head(entity).unwrap_or(current_cell);
    let current_layer = entity.movement_layer_or_ground();
    // Derive movement_zone from the entity's locomotor — no parameter needed.
    let movement_zone: Option<MovementZone> = entity.locomotor.as_ref().map(|l| l.movement_zone);
    let speed_type = entity.locomotor.as_ref().map(|l| l.speed_type);
    let layered_pathing = entity
        .locomotor
        .as_ref()
        .is_some_and(|loco| supports_layered_bridge_pathing(loco, grid, entity.on_bridge));
    if locomotor_kind == Some(LocomotorKind::Walk)
        && (!queue
            || entities
                .get(entity_id)
                .is_some_and(|e| e.movement_target.is_none()))
    {
        // Represented Infantry orders return through the full class owner
        // in Simulation::issue_ground_move. This legacy boundary remains for
        // context-free fixtures and unrepresented receiver classes; it is
        // only an accepted Foot/Walk suffix, not another51AA40 port.
        let destination = object_destination.unwrap_or_else(|| {
            (
                crate::sim::components::NavTargetRef::cell(target.0, target.1),
                super::navcom::target_cell_coord(
                    target.0,
                    target.1,
                    resolved_terrain
                        .map(crate::map::resolved_terrain::NativeCellQuery::canonical)
                        .as_ref(),
                ),
            )
        });
        entities
            .get_mut(entity_id)
            .expect("resolved mover")
            .navigation
            .path_replay
            .clear_live_head();
        return prepare_walk_destination(
            entities,
            entity_id,
            destination,
            speed,
            resolved_terrain,
            timing,
        );
    }
    // Retain the existing non-Walk recovery policy. Infantry51AA40 ->
    // Foot4D94B0 -> Walk75ACB0 has no PowerOn; a powered-down Walk still
    // accepts its destination, with power unchanged (scatter_destination).
    if locomotor_kind != Some(LocomotorKind::Walk)
        && let Some(loco) = entities
            .get_mut(entity_id)
            .and_then(|e| e.locomotor.as_mut())
    {
        loco.power_on();
    }
    let mut merged_entity_blocks = entity_blocks.cloned().unwrap_or_default();
    if let Some(occupation) = cell_occupation.as_deref() {
        merged_entity_blocks.extend(occupation.occupied_cells_ignoring(
            crate::sim::movement::locomotor::MovementLayer::Ground,
            entity_id,
        ));
    }
    let merged_entity_blocks_ref =
        (!merged_entity_blocks.is_empty()).then_some(&merged_entity_blocks);
    let Some(effective_target) = resolve_requested_move_goal(
        grid,
        target,
        merged_entity_blocks_ref,
        movement_zone,
        resolved_terrain,
        10,
    ) else {
        log::warn!(
            "No walkable cell near ({},{}) - cannot issue move",
            target.0,
            target.1,
        );
        return false;
    };
    if effective_target != target {
        log::info!(
            "Move: goal ({},{}) blocked, redirecting to ({},{})",
            target.0,
            target.1,
            effective_target.0,
            effective_target.1,
        );
    }

    if queue {
        // Remaining adapter families can append to their prepared path.
        // Ordinary Drive/Ship requests have already published the destination.
        let entity_mut = entities.get_mut(entity_id);
        if let Some(entity_mut) = entity_mut {
            if let Some(ref mut movement) = entity_mut.movement_target {
                // The adapter keeps its goal, not its cells: the queued
                // search starts where the previous order ends. RESIDUAL: a
                // second append starts from the first order's goal, where the
                // removed cell copy started from the first append's end; the
                // search only gates acceptance (a Jumpjet object order queued
                // twice), and its cells were never read.
                let append_start = movement.final_goal.unwrap_or((start_rx, start_ry));
                // The setters record any destination unsearched; AStar has no
                // route for a goal in its start cell (0x00429BF3..0x00429C0A).
                if append_start == effective_target {
                    return true;
                }
                let append_layer = current_layer;
                let zone_mz = movement_zone.unwrap_or(MovementZone::Normal);
                let Some((appended, _)) = find_move_path(
                    PathfindingContext {
                        wall_tables: None,
                        path_grid: Some(grid),
                        zone_grid,
                        resolved_terrain,
                        playfield_bounds,
                        blocker_neighbor_counts,
                    },
                    layered_pathing,
                    append_start,
                    append_layer,
                    effective_target,
                    terrain_costs,
                    // Pass the merged entity_blocks set to both layered slots so
                    // the layered A* sees building footprints regardless of which
                    // layer it expands.
                    merged_entity_blocks_ref,
                    merged_entity_blocks_ref,
                    merged_entity_blocks_ref,
                    zone_mz,
                    movement_zone,
                    entity_block_map,
                    path_facts,
                    allow_zone_hierarchy,
                ) else {
                    return false;
                };
                if appended.len() >= 2 {
                    movement.speed = speed;
                    entity_mut
                        .navigation
                        .path_runtime
                        .start_blocked(timing.binary_frame, 0);
                    entity_mut.navigation.path_runtime.path_blocked = false;
                }
                return true;
            }
        }
    }
    let zone_mz = movement_zone.unwrap_or(MovementZone::Normal);
    let ctx = PathfindingContext {
        wall_tables: None,
        path_grid: Some(grid),
        zone_grid,
        resolved_terrain,
        playfield_bounds,
        blocker_neighbor_counts,
    };
    let search = |goal: (u16, u16)| {
        find_move_path(
            ctx,
            layered_pathing,
            (start_rx, start_ry),
            current_layer,
            goal,
            terrain_costs,
            // Pass the merged entity_blocks set to both layered slots so the
            // layered A* sees building footprints regardless of which layer
            // it expands.
            merged_entity_blocks_ref,
            merged_entity_blocks_ref,
            merged_entity_blocks_ref,
            zone_mz,
            movement_zone,
            entity_block_map,
            path_facts,
            allow_zone_hierarchy,
        )
    };
    // Order-time reachability recovery. Gamemd runs `Can_Reach_Zone` before it
    // installs the mission and, when it fails, retargets to the nearest cell in
    // the mover's OWN zone rather than dropping the order — the unit drives to
    // the near bank. VERA evaluates it only after the search has already failed:
    // the reduced per-row zone map alone under-reports reachability relative to
    // the hierarchy-backed layered search (a high-bridge route the search finds
    // can read as cross-zone), so gating ahead of the search would refuse
    // destinations gamemd accepts. The recovered set is the same; only the
    // evaluation order differs.
    let mut effective_target = effective_target;
    // The setters record any destination unsearched; AStar has no route for a
    // goal in its start cell (0x00429BF3..0x00429C0A), so it is admitted here.
    let same_cell = effective_target == (start_rx, start_ry);
    let mut found = if same_cell {
        None
    } else {
        search(effective_target)
    };
    if !same_cell
        && found.is_none()
        && let Some(st) = speed_type
    {
        match resolve_reachable_move_goal(
            grid,
            zone_grid,
            resolved_terrain,
            (start_rx, start_ry),
            current_layer,
            effective_target,
            zone_mz,
            st,
        ) {
            Some(near_bank) if near_bank != effective_target => {
                log::info!(
                    "Move: ({},{}) is out of the mover's zone, retargeting to ({},{})",
                    effective_target.0,
                    effective_target.1,
                    near_bank.0,
                    near_bank.1,
                );
                effective_target = near_bank;
                found = search(effective_target);
            }
            _ => {}
        }
    }
    let path = match found {
        Some((path, _)) => path,
        None if same_cell => vec![effective_target],
        None => {
            let eb_count = merged_entity_blocks_ref.map_or(0, |s| s.len());
            log::warn!(
                "No path from ({},{}) to ({},{}) [entity_blocks={}, start_walkable={}, goal_walkable={}]",
                start_rx,
                start_ry,
                effective_target.0,
                effective_target.1,
                eb_count,
                grid.is_walkable(start_rx, start_ry),
                grid.is_walkable(effective_target.0, effective_target.1),
            );
            return false;
        }
    };

    // Log path with walkability check for each cell — helps diagnose paths
    // that go through blocked cells (indicates PathGrid mismatch).
    let path_desc: String = path
        .iter()
        .map(|&(px, py)| {
            let w = grid.is_walkable(px, py);
            if w {
                format!("({},{})", px, py)
            } else {
                format!("({},{})!BLOCKED", px, py)
            }
        })
        .collect::<Vec<_>>()
        .join("→");
    log::info!(
        "Path: grid={}x{} entity_blocks={} {}",
        grid.width(),
        grid.height(),
        merged_entity_blocks_ref.map_or(0, |s| s.len()),
        path_desc,
    );

    // Attach the order adapter. The search only admitted the order: the
    // adapter keeps its goal and speed, not the found cells.
    let movement = MovementTarget {
        speed,
        final_goal: Some(effective_target),
    };
    if let Some(entity_mut) = entities.get_mut(entity_id) {
        // A Walk never reaches this install: it returns from its setter arm
        // or from the queued append above.
        if let Some((reference, coord)) = object_destination {
            super::navcom::set_destination_internal_coord(
                entity_mut,
                reference,
                coord,
                resolved_terrain,
                timing.binary_frame,
            );
        }
        // Unit's accepted setter reaches Foot4D96C2..9707 just as Walk's
        // does. Preserve +64C; this is not a Foot constructor.
        timing.accept(entity_mut);
        entity_mut.movement_target = Some(movement);
    }

    true
}

/// Ordinary Unit741970 -> Foot4D94B0 -> Drive4AFD40/Ship69F450 accepts
/// before any FindPath, preserves power and a paid head, and clears only the
/// live path word (741E88) unless Enter lacks a radio contact
/// (tools/spatial_oracle/track_path_continuation, Enter rows); the setter's
/// flag-gated NavQueue Clear (7422E8..7422F4) empties the waypoint queue
/// (track_destination NavQueue rows). Neither map availability nor an A* result admits the
/// destination: the first no-queue Process owns the request (Drive 4B28A3,
/// Ship 6A1EF3). Native evidence: track_destination unit rows and
/// track_order_path. The class preprocessing (the Teleporter arm, +1F8 and
/// the Foot+0x6AC skip) belongs to `Simulation::set_unit_destination`;
/// this command-path adapter has none of it, nor the radio-building and
/// deploy-byte arms.
pub(crate) fn prepare_track_destination(
    entity: &mut GameEntity,
    target: (u16, u16),
    object_destination: Option<(
        crate::sim::components::NavTargetRef,
        crate::sim::components::DriveCoord,
    )>,
    speed: SimFixed,
    resolved_terrain: Option<&ResolvedTerrainGrid>,
    timing: crate::sim::movement::DestinationTiming,
) {
    clear_destination_path_head(entity);
    entity.navigation.nav_queue.clear();
    if let Some((reference, coord)) = object_destination {
        super::navcom::set_destination_internal_coord(
            entity,
            reference,
            coord,
            resolved_terrain,
            timing.binary_frame,
        );
    } else {
        super::navcom::set_destination_internal_cell(
            entity,
            target,
            resolved_terrain,
            timing.binary_frame,
        );
    }
    timing.accept(entity);
    prepare_destination_execution(entity, target, speed);
}

/// Accepted destination -> Walk75ACB0, without searching or advancing
/// Process. The class caller owns preceding admission and its path-head
/// write; the captured coordinate comes from the shared target +4C owner.
/// No PathGrid is needed until the next ordinary Process (or the immediate
/// Process specifically required by NULL-source Scatter51D478).
pub(crate) fn prepare_walk_destination(
    entities: &mut EntityStore,
    entity_id: u64,
    destination: (
        crate::sim::components::NavTargetRef,
        crate::sim::components::DriveCoord,
    ),
    speed: SimFixed,
    resolved_terrain: Option<&ResolvedTerrainGrid>,
    timing: crate::sim::movement::DestinationTiming,
) -> bool {
    let Some(entity) = entities.get_mut(entity_id) else {
        return false;
    };
    if !entity
        .locomotor
        .as_ref()
        .is_some_and(|l| l.kind == LocomotorKind::Walk)
    {
        return false;
    }
    let (reference, coord) = destination;
    super::navcom::set_destination_internal_coord(
        entity,
        reference,
        coord,
        resolved_terrain,
        timing.binary_frame,
    );
    timing.accept(entity);
    prepare_destination_execution(
        entity,
        ((coord.x / 256) as u16, (coord.y / 256) as u16),
        speed,
    );
    true
}

/// The setters' one-word PathHead clear, which Enter (current or queued)
/// without a radio contact skips: Infantry 51AC25..51AD17, Unit
/// 741C4F..741C78 -> 741E88 (both the NULL and the non-NULL destination).
/// NavQueue, suffix and reference survive.
pub(crate) fn clear_destination_path_head(entity: &mut GameEntity) {
    if (entity.mission.effective().raw() != 7 && entity.mission.queued().raw() != 7)
        || !entity.radio_contacts.is_empty()
    {
        entity.navigation.path_replay.clear_live_head();
    }
}

/// Install the empty-route scheduling adapter for a Drive/Ship Foot whose
/// locomotor destination was set outside the ordinary setter (the outer
/// Process NavCom reissue), so its next visit reaches Process_Movement.
pub(super) fn schedule_track_process(entity: &mut GameEntity, target: (u16, u16), speed: SimFixed) {
    prepare_destination_execution(entity, target, speed);
}

/// A track terminal that found the Foot+5E0 queue empty (Drive 0x4B280D
/// compares it with -1) short of the retained locomotor
/// destination (+34): the adapter keeps scheduling, so the next
/// Process_Movement reaches the no-queue arm (Drive 0x4B281C / Ship
/// 0x6A1E75), in the same Process after the track end
/// (`track_continuation`) or on the next visit. With no destination left it
/// retires.
pub(super) fn spend_track_route(entity: &mut GameEntity) {
    let destination = entity.locomotor.as_ref().and_then(|loco| {
        loco.track_destination(super::track_process::TrackFamily::from_kind(
            loco.active_kind(),
        )?)
    });
    if destination.is_none() {
        entity.movement_target = None;
    }
}

/// MovementTarget is only the scheduling adapter. Retain a paid head, never
/// turn or search at order time. The ordinary Process owns the first route
/// and subsequent head selection for Walk, Drive and Ship.
pub(super) fn prepare_destination_execution(
    entity: &mut GameEntity,
    target: (u16, u16),
    speed: SimFixed,
) {
    // Walk/Drive/Ship keep their goal in the locomotor destination, not in
    // this scheduling adapter. Their movement hosts already read that owner.
    let final_goal = (!entity.locomotor.as_ref().is_some_and(|l| {
        matches!(
            l.active_kind(),
            LocomotorKind::Walk | LocomotorKind::Drive | LocomotorKind::Ship
        )
    }))
    .then_some(target);
    entity.movement_target = Some(MovementTarget {
        speed,
        final_goal,
        ..Default::default()
    });
}
