//! Native TubeClass movement for explicit map tubes.
//!
//! gamemd: `UnitClass::TubeMovement` 0x007359F0, which reads the cell's tube
//! through `CellClass::GetTubeAtCell` 0x00484F20 and, on exit, drives
//! `FacingClass::UpdateFacing` from the record's +0x2C direction field.
//!
//! Automatic tunnel/low-bridge records have a zero-length path. They are
//! predicate and zone metadata only: the retail producer divides by path
//! length, so a zero-step shell must never become active movement state.

use std::sync::OnceLock;

use crate::map::entities::EntityCategory;
use crate::map::resolved_terrain::ResolvedTerrainGrid;
use crate::map::retail_trig::TrigTable;
use crate::map::tube_facts::{TubeFact, TubeId, TubeSource};
use crate::rules::ruleset::RuleSet;
use crate::sim::components::{DriveCoord, DriveLocomotionRuntime, MovementTarget, Position};
use crate::sim::entity_store::EntityStore;
use crate::sim::game_entity::GameEntity;
use crate::sim::intern::StringInterner;
use crate::sim::movement::ScatterFlags;
use crate::sim::movement::bump_crush;
use crate::sim::movement::ground_pose;
use crate::sim::movement::locomotor::MovementLayer;
use crate::sim::movement::scatter::ScatterRequests;
use crate::sim::occupancy::{
    CellListInsertion, CellOccupationGrid, OccupancyGrid, RawCellOccupationGrid,
    VEHICLE_OCCUPATION_BIT, infantry_raw_occupation_mask,
};
use crate::sim::pathfinding::PathGrid;
use crate::sim::rng::SimRng;
use crate::util::fixed_math::{SIM_ONE, SIM_ZERO};
use crate::util::lepton::{self, CELL_CENTER_LEPTON};
use crate::util::native_x87::{NativeF32Bits, NativeF64Bits, X87Chop53, distance_3d_leptons};

/// FootClass-owned active TubeMovement payload.
///
/// `Some` also means the mover is absent from CellClass object lists and raw
/// occupation. Immutable path bytes remain in the map-owned [`TubeFact`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct LowBridgeTubeMovementState {
    pub tube_id: TubeId,
    pub cursor: u8,
    pub target: DriveCoord,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TubeBeginError {
    ZeroLengthTube,
    UnsupportedCategory,
    MissingTerrain,
}

/// Identify the native direction-8 path shape represented by Rust's
/// non-adjacent path node. This is a producer admission check only; state and
/// substrate mutation happen atomically in [`begin_path_tube_step`].
pub fn pending_path_tube_id(
    target: &MovementTarget,
    position: &Position,
    current_layer: MovementLayer,
    terrain: Option<&ResolvedTerrainGrid>,
) -> Option<TubeId> {
    if current_layer != MovementLayer::Ground
        || target.bypass_grid
        || target.next_index >= target.path.len()
    {
        return None;
    }
    let current = (position.rx, position.ry);
    let next = target.path[target.next_index];
    let dx = i32::from(next.0) - i32::from(current.0);
    let dy = i32::from(next.1) - i32::from(current.1);
    if (dx.abs() <= 1 && dy.abs() <= 1) || (dx == 0 && dy == 0) {
        return None;
    }
    let terrain = terrain?;
    let tube_id = terrain.cell(current.0, current.1)?.tube_index?;
    if tube_id.0 > i8::MAX as u16 {
        return None;
    }
    let tube = terrain.tube(tube_id)?;
    (tube.source == TubeSource::ExplicitMap && tube.path_len() > 0 && tube.exit == next)
        .then_some(tube_id)
}

/// Begin a verified explicit tube and perform native Mark(REMOVE) substrate
/// teardown. The pending non-adjacent node is consumed immediately, preserving
/// the route tail for the post-tube object turn.
#[allow(clippy::too_many_arguments)]
pub(crate) fn begin_path_tube_step(
    foot_occupation_enabled: &mut bool,
    path_replay: &mut crate::sim::components::FootPathQueue,
    entity_id: u64,
    category: EntityCategory,
    position: &mut Position,
    drive_locomotion: &mut Option<DriveLocomotionRuntime>,
    low_bridge_tube_state: &mut Option<LowBridgeTubeMovementState>,
    target: &mut MovementTarget,
    cell_marked: &mut bool,
    tube_id: TubeId,
    terrain: &ResolvedTerrainGrid,
    occupancy: &mut OccupancyGrid,
    cell_occupation: &mut CellOccupationGrid,
    raw_cell_occupation: &mut RawCellOccupationGrid,
) -> Result<(), TubeBeginError> {
    if !matches!(category, EntityCategory::Unit | EntityCategory::Infantry) {
        return Err(TubeBeginError::UnsupportedCategory);
    }
    if category == EntityCategory::Unit
        && drive_locomotion
            .as_ref()
            .is_some_and(|drive| drive.track.turn_index != -1)
    {
        return Err(TubeBeginError::MissingTerrain);
    }
    let tube = terrain
        .tube(tube_id)
        .ok_or(TubeBeginError::MissingTerrain)?;
    let state = initial_state(category, position, tube_id, tube, terrain)?;

    detach_for_tube(
        foot_occupation_enabled,
        entity_id,
        category,
        position,
        drive_locomotion,
        cell_marked,
        occupancy,
        cell_occupation,
        raw_cell_occupation,
    );
    if category == EntityCategory::Unit
        && let Some(drive) = drive_locomotion.as_mut()
    {
        // Original4B1352/1357/135A stores the signed exit center in Head_To
        // with Z=0. Destination, cursor, short selection and residual survive.
        // The queue shift4B1362..136E does not rewrite Foot+558's reference.
        drive.head_to = Some(DriveCoord {
            x: i32::from(tube.exit.0 as i16)
                .wrapping_mul(256)
                .wrapping_add(128),
            y: i32::from(tube.exit.1 as i16)
                .wrapping_mul(256)
                .wrapping_add(128),
            z: 0,
        });
        super::path_markers::consume_path_replay(path_replay, 1);
        drive.track_valid = true; // Original4B1480.
        drive.track.turn_index = -1; // Original4B1484; cursor is untouched.
    }
    *low_bridge_tube_state = Some(state);
    target.next_index = target.next_index.saturating_add(1).min(target.path.len());
    Ok(())
}

