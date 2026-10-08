//! Foot4D5690, reached through Unit7414E0 by the ordinary Cell Attack mission.
//! Native execution: tools/spatial_oracle/fv_cell_attack. The shared range,
//! Cell admission, zone-cost, target and destination owners retain their work.

use super::foot_path::{cell_centre, chebyshev, coord_cell};
use crate::map::entities::EntityCategory;
use crate::map::resolved_terrain::NativeCellQuery;
use crate::rules::locomotor_type::LocomotorKind;
use crate::rules::overlay_types::OverlayTypeRegistry;
use crate::rules::ruleset::RuleSet;
use crate::sim::cell_rect::{self, CellRect, CellRectPassabilityContext};
use crate::sim::combat::{self, TargetKind, in_range};
use crate::sim::components::{DriveCoord, NavTargetRef};
use crate::sim::mission::{MissionType, concrete_effects::represented_assign_target};
use crate::sim::world::Simulation;
use crate::util::direction_tables::{cell_delta_unchecked, facing16_between};
use crate::util::native_trig::facing_step_world_xy;
use crate::util::native_x87::distance_3d_leptons;

const OFFSETS: [i16; 25] = [
    0, 1, -1, 2, -2, 3, -3, 4, -4, 5, -5, 6, -6, 8, -8, 16, -16, 24, -24, 32, -32, 48, -48, 64, -64,
]; // original signed table8224DC

#[cfg(test)]
#[path = "foot_approach_tests.rs"]
mod tests;

impl Simulation {
    /// Ownership boundary for the absorbed common Unit override. CloseRange,
    /// BalloonHover, non-Cell queued receivers and DestroyableCliffs redirect
    /// have separate native branches; their old adapter remains explicit.
    /// A mapless fixture cannot execute native cell/zone queries.
    pub(crate) fn owns_unit_cell_approach(&self, id: u64, rules: &RuleSet) -> bool {
        let Some(actor) = self.substrate.entities.get(id) else {
            return false;
        };
        let Some(terrain) = self.resolved_terrain.as_ref() else {
            return false;
        };
        let Some(TargetKind::Cell(rx, ry)) = actor.attack_target.as_ref().map(|a| a.target) else {
            return false;
        };
        actor.category == EntityCategory::Unit
            && actor.locomotor.as_ref().is_some_and(|l| {
                matches!(l.active_kind(), LocomotorKind::Drive | LocomotorKind::Ship)
            })
            && self.zone_grid.is_some()
            && self
                .object_type(actor.type_ref(), rules)
                .is_some_and(|t| !t.close_range && !t.balloon_hover)
            && actor
                .navigation
                .nav_queue
                .iter()
                .all(|target| matches!(target, NavTargetRef::Cell { .. }))
            && !terrain.is_destroyable_cliff(rx, ry)
    }

