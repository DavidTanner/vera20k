//! Action4/7 Team creation, with the existing VM, Techno constructor, class
//! Unlimbo, admission and destructor owners. Original YR1.001 SHA256
//! 1cdd1180e49024fbda8ad568caac2e86e856063ff67ab38f62b7d2c7bb84298c:
//! CreateTeam6F09C0, SpawnReinforcements65D8E0, CreateMembers65DD30 and
//! PlaceMembers65E010. Executable controls: tools/spatial_oracle/team_creation.
//!
//! Bounded normal TaskForce chain: Unit, Infantry and Aircraft without Full
//! passenger loading or Droppod transport. Those are separate transport
//! mechanisms; reaching either refuses explicitly before construction. Team
//! retention of an off-map Dummy Cell identity remains a TeamTarget residual.

use super::{TeamScriptAction, TeamTarget, TeamTypeHouse};
use crate::map::entities::EntityCategory;
use crate::rules::locomotor_type::MovementZone;
use crate::rules::overlay_types::OverlayTypeRegistry;
use crate::rules::ruleset::RuleSet;
use crate::sim::intern::InternedId;
use crate::sim::mission::MissionType;
use crate::sim::movement::infantry_entry::{EntryQueryMode, InfantryEntryArgs};
use crate::sim::world::{FrameEffects, PlacementEvidence, Simulation};

impl Simulation {
    /// GetHouse6F2070: fixed Country attachment, otherwise the native
    /// Scenario1180 start-slot mapping used by Player@A..H510ED0. Country
    /// resolution uses House array order502D30, never sorted owner names.
    fn team_type_house(&self, team_type: InternedId, rules: &RuleSet) -> Option<InternedId> {
        match self.team_script_vm.team_type_ini.get(&team_type)?.house? {
            TeamTypeHouse::Country(country) => {
                self.session.house_order.iter().copied().find(|owner| {
                    self.houses
                        .get(owner)
                        .and_then(|house| house.country)
                        .and_then(|id| rules.country_index(self.interner.resolve(id)))
                        == Some(country)
                })
            }
            TeamTypeHouse::PlayerAt(slot) => self
                .session
                .start_slot_houses
                .get(&u32::from(slot))
                .copied(),
        }
    }

    fn team_initial_zone(&self, team_type: InternedId) -> Option<TeamTarget> {
        let waypoint = self.team_script_vm.team_type_ini.get(&team_type)?.waypoint;
        self.session
            .waypoint_cell(waypoint)
            .filter(|&cell| cell != (0, 0))
            .map(|cell| self.canonical_team_cell(cell))
    }

    /// Original CreateTeam6F09C0 owner. The VM performs its shared Max gate
    /// and sole6E8A90 constructor; the Simulation resolves House and the
    /// Scenario-owned initial waypoint. Trigger4 calls with no explicit House,
    /// while House AI supplies itself, as4F8AAD does.
    pub(crate) fn create_team(
        &mut self,
        team_type: InternedId,
        explicit_owner: Option<InternedId>,
        rules: &RuleSet,
    ) -> Option<u64> {
        let owner = explicit_owner.or_else(|| self.team_type_house(team_type, rules))?;
        let zone = self.team_initial_zone(team_type);
        let placement_scope_active = self.object_placement_scope_active();
        let id = self.team_script_vm.construct_team(
            team_type,
            owner,
            self.session.game_mode_nonzero,
            self.session.binary_frame as i32,
            placement_scope_active,
            zone,
        )?;
        // The trigger definition owner retains the actual6F1341-resolved
        // TagType ordinal; never re-find it through a copied raw name.
        self.attach_team_trigger_tag(id, team_type);
        Some(id)
    }

    /// Trigger4 original6DEB6B brackets CreateTeam with A8E7AC. Recruiting
    /// remains in TeamAI's normal same-frame visit, using real class arrays.
    pub(crate) fn trigger_create_team(
        &mut self,
        team_type: InternedId,
        rules: &RuleSet,
        _registry: Option<&OverlayTypeRegistry>,
        _effects: FrameEffects<'_>,
    ) -> Option<u64> {
        self.with_object_placement_scope(|sim| sim.create_team(team_type, None, rules))
    }