fn initial_state(
    category: EntityCategory,
    position: &mut Position,
    tube_id: TubeId,
    tube: &TubeFact,
    terrain: &ResolvedTerrainGrid,
) -> Result<LowBridgeTubeMovementState, TubeBeginError> {
    let path_len = tube.path_len();
    if path_len == 0 {
        return Err(TubeBeginError::ZeroLengthTube);
    }
    let raw_step = tube.path_steps[0];
    let (dx, dy) = direction_delta(raw_step);
    let [current_x, current_y] = ground_pose::position_world_xy(position);
    let current_ground =
        ground_pose::ground_surface_z_at([current_x, current_y], false, Some(terrain), None)
            .ok_or(TubeBeginError::MissingTerrain)?;
    let exit_ground =
        floor_at_cell_center(terrain, tube.exit).ok_or(TubeBeginError::MissingTerrain)?;
    let z_step = exit_ground.wrapping_sub(current_ground) / path_len as i32;
    let (target_x, target_y) = if category == EntityCategory::Infantry {
        (
            current_x.wrapping_add(dx.wrapping_mul(256)),
            current_y.wrapping_add(dy.wrapping_mul(256)),
        )
    } else {
        let next_x = i32::from(tube.entry.0).wrapping_add(dx);
        let next_y = i32::from(tube.entry.1).wrapping_add(dy);
        (
            next_x.wrapping_mul(256).wrapping_add(128),
            next_y.wrapping_mul(256).wrapping_add(128),
        )
    };
    position.exact_z_leptons.get_or_insert(current_ground);
    Ok(LowBridgeTubeMovementState {
        tube_id,
        cursor: 0,
        target: DriveCoord {
            x: target_x,
            y: target_y,
            z: current_ground.wrapping_add(z_step),
        },
    })
}

fn detach_for_tube(
    foot_occupation_enabled: &mut bool,
    entity_id: u64,
    category: EntityCategory,
    position: &Position,
    drive_locomotion: &mut Option<DriveLocomotionRuntime>,
    cell_marked: &mut bool,
    occupancy: &mut OccupancyGrid,
    cell_occupation: &mut CellOccupationGrid,
    raw_cell_occupation: &mut RawCellOccupationGrid,
) {
    let rx = position.rx;
    let ry = position.ry;
    occupancy.remove_on_layer(rx, ry, entity_id, MovementLayer::Ground);
    match category {
        EntityCategory::Unit => {
            if let Some(drive) = drive_locomotion.as_mut() {
                crate::sim::occupancy::clear_drive_head_to_occupation_for_remove(
                    drive,
                    cell_occupation,
                    entity_id,
                );
                *foot_occupation_enabled = false;
            }
            cell_occupation.clear_vehicle_on_layer(rx, ry, entity_id, MovementLayer::Ground);
            raw_cell_occupation.clear_ground(rx, ry, VEHICLE_OCCUPATION_BIT);
        }
        EntityCategory::Infantry => raw_cell_occupation.clear_ground(
            rx,
            ry,
            infantry_raw_occupation_mask(position.sub_x, position.sub_y),
        ),
        _ => {}
    }
    *cell_marked = false;
}

