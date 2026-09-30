//! Grounded ObjectClass coordinate writes shared by movement and placement.
//!
//! `ObjectClass::SetHeight @ 0x005F5FA0` ([`set_height`]) samples the committed
//! world XY through GetGroundHeight @ 0x00578080, then adds the explicit
//! OnBridge offset. Callers own cadence: Drive/Ship residual movement does
//! not call this setter and retains the last raw coordinate Z.

use crate::map::cell_index::NativeCellIdentity;
use crate::map::resolved_terrain::{NativeCellQuery, ResolvedTerrainGrid};
use crate::sim::components::{DriveCoord, Position};
use crate::sim::pathfinding::PathGrid;
use crate::sim::world::Simulation;
use crate::util::lepton::{
    BRIDGE_DECK_HEIGHT_LEPTONS, GROUND_LEVEL_HEIGHT_LEPTONS, ground_height_leptons,
};

/// Map578080 through the caller's query identity. Input queries isolate Dummy;
/// simulation callbacks use the canonical retained Dummy and its lookup order.
pub(crate) fn query_ground_height(
    cells: &NativeCellQuery<'_>,
    point: DriveCoord,
) -> Result<i32, String> {
    let cell = cells.lookup_world(point.x, point.y);
    let (level, slope) = cells.ground_fields(cell);
    ground_height_leptons(level, slope, point.x, point.y)
        .map_err(|error| format!("native ground query: {error:?}"))
}

/// Object+1BC receiver5F6960 performs two Map565730 lookups from physical
/// Object+9C. The first lookup is observable when it stamps the shared Dummy;
/// the second returns the identity retained by the caller.
pub(crate) fn query_object_cell(
    cells: &NativeCellQuery<'_>,
    physical: DriveCoord,
) -> NativeCellIdentity {
    let _ = cells.lookup_world(physical.x, physical.y);
    cells.lookup_world(physical.x, physical.y)
}

/// Object5F5F00: signed current-cell level plus four for OnBridge, through
/// the same Object+1BC query used by firing and movement.
pub(crate) fn query_object_cell_height(
    cells: &NativeCellQuery<'_>,
    physical: DriveCoord,
    on_bridge: bool,
) -> i32 {
    let cell = query_object_cell(cells, physical);
    i32::from(cells.ground_fields(cell).0 as i8) + if on_bridge { 4 } else { 0 }
}

/// Foot+BC4DDC40(false) -> Object5F6A70. The navigation coordinate can be a
/// paid head; source bridge selection is independent of the cached path layer.
/// Both ground samples precede the conditional structural-cell lookup.
/// Original-byte layer and consumer-seam receipts, including312/313 and
/// Tube/Dummy ordering: tools/spatial_oracle/foot_bridge_layer.{json,md}.
pub(crate) fn navigation_should_be_on_bridge(
    cells: &NativeCellQuery<'_>,
    navigation: DriveCoord,
    current: DriveCoord,
    on_bridge: bool,
    in_tube: bool,
) -> Result<bool, String> {
    if in_tube {
        return Ok(false);
    }
    let head_ground = query_ground_height(cells, navigation)?;
    let current_ground = query_ground_height(cells, current)?;
    if !on_bridge && current_ground.wrapping_sub(head_ground) > 3 * GROUND_LEVEL_HEIGHT_LEPTONS {
        return Ok(cells.flags(cells.lookup_world(navigation.x, navigation.y)) & 0x100 != 0);
    }
    if on_bridge && head_ground.wrapping_sub(current_ground) > 3 * GROUND_LEVEL_HEIGHT_LEPTONS {
        return Ok(false);
    }
    Ok(on_bridge)
}

pub(crate) fn position_world_xy(position: &Position) -> [i32; 2] {
    [
        i32::from(position.rx)
            .wrapping_mul(256)
            .wrapping_add(position.sub_x.to_num::<i32>()),
        i32::from(position.ry)
            .wrapping_mul(256)
            .wrapping_add(position.sub_y.to_num::<i32>()),
    ]
}