    /// Normal SpawnReinforcements65D8E0. The action's native override is -1;
    /// a valid explicit override inside the playfield asks shared FNPC first.
    /// All members are constructed before any placement, preserving concrete
    /// constructor RNG→Create_ID order. Team/Script themselves spend no ID.
    pub(crate) fn spawn_team_reinforcements(
        &mut self,
        team_type: InternedId,
        waypoint_override: i32,
        rules: &RuleSet,
        registry: Option<&OverlayTypeRegistry>,
        effects: FrameEffects<'_>,
    ) -> (bool, Option<(i16, i16)>) {
        let Some(definition) = self.team_script_vm.team_types.get(&team_type).copied() else {
            return (false, None);
        };
        let Some(task_force) = self
            .team_script_vm
            .task_forces
            .get(&definition.task_force_id)
        else {
            return (false, None);
        };
        if task_force.entries.is_empty() {
            return (false, None);
        }
        let entries = task_force.entries.clone();
        let metadata = self
            .team_script_vm
            .team_type_ini
            .get(&team_type)
            .cloned()
            .unwrap_or_default();
        if metadata.droppod || metadata.full {
            log::warn!(
                "TeamType {} reinforcement transport Full/Droppod mechanism is unported",
                self.interner.resolve(team_type)
            );
            return (false, None);
        }
        //65D90F..65D970: an empty referenced program gains Guard(0), in
        //that same ScriptType owner. No duplicate fallback Script instance.
        let Some(script) = self.team_script_vm.scripts.get_mut(&definition.script_id) else {
            return (false, None);
        };
        if script.actions.is_empty() {
            script.actions.push(TeamScriptAction {
                action_id: 5,
                argument: 0,
            });
        }
        let Some(owner) = self.team_type_house(team_type, rules) else {
            return (false, None);
        };
        let zone = self.team_initial_zone(team_type);
        let Some(team) = self.team_script_vm.construct_team_instance(
            team_type,
            owner,
            self.session.binary_frame as i32,
            zone,
        ) else {
            return (false, None);
        };
        self.attach_team_trigger_tag(team, team_type);
        let state = self
            .team_script_vm
            .teams
            .get_mut(&team)
            .expect("constructed Team");
        state.forced_active = true;
        state.under_strength = false;
        let owner_name = self.interner.resolve(owner).to_string();
        let mut members = Vec::new();
        for entry in entries {
            let type_name = self.interner.resolve(entry.member_type.id).to_string();
            for _ in 0..entry.count {
                let member = self.with_object_placement_scope(|sim| {
                    sim.construct_object_limbo_at_height(&type_name, &owner_name, 0, 0, 0, 0, rules)
                });
                let Some(member) = member else {
                    continue;
                };
                let entity = self
                    .substrate
                    .entities
                    .get_mut(member)
                    .expect("constructed member");
                match metadata.veteran_level {
                    0 => entity.veterancy.set_rookie(),
                    2 => entity.veterancy.set_veteran(),
                    3 => entity.veterancy.set_elite(),
                    _ => {}
                }
                self.with_object_placement_scope(|sim| {
                    sim.team_add_member(team, member, false, rules, registry, effects)
                });
                //65DEE4 explicitly initiates each new member, even when it
                //was not the first one AddMember prepended.
                if let Some(link) = self
                    .team_script_vm
                    .teams
                    .get_mut(&team)
                    .and_then(|team| team.members.iter_mut().find(|link| link.id == member))
                {
                    link.initiated = true;
                }
                members.push(member);
            }
        }
        if members.is_empty() {
            self.destroy_team(team, rules, effects);
            return (false, None);
        }
        let waypoint = if waypoint_override == -1 {
            metadata.waypoint
        } else {
            waypoint_override
        };
        let origin = self.session.waypoint_cell(waypoint).unwrap_or((0, 0));
        let nearby = waypoint_override != -1
            && crate::sim::cell_rect::cell_is_in_playfield_height_aware(
                (i32::from(origin.0), i32::from(origin.1)),
                self.playfield_bounds,
                self.resolved_terrain.as_ref(),
            );
        members.reverse(); //65DF1F prepends the temporary placement chain.
        let placed = self
            .place_reinforcement_members(&members, owner, origin, nearby, rules, registry, effects);
        //65DB6B..65DB97: only successful placement for an allied current
        //House records the REQUESTED origin in the existing review-cell ring.
        //The trigger dispatcher publishes this transient result in action order.
        let review_cell = self
            .session
            .current_house()
            .filter(|&current| {
                placed
                    && crate::map::houses::is_allied_with(
                        &self.house_alliances,
                        self.interner.resolve(owner),
                        self.interner.resolve(current),
                    )
            })
            .map(|_| origin);
        //65DB9C returns true regardless of PlaceMembers' result. Failed
        //members have gone through their ordinary scalar destructor owner.
        (true, review_cell)
    }

