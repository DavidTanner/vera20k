//! Live source-aware Infantry51D0D0 selection. The damage receiver commits
//! the destination before fear; class entry and navigation keep their owners.
use super::bump_crush::{
    InfantryDamageScatter, infantry_damage_scatter_admitted, scatter_movement_speed,
};
use super::infantry_entry::InfantryEntryArgs;
use crate::map::overlay_types::OverlayTypeRegistry;
use crate::map::resolved_terrain::NativeCellQuery;
use crate::rules::ruleset::RuleSet;
use crate::sim::components::NavTargetRef;
use crate::sim::world::Simulation;
use crate::util::fixed_math::SimFixed;

fn represented_infantry_destination(
    actor: &crate::sim::game_entity::GameEntity,
    object: &crate::rules::object_type::ObjectType,
    requested: NavTargetRef,
) -> bool {
    use crate::rules::locomotor_type::LocomotorKind;
    actor.category == crate::map::entities::EntityCategory::Infantry
        && !object.jumpjet
        && actor
            .locomotor
            .as_ref()
            .is_some_and(|loco| match loco.kind {
                LocomotorKind::Walk => true,
                LocomotorKind::Teleport => matches!(requested, NavTargetRef::Cell { .. }),
                _ => false,
            })
}

impl Simulation {
    /// 51D52B..51D6BA: only numeric entry zero is legal. A soft obstruction
    /// is not a passable scatter destination. Evidence: original class calls
    /// in tools/spatial_oracle/infantry_scatter_entry.{py,json,meta.json}.
    ///
    /// Missing map state retains headless fixture compatibility; malformed live
    /// class state is an error, never a manufactured native refusal. NULL-source
    /// FNPC and additional class-setter branches remain separate continuations.
    pub(crate) fn select_infantry_damage_scatter(
        &mut self,
        id: u64,
        source: (i32, i32),
        rules: &RuleSet,
        registry: Option<&OverlayTypeRegistry>,
    ) -> Result<Option<InfantryDamageScatter>, String> {
        let Some(infantry) = self.substrate.entities.get(id) else {
            return Ok(None);
        };
        let human = self
            .houses
            .get(&infantry.owner())
            .is_some_and(|house| house.is_controlled_by_human(self.session.game_mode_nonzero));
        if !infantry_damage_scatter_admitted(
            infantry,
            rules,
            human,
            &self.team_script_vm,
            &self.interner,
        ) {
            return Ok(None);
        }
        self.select_infantry_scatter_away_from(id, source, rules, registry)
    }

    /// The away-from-a-coordinate arm of `InfantryClass::Scatter @
    /// 0x0051D0D0` once its gates have passed (`0x0051D258..0x0051D6BA`):
    /// the heading away from `source` rounded to an octant plus
    /// `RandomRanged(0, 4) - 2`, then the eight-neighbour search from the
    /// navigation cell. Shared by the damage receiver and the forced
    /// Scatter (`scatter_infantry_forced_from`).
    pub(crate) fn select_infantry_scatter_away_from(
        &mut self,
        id: u64,
        source: (i32, i32),
        rules: &RuleSet,
        registry: Option<&OverlayTypeRegistry>,
    ) -> Result<Option<InfantryDamageScatter>, String> {
        let Some(infantry) = self.substrate.entities.get(id) else {
            return Ok(None);
        };
        let current = super::foot_coordinate::current_coordinate(infantry);
        let start = super::scatter_cell::source_start_direction(
            (current.x, current.y),
            source,
            &mut self.scenario_rng,
        );
        let Some(terrain) = self.resolved_terrain.as_ref() else {
            return Ok(None);
        };
        let Some(bounds) = self.playfield_bounds else {
            return Ok(None);
        };
        let navigation = super::foot_coordinate::navigation_coordinate(infantry, Some(terrain))?;
        let seed = ((navigation.x / 256) as i16, (navigation.y / 256) as i16);
        let on_bridge = infantry.on_bridge;
        let speed = scatter_movement_speed(infantry, Some(rules), &self.interner);
        let destination =
            super::scatter_cell::select_neighbor(seed, start, |candidate, direction| {
                let terrain = self.resolved_terrain.as_ref().expect("retained map cells");
                let cells = NativeCellQuery::canonical(terrain);
                // Preserve the retained Cell identity across the height-aware
                // playfield lookup and Object5F5F00's current-cell lookup. A dummy
                // identity is shared and can change its coordinate between reads.
                let cell = cells.lookup(candidate);
                if !crate::sim::cell_rect::cell_is_in_playfield_height_aware(
                    (i32::from(candidate.0), i32::from(candidate.1)),
                    Some(bounds),
                    Some(terrain),
                ) {
                    return Ok(None);
                }
                let height =
                    super::ground_pose::query_object_cell_height(&cells, current, on_bridge);
                if self
                    .infantry_can_enter(
                        id,
                        cell,
                        InfantryEntryArgs {
                            direction,
                            height,
                            previous_cell: None,
                        },
                        rules,
                        registry,
                    )?
                    .is_nonzero()
                {
                    return Ok(None);
                }
                Ok::<_, String>(Some(super::scatter_cell::preferred_surface(
                    self.resolved_terrain.as_ref().expect("retained map cells"),
                    candidate,
                )))
            })?;
        Ok(destination.map(|destination| InfantryDamageScatter {
            destination: (destination.0 as u16, destination.1 as u16),
            speed,
        }))
    }
}

