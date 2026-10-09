//! World effects of `AircraftClass::Mission_Move @ 0x004166E0`
//! (`aircraft::move_mission`), run in the aircraft's own dispatch, and of its
//! state 0 search `AircraftClass::Find_Attack_Cell @ 0x00418E20`. Both read
//! live objects and NavCom reservations; the search and the Rate epilogue
//! draw Scenario RNG. Executable witnesses: tools/spatial_oracle/aircraft_move.*.
use super::Simulation;
use crate::rules::overlay_types::OverlayTypeRegistry;
use crate::rules::ruleset::RuleSet;
use crate::sim::aircraft::{enter_idle_mode_for, move_mission};
use crate::sim::components::NavTargetRef;
use crate::sim::mission::MissionType;
use crate::sim::movement::{ground_pose, motion_query, nav_target_coordinate};

#[cfg(test)]
#[path = "aircraft_move_tests.rs"]
mod tests;

/// `Rules+0x1478`, the leptons Find_Attack_Cell searches out to: the
/// RulesClass constructor's 0x2000 (`0x00667235`), which no INI reader
/// writes, so 32 rings.
const ATTACK_CELL_SEARCH_LEPTONS: i32 = 0x2000;

impl Simulation {
    /// One Mission_Move visit of aircraft `id` from `state`: Mission+0xBC and
    /// the frames it returns.
    pub(crate) fn aircraft_move(
        &mut self,
        id: u64,
        state: u8,
        rules: &RuleSet,
        registry: Option<&OverlayTypeRegistry>,
    ) -> (u8, i32) {
        move_mission::move_visit(
            state,
            &mut WorldMove {
                sim: self,
                id,
                rules,
                registry,
            },
        )
    }

    /// `AircraftClass::Find_Attack_Cell @ 0x00418E20`: `target` itself when
    /// it is NULL, or when its landing zone is clear (`IsLandZoneClear`,
    /// vt+0x550) and the cell of its `vt+0x4C` coordinate (no requester) is
    /// free for landing apart from this aircraft (`0x00419B00(.., 0)`).
    /// Otherwise the first cell both admit on rings 0..31 around the cell of
    /// its center (vt+0x48): each ring draws a Scenario `RandomRanged(0, 7)`
    /// start (`0x00418F4F`) and steps its eight directions (`(start + i) <<
    /// 13`, the Walk/Fly polar step), skipping cells outside the playfield
    /// and repeats of the last cell it tried. `target` again when none is.
    ///
    /// A team member first asks `0x006EC300`, which answers false at each
    /// exit before its waypoint read (`TeamScriptVm::
    /// member_step_reads_waypoint`). RESIDUAL: that read. A formed team's
    /// member on a move-to-waypoint step whose waypoint lies outside the
    /// playfield keeps `target` natively; VERA has no waypoint table in the
    /// simulation and searches. Trigger: campaign team aircraft sent past
    /// the playfield. The map editor's early return (`[0x00A8E7AC]`) has no
    /// game state.
    pub(crate) fn aircraft_find_attack_cell(
        &mut self,
        id: u64,
        target: Option<NavTargetRef>,
        rules: &RuleSet,
    ) -> Option<NavTargetRef> {
        let target = target?;
        let mut claims = None;
        if self.foot_land_zone_clear(id, target, Some(rules)) {
            let coord = nav_target_coordinate(
                target,
                None,
                &self.substrate.entities,
                self.resolved_terrain.as_ref(),
                Some((rules, &self.interner)),
            )
            .expect("live Find_Attack_Cell target");
            let cell = ((coord.x / 256) as i16, (coord.y / 256) as i16);
            if self.aircraft_cell_free_for_landing(id, cell, false, rules, &mut claims) {
                return Some(target);
            }
        }
        let center = self
            .fire_location_center(target)
            .expect("live Find_Attack_Cell target");
        let center = [
            i32::from((center.x / 256) as i16),
            i32::from((center.y / 256) as i16),
        ];
        for ring in 0..ATTACK_CELL_SEARCH_LEPTONS / 256 {
            let start = self.scenario_rng.next_range_i32_inclusive(0, 7);
            let mut previous = (0, 0);
            for step in 0..8 {
                let [x, y] = crate::util::native_trig::facing_step_world_xy(
                    center,
                    (start.wrapping_add(step) << 13) as u16,
                    ring,
                );
                let candidate = (x as i16, y as i16);
                if !crate::sim::cell_rect::cell_is_in_playfield_height_aware(
                    (i32::from(candidate.0), i32::from(candidate.1)),
                    self.playfield_bounds,
                    self.resolved_terrain.as_ref(),
                ) {
                    continue;
                }
                if candidate != previous {
                    let cell = self.resolved_terrain.as_ref().map_or(candidate, |t| {
                        t.native_cell_coord(t.native_cell_identity(candidate))
                    });
                    let cell = NavTargetRef::cell(cell.0 as u16, cell.1 as u16);
                    if self.foot_land_zone_clear(id, cell, Some(rules))
                        && self.aircraft_cell_free_for_landing(
                            id,
                            candidate,
                            false,
                            rules,
                            &mut claims,
                        )
                    {
                        return Some(cell);
                    }
                }
                previous = candidate;
            }
        }
        Some(target)
    }