/// Read retained ObjectClass coordinates without resampling changed terrain.
/// Legacy positions without an exact Z use their stored signed level until a
/// real coordinate writer supplies raw leptons. That level omits the slope, a
/// Hover or Air height and a parachute's height, so a reader of an object's
/// Z asks [`object_location`] or [`object_get_coords`] instead.
pub(crate) fn position_world_coord(position: &Position) -> DriveCoord {
    let [x, y] = position_world_xy(position);
    DriveCoord {
        x,
        y,
        z: position.exact_z_leptons.unwrap_or_else(|| {
            i32::from(position.z as i8) * crate::util::lepton::GROUND_LEVEL_HEIGHT_LEPTONS
        }),
    }
}

/// ObjectClass GetCoords Z (virtual +0x48, Object+0xA4) of any object: the
/// one answer to "how high is this object". An exact coordinate a native
/// writer retained is already total world Z. Otherwise VERA still keeps the
/// object's height in parts: the live sloped ground at its XY (Map578080
/// through `ground_height_leptons`), the OnBridge deck, and its one active
/// altitude source ([`object_altitude_leptons`]). Without terrain the stored
/// signed level stands in for the ground. InRange's low-flying snap is its
/// caller's rule, not part of this read.
pub(crate) fn object_world_z_leptons(
    entity: &crate::sim::game_entity::GameEntity,
    terrain: Option<&ResolvedTerrainGrid>,
) -> i32 {
    entity.position.exact_z_leptons.unwrap_or_else(|| {
        object_ground_z_leptons(entity, terrain).wrapping_add(object_altitude_leptons(entity))
    })
}

/// The ground an object without an exact coordinate stands on: the live
/// sloped ground at its XY plus the OnBridge deck, or without terrain its
/// stored signed level (which already includes a deck).
pub(crate) fn object_ground_z_leptons(
    entity: &crate::sim::game_entity::GameEntity,
    terrain: Option<&ResolvedTerrainGrid>,
) -> i32 {
    let [x, y] = position_world_xy(&entity.position);
    terrain
        .and_then(|terrain| terrain.cell(entity.position.rx, entity.position.ry))
        .and_then(|cell| ground_height_leptons(cell.level, cell.slope_type, x, y).ok())
        .map(|ground| {
            ground.wrapping_add(if entity.on_bridge {
                BRIDGE_DECK_HEIGHT_LEPTONS
            } else {
                0
            })
        })
        .unwrap_or_else(|| i32::from(entity.position.z as i8) * GROUND_LEVEL_HEIGHT_LEPTONS)
}

/// Height above the ground of an object without an exact coordinate: a
/// rocket's own flight state (its locomotor keeps only a lagging piggyback
/// copy), else the altitude of an Air-layer locomotor or of an active Hover
/// (which floats on the Ground layer). Any other Ground-layer locomotor never
/// lifts, which keeps a landed or docked aircraft on the floor whatever its
/// stale altitude. A falling object always has an exact coordinate.
pub(crate) fn object_altitude_leptons(entity: &crate::sim::game_entity::GameEntity) -> i32 {
    if let Some(state) = entity.rocket_state.as_ref() {
        return state.altitude.to_num::<i32>();
    }
    entity
        .locomotor
        .as_ref()
        .filter(|locomotor| {
            use crate::rules::locomotor_type::LocomotorKind;
            (locomotor.layer == crate::sim::movement::locomotor::MovementLayer::Air
                && locomotor.kind != LocomotorKind::Rocket)
                || locomotor.active_kind() == LocomotorKind::Hover
        })
        .map_or(0, |locomotor| locomotor.altitude.to_num::<i32>())
}

/// Building render-coordinate459EF0 and GetYSort449410's type adjustment.
/// Shared by display registration and presentation; neither uses center coords.
pub(crate) fn building_render_order_parts(
    mut location: DriveCoord,
    turret_anim_is_voxel: bool,
    gate: bool,
) -> (DriveCoord, i32) {
    location.x = location.x.wrapping_sub(128);
    location.y = location.y.wrapping_sub(128);
    (
        location,
        i32::from(turret_anim_is_voxel) * 32 - i32::from(gate) * 16,
    )
}

/// `BuildingClass::GetCoords @ 0x00447AC0`'s XY: a building's Location plus
/// `(dimension - 1) * 128` leptons along each axis of its foundation. The
/// Location is its north-west cell's centre, so the result is the
/// foundation's geometric centre.
fn foundation_center_xy(location: [i32; 2], foundation: &str) -> [i32; 2] {
    let (width, height) = crate::rules::foundation::foundation_dimensions(foundation);
    [
        location[0].wrapping_add(i32::from(width).wrapping_mul(128).wrapping_sub(128)),
        location[1].wrapping_add(i32::from(height).wrapping_mul(128).wrapping_sub(128)),
    ]
}

