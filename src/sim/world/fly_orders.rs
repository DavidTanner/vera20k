//! Fly MoveTo4CCC80 and BeginTakeoff4CF950 share the world-owned spatial and
//! sound transaction. Foot destination timing is an optional enclosing caller;
//! locomotor retries must not reset those timers. Stop_Moving4CCFD0 re-targets
//! a moving aircraft through its own class setter.
use super::Simulation;
use crate::rules::ruleset::RuleSet;
use crate::sim::components::{DriveCoord, NavTargetRef};
use crate::sim::movement::{DestinationTiming, air_movement, ground_pose};

impl Simulation {
    /// `AircraftClass::Assign_Destination @ 0x0041AA80` (vt+0x480) ->
    /// Foot4D94B0: a Move order's, a spawn manager's, Mission_Attack's and
    /// Mission_Move's destination for an aircraft.
    /// NULL uses the shared Foot gate (`0x004D9672`,
    /// [`Simulation::foot_null_destination`]), retaining the Fly request
    /// while the aircraft's current or queued dispatch is Attack with a TarCom.
    /// This does not
    /// replace the general destination setter: queued Enter preprocessing,
    /// linked-lift detach (+2AC/+2B0), retained fire-particle cleanup (+304), and
    /// the Unit-produced +6AC latch still need their native owner migrations.
    pub(crate) fn assign_aircraft_destination(
        &mut self,
        id: u64,
        requested: Option<NavTargetRef>,
        rules: &RuleSet,
    ) {
        let target_id = |target| match target {
            NavTargetRef::Cell { .. } => None,
            NavTargetRef::Entity { id }
            | NavTargetRef::Building { id }
            | NavTargetRef::Object { id } => Some(id),
        };
        //41AA8C: Target+54 (IsHighFlying). Cells implement the false stub.
        // High targets take the NULL Foot entry immediately, before departure
        // power/radio work.
        let high = requested.and_then(target_id).is_some_and(|target| {
            self.substrate.entities.get(target).is_some_and(|e| {
                air_movement::is_high_flying(
                    e,
                    self.resolved_terrain.as_ref(),
                    Some((rules, &self.interner)),
                )
            })
        });
        let requested = if high { None } else { requested };
        if requested.is_some() {
            let entity = self
                .substrate
                .entities
                .get(id)
                .expect("aircraft destination owner");
            let coord = ground_pose::position_world_coord(&entity.position);
            let bridge = self.resolved_terrain.as_ref().is_some_and(|terrain| {
                let cell =
                    terrain.native_cell_identity(((coord.x / 256) as i16, (coord.y / 256) as i16));
                terrain.native_cell_flags(cell) & 0x100 != 0
            });
            let pad = (!bridge)
                .then(|| self.fly_building_at(coord))
                .flatten()
                .filter(|&building| {
                    self.substrate
                        .entities
                        .get(building)
                        .and_then(|e| rules.object(self.interner.resolve(e.type_ref())))
                        .is_some_and(|o| o.unit_repair || o.unit_reload)
                });
            if let Some(pad) = pad {
                //41AD39..41AD80;53A130 is the native false stub.
                let entity = self.substrate.entities.get_mut(id).unwrap();
                entity
                    .locomotor
                    .as_mut()
                    .expect("aircraft destination owner has a locomotor")
                    .power_on();
                let detach = entity.radio_contacts.slot(0) == Some(pad)
                    && requested.and_then(target_id) != Some(pad);
                if detach {
                    crate::sim::radio::broadcast_break(self, id, Some(rules));
                }
            }
        }
        if requested.is_none() {
            self.foot_null_destination(id, Some(rules), None);
            return;
        }
        if !self.begin_foot_destination(id, true) {
            return;
        }
        let entity = self.substrate.entities.get_mut(id).unwrap();
        crate::sim::mission::concrete_effects::represented_assign_destination_mode_one(
            entity, requested,
        );
        if let Some(destination) = requested {
            let coord = crate::sim::movement::nav_target_coordinate(
                destination,
                Some(id),
                &self.substrate.entities,
                self.resolved_terrain.as_ref(),
                Some((rules, &self.interner)),
            )
            .expect("live aircraft NavCom coordinate");
            self.aircraft_locomotor_move_to(id, coord, rules);
        }
        // Accepted Foot setter resets both timers even when Fly MoveTo refuses
        // (e.g. powered off). Retry count is preserved.
        DestinationTiming::from_rules(self.session.binary_frame, Some(rules))
            .accept(self.substrate.entities.get_mut(id).unwrap());
    }