    /// The cell of the NavCom's `vt+0x4C` coordinate (this aircraft asking),
    /// over 256 toward zero, as Mission_Move states 2 and 4 read it.
    fn aircraft_nav_cell(&self, id: u64, rules: &RuleSet) -> Option<(i16, i16)> {
        let nav = self.substrate.entities.get(id)?.navigation.nav_com?;
        let coord = nav_target_coordinate(
            nav,
            Some(id),
            &self.substrate.entities,
            self.resolved_terrain.as_ref(),
            Some((rules, &self.interner)),
        )
        .expect("live aircraft NavCom coordinate");
        Some(((coord.x / 256) as i16, (coord.y / 256) as i16))
    }

    /// Mission_Move's own-cell test: the NavCom's cell is the aircraft's
    /// (`vt+0x1B8`, `0x0041BEA0`: its coordinate over 256 toward zero).
    pub(crate) fn aircraft_nav_cell_is_own(&self, id: u64, rules: &RuleSet) -> bool {
        let coord =
            ground_pose::position_world_coord(&self.substrate.entities.get(id).unwrap().position);
        self.aircraft_nav_cell(id, rules) == Some(((coord.x / 256) as i16, (coord.y / 256) as i16))
    }

    /// Mission_Move's `Is_Cell_Free_For_Landing(NavCom cell, 1)`.
    pub(crate) fn aircraft_nav_cell_free(&self, id: u64, rules: &RuleSet) -> bool {
        let cell = self
            .aircraft_nav_cell(id, rules)
            .expect("Mission_Move asks with a NavCom");
        self.aircraft_cell_free_for_landing(id, cell, true, rules, &mut None)
    }
}

/// A Mission_Move visit's world.
struct WorldMove<'a> {
    sim: &'a mut Simulation,
    id: u64,
    rules: &'a RuleSet,
    registry: Option<&'a OverlayTypeRegistry>,
}

impl WorldMove<'_> {
    fn nav(&self) -> Option<NavTargetRef> {
        self.sim
            .substrate
            .entities
            .get(self.id)
            .expect("aircraft dispatch")
            .navigation
            .nav_com
    }
}

impl move_mission::MoveHost for WorldMove<'_> {
    fn nav_com(&self) -> bool {
        self.nav().is_some()
    }

    fn enter_idle_mode(&mut self) {
        enter_idle_mode_for(self.sim, self.id, self.rules, self.registry);
    }

    fn assign_attack_cell(&mut self) {
        let cell = self
            .sim
            .aircraft_find_attack_cell(self.id, self.nav(), self.rules);
        self.sim
            .assign_aircraft_destination(self.id, cell, self.rules);
    }

    fn move_to_nav_com(&mut self) {
        let coord = nav_target_coordinate(
            self.nav().expect("state 1 moves with a NavCom"),
            Some(self.id),
            &self.sim.substrate.entities,
            self.sim.resolved_terrain.as_ref(),
            Some((self.rules, &self.sim.interner)),
        )
        .expect("live aircraft NavCom coordinate");
        self.sim
            .aircraft_locomotor_move_to(self.id, coord, self.rules);
    }

    fn is_moving(&mut self) -> bool {
        motion_query::is_moving(self.sim.substrate.entities.get(self.id).unwrap()) == Some(true)
    }

    fn nav_cell_is_own(&mut self) -> bool {
        self.sim.aircraft_nav_cell_is_own(self.id, self.rules)
    }

    fn nav_cell_free(&mut self) -> bool {
        self.sim.aircraft_nav_cell_free(self.id, self.rules)
    }

    fn epilogue(&mut self) -> i32 {
        self.sim
            .mission_rate_epilogue_for(self.rules, self.id, MissionType::Move)
    }
}