/// The XY of an object's GetCoords (virtual +0x48). A Unit, Infantry or
/// Aircraft returns its Location (`ObjectClass::GetCoords @ 0x005F65A0`). A
/// building returns its foundation centre ([`foundation_center_xy`]), read
/// from the foundation its type stamped on it at construction.
pub(crate) fn object_center_xy(entity: &crate::sim::game_entity::GameEntity) -> [i32; 2] {
    let location = position_world_xy(&entity.position);
    if entity.category == crate::map::entities::EntityCategory::Structure {
        foundation_center_xy(location, &entity.foundation)
    } else {
        location
    }
}

/// An object's Location (`ObjectClass+0x9C..+0xA4`) at its world Z
/// ([`object_world_z_leptons`]).
pub(crate) fn object_location(
    entity: &crate::sim::game_entity::GameEntity,
    terrain: Option<&ResolvedTerrainGrid>,
) -> DriveCoord {
    let [x, y] = position_world_xy(&entity.position);
    DriveCoord {
        x,
        y,
        z: object_world_z_leptons(entity, terrain),
    }
}

/// An object's GetCoords (virtual +0x48) in full: [`object_center_xy`] at the
/// object's world Z ([`object_world_z_leptons`]). A building keeps its
/// Location's Z. That Z is the floor at its Location: its Unlimbo coordinate
/// passes through `BuildingTypeClass` virtual +0x6C (`0x00464A70`), which
/// replaces the Z with `0x00578080`'s ground height at that XY.
///
/// This is not a building's +0x4C approach coordinate for docks and bunkers.
pub(crate) fn object_get_coords(
    entity: &crate::sim::game_entity::GameEntity,
    terrain: Option<&ResolvedTerrainGrid>,
) -> DriveCoord {
    let [x, y] = object_center_xy(entity);
    DriveCoord {
        x,
        y,
        z: object_world_z_leptons(entity, terrain),
    }
}

/// The level and slope `CellClass::GetGroundHeight @ 0x00578080` reads at
/// full world XY. With resolved terrain that is Map[coord] (`0x00565730`),
/// which forms the wrapping fixed-stride index before narrowing fallback
/// coordinates; keep the common lookup owner. A PathGrid supplies the same
/// fields only for callers without resolved terrain.
fn ground_fields_at(
    world_xy: [i32; 2],
    terrain: Option<&ResolvedTerrainGrid>,
    path_grid: Option<&PathGrid>,
) -> Option<(u8, u8)> {
    if let Some(terrain) = terrain {
        let cells = NativeCellQuery::canonical(terrain);
        return Some(cells.ground_fields(cells.lookup_world(world_xy[0], world_xy[1])));
    }
    let rx = (world_xy[0] / 256) as i16;
    let ry = (world_xy[1] / 256) as i16;
    let cell = path_grid?.cell(rx as u16, ry as u16)?;
    Some((cell.ground_level, cell.slope_type))
}

fn supported_ground(level: u8, slope: u8, world_xy: [i32; 2]) -> Option<i32> {
    match ground_height_leptons(level, slope, world_xy[0], world_xy[1]) {
        Ok(ground) => Some(ground),
        Err(_) => {
            log::warn!(
                "ground pose at {world_xy:?} has unsupported slope {slope}; retaining raw Z"
            );
            None
        }
    }
}

/// Sample the live surface at full world XY: the ground plus the deck when
/// `on_bridge`, which is the Z SetHeight(0) writes. Missing headless terrain
/// leaves the caller's existing coordinate authoritative.
pub(crate) fn ground_surface_z_at(
    world_xy: [i32; 2],
    on_bridge: bool,
    terrain: Option<&ResolvedTerrainGrid>,
    path_grid: Option<&PathGrid>,
) -> Option<i32> {
    let (level, slope) = ground_fields_at(world_xy, terrain, path_grid)?;
    Some(z_at_height(
        supported_ground(level, slope, world_xy)?,
        0,
        on_bridge,
    ))
}