    /// An aircraft's `ILocomotion::Move_To` (`+0x44`), as Foot4D94B0 and
    /// Mission_Move state 1 call it: a spawned missile's Rocket
    /// (`0x006632E0`), every other aircraft's Fly (`0x004CCC80`).
    pub(crate) fn aircraft_locomotor_move_to(
        &mut self,
        id: u64,
        coord: DriveCoord,
        rules: &RuleSet,
    ) {
        let entity = self.substrate.entities.get(id).unwrap();
        if entity
            .locomotor
            .as_ref()
            .is_some_and(|locomotor| locomotor.rocket_runtime().is_some())
        {
            self.rocket_move_to(id, coord, rules);
        } else {
            self.move_air_coordinate(id, coord, None, Some(rules));
        }
    }

    /// `MapClass::PickCellOnEdge @ 0x004AA440` as the aircraft missions call
    /// it, with the empty cell `0x00889E68` as both references and criterion
    /// 4 ([`edge_cell::find_paradrop_edge_cell`](super::edge_cell::find_paradrop_edge_cell)):
    /// Mission_Attack's state 10, Mission_Retreat and the Spy Plane missions.
    /// `None` with no playfield (headless fixtures), where there is no edge
    /// to pick and no draw.
    pub(crate) fn aircraft_edge_cell(
        &mut self,
        edge: super::edge_cell::Edge,
    ) -> Option<(u16, u16)> {
        super::edge_cell::find_paradrop_edge_cell(
            self.playfield_bounds,
            self.resolved_terrain.as_ref(),
            edge,
            &mut self.scenario_rng,
        )
    }

    /// The waypoint edge (`HouseClass+0x577C`) of aircraft `id`'s house.
    pub(crate) fn aircraft_house_waypoint_edge(&self, id: u64) -> u8 {
        self.substrate
            .entities
            .get(id)
            .and_then(|entity| self.houses.get(&entity.owner()))
            .map_or(0, |house| house.waypoint_edge)
    }

    /// Fly `Move_To @ 0x004CCC80` with a non-null coordinate. Answers whether
    /// it took the destination (native MoveTo is void): its refusals are a
    /// landing toward the same cell, the empty coordinate and an unpowered or
    /// warping owner. `timing` is an enclosing Foot setter's tail, run on
    /// acceptance.
    pub(crate) fn move_air_coordinate(
        &mut self,
        id: u64,
        request: DriveCoord,
        timing: Option<DestinationTiming>,
        rules: Option<&RuleSet>,
    ) -> bool {
        let Some(entity) = self.substrate.entities.get(id) else {
            return false;
        };
        if entity
            .locomotor
            .as_ref()
            .and_then(|l| l.fly_runtime())
            .is_some_and(|state| state.ignores_destination(request))
            || request == (DriveCoord { x: 0, y: 0, z: 0 })
            || !air_movement::fly_coordinate_admitted(entity)
        {
            return false;
        }
        let flight_level = rules.map_or(500, |rules| {
            rules
                .object(self.interner.resolve(entity.type_ref()))
                .map_or(rules.general.flight_level, |o| {
                    o.flight_level(rules.general.flight_level)
                })
        });
        let armed_flight_level = (entity.attack_target.is_some()
            && entity
                .aircraft_ammo
                .as_ref()
                .is_some_and(|a| a.current != 0))
        .then_some(flight_level);
        //4CCE1C..4CCE6E stores destination before landing-base/height reads.
        if let Some(state) = self
            .substrate
            .entities
            .get_mut(id)
            .and_then(|e| e.locomotor.as_mut())
            .and_then(|l| l.fly_runtime_mut())
        {
            state.retain_destination(request, armed_flight_level, || {
                ground_pose::ground_surface_z_at(
                    [request.x, request.y],
                    false,
                    self.resolved_terrain.as_ref(),
                    None,
                )
                .unwrap_or(0)
            });
        }
        let entity = self.substrate.entities.get(id).unwrap();
        let begin_takeoff = entity
            .locomotor
            .as_ref()
            .and_then(|l| l.fly_runtime())
            .is_some_and(|state| {
                let base = crate::sim::aircraft::landing_base::landing_base(
                    entity,
                    &self.substrate.entities,
                    rules.map(|r| (r, &self.interner)),
                );
                state.should_begin_takeoff(
                    entity.health.current,
                    || air_movement::current_fly_height(entity, self.resolved_terrain.as_ref()),
                    base,
                )
            });
        if begin_takeoff {
            self.begin_fly_takeoff(id, rules);
        }
        //4CCED9 follows ALL BeginTakeoff effects. Its stored-destination ground
        // query must be last, including when both lookups stamp the Cell Dummy.
        let entity = self.substrate.entities.get_mut(id).unwrap();
        let aircraft = entity.category == crate::map::entities::EntityCategory::Aircraft;
        let ready = aircraft
            && entity
                .mission_leaf
                .as_aircraft()
                .is_some_and(|l| l.action_latch() != 0);
        let non_landable = aircraft
            && rules
                .and_then(|r| r.object(self.interner.resolve(entity.type_ref())))
                .is_some_and(|o| !o.landable);
        if let Some(state) = entity.locomotor.as_mut().and_then(|l| l.fly_runtime_mut()) {
            let destination = state.destination();
            let ground = ground_pose::ground_surface_z_at(
                [destination.x, destination.y],
                false,
                self.resolved_terrain.as_ref(),
                None,
            )
            .unwrap_or(0);
            state.select_destination_mode(
                ground,
                armed_flight_level.is_some(),
                ready,
                non_landable,
            );
        }
        if let Some(timing) = timing {
            timing.accept(entity);
        }
        true
    }