impl Simulation {
    /// Whether the non-null Infantry51AA40 destination owner represents this
    /// receiver and target. This checks port coverage, not native admission.
    pub(crate) fn infantry_setter_receiver(
        &self,
        id: u64,
        requested: NavTargetRef,
        rules: &RuleSet,
    ) -> bool {
        self.substrate.entities.get(id).is_some_and(|actor| {
            self.object_type(actor.type_ref(), rules)
                .is_some_and(|object| represented_infantry_destination(actor, object, requested))
        })
    }

    /// Pure dependency availability for the represented 51AA40 owner below.
    /// This neither evaluates4834A0 admission nor performs its Dummy lookup;
    /// the caller can validate a transaction before the actual class call.
    pub(crate) fn infantry_destination_inputs_available(
        &self,
        id: u64,
        requested: NavTargetRef,
        rules: &RuleSet,
        registry: Option<&OverlayTypeRegistry>,
    ) -> bool {
        use crate::map::cell_index::NativeCellIdentity;
        use crate::rules::locomotor_type::{LocomotorKind, SpeedType};

        let Some(actor) = self.substrate.entities.get(id) else {
            return false;
        };
        let Some(object) = self.object_type(actor.type_ref(), rules) else {
            return false;
        };
        if !represented_infantry_destination(actor, object, requested) || actor.infantry.is_none() {
            return false;
        }
        let Some(leaf) = actor.mission_leaf.as_infantry() else {
            return false;
        };
        let human = self
            .houses
            .get(&actor.owner())
            .is_some_and(|house| house.is_controlled_by_human(self.session.game_mode_nonzero));
        if human && (27..=30).contains(&leaf.doing()) {
            return true;
        }
        // Non-cell +4C projections read retained state only. The shared Foot
        // owner validates active Tube/Jumpjet payloads; Building's owner
        // validates its type and, for a Bunker, the requester. Cell+4C cannot
        // return an error and is left to the actual call's lookup order.
        if !matches!(requested, NavTargetRef::Cell { .. })
            && super::navcom::nav_target_coordinate(
                requested,
                Some(id),
                &self.substrate.entities,
                self.resolved_terrain.as_ref(),
                Some((rules, &self.interner)),
            )
            .is_err()
        {
            return false;
        }
        let loco = actor.locomotor.as_ref().expect("represented receiver");
        if loco.active_kind() != LocomotorKind::Walk {
            return true; // The retained Teleport arm is restricted to Cell.
        }
        let Some(moving) = loco.walk_is_moving() else {
            return false;
        };
        if !moving {
            return true;
        }
        let Some(terrain) = self.resolved_terrain.as_ref() else {
            return false;
        };
        // 51ABA2 resolves the current Cell even for Winged. Availability is
        // established without issuing native_cell_identity (which stamps
        // the shared Dummy) or reading occupation/admission.
        if object.speed_type == SpeedType::Winged {
            return true;
        }
        let xyz = super::foot_coordinate::current_coordinate(actor);
        let (cell, overlay, cost) =
            match terrain.native_fixed_cell_index((xyz.x / 256) as i16, (xyz.y / 256) as i16) {
                Some(index) => {
                    let cell = &terrain.cells()[index];
                    (
                        NativeCellIdentity::Real(index),
                        cell.bridge_facts.overlay_id,
                        cell.speed_costs.cost_for_speed_type(object.speed_type),
                    )
                }
                None => (
                    NativeCellIdentity::Dummy,
                    u8::try_from(terrain.shared_cell_dummy().overlay_identity_state().0).ok(),
                    None,
                ),
            };
        let wall = match overlay {
            Some(overlay) => {
                let Some(flags) = registry.and_then(|registry| registry.flags(overlay)) else {
                    return false;
                };
                flags.wall
            }
            None => false,
        };
        terrain.native_cell_flags(cell) & 0x100 != 0 || wall || cost.is_some()
    }