/// Execute one active Unit/Infantry TubeMovement object turn. Returns `true`
/// whenever the entity was active at entry, including a successful final which
/// cleared the state; the caller must therefore suppress ordinary movement for
/// the remainder of this tick.
#[allow(clippy::too_many_arguments)]
pub(crate) fn tick_active_tube_object(
    entities: &mut EntityStore,
    entity_id: u64,
    terrain: &ResolvedTerrainGrid,
    path_grid: Option<&PathGrid>,
    occupancy: &mut OccupancyGrid,
    cell_occupation: &mut CellOccupationGrid,
    raw_cell_occupation: &mut RawCellOccupationGrid,
    rules: Option<&RuleSet>,
    interner: &StringInterner,
    rng: &mut SimRng,
    native_frame: u32,
    scatters: &mut ScatterRequests,
) -> bool {
    let Some(entity) = entities.get(entity_id) else {
        return false;
    };
    let Some(mut state) = entity.low_bridge_tube_state else {
        return false;
    };
    let Some(tube) = terrain.tube(state.tube_id) else {
        return true;
    };
    let category = entity.category;
    let speed = rules
        .and_then(|rules| rules.object(interner.resolve(entity.type_ref())))
        .map_or(0, |object| {
            crate::util::fixed_math::ra2_speed_to_leptons_per_frame(object.speed)
        });
    let budget = if category == EntityCategory::Unit {
        speed.wrapping_mul(3) / 2
    } else {
        speed
    };
    let z_step = live_z_step(terrain, tube).unwrap_or(0);

    if usize::from(state.cursor) >= tube.path_len() {
        return finalize_tube_object(
            entities,
            entity_id,
            state,
            terrain,
            path_grid,
            occupancy,
            cell_occupation,
            raw_cell_occupation,
            rules,
            interner,
            rng,
            native_frame,
            scatters,
        );
    }

    // 0x00735A2F..0x00735A47: GetCoords (vt+0x48) less the target (+0x568),
    // through Distance3D.
    let current = ground_pose::object_get_coords(entity, Some(terrain));
    let distance = distance_3d_leptons(
        [current.x, current.y, current.z],
        [state.target.x, state.target.y, state.target.z],
    );
    if distance > budget {
        let Some(trig) = active_tube_trig() else {
            return true;
        };
        let raw_step = tube.path_steps[usize::from(state.cursor)];
        let next = advance_amount(current, state.target, budget, raw_step, z_step, trig);
        ground_pose::foot_set_location(entities, entity_id, next, rules, interner);
        return true;
    }

    state.cursor = state.cursor.saturating_add(1);
    if let Some(entity) = entities.get_mut(entity_id) {
        entity.low_bridge_tube_state = Some(state);
    }
    ground_pose::foot_set_location(entities, entity_id, state.target, rules, interner);
    if usize::from(state.cursor) >= tube.path_len() {
        return finalize_tube_object(
            entities,
            entity_id,
            state,
            terrain,
            path_grid,
            occupancy,
            cell_occupation,
            raw_cell_occupation,
            rules,
            interner,
            rng,
            native_frame,
            scatters,
        );
    }

    let next_raw = tube.path_steps[usize::from(state.cursor)];
    let (dx, dy) = direction_delta(next_raw);
    let old_target = state.target;
    state.target.x = state.target.x.wrapping_add(dx.wrapping_mul(256));
    state.target.y = state.target.y.wrapping_add(dy.wrapping_mul(256));
    state.target.z = state.target.z.wrapping_add(z_step);
    let leftover = budget.wrapping_sub(distance);
    let Some(trig) = active_tube_trig() else {
        return true;
    };
    let next = advance_amount(old_target, state.target, leftover, next_raw, z_step, trig);
    if let Some(entity) = entities.get_mut(entity_id) {
        entity.low_bridge_tube_state = Some(state);
    }
    ground_pose::foot_set_location(entities, entity_id, next, rules, interner);
    true
}

fn active_tube_trig() -> Option<&'static TrigTable> {
    let trig = crate::map::retail_trig::global();
    if trig.is_none() {
        static WARNED: OnceLock<()> = OnceLock::new();
        if WARNED.set(()).is_ok() {
            log::warn!("explicit TubeMovement requires the verified retail sine table");
        }
    }
    trig
}

#[allow(clippy::too_many_arguments)]
fn finalize_tube_object(
    entities: &mut EntityStore,
    entity_id: u64,
    state: LowBridgeTubeMovementState,
    terrain: &ResolvedTerrainGrid,
    path_grid: Option<&PathGrid>,
    occupancy: &mut OccupancyGrid,
    cell_occupation: &mut CellOccupationGrid,
    raw_cell_occupation: &mut RawCellOccupationGrid,
    rules: Option<&RuleSet>,
    interner: &StringInterner,
    rng: &mut SimRng,
    native_frame: u32,
    scatters: &mut ScatterRequests,
) -> bool {
    let Some(tube) = terrain.tube(state.tube_id) else {
        return true;
    };
    let Some(entity) = entities.get(entity_id) else {
        return true;
    };
    let category = entity.category;
    let reached_cell = (entity.position.rx, entity.position.ry);

    let infantry_subcell = if category == EntityCategory::Infantry {
        let terrain_blocked =
            path_grid.is_some_and(|grid| !grid.is_walkable(reached_cell.0, reached_cell.1));
        let passable = !terrain_blocked
            && bump_crush::cell_passable_for_infantry(
                occupancy.get(reached_cell.0, reached_cell.1),
                MovementLayer::Ground,
            );
        if !passable {
            scatter_exit_blockers(entities, entity_id, reached_cell, occupancy, scatters);
            stop_blocked_mover(entities, entity_id);
            return true;
        }
        let (sub_x, sub_y) = entities
            .get(entity_id)
            .map(|entity| (entity.position.sub_x, entity.position.sub_y))
            .unwrap_or((CELL_CENTER_LEPTON, CELL_CENTER_LEPTON));
        bump_crush::allocate_sub_cell_with_preference(
            occupancy.get(reached_cell.0, reached_cell.1),
            MovementLayer::Ground,
            None,
            sub_x,
            sub_y,
            rng,
        )
    } else {
        None
    };

    if category == EntityCategory::Unit {
        let blockers: Vec<u64> = occupancy
            .get(reached_cell.0, reached_cell.1)
            .map(|cell| {
                cell.iter_layer(MovementLayer::Ground)
                    .map(|occupant| occupant.entity_id)
                    .collect()
            })
            .unwrap_or_default();
        if !blockers.is_empty() {
            scatter_exit_blockers(entities, entity_id, reached_cell, occupancy, scatters);
            stop_blocked_mover(entities, entity_id);
            return true;
        }
    }

    if category == EntityCategory::Infantry && infantry_subcell.is_none() {
        stop_blocked_mover(entities, entity_id);
        return true;
    }

    // Both exits set the Location through SetLocation (vt+0x1B4): a unit at
    // the exit cell's centre at the target's Z (+0x570,
    // 0x00735FA1..0x00735FEC), infantry at the spot PlaceInfantryInCell
    // (0x00481180, `allocate_sub_cell_with_preference` above) chose, at the
    // floor there (0x00578080, 0x0051B967..0x0051B99C).
    let (exit, floor_supported) = if let Some(sub_cell) = infantry_subcell {
        let (sub_x, sub_y) = lepton::subcell_lepton_offset(Some(sub_cell));
        let xy = [
            i32::from(reached_cell.0) * 256 + sub_x.to_num::<i32>(),
            i32::from(reached_cell.1) * 256 + sub_y.to_num::<i32>(),
        ];
        let floor = ground_pose::ground_surface_z_at(xy, false, Some(terrain), None);
        let z = floor.unwrap_or_else(|| {
            entities.get(entity_id).map_or(state.target.z, |entity| {
                ground_pose::position_world_coord(&entity.position).z
            })
        });
        let exit = DriveCoord {
            x: xy[0],
            y: xy[1],
            z,
        };
        (exit, floor.is_some())
    } else {
        let exit = DriveCoord::cell(tube.exit.0, tube.exit.1, state.target.z);
        (exit, false)
    };
    ground_pose::foot_set_location(entities, entity_id, exit, rules, interner);
    if let Some(entity) = entities.get_mut(entity_id) {
        if let Some(sub_cell) = infantry_subcell {
            entity.sub_cell = Some(sub_cell);
            if floor_supported {
                entity.position.z = terrain
                    .cell(reached_cell.0, reached_cell.1)
                    .map_or(entity.position.z, |cell| cell.level);
            }
        } else {
            if let Some(cell) = terrain.cell(tube.exit.0, tube.exit.1) {
                entity.position.z = cell.level;
            }
            if let Some(drive) = entity.drive_locomotion.as_mut() {
                drive.target_speed_fraction = SIM_ONE;
            }
            // Unit73604F writes the live Foot owner even if PerCell replaced
            // Drive. Exact post-callback timing remains part of the Process host.
            entity.foot_speed.set_speed_fraction(SIM_ONE);
        }
        entity.low_bridge_tube_state = None;
    }
    put_after_tube(
        entities,
        entity_id,
        occupancy,
        cell_occupation,
        raw_cell_occupation,
    );

    if category == EntityCategory::Unit {
        update_unit_final_facing(entities, entity_id, terrain, native_frame);
    }
    true
}