    ///4CF950: owner refusal gates precede flags, AirTracker admission, live type
    /// FlightLevel, ground-facing snap and AuxSound1. Power is a MoveTo gate,
    /// not a second BeginTakeoff gate.
    pub(crate) fn begin_fly_takeoff(&mut self, id: u64, rules: Option<&RuleSet>) -> bool {
        let Some(entity) = self.substrate.entities.get(id) else {
            return false;
        };
        if crate::sim::movement::locomotor_owner::owner_is_warping(entity)
            || entity
                .locomotor
                .as_ref()
                .and_then(|l| l.fly_runtime())
                .is_none()
        {
            return false;
        }
        let level = rules.map_or(500, |rules| {
            rules
                .object(self.interner.resolve(entity.type_ref()))
                .map_or(rules.general.flight_level, |o| {
                    o.flight_level(rules.general.flight_level)
                })
        });
        self.substrate
            .entities
            .get_mut(id)
            .unwrap()
            .locomotor
            .as_mut()
            .unwrap()
            .begin_fly_takeoff(level);
        self.finish_fly_takeoff_entry(id, rules);
        true
    }

    /// `FlyLocomotionClass::Begin_Landing @ 0x004CFA70`. Its first gate
    /// (Mission != Enter and `0x006385C0`) only refuses while Techno `+0x514`
    /// holds a planning path; VERA has no planning mode, so it always passes.
    /// A live-type AirportBound Aircraft must be in radio contact with the
    /// building in its current cell (`+0x1BC`); otherwise native calls
    /// Enter_Idle_Mode(0, 1) (vt+0x484, `0x004176F0`) and does not land.
    /// Callers: Horizontal_Step's arrival (`0x004CF520`), Process's landing
    /// trigger (`0x004CE43C`), null MoveTo (`0x004CCDDB`).
    ///
    /// RESIDUAL: the refusal sets VERA's idle state, whose tree stands in for
    /// Enter_Idle_Mode's airborne arm (`aircraft::idle_entry`'s RESIDUAL).
    /// Trigger: an AirportBound aircraft arriving over a cell without a dock
    /// it is in contact with. Effect: VERA's tree picks its next move.
    pub(crate) fn begin_fly_landing(&mut self, id: u64, rules: Option<&RuleSet>) -> bool {
        let Some(entity) = self.substrate.entities.get(id) else {
            return false;
        };
        if entity
            .locomotor
            .as_ref()
            .and_then(|l| l.fly_runtime())
            .is_none()
        {
            return false;
        }
        let airport_bound = entity.category == crate::map::entities::EntityCategory::Aircraft
            && rules
                .and_then(|r| r.object(self.interner.resolve(entity.type_ref())))
                .is_some_and(|o| o.airport_bound);
        if airport_bound {
            let cell = ground_pose::position_world_coord(&entity.position);
            let contact = self
                .fly_building_at(cell)
                .is_some_and(|building| entity.radio_contacts.contains(building));
            if !contact {
                let entity = self.substrate.entities.get_mut(id).unwrap();
                if entity.aircraft_mission.is_some() {
                    entity.aircraft_mission = Some(crate::sim::aircraft::AircraftMission::Idle);
                }
                return false;
            }
        }
        self.substrate
            .entities
            .get_mut(id)
            .unwrap()
            .locomotor
            .as_mut()
            .unwrap()
            .begin_fly_landing();
        true
    }