/// `ObjectClass::SetHeight @ 0x005F5FA0`'s Z for a sampled ground: the
/// requested height gains the deck (`[0x00AC13BC]`) when OnBridge
/// (`0x005F5FAB..0x005F5FB5`), then the ground under the Location
/// (`0x00578080`) is added (`0x005F5FFB`, `0x005F6044`).
pub(crate) fn z_at_height(ground: i32, height: i32, on_bridge: bool) -> i32 {
    ground.wrapping_add(height).wrapping_add(if on_bridge {
        BRIDGE_DECK_HEIGHT_LEPTONS
    } else {
        0
    })
}

/// `ObjectClass::GetHeight @ 0x005F5F40` for a sampled ground: the Z less the
/// ground and, OnBridge, the deck. The inverse of [`z_at_height`].
pub(crate) fn height_at_z(z: i32, ground: i32, on_bridge: bool) -> i32 {
    z.wrapping_sub(ground).wrapping_sub(if on_bridge {
        BRIDGE_DECK_HEIGHT_LEPTONS
    } else {
        0
    })
}

/// `ObjectClass::SetHeight @ 0x005F5FA0` (every object vtable's `+0x1CC`)
/// writing the Location's Z: [`z_at_height`] over the ground under its XY.
///
/// Callers that hold the object through a borrow call this where native runs
/// SetHeight unmarked: between Mark(REMOVE) and Mark(PUT), or with `+0x74`
/// cleared around it, as Drive (`0x004B1A88`, `0x004B209F`) and Walk
/// (`0x0075C1FB`) do. [`Simulation::set_object_height`] is the entry point for
/// an object id.
///
/// Without resolved terrain a PathGrid supplies the cell's level and slope.
/// Without either (mapless fixtures), every lookup would be the Dummy cell,
/// whose constructor ground is flat level 0. An unsupported slope leaves Z
/// unchanged.
pub(crate) fn set_height(
    position: &mut Position,
    on_bridge: bool,
    height: i32,
    terrain: Option<&ResolvedTerrainGrid>,
    path_grid: Option<&PathGrid>,
) {
    let world_xy = position_world_xy(position);
    let (level, slope) = ground_fields_at(world_xy, terrain, path_grid).unwrap_or((0, 0));
    if let Some(ground) = supported_ground(level, slope, world_xy) {
        position.exact_z_leptons = Some(z_at_height(ground, height, on_bridge));
    }
}

impl Simulation {
    /// [`set_height`] for an object id. It also keeps `LocomotorState.altitude`
    /// equal to the requested height, for the readers that still take it as
    /// the object's height (#692).
    ///
    /// RESIDUAL: native SetHeight on a marked object (`+0x74`) removes it from
    /// its cell before the write and marks it again after (vt+0x124,
    /// `0x005F5FC2..0x005F6009`). For a Foot that is `FootClass::Mark`
    /// (`0x004D3780`), which re-adds it at the head of its cell's list. VERA
    /// writes Z only.
    /// - Trigger: SetHeight on a marked owner: a paratrooper landing
    ///   (`0x005F3F7A`, just after the falling block's own Mark(PUT) at
    ///   `0x005F3F58`) and a Jumpjet's crash notice (`0x007461B9`,
    ///   `0x00522B7D`/`0x00522B8B`).
    /// - Effect: the landed paratrooper keeps its place in its cell's list.
    ///   A Jumpjet wreck is not put into the lists of its cell before its
    ///   death weapon or AirDeathFinish.
    /// - Frequency: every paratrooper landing and Jumpjet crash.
    /// - Risk: first-object reads and list walks in that cell.
    /// - Blocked by two ports of `FootClass::Mark` that disagree (#922).
    pub(crate) fn set_object_height(&mut self, id: u64, height: i32) {
        let Some(entity) = self.substrate.entities.get_mut(id) else {
            return;
        };
        set_height(
            &mut entity.position,
            entity.on_bridge,
            height,
            self.resolved_terrain.as_ref(),
            self.path_grid.as_deref(),
        );
        if let Some(locomotor) = entity.locomotor.as_mut() {
            locomotor.altitude =
                crate::util::fixed_math::SimFixed::from_num(height.clamp(-32768, 32767));
        }
    }
}