fn put_after_tube(
    entities: &mut EntityStore,
    entity_id: u64,
    occupancy: &mut OccupancyGrid,
    cell_occupation: &mut CellOccupationGrid,
    raw_cell_occupation: &mut RawCellOccupationGrid,
) {
    let Some(entity) = entities.get_mut(entity_id) else {
        return;
    };
    let rx = entity.position.rx;
    let ry = entity.position.ry;
    occupancy.add(
        rx,
        ry,
        entity_id,
        MovementLayer::Ground,
        (entity.category == EntityCategory::Infantry)
            .then_some(entity.sub_cell)
            .flatten(),
        CellListInsertion::from_category(entity.category),
    );
    match entity.category {
        EntityCategory::Unit => {
            raw_cell_occupation.mark_ground(rx, ry, VEHICLE_OCCUPATION_BIT);
            cell_occupation.mark_vehicle_on_layer(rx, ry, entity_id, MovementLayer::Ground);
            entity.foot_occupation_enabled = true;
        }
        EntityCategory::Infantry => {
            let mask = infantry_raw_occupation_mask(entity.position.sub_x, entity.position.sub_y);
            raw_cell_occupation.mark_ground(rx, ry, mask);
            // Native Infantry final explicitly calls the same raw mark again.
            raw_cell_occupation.mark_ground(rx, ry, mask);
        }
        _ => {}
    }
    entity.lifecycle.cell_marked = true;
}

/// `UnitClass::TubeMovement`'s blocked exit (`0x00735F55`): `Scatter(null,
/// 1, 1)` on each ground occupant that is a Unit or Infantry whose locomotor
/// is not moving. The object turn runs the calls once the pass returns
/// ([`ScatterRequests`]).
fn scatter_exit_blockers(
    entities: &EntityStore,
    mover_id: u64,
    cell: (u16, u16),
    occupancy: &OccupancyGrid,
    scatters: &mut ScatterRequests,
) {
    let blockers: Vec<u64> = occupancy
        .get(cell.0, cell.1)
        .map(|cell| {
            cell.iter_layer(MovementLayer::Ground)
                .map(|occupant| occupant.entity_id)
                .collect()
        })
        .unwrap_or_default();
    for blocker_id in blockers {
        if blocker_id == mover_id {
            continue;
        }
        let Some(blocker) = entities.get(blocker_id) else {
            continue;
        };
        if !matches!(
            blocker.category,
            EntityCategory::Unit | EntityCategory::Infantry
        ) || blocker.locomotor.is_none()
            || locomotor_is_moving(blocker)
        {
            continue;
        }
        scatters.request(blocker_id, ScatterFlags::new(true, true));
    }
}

fn locomotor_is_moving(entity: &GameEntity) -> bool {
    // Other families still require their retained-state/lifecycle migration.
    super::motion_query::is_moving(entity).unwrap_or_else(|| entity.movement_target.is_some())
}