    /// The Infantry class setter `vt+0x480(target, 1)` (`0x0051AA40`) for a
    /// Walk or Teleport infantryman: Infantry51AA40 -> Foot4D94B0 -> the
    /// active locomotor's Move_To (Walk75ACB0 or Teleport `0x00718100`; the
    /// setter never reads `Teleporter=`). Reached from Scatter (`0x0051D6E0`),
    /// the slave manager's sends, team scripts and a Teleport infantryman's
    /// Move and AttackMove orders. The human same-reference prone DoAction7
    /// arm (`0x0051ABD7..0x0051AC1F`) uses the existing action owner.
    /// Non-cell targets retain their reference and read the receiver's +4C
    /// navigation coordinate, including a Foot's committed head. No Process
    /// runs until the ordinary object turn.
    /// Native comparisons: infantry_scatter_destination.{py,json,meta.json}
    /// and anytown_damage/foot_missions.{py,json,meta.json}.
    ///
    /// Returns false for the still-unmigrated Jumpjet and other class setters.
    /// DirectRocker reciprocal links, lifted-unit release, retained fire particles
    /// and the Unit-produced +6AC latch still lack their production owners here.
    pub(crate) fn assign_infantry_walk_destination(
        &mut self,
        id: u64,
        requested: NavTargetRef,
        speed: SimFixed,
        rules: &RuleSet,
        registry: Option<&OverlayTypeRegistry>,
    ) -> Result<bool, String> {
        use crate::rules::locomotor_type::LocomotorKind;
        let actor = self
            .substrate
            .entities
            .get(id)
            .ok_or("scatter destination lost actor")?;
        let object = self
            .object_type(actor.type_ref(), rules)
            .ok_or("scatter destination requires type")?;
        let teleport = actor
            .locomotor
            .as_ref()
            .is_some_and(|l| l.kind == LocomotorKind::Teleport);
        if !represented_infantry_destination(actor, object, requested) {
            return Ok(false);
        }
        // The retained Teleport Move_To port still takes a Cell. Ordinary
        // Walk executes non-cell +4C below; do not snap an unported Teleport
        // object destination to that object's physical cell.
        let teleport_cell = match requested {
            NavTargetRef::Cell { rx, ry } => Some((rx, ry)),
            _ => None,
        };
        let human = self
            .houses
            .get(&actor.owner())
            .is_some_and(|h| h.is_controlled_by_human(self.session.game_mode_nonzero));
        if human
            && actor
                .mission_leaf
                .as_infantry()
                .is_some_and(|l| (27..=30).contains(&l.doing()))
        {
            return Ok(true);
        }
        let speed_type = object.speed_type;
        let type_allows_up = !object.fraidycat && !object.cyborg;
        let moving = actor.locomotor.as_ref().and_then(|l| l.walk_is_moving()) == Some(true);
        // 51ABA2 invokes the shared Cell leaf BEFORE the Attack/same-NavCom
        // exception. The current physical coordinate owns this lookup.
        if moving && self.infantry_destination_current_cell_clear(id, speed_type, registry)? {
            let actor = self.substrate.entities.get(id).unwrap();
            if actor.mission.current().raw() != 1
                || !super::navcom::nav_targets_same_receiver(actor.navigation.nav_com, requested)
            {
                self.infantry_stop_driver(id, rules, registry)?;
            }
        }
        // 51ABD7..51AC1F: the human same-reference prone request is an
        // unforced DoAction(Up,0,0), before the class path-head write. Its
        // sequence/action admission remains with the sole Do_Action owner.
        let actor = self
            .substrate
            .entities
            .get(id)
            .ok_or("destination lost infantry after Stop_Driver")?;
        if human
            && type_allows_up
            && super::navcom::nav_targets_same_receiver(actor.navigation.nav_com, requested)
            && actor.infantry.as_ref().is_some_and(|state| state.is_prone)
        {
            self.infantry_do_action(id, 7, false, rules)?;
        }
        super::movement_commands::clear_destination_path_head(
            self.substrate.entities.get_mut(id).unwrap(),
        );
        if !self.begin_foot_destination(id, true) {
            return Ok(true);
        }
        if teleport {
            // Foot tail: NavCom, then Teleport Move_To, then the accepted
            // timers whatever it answered (`0x004D96C2..0x004D9707`).
            let frame = self.session.binary_frame;
            let actor = self.substrate.entities.get_mut(id).unwrap();
            super::navcom::publish_nav_com(actor, requested);
            let accepted = super::teleport_movement::teleport_move_to(
                actor,
                teleport_cell.expect("represented Teleport target is a Cell"),
                &rules.general,
                false,
                frame,
            );
            super::DestinationTiming::from_rules(frame, Some(rules)).accept(actor);
            return Ok(accepted);
        }
        let coord = super::navcom::nav_target_coordinate(
            requested,
            Some(id),
            &self.substrate.entities,
            self.resolved_terrain.as_ref(),
            Some((rules, &self.interner)),
        )?;
        super::prepare_walk_destination(
            &mut self.substrate.entities,
            id,
            (requested, coord),
            speed,
            self.resolved_terrain.as_ref(),
            super::DestinationTiming::from_rules(self.session.binary_frame, Some(rules)),
        );
        Ok(true)
    }