    /// Normal65E010 without Tunnel/Droppod. A successful member leaves the
    /// candidate unchanged for the next member; never invent formation
    /// offsets. Failure tests native eight directions and the inverted Size
    /// gate65E554: only OUTSIDE Size candidates reach class admission. Original
    /// execution on an already placed Unit pins all8 XY candidates and scalar
    /// cleanup; that control is not a fresh-reinforcement refusal claim.
    #[allow(clippy::too_many_arguments)]
    fn place_reinforcement_members(
        &mut self,
        members: &[u64],
        owner: InternedId,
        mut cell: (i16, i16),
        nearby: bool,
        rules: &RuleSet,
        registry: Option<&OverlayTypeRegistry>,
        effects: FrameEffects<'_>,
    ) -> bool {
        let edge = self
            .houses
            .get(&owner)
            .map_or(0, |house| house.authored_edge());
        let edge = if edge == -1 { 0 } else { edge };
        if nearby {
            let speed = self
                .substrate
                .entities
                .get(members[0])
                .and_then(|member| self.object_type(member.type_ref(), rules))
                .expect("reinforcement has its resolved TechnoType")
                .speed_type;
            cell = self
                .find_plain_passable_cell(
                    (i32::from(cell.0), i32::from(cell.1)),
                    speed,
                    None,
                    MovementZone::Normal,
                    (1, 1),
                )
                .map_or((0, 0), |(x, y)| (x as i16, y as i16));
        }
        let mut placed_any = false;
        for &member in members {
            if cell == (0, 0) {
                self.discard_constructed_limbo(member, Some(rules), effects);
                continue;
            }
            let aircraft = self
                .substrate
                .entities
                .get(member)
                .is_some_and(|member| member.category() == EntityCategory::Aircraft);
            let heading = if nearby {
                0
            } else {
                edge.wrapping_mul(2).wrapping_shl(13) as u16
            };
            let heading = if aircraft {
                heading.wrapping_sub(0x8000)
            } else {
                heading
            };
            let facing = (((u32::from(heading) >> 7) + 1) >> 1) as u8;
            loop {
                let target = self.canonical_team_cell(cell);
                let coord = self
                    .team_target_coord(target)
                    .expect("native Cell center coordinate");
                let placed = self
                    .with_object_placement_scope(|sim| {
                        sim.reveal_constructed_object_at_coord_with_overlay_context(
                            member,
                            coord,
                            facing,
                            PlacementEvidence::UnitEntryAdmitted,
                            rules,
                            registry,
                            effects,
                        )
                    })
                    .is_some();
                if placed {
                    placed_any = true;
                    if !aircraft {
                        self.team_member_queue_mission(member, MissionType::Guard, rules);
                        self.team_member_commence(member, rules);
                    }
                    break;
                }
                let next = crate::util::direction_tables::CELL_DELTAS
                    .iter()
                    .enumerate()
                    .find_map(|(direction, &(dx, dy))| {
                        let candidate = (
                            cell.0.wrapping_add(dx as i16),
                            cell.1.wrapping_add(dy as i16),
                        );
                        if self.map_cell_in_bounds(candidate) {
                            return None;
                        }
                        (self
                            .mover_can_enter(
                                member,
                                candidate,
                                InfantryEntryArgs {
                                    direction: direction as i32,
                                    height: -1,
                                    previous_cell: None,
                                },
                                EntryQueryMode::CheckLocomotor,
                                rules,
                                registry,
                            )
                            .expect("reinforcement uses its shared class admission receiver")
                            == 0)
                            .then_some(candidate)
                    });
                cell = next.unwrap_or((0, 0));
                if cell == (0, 0) {
                    self.discard_constructed_limbo(member, Some(rules), effects);
                    break;
                }
            }
        }
        placed_any
    }
}