    /// Mutable Unit common tail (741689 passes false to Foot). No RNG belongs
    /// to Approach itself; MissionAttack draws its cadence after this returns.
    pub(crate) fn approach_unit_cell_target(
        &mut self,
        id: u64,
        rules: &RuleSet,
        registry: Option<&OverlayTypeRegistry>,
    ) -> Result<Option<(u16, u16)>, String> {
        let Some(actor) = self.substrate.entities.get(id) else {
            return Ok(None);
        };
        let Some(target) = actor.attack_target.as_ref().map(|a| a.target) else {
            return Ok(None);
        };
        let Some(selected) = combat::pursuit_selection(
            actor,
            &target,
            &self.substrate.entities,
            rules,
            &self.interner,
            self.resolved_terrain.as_ref(),
            Some(&self.house_alliances),
        ) else {
            return Ok(None);
        };
        let object = self
            .object_type(actor.type_ref(), rules)
            .ok_or("Approach requires mover type")?;
        let terrain = self
            .resolved_terrain
            .as_ref()
            .ok_or("Approach requires map cells")?;
        let identity = target
            .cell_identity(terrain)
            .ok_or("ordinary approach requires Cell TarCom")?;
        let mut range = combat::combat_weapon::weapon_range(
            actor,
            object,
            selected.index,
            &self.substrate.entities,
            rules,
            &self.interner,
        );
        if range >= 512 {
            range -= 128;
        }
        let can_fire = in_range::cell_target_in_range(
            actor,
            identity,
            selected.weapon,
            rules,
            &self.interner,
            &self.substrate.entities,
            terrain,
            &combat::line_of_fire::LineOfFireInputs {
                overlay_grid: self.overlay_grid.as_ref(),
                overlay_registry: registry,
                alliances: Some(&self.fog.alliances),
            },
        )
        .ok_or("Approach CanFireAt coordinate failed")?;
        let mission = actor.mission.current().known();
        let human = self
            .houses
            .get(&actor.owner())
            .is_none_or(|h| h.is_controlled_by_human(self.session.game_mode_nonzero));
        let may_approach = object.can_approach_target
            && actor.drain_target.is_none()
            && actor.bunker_link.installed_in().is_none()
            && !actor.passenger_role.in_open_transport();
        if !can_fire
            && (mission == Some(MissionType::Sticky)
                || !may_approach
                    && !(mission == Some(MissionType::Attack)
                        || human && mission == Some(MissionType::AreaGuard)
                        || !human && mission == Some(MissionType::Hunt)))
        {
            represented_assign_target(self.substrate.entities.get_mut(id).unwrap(), None);
            self.set_unit_null_destination(id, Some(rules), None);
            return Ok(None);
        }

        // Read target+48 before NavCom+48. Neither retained Cell receiver is a
        // fresh coord-to-Cell query: a Dummy may already have been restamped.
        let cells = NativeCellQuery::canonical(terrain);
        let mut clear_nav = false;
        if let Some(nav) = actor.navigation.nav_com
            && object.can_recalc_approach_target
            && !can_fire
        {
            let own = crate::sim::cell_kernel::native_cell_own_coords(identity, &cells)
                .ok_or("Approach target coordinate failed")?;
            let nav = self.approach_nav_center(nav, &cells)?;
            clear_nav = distance_3d_leptons(
                [own.0 as i32, own.1 as i32, own.2 as i32],
                [nav.x, nav.y, nav.z],
            ) > range.wrapping_mul(rules.general.approach_target_reset_multiplier);
        }
        if clear_nav {
            //4D59FB/5A01 are precisely FootStopMoving's two direct writes.
            // The independently retained locomotor head/destination stay live.
            super::navcom::foot_stop_moving(self.substrate.entities.get_mut(id).unwrap());
        }
        let actor = self.substrate.entities.get(id).unwrap();
        let terrain = self.resolved_terrain.as_ref().unwrap();
        let cells = NativeCellQuery::canonical(terrain);
        if actor.navigation.nav_com.is_some()
            && !super::air_movement::is_high_flying_in_query(
                actor,
                &cells,
                Some((rules, &self.interner)),
            )
            || can_fire && actor.in_playfield
        {
            return Ok(None);
        }
        if let Some(NavTargetRef::Cell { rx, ry }) = actor.navigation.nav_queue.first().copied() {
            self.set_unit_destination(id, NavTargetRef::cell(rx, ry), rules, false);
            let queue = &mut self
                .substrate
                .entities
                .get_mut(id)
                .unwrap()
                .navigation
                .nav_queue;
            if !queue.is_empty() {
                queue.remove(0);
            }
            return Ok(Some((rx, ry)));
        }

        // For integral range, trunc(range - binary64 179.2), followed by
        // max(0), equals this integer expression throughout the i32 domain.
        // No host floating point is needed. Original465 numeric controls and
        // executed radius1228/972/716/460 are retained in candidates.json.gz.
        let radius = range.saturating_sub(180).max(0);
        let mut radius = if radius > 0 { radius.max(256) } else { 0 };
        let own = crate::sim::cell_kernel::native_cell_own_coords(identity, &cells)
            .ok_or("Approach target coordinate failed")?;
        let target_cell = ((own.0 / 256) as i16, (own.1 / 256) as i16);
        let source = super::ground_pose::position_world_coord(&actor.position);
        let facing = facing16_between([own.0 as i32, own.1 as i32], [source.x, source.y]);
        let facing_byte = ((u32::from(facing) + 128) >> 8) as u8;
        let target_bridge = cells.flags(identity) & 0x100 != 0;
        //4D5F6F target+50 may resample ground and flags before any candidate.
        let _ = in_range::native_cell_range_coords(identity, &cells)
            .ok_or("Approach target surface failed")?;
        let mut retained_candidate = target_cell;
        let mut chosen = None;
        'rings: while radius > 204 {
            for offset in OFFSETS {
                let direction = (i16::from(facing_byte).wrapping_add(offset) as u16) << 8;
                let [x, y] = facing_step_world_xy([own.0 as i32, own.1 as i32], direction, radius);
                let radial = ((x / 256) as i16, (y / 256) as i16);
                let mut point = cell_centre(radial);
                point.z = in_range::native_ground_target_z(point.x, point.y, &cells)
                    .ok_or("Approach candidate surface failed")? as i32;
                let in_range = |point: DriveCoord| {
                    in_range::compute_in_range_in_query(
                        actor,
                        (i64::from(point.x), i64::from(point.y), i64::from(point.z)),
                        &target,
                        selected.weapon,
                        rules,
                        &self.interner,
                        &self.substrate.entities,
                        &cells,
                        &combat::line_of_fire::LineOfFireInputs {
                            overlay_grid: self.overlay_grid.as_ref(),
                            overlay_registry: registry,
                            alliances: Some(&self.fog.alliances),
                        },
                    )
                };
                if !in_range(point) {
                    continue;
                }
                retained_candidate = radial;
                if !cell_rect::cell_is_in_playfield_height_aware_in_query(
                    (i32::from(radial.0), i32::from(radial.1)),
                    self.playfield_bounds,
                    Some(terrain),
                    Some(&cells),
                ) {
                    continue;
                }
                let mut candidate = radial;
                let mut admitted = self.approach_cell_is_clear(id, candidate, rules, &cells)?;
                if !admitted {
                    for direction in 0..8 {
                        let (dx, dy) = cell_delta_unchecked(direction);
                        candidate = (
                            candidate.0.wrapping_add(dx as i16),
                            candidate.1.wrapping_add(dy as i16),
                        );
                        //4D643D..671E: cumulative neighbors, Z0, no new bounds
                        // or ground lookup. Destination setting resolves Z later.
                        if in_range(cell_centre(candidate))
                            && self.approach_cell_is_clear(id, candidate, rules, &cells)?
                        {
                            admitted = true;
                            retained_candidate = candidate;
                            break;
                        }
                    }
                }
                if !admitted {
                    continue;
                }
                let candidate_bridge = cells.flags(cells.lookup(candidate)) & 0x100 != 0;
                if self.estimate_zone_cost(
                    id,
                    candidate,
                    target_cell,
                    candidate_bridge,
                    target_bridge,
                    rules,
                )? <= chebyshev(candidate, target_cell) + 8
                {
                    chosen = Some(candidate);
                    break 'rings;
                }
                let source_nav = self.foot_navigation_coordinate(id)?;
                let start = coord_cell(source_nav);
                let source_bridge = super::ground_pose::navigation_should_be_on_bridge(
                    &cells,
                    source_nav,
                    source,
                    actor.on_bridge,
                    actor.low_bridge_tube_state.is_some(),
                )?;
                if self.estimate_zone_cost(
                    id,
                    start,
                    candidate,
                    source_bridge,
                    candidate_bridge,
                    rules,
                )? <= chebyshev(start, candidate) + 8
                {
                    chosen = Some(candidate);
                    break 'rings;
                }
            }
            radius -= 256;
        }
        if let Some(cell) = chosen {
            self.set_unit_destination(
                id,
                NavTargetRef::cell(cell.0 as u16, cell.1 as u16),
                rules,
                true,
            );
            //4D68B8 and4D68C7 perform separate lookups for setter and return.
            self.resolved_terrain
                .as_ref()
                .unwrap()
                .native_cell_identity(cell);
            return Ok(Some((cell.0 as u16, cell.1 as u16)));
        }
        if self
            .object_type(actor.type_ref(), rules)
            .unwrap()
            .hunter_seeker
        {
            let cell = (target_cell.0 as u16, target_cell.1 as u16);
            self.set_unit_destination(id, NavTargetRef::cell(cell.0, cell.1), rules, true);
            return Ok(Some(cell));
        }
        let fallback = self.approach_fallback(id, retained_candidate, rules, &cells)?;
        represented_assign_target(self.substrate.entities.get_mut(id).unwrap(), None);
        if let Some(cell) = fallback {
            self.set_unit_destination(id, NavTargetRef::cell(cell.0, cell.1), rules, true);
        } else {
            self.set_unit_null_destination(id, Some(rules), None);
        }
        Ok(None)
    }

    fn approach_nav_center(
        &self,
        target: NavTargetRef,
        cells: &NativeCellQuery<'_>,
    ) -> Result<DriveCoord, String> {
        let id = match target {
            NavTargetRef::Cell { rx, ry } => {
                let cell = TargetKind::Cell(rx, ry)
                    .cell_identity(cells.terrain())
                    .unwrap();
                let (x, y, z) = crate::sim::cell_kernel::native_cell_own_coords(cell, cells)
                    .ok_or("NavCom Cell coordinate failed")?;
                return Ok(DriveCoord {
                    x: x as i32,
                    y: y as i32,
                    z: z as i32,
                });
            }
            NavTargetRef::Entity { id }
            | NavTargetRef::Object { id }
            | NavTargetRef::Building { id } => id,
        };
        let entity = self
            .substrate
            .entities
            .get(id)
            .ok_or("retired Approach NavCom")?;
        Ok(super::ground_pose::object_get_coords(
            entity,
            Some(cells.terrain()),
        ))
    }

    fn approach_cell_is_clear(
        &self,
        id: u64,
        candidate: (i16, i16),
        rules: &RuleSet,
        cells: &NativeCellQuery<'_>,
    ) -> Result<bool, String> {
        let actor = self
            .substrate
            .entities
            .get(id)
            .ok_or("Approach actor retired")?;
        let object = self
            .object_type(actor.type_ref(), rules)
            .ok_or("Approach mover type absent")?;
        let zones = self.zone_grid.as_ref().ok_or("Approach requires zones")?;
        let source = coord_cell(self.foot_navigation_coordinate(id)?);
        let zone = zones
            .get_zone_id_native_in_query(
                cells.terrain(),
                (source.0 as u16, source.1 as u16),
                object.movement_zone,
                actor.on_bridge,
                Some(cells),
            )
            .ok_or("Approach source zone absent")?;
        Ok(cell_rect::check_cell_passability(
            &CellRectPassabilityContext {
                native_cells: Some(cells),
                rect: CellRect {
                    x: i32::from(candidate.0),
                    y: i32::from(candidate.1),
                    width: 1,
                    height: 1,
                },
                speed_type: object.speed_type,
                required_zone_id: (zone != u32::MAX).then_some(zone),
                movement_zone: object.movement_zone,
                required_height_or_level: None,
                bridge_aware_zone: true,
                reject_any_overlay: false,
                path_grid: self.path_grid.as_deref(),
                resolved_terrain: Some(cells.terrain()),
                overlay_grid: self.overlay_grid.as_ref(),
                occupancy: Some(&self.substrate.occupancy),
                zone_grid: Some(zones),
            },
            Some(&self.substrate.raw_cell_occupation),
            i32::from(candidate.0),
            i32::from(candidate.1),
        ))
    }

    fn approach_fallback(
        &self,
        id: u64,
        retained: (i16, i16),
        rules: &RuleSet,
        cells: &NativeCellQuery<'_>,
    ) -> Result<Option<(u16, u16)>, String> {
        use crate::sim::find_nearby_cell::{
            NearbyAnchorGate, NearbyFootprint, NearbyQuery, PassabilityArgs,
            find_nearby_passable_cell, map_owned_radius_cap,
        };
        let actor = self
            .substrate
            .entities
            .get(id)
            .ok_or("Approach actor retired")?;
        let object = self
            .object_type(actor.type_ref(), rules)
            .ok_or("Approach mover type absent")?;
        let source = self.foot_navigation_coordinate(id)?;
        //4D6994 reads the retained candidate before sourceShouldBeOnBridge.
        let bridge_aware = cells.flags(cells.lookup(retained)) & 0x100 != 0;
        let should_bridge = super::ground_pose::navigation_should_be_on_bridge(
            cells,
            source,
            super::ground_pose::position_world_coord(&actor.position),
            actor.on_bridge,
            actor.low_bridge_tube_state.is_some(),
        )?;
        let source = coord_cell(source);
        let zone = self
            .zone_grid
            .as_ref()
            .ok_or("Approach requires zones")?
            .get_zone_id_native_in_query(
                cells.terrain(),
                (source.0 as u16, source.1 as u16),
                object.movement_zone,
                should_bridge,
                Some(cells),
            )
            .ok_or("Approach source zone absent")?;
        let (bounds, height) = self
            .playfield_bounds
            .zip(self.playfield_size_height)
            .ok_or("Approach requires map dimensions")?;
        Ok(find_nearby_passable_cell(
            (i32::from(retained.0), i32::from(retained.1)),
            &NearbyQuery {
                native_cells: Some(cells),
                raw_occupation: Some(&self.substrate.raw_cell_occupation),
                passability: PassabilityArgs {
                    speed_type: object.speed_type,
                    required_zone_id: (zone != u32::MAX).then_some(zone),
                    movement_zone: object.movement_zone,
                    bridge_aware_zone: bridge_aware,
                },
                footprint: NearbyFootprint::SINGLE,
                anchor_gate: NearbyAnchorGate::NativeHeightAware,
                allow_bridge_cells: false,
                check_height: true,
                check_occupancy: false,
                radius_cap: map_owned_radius_cap(bounds.base, height),
                target_cell: None,
                path_grid: self.path_grid.as_deref(),
                resolved_terrain: Some(cells.terrain()),
                overlay_grid: self.overlay_grid.as_ref(),
                occupancy: Some(&self.substrate.occupancy),
                entities: Some(&self.substrate.entities),
                zone_grid: self.zone_grid.as_ref(),
                playfield_bounds: Some(bounds),
            },
            self.session.binary_frame,
        )
        .filter(|cell| *cell != (0, 0))
        .map(|(x, y)| (x as u16, y as u16)))
    }
}