fn stop_blocked_mover(entities: &mut EntityStore, entity_id: u64) {
    if let Some(entity) = entities.get_mut(entity_id) {
        if let Some(drive) = entity.drive_locomotion.as_mut() {
            drive.target_speed_fraction = SIM_ZERO;
        }
        // Unit735F6A / Infantry51B8FC apply zero on the live Foot owner.
        entity.foot_speed.set_speed_fraction(SIM_ZERO);
    }
}

fn update_unit_final_facing(
    entities: &mut EntityStore,
    entity_id: u64,
    terrain: &ResolvedTerrainGrid,
    native_frame: u32,
) {
    let Some(entity) = entities.get_mut(entity_id) else {
        return;
    };
    let Some(tube) = terrain
        .cell(entity.position.rx, entity.position.ry)
        .and_then(|cell| cell.tube_index)
        .and_then(|tube_id| terrain.tube(tube_id))
    else {
        return;
    };
    let q32 = tube.direction.wrapping_shl(13).wrapping_sub(0x6001) & 0xffff_e000_u32 as i32;
    entity.body_facing.snap(q32 as u16, native_frame);
}

fn live_z_step(terrain: &ResolvedTerrainGrid, tube: &TubeFact) -> Option<i32> {
    let count = i32::try_from(tube.path_len()).ok()?;
    if count == 0 {
        return None;
    }
    let entry = floor_at_cell_center(terrain, tube.entry)?;
    let exit = floor_at_cell_center(terrain, tube.exit)?;
    Some(exit.wrapping_sub(entry) / count)
}

/// The floor height (`0x00578080`) at a cell's centre, as the live Z step
/// reads it at the tube's entry and exit (`0x00735B05`, `0x00735B16`).
fn floor_at_cell_center(terrain: &ResolvedTerrainGrid, cell: (u16, u16)) -> Option<i32> {
    let center = DriveCoord::cell(cell.0, cell.1, 0);
    ground_pose::ground_surface_z_at([center.x, center.y], false, Some(terrain), None)
}

fn direction_delta(raw: i32) -> (i32, i32) {
    crate::util::direction::DIRECTION_DELTAS[(raw & 7) as usize]
}

fn advance_amount(
    base: DriveCoord,
    target: DriveCoord,
    amount: i32,
    raw_step: i32,
    z_step: i32,
    trig: &TrigTable,
) -> DriveCoord {
    let facing = crate::util::direction_tables::facing16_from_delta(
        target.x.wrapping_sub(base.x),
        target.y.wrapping_sub(base.y),
    );
    let angle_scale = f64::from_bits(0xbf19_222d_989f_5e57);
    let angle = f64::from(i32::from(facing as i16).wrapping_sub(0x3fff)) * angle_scale;
    let cosine = trig.cos_radians(angle);
    let sine = trig.sin_radians(angle);
    let amount_x87 = X87Chop53::load_i32(amount);
    let x = ftol_add_f32_product(base.x, cosine, amount_x87);
    let y = ftol_add_f32_product(base.y, -sine, amount_x87);

    let nominal_bits = if raw_step & 1 == 0 {
        256.0_f64.to_bits()
    } else {
        0x4076_a09e_667f_3bcd
    };
    let nominal = X87Chop53::load_f64(NativeF64Bits::from_bits(nominal_bits))
        .expect("nominal tube segment length is finite");
    let ratio = X87Chop53::div(amount_x87, nominal).expect("nominal length is nonzero");
    let z_delta = X87Chop53::mul(ratio, X87Chop53::load_i32(z_step));
    let z = X87Chop53::ftol_i64(X87Chop53::add(X87Chop53::load_i32(base.z), z_delta))
        .expect("tube Z fits i32") as i32;
    DriveCoord { x, y, z }
}