    /// `FlyLocomotionClass::Stop_Moving @ 0x004CCFD0` (ILocomotion `+0x48`),
    /// [`Self::locomotor_stop_moving`]'s Fly arm: [`Self::fly_stop_order`],
    /// then its effect. A self-destruct is `ReceiveDamage` of the owner's own
    /// Health with `C4Warhead=` ([`Self::receive_own_health_c4`]), after which
    /// the Fly loses its destination (`0x004CD273`); a destination goes
    /// through the owner's class setter (vt+0x480,
    /// [`Self::assign_aircraft_destination`]). Answers false for a moving
    /// Aircraft without rules.
    pub(crate) fn fly_stop_moving(
        &mut self,
        id: u64,
        rules: Option<&RuleSet>,
        registry: Option<&crate::map::overlay_types::OverlayTypeRegistry>,
    ) -> bool {
        let Some(rules) = rules else {
            return !self.substrate.entities.get(id).is_some_and(|entity| {
                entity.category == crate::map::entities::EntityCategory::Aircraft
                    && crate::sim::movement::motion_query::is_moving(entity).unwrap_or(false)
            });
        };
        match self.fly_stop_order(id, rules) {
            None => {}
            Some(FlyStopOrder::SelfDestruct) => {
                self.receive_own_health_c4(id, rules, registry);
                if let Some(state) = self
                    .substrate
                    .entities
                    .get_mut(id)
                    .and_then(|entity| entity.locomotor.as_mut())
                    .and_then(|locomotor| locomotor.fly_runtime_mut())
                {
                    state.clear_destination();
                }
            }
            Some(FlyStopOrder::Destination(target)) => {
                self.assign_aircraft_destination(id, target, rules);
            }
        }
        true
    }