    /// [`Self::assign_infantry_walk_destination`] at the mover's own
    /// movement speed (`GetCurrentSpeed`, as an ordinary Move order).
    pub(crate) fn set_infantry_destination(
        &mut self,
        id: u64,
        requested: NavTargetRef,
        rules: &RuleSet,
        registry: Option<&OverlayTypeRegistry>,
    ) -> Result<bool, String> {
        let speed = {
            let infantry = self
                .substrate
                .entities
                .get(id)
                .ok_or("destination lost infantry actor")?;
            scatter_movement_speed(infantry, Some(rules), &self.interner)
        };
        self.assign_infantry_walk_destination(id, requested, speed, rules, registry)
    }

    /// 51AB73..51ABA7 passes (SpeedType,1,0,-1,Normal,-1,true) to4834A0.
    /// This is not Infantry CanEnter: ignore the five infantry bits, retain
    /// vehicles, choose the deck on a structural bridge, and reject all walls.
    fn infantry_destination_current_cell_clear(
        &self,
        id: u64,
        speed: crate::rules::locomotor_type::SpeedType,
        registry: Option<&OverlayTypeRegistry>,
    ) -> Result<bool, String> {
        use super::locomotor::MovementLayer;
        use crate::map::cell_index::NativeCellIdentity;
        use crate::rules::locomotor_type::{MovementZone, SpeedType};
        use crate::sim::cell_rect::{
            IsClearToMoveRequest, IsClearToMoveResult, evaluate_is_clear_to_move,
        };
        use crate::sim::occupancy::RawCellKey;
        let actor = self
            .substrate
            .entities
            .get(id)
            .ok_or("scatter lost current cell owner")?;
        let coord = super::foot_coordinate::current_coordinate(actor);
        let terrain = self
            .resolved_terrain
            .as_ref()
            .ok_or("scatter setter requires map cells")?;
        let cell = terrain.native_cell_identity(((coord.x / 256) as i16, (coord.y / 256) as i16));
        if speed == SpeedType::Winged {
            return Ok(true);
        }
        let raw = &self.substrate.raw_cell_occupation;
        let key = RawCellKey::from_native(terrain, cell);
        let (level, overlay, cost) = match cell {
            NativeCellIdentity::Real(index) => {
                let c = &terrain.cells()[index];
                (
                    i16::from(c.level as i8),
                    c.bridge_facts.overlay_id,
                    c.speed_costs.cost_for_speed_type(speed),
                )
            }
            NativeCellIdentity::Dummy => {
                let dummy = terrain.shared_cell_dummy();
                (
                    i16::from(dummy.snapshot().level),
                    u8::try_from(dummy.overlay_identity_state().0).ok(),
                    None,
                )
            }
        };
        let mut input = IsClearToMoveRequest {
            speed_type: speed,
            movement_zone: MovementZone::Normal,
            requested_zone: None,
            actual_zone: 0,
            base_level: level,
            has_bridge: terrain.native_cell_flags(cell) & 0x100 != 0,
            requested_level: None,
            is_bridge: true,
            ground_occupation_bits: raw.bits_at(key, MovementLayer::Ground),
            deck_occupation_bits: raw.bits_at(key, MovementLayer::Bridge),
            ignore_infantry: true,
            ignore_vehicles: false,
            land_passable: true,
            is_wall_overlay: false,
            wall_allows_crusher: false,
        };
        if !matches!(
            evaluate_is_clear_to_move(input),
            IsClearToMoveResult::Clear { .. }
        ) {
            return Ok(false);
        }
        if let Some(overlay) = overlay {
            input.is_wall_overlay = registry
                .and_then(|r| r.flags(overlay))
                .ok_or("scatter setter overlay lacks registered type")?
                .wall;
        }
        // Bridge land rows cannot refuse; walls still can. Do not invent a
        // Clear land speed for a dummy whose native row is not represented.
        if !input.has_bridge && !input.is_wall_overlay {
            input.land_passable =
                cost.ok_or("scatter setter requires current cell speed row")? != 0;
        }
        Ok(matches!(
            evaluate_is_clear_to_move(input),
            IsClearToMoveResult::Clear { .. }
        ))
    }
}