fn ftol_add_f32_product(base: i32, factor: f32, amount: crate::util::native_x87::X87Value) -> i32 {
    let factor = X87Chop53::load_f32(NativeF32Bits::from_bits(factor.to_bits()))
        .expect("retail trig entry is finite");
    X87Chop53::ftol_i64(X87Chop53::add(
        X87Chop53::load_i32(base),
        X87Chop53::mul(factor, amount),
    ))
    .expect("tube XY fits i32") as i32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::resolved_terrain::{ResolvedTerrainCell, ResolvedTerrainGrid};
    use crate::map::tube_facts::TubeSource;
    use crate::sim::components::{DriveLocomotionRuntime, Health};
    use crate::sim::game_entity::GameEntity;
    use crate::sim::occupancy::CellListInsertion;

    #[test]
    fn tube_exit_scatter_skips_retained_motion_without_an_order() {
        use crate::rules::locomotor_type::LocomotorKind;
        use crate::sim::movement::locomotor::LocomotorState;
        for kind in [LocomotorKind::Walk, LocomotorKind::Jumpjet] {
            let mut actor = GameEntity::test_default(2, "E1", "Americans", 5, 5);
            actor.category = EntityCategory::Infantry;
            let mut loco = LocomotorState::for_test_kind(kind);
            if let Some(state) = loco.jumpjet_runtime_mut() {
                state.moving = true;
                state.phase = 2;
            } else {
                loco.set_walk_destination(Some(DriveCoord::cell(6, 5, 0)));
            }
            actor.locomotor = Some(loco);
            let before = serde_json::to_value(&actor).unwrap();
            let mut entities = EntityStore::new();
            entities.insert(actor);
            let mut occupancy = OccupancyGrid::new();
            occupancy.add(
                5,
                5,
                2,
                MovementLayer::Ground,
                Some(0),
                CellListInsertion::PrependNonBuilding,
            );
            let mut scatters = ScatterRequests::default();
            scatter_exit_blockers(&entities, 1, (5, 5), &occupancy, &mut scatters);
            assert!(scatters.take().is_empty());
            assert_eq!(
                serde_json::to_value(entities.get(2).unwrap()).unwrap(),
                before
            );
        }
    }

    fn flat_cell(rx: u16, ry: u16, tube_index: Option<TubeId>) -> ResolvedTerrainCell {
        ResolvedTerrainCell {
            tube_index,
            ..crate::map::resolved_terrain::test_loader_clear_cell(rx, ry)
        }
    }

    fn explicit_terrain(path: Vec<i32>) -> ResolvedTerrainGrid {
        let mut cells = Vec::new();
        for x in 0..=3 {
            cells.push(flat_cell(x, 0, (x == 0).then_some(TubeId(0))));
        }
        ResolvedTerrainGrid::from_cells_with_tubes(
            4,
            1,
            cells,
            vec![TubeFact {
                entry: (0, 0),
                exit: (path.len() as u16, 0),
                direction: 2,
                path_steps: path,
                source: TubeSource::ExplicitMap,
            }],
        )
    }

    fn unit(id: u64) -> GameEntity {
        unit_of(id, "MTNK")
    }

    fn unit_of(id: u64, kind: &str) -> GameEntity {
        let mut entity = GameEntity::new_at_frame_zero_for_test(
            id,
            0,
            0,
            0,
            0,
            crate::sim::intern::test_intern("Americans"),
            Health { current: 100 },
            crate::sim::intern::test_intern(kind),
            EntityCategory::Unit,
            0,
            5,
            true,
        );
        entity.lifecycle.in_limbo = false;
        entity.lifecycle.cell_marked = true;
        entity.drive_locomotion = Some(DriveLocomotionRuntime::default());
        entity
    }

    #[test]
    fn gsi_04_15_zero_step_auto_shell_is_never_a_path_producer() {
        let terrain = ResolvedTerrainGrid::from_cells_with_tubes(
            1,
            1,
            vec![flat_cell(0, 0, Some(TubeId(0)))],
            vec![TubeFact::auto_low_bridge((0, 0), 2)],
        );
        let mut target = MovementTarget::default();
        target.path = vec![(0, 0), (2, 0)];
        target.next_index = 1;
        let entity = unit(1);
        assert_eq!(
            pending_path_tube_id(
                &target,
                &entity.position,
                MovementLayer::Ground,
                Some(&terrain)
            ),
            None
        );
    }

    /// Supplied post-Tube state exercises the real finalizer's owner writes.
    /// It does not emulate the preceding native PerCell callback or its timing.
    #[test]
    fn tube_finalizer_updates_live_owner_speed_without_drive_payload() {
        use crate::rules::locomotor_type::LocomotorKind;
        use crate::sim::movement::locomotor::LocomotorState;
        use crate::util::fixed_math::SimFixed;
        for (category, blocked) in [
            (EntityCategory::Unit, false),
            (EntityCategory::Unit, true),
            (EntityCategory::Infantry, true),
        ] {
            let terrain = explicit_terrain(vec![2, 2]);
            let mut entity = unit(1);
            entity.category = category;
            entity.drive_locomotion = None;
            entity.locomotor = Some(LocomotorState::for_test_kind(LocomotorKind::Teleport));
            entity.lifecycle.cell_marked = false;
            entity.position.rx = 2;
            entity.foot_speed.set_speed_fraction(SimFixed::lit("0.625"));
            entity.movement_target = Some(MovementTarget {
                speed: SimFixed::from_num(150),
                ..Default::default()
            });
            let state = LowBridgeTubeMovementState {
                tube_id: TubeId(0),
                cursor: 2,
                target: DriveCoord::cell(2, 0, 0),
            };
            entity.low_bridge_tube_state = Some(state);
            let mut entities = EntityStore::new();
            entities.insert(entity);
            let mut occupancy = OccupancyGrid::new();
            if blocked && category == EntityCategory::Unit {
                let mut blocker = unit(2);
                blocker.position.rx = 2;
                entities.insert(blocker);
                occupancy.add(
                    2,
                    0,
                    2,
                    MovementLayer::Ground,
                    None,
                    CellListInsertion::PrependNonBuilding,
                );
            }
            let grid = PathGrid::test_all_blocked(4, 1);
            assert!(finalize_tube_object(
                &mut entities,
                1,
                state,
                &terrain,
                (category == EntityCategory::Infantry).then_some(&grid),
                &mut occupancy,
                &mut CellOccupationGrid::new(),
                &mut RawCellOccupationGrid::new(),
                None,
                &crate::sim::intern::test_interner(),
                &mut SimRng::new(7),
                21,
                &mut ScatterRequests::default(),
            ));
            let owner = entities.get(1).unwrap();
            assert!(owner.drive_locomotion.is_none());
            assert_eq!(
                owner.locomotor.as_ref().unwrap().active_kind(),
                LocomotorKind::Teleport
            );
            assert_eq!(
                owner.foot_speed.applied_fraction(),
                if blocked { SIM_ZERO } else { SIM_ONE }
            );
            // GetCurrentSpeed follows the fraction; the rules-less getter
            // reads the order's stamped 150 leptons/s.
            assert_eq!(
                crate::sim::movement::owner_current_speed(owner, None, 1.0),
                if blocked { 0 } else { 10 }
            );
            assert_eq!(owner.low_bridge_tube_state.is_some(), blocked);
            if !blocked {
                assert!(owner.lifecycle.cell_marked);
                assert_eq!(occupancy.count_on_layer(2, 0, MovementLayer::Ground), 1);
            }
        }
    }

    #[test]
    fn gsi_04_15_tube_admission_requires_ground_object_list_layer() {
        let terrain = explicit_terrain(vec![2, 2]);
        let entity = unit(1);
        let mut target = MovementTarget {
            path: vec![(0, 0), (2, 0)],
            next_index: 1,
            ..MovementTarget::default()
        };

        assert_eq!(
            pending_path_tube_id(
                &target,
                &entity.position,
                MovementLayer::Bridge,
                Some(&terrain)
            ),
            None
        );
        assert_eq!(
            pending_path_tube_id(
                &target,
                &entity.position,
                MovementLayer::Ground,
                Some(&terrain)
            ),
            Some(TubeId(0))
        );

        target.bypass_grid = true;
        assert_eq!(
            pending_path_tube_id(
                &target,
                &entity.position,
                MovementLayer::Ground,
                Some(&terrain)
            ),
            None
        );
    }

    #[test]
    fn gsi_04_15_begin_detaches_ground_list_owner_mark_and_raw_byte() {
        let terrain = explicit_terrain(vec![2, 2]);
        let mut entity = unit(1);
        let destination = DriveCoord {
            x: 901,
            y: -17,
            z: 731,
        };
        let retained = crate::sim::components::TrackProgress {
            turn_index: -1,
            cursor: -1,
            reversed: true,
            residual: 6,
        };
        let drive = entity.drive_locomotion.as_mut().unwrap();
        drive.destination = Some(destination);
        drive.head_to = Some(DriveCoord {
            x: 43,
            y: 81,
            z: 512,
        });
        drive.track = retained;
        entity.navigation.path_replay = crate::sim::components::FootPathQueue {
            directions: vec![8, 2],
            cursor: 0,
            reference_cell: Some((-3, 7)),
        };
        entity.movement_target = Some(MovementTarget {
            path: vec![(0, 0), (2, 0), (3, 0)],
            next_index: 1,
            ..MovementTarget::default()
        });
        let mut occupancy = OccupancyGrid::new();
        occupancy.add(
            0,
            0,
            1,
            MovementLayer::Ground,
            None,
            CellListInsertion::PrependNonBuilding,
        );
        let mut cell_occupation = CellOccupationGrid::new();
        cell_occupation.mark_vehicle_on_layer(0, 0, 1, MovementLayer::Ground);
        let mut raw = RawCellOccupationGrid::new();
        raw.mark_ground(0, 0, VEHICLE_OCCUPATION_BIT);

        begin_path_tube_step(
            &mut true,
            &mut entity.navigation.path_replay,
            entity.stable_id,
            entity.category,
            &mut entity.position,
            &mut entity.drive_locomotion,
            &mut entity.low_bridge_tube_state,
            entity.movement_target.as_mut().unwrap(),
            &mut entity.lifecycle.cell_marked,
            TubeId(0),
            &terrain,
            &mut occupancy,
            &mut cell_occupation,
            &mut raw,
        )
        .unwrap();

        assert!(!entity.lifecycle.cell_marked);
        assert_eq!(entity.movement_target.as_ref().unwrap().next_index, 2);
        assert_eq!(occupancy.count_on_layer(0, 0, MovementLayer::Ground), 0);
        assert_eq!(raw.ground_bits(0, 0), 0);
        assert_eq!(cell_occupation.vehicle_bits(0, 0, MovementLayer::Ground), 0);
        assert_eq!(entity.low_bridge_tube_state.unwrap().target.x, 384);
        let drive = entity.drive_locomotion.as_ref().unwrap();
        assert_eq!(drive.destination, Some(destination));
        assert_eq!(
            drive.head_to,
            Some(DriveCoord {
                x: 640,
                y: 128,
                z: 0
            })
        );
        assert!(drive.track_valid);
        assert_eq!(drive.track, retained);
        assert_eq!(entity.navigation.path_replay.cursor, 1);
        assert_eq!(entity.navigation.path_replay.reference_cell, Some((-3, 7)));
    }

    #[test]
    fn gsi_04_15_cardinal_partial_uses_full_3d_distance_but_nominal_z_denominator() {
        let trig = TrigTable::synthetic();
        let current = DriveCoord { x: 0, y: 0, z: 0 };
        let target = DriveCoord {
            x: 256,
            y: 0,
            z: 100,
        };
        assert!(distance_3d_leptons([0, 0, 0], [target.x, target.y, target.z]) > 256);
        assert_eq!(
            advance_amount(current, target, 100, 2, 100, &trig),
            DriveCoord {
                x: 100,
                y: 0,
                z: 39
            }
        );
    }

    #[test]
    fn gsi_04_15_type_speed_budget_uses_native_scaled_field() {
        use crate::util::fixed_math::ra2_speed_to_leptons_per_frame;
        assert_eq!(ra2_speed_to_leptons_per_frame(-1), 0);
        assert_eq!(ra2_speed_to_leptons_per_frame(4), 10);
        assert_eq!(ra2_speed_to_leptons_per_frame(100), 255);
        assert_eq!(ra2_speed_to_leptons_per_frame(200), 255);
        assert_eq!(ra2_speed_to_leptons_per_frame(11) * 3 / 2, 42);
    }

    #[test]
    fn gsi_04_15_unit_final_facing_uses_final_cells_raw_tube_direction() {
        let terrain = explicit_terrain(vec![2, 2]);
        let mut entities = EntityStore::new();
        let mut entity = unit(1);
        entity.position.rx = 2;
        entity.low_bridge_tube_state = Some(LowBridgeTubeMovementState {
            tube_id: TubeId(0),
            cursor: 2,
            target: DriveCoord {
                x: 640,
                y: 128,
                z: 0,
            },
        });
        entities.insert(entity);
        update_unit_final_facing(&mut entities, 1, &terrain, 0);
        // Exit has no tube index in this fixture: facing is preserved.
        assert_eq!(entities.get(1).unwrap().body_facing_current(0), 0);
    }

    /// Infantry leaves a tube at its chosen spot, at the floor there
    /// (`0x00578080` at `0x0051B98A`), through SetLocation (`0x0051B99C`).
    /// The floor is native's: `tools/ramp_height_vectors.json`
    /// `ramp_1_sub_64_192` (level 0, ramp 1, sub-cell (64, 192)) is 26.
    #[test]
    fn infantry_leaves_a_tube_at_the_floor_of_its_spot() {
        let mut terrain = explicit_terrain(vec![2, 2]);
        terrain.cell_mut(2, 0).unwrap().slope_type = 1;
        let mut entity = unit(1);
        entity.category = EntityCategory::Infantry;
        entity.drive_locomotion = None;
        entity.lifecycle.cell_marked = false;
        entity.position.rx = 2;
        entity.position.sub_x = crate::util::fixed_math::SimFixed::from_num(64);
        entity.position.sub_y = crate::util::fixed_math::SimFixed::from_num(192);
        entity.position.exact_z_leptons = Some(0);
        let state = LowBridgeTubeMovementState {
            tube_id: TubeId(0),
            cursor: 2,
            target: DriveCoord {
                x: 2 * 256 + 64,
                y: 192,
                z: 0,
            },
        };
        entity.low_bridge_tube_state = Some(state);
        let mut entities = EntityStore::new();
        entities.insert(entity);
        assert!(finalize_tube_object(
            &mut entities,
            1,
            state,
            &terrain,
            None,
            &mut OccupancyGrid::new(),
            &mut CellOccupationGrid::new(),
            &mut RawCellOccupationGrid::new(),
            None,
            &crate::sim::intern::test_interner(),
            &mut SimRng::new(7),
            21,
            &mut ScatterRequests::default(),
        ));
        let owner = entities.get(1).unwrap();
        assert!(owner.low_bridge_tube_state.is_none());
        assert_eq!(owner.sub_cell, Some(3));
        assert_eq!(
            ground_pose::position_world_coord(&owner.position),
            DriveCoord {
                x: 2 * 256 + 64,
                y: 192,
                z: 26
            }
        );
    }

    /// A step that reaches its target moves the unit through
    /// `SetLocation(target)` (vt+0x1B4 = `FootClass::SetLocation 0x004DB810`,
    /// `0x00735B58`), and the leftover step through `0x00735D27`; the
    /// `OpenTopped=` tail carries the riders along (`0x004DB88A` ->
    /// `0x007104F0`).
    #[test]
    fn an_open_topped_transport_carries_its_riders_through_a_tube() {
        use crate::rules::ini_parser::IniFile;
        use crate::sim::passenger::{PassengerCargo, PassengerRole};
        let rules = RuleSet::from_ini(&IniFile::from_str(
            "[VehicleTypes]\n0=BFRT\n[BFRT]\nSpeed=4\nOpenTopped=yes\nPassengers=5\n",
        ))
        .expect("rules");
        let terrain = explicit_terrain(vec![2, 2]);
        let mut transport = unit_of(1, "BFRT");
        // Ten leptons short of the first step's target, inside the budget.
        transport.position.rx = 1;
        transport.position.sub_x = crate::util::fixed_math::SimFixed::from_num(118);
        transport.low_bridge_tube_state = Some(LowBridgeTubeMovementState {
            tube_id: TubeId(0),
            cursor: 0,
            target: DriveCoord::cell(1, 0, 0),
        });
        let mut cargo = PassengerCargo::new(5, 2);
        assert!(cargo.board(2, 1));
        transport.passenger_role = PassengerRole::Transport { cargo };
        let mut rider = unit_of(2, "E1");
        rider.category = EntityCategory::Infantry;
        rider.position.rx = 3;
        rider.passenger_role = PassengerRole::Inside {
            transport_id: 1,
            open_topped: true,
        };
        let mut entities = EntityStore::new();
        entities.insert(transport);
        entities.insert(rider);
        assert!(tick_active_tube_object(
            &mut entities,
            1,
            &terrain,
            None,
            &mut OccupancyGrid::new(),
            &mut CellOccupationGrid::new(),
            &mut RawCellOccupationGrid::new(),
            Some(&rules),
            &crate::sim::intern::test_interner(),
            &mut SimRng::new(7),
            21,
            &mut ScatterRequests::default(),
        ));
        let [transport, rider] = [1, 2].map(|id| &entities.get(id).unwrap().position);
        let coord = |p: &Position| (ground_pose::position_world_xy(p), p.exact_z_leptons);
        // With the retail sine table loaded (another test may have), the
        // leftover budget then steps on towards the next target as well.
        assert!(
            ground_pose::position_world_xy(transport)[0] >= 384,
            "the step reached its target"
        );
        assert_eq!(coord(rider), coord(transport), "the rider came along");
    }
}