    /// What Fly's `Stop_Moving` (`0x004CCFD0`) does to Aircraft `id`. Nothing
    /// while the Fly is not moving (Is_Moving, `0x004CCA90`). Otherwise it
    /// takes the cell of the owner's Location (over 256 toward zero); an owner
    /// that is no loaner (`+0x3D4`), lies outside the playfield
    /// (`MapClass::IsCellInPlayfield @ 0x00578460`, mode 1) and may not leave
    /// the map (vt+0x4DC, [`Self::aircraft_may_leave_map`]) takes the LocalSize
    /// edge cell instead
    /// ([`playfield_edge_cell`](crate::map::playfield::playfield_edge_cell),
    /// inset 1). Then:
    /// - cell (0, 0) is a self-destruct;
    /// - with Attack its current mission (vt+0x184), the destination is the
    ///   cell [`Self::aircraft_find_nearest_friendly_airfield`] answers;
    /// - otherwise it is the cell Find_Attack_Cell picks from that cell
    ///   ([`Self::aircraft_find_attack_cell`]), which draws on the Scenario
    ///   RNG when the cell will not do.
    ///
    /// Both cells come through `MapClass::GetCellAt`. Native execution:
    /// `tools/spatial_oracle/fly_stop`, replayed in
    /// `fly_process_tests::fly_stop_matches_native_rows`.
    ///
    /// RESIDUAL: a cell beyond the MapClass cell array is the shared dummy
    /// cell, which every later miss restamps. Native's Find_Attack_Cell then
    /// centres its rings on the dummy's latest coordinate and answers the
    /// dummy itself, read again by the setter; VERA holds the cell it asked
    /// for (`aircraft::leave_map`'s RESIDUAL). Trigger: Stop on an aircraft
    /// whose cell, or edge cell, lies beyond the array; only an aircraft the
    /// map edge lets past the Size diamond (vt+0x4DC false, or FlyBy) gets
    /// there. Effect: another destination and Scenario draw count.
    ///
    /// RESIDUAL: the arm for an owner that is not an Aircraft
    /// (`0x004CD132..0x004CD28F`): a 1x1 Track/Fly passable cell, given as its
    /// ground coordinate through the owner's vt+0x480 twice unless the first
    /// makes it land, else the C4 self-destruct of a live owner. No retail
    /// Unit or Infantry type has the Fly locomotor
    /// (`fly_process_tests::retail_fly_owners_are_aircraft`); VERA leaves such
    /// an owner flying as it was.
    pub(crate) fn fly_stop_order(&mut self, id: u64, rules: &RuleSet) -> Option<FlyStopOrder> {
        use crate::sim::mission::{MissionId, MissionType};
        let entity = self.substrate.entities.get(id)?;
        if entity.category != crate::map::entities::EntityCategory::Aircraft
            || !crate::sim::movement::motion_query::is_moving(entity).unwrap_or(false)
        {
            return None;
        }
        let location = ground_pose::position_world_coord(&entity.position);
        let mut cell = ((location.x / 256) as i16, (location.y / 256) as i16);
        if let Some(bounds) = self.playfield_bounds
            && !entity.is_mission_only()
            && !crate::sim::cell_rect::cell_is_in_playfield_height_aware(
                (i32::from(cell.0), i32::from(cell.1)),
                Some(bounds),
                self.resolved_terrain.as_ref(),
            )
            && !self.aircraft_may_leave_map(id)
        {
            cell = crate::map::playfield::playfield_edge_cell(bounds, cell, true);
        }
        if cell == (0, 0) {
            return Some(FlyStopOrder::SelfDestruct);
        }
        // AircraftMission owns the current aircraft dispatch while it exists
        // (`foot_null_destination`'s attack gate reads it the same way).
        let entity = self.substrate.entities.get(id)?;
        let attack = MissionId::from_known(MissionType::Attack);
        let attacking = entity.aircraft_mission.as_ref().map_or_else(
            || entity.mission.effective() == attack,
            crate::sim::aircraft::AircraftMission::is_attacking,
        );
        let cell_at = |sim: &Self, cell: (i16, i16)| {
            let (x, y) = sim
                .resolved_terrain
                .as_ref()
                .map_or(cell, |t| t.native_cell_coord(t.native_cell_identity(cell)));
            NavTargetRef::cell(x as u16, y as u16)
        };
        Some(FlyStopOrder::Destination(if attacking {
            let airfield = self.aircraft_find_nearest_friendly_airfield(id, rules);
            Some(cell_at(self, airfield))
        } else {
            let under = cell_at(self, cell);
            self.aircraft_find_attack_cell(id, Some(under), rules)
        }))
    }
}

/// What Fly's `Stop_Moving` does to a moving Aircraft
/// ([`Simulation::fly_stop_order`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FlyStopOrder {
    /// `ReceiveDamage` with `C4Warhead=` (`0x004CD0CD`).
    SelfDestruct,
    /// The owner's class setter (vt+0x480) with this target (`0x004CD0F7`,
    /// `0x004CD124`).
    Destination(Option<NavTargetRef>),
}

#[cfg(test)]
mod tests {
    use crate::rules::{
        ini_parser::IniFile,
        object_type::{ObjectCategory, ObjectType},
    };
    use crate::sim::movement::locomotor::LocomotorState;

    #[test]
    fn fly_link_retains_native_aircraft_only_airport_binding() {
        let rows: Vec<serde_json::Value> = serde_json::from_str(crate::test_fixture::text(
            "tools/spatial_oracle/fly_instance_link.json",
        ))
        .unwrap();
        assert_eq!(rows.len(), 4);
        for row in rows {
            let ini = IniFile::from_str(&format!(
                "[TEST]\nAirportBound={}\nLocomotor={{4A582746-9839-11D1-B709-00A024DDAFD1}}\n",
                row["type_airport_bound"].as_bool().unwrap()
            ));
            let category = if row["aircraft"].as_bool().unwrap() {
                ObjectCategory::Aircraft
            } else {
                ObjectCategory::Vehicle
            };
            let mut object =
                ObjectType::from_ini_section("TEST", ini.section("TEST").unwrap(), category);
            let loco = LocomotorState::from_object_type(&object, 0);
            assert_eq!(
                loco.fly_runtime().unwrap().airport_bound(),
                row["linked"].as_bool().unwrap(),
                "{row}"
            );
            object.airport_bound = !object.airport_bound;
            assert_eq!(
                loco.fly_runtime().unwrap().airport_bound(),
                row["after_type_change"].as_bool().unwrap(),
                "{row}"
            );
        }
    }
}
