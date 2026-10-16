//! The live command path: issue into the native queue and its tail dispatch.
//!
//! `Simulation` owns the one [`SynchronizedCommandQueue`] (persisted in
//! snapshots). Local producers issue native records into its OutList
//! (`LocalEvent`), the planning publisher prepends to its DoList, and the
//! Main_Tick tail transfers and dispatches it
//! ([`Simulation::dispatch_due_command_queue`]). Records decode at execution;
//! `world_commands` implements each decoded payload.

use super::Simulation;
use crate::sim::world::FrameEffects;

use crate::map::bridge_facts::BRIDGE_FLAG_STRUCTURAL;
use crate::map::cell_index::NativeCellIdentity;
use crate::map::entities::EntityCategory;
use crate::map::resolved_terrain::{NativeCellQuery, ResolvedTerrainGrid};
use crate::rules::locomotor_type::MovementZone;
use crate::rules::ruleset::RuleSet;
use crate::sim::command::{
    Command, CommandEnvelope, CommandRecord, MEGAMISSION_OPCODE, MegaMissionTarget,
};
use crate::sim::intern::InternedId;
use crate::sim::lockstep::{CommandDispatchHouse, SynchronizedCommand};
use crate::sim::movement::group_destination;
use crate::sim::movement::infantry_entry::InfantryEntryArgs;
use crate::sim::pathfinding::zone_map::ZoneGrid;
use crate::util::lepton::BRIDGE_DECK_HEIGHT_LEVELS;

/// Which local producer a command comes from, where the two send different
/// records for the same order: the deploy key sends DEPLOY
/// (`Selection::IssueDeployCommand 0x00730AF0`), a self-click sends the Foot
/// self-click's MEGAMISSION Unload (`0x004D74E0` action 4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LocalProducer {
    /// Keys, the sidebar and targeting clicks.
    Key,
    /// Order clicks on the tactical map (`ClickedAction` routes).
    Click,
}

impl Simulation {
    /// `LocalEvent` (`0x006521C0`): stamp the current frame and append one
    /// locally issued record to the OutList. Retail drops a record while the
    /// 128-slot OutList is full; so does this.
    pub(crate) fn issue_command_record(&mut self, mut record: CommandRecord) -> bool {
        record.set_frame_stamp(self.session.binary_frame as i32);
        self.issue_recorded_command(record)
    }

    /// Append a record to the OutList with the frame it already carries: a
    /// diagnostic log's record, or a tooling order for a later frame.
    pub(crate) fn issue_recorded_command(&mut self, record: CommandRecord) -> bool {
        self.command_queue
            .issue(SynchronizedCommand::opaque(record))
    }

    /// The planning publisher `0x00652230`: prepend an already stamped record
    /// to the DoList head, bypassing the OutList.
    pub(super) fn prepend_command_record(&mut self, record: CommandRecord) -> bool {
        self.command_queue
            .prepend(SynchronizedCommand::opaque(record))
    }

    /// Encode one local order as its producer's records and issue them, with
    /// the record frame `execute_tick` names: the current frame for the next
    /// tick or earlier, one frame later per later tick. Offline Options speed
    /// is not an event (native stores it at once in modes 0 and 5), so a
    /// SetGameSpeed applies here; a planning record is issued as it is. An
    /// order without records is dropped, as its producer drops it. Returns
    /// the records the OutList accepted, in issue order.
    ///
    /// Tooling and fixtures issue semantic orders through this; the app's
    /// producers encode through [`Self::encode_local_command_records`].
    pub fn issue_local_commands(
        &mut self,
        commands: &[CommandEnvelope],
        rules: Option<&RuleSet>,
    ) -> Vec<CommandRecord> {
        let mut issued = Vec::new();
        for command in commands {
            let records = match &command.payload {
                Command::SetGameSpeed { speed } => {
                    self.apply_local_game_speed(command.owner, *speed);
                    continue;
                }
                Command::PlanningRecord { record } => vec![record.clone()],
                payload => self
                    .encode_local_command_records(
                        command.owner,
                        payload,
                        rules,
                        LocalProducer::Click,
                    )
                    .unwrap_or_default(),
            };
            let ahead = command
                .execute_tick
                .saturating_sub(self.session.tick.saturating_add(1));
            for mut record in records {
                if !matches!(command.payload, Command::PlanningRecord { .. }) {
                    record.set_frame_stamp(
                        (self.session.binary_frame as i32).wrapping_add(ahead as i32),
                    );
                }
                if self.issue_recorded_command(record.clone()) {
                    issued.push(record);
                }
            }
        }
        issued
    }

    /// Fixture issue ([`Self::issue_local_commands`]) for orders from the
    /// player. An owner without a house is registered as a human house, and
    /// a house outside the HouseClass order joins it, as the former
    /// dispatcher appended standalone fixture owners. An order from a house
    /// the tail would skip (neither human nor PlayerControl) panics: a
    /// computer house sends no events.
    #[cfg(test)]
    pub(crate) fn issue_fixture_commands(
        &mut self,
        commands: &[CommandEnvelope],
        rules: Option<&RuleSet>,
    ) -> Vec<CommandRecord> {
        for command in commands {
            let name = self.interner.resolve(command.owner).to_string();
            assert!(
                self.houses
                    .get(&command.owner)
                    .is_none_or(|house| house.event_dispatch_eligible()),
                "fixture order from computer house {name}: the tail dispatches only \
                 IsHuman || PlayerControl houses"
            );
            self.register_command_house_for_test(&name);
        }
        self.issue_local_commands(commands, rules)
    }

    /// Offline In-game Options: `OptionsClass::ApplyFromInGameDialog`
    /// (`0x004E1E54`) stores the speed at once in modes 0 and 5 and sends no
    /// event. Refuses an unregistered house and a speed outside the Options
    /// range.
    pub(crate) fn apply_local_game_speed(&mut self, owner: InternedId, speed: u8) -> bool {
        let applied = self.houses.contains_key(&owner)
            && self.session.game_options.apply_in_game_speed(speed);
        if applied {
            self.local_game_speed_input = Some(speed);
        }
        applied
    }

    /// Re-issue one diagnostic-log tick's external input before its frame:
    /// the offline speed it stored, then its records in issue order.
    #[cfg(test)]
    pub(crate) fn replay_tick_input(&mut self, tick: &crate::sim::replay::ReplayTick) {
        if let Some(speed) = tick.game_speed {
            self.session.game_options.apply_in_game_speed(speed);
        }
        for record in &tick.commands {
            self.issue_recorded_command(record.clone());
        }
    }

    /// The records issued since the last tail and still in the OutList, in
    /// issue order, and the offline speed stored since the last frame: the
    /// external input a frame starts with before its own producers run.
    pub(super) fn take_frame_input(&mut self) -> (Vec<CommandRecord>, Option<u8>) {
        (
            self.command_queue
                .local_records()
                .map(|command| command.record().clone())
                .collect(),
            self.local_game_speed_input.take(),
        )
    }

    /// Diagnostic decoded view of the queue: DoList records not yet executed,
    /// then the OutList in issue order. `execute_tick` follows the issue
    /// receipts' convention: the current tick for a record stamped with the
    /// current frame, one more per later frame. Records decode against the
    /// current state; one the decoder does not represent is omitted. The
    /// tail decodes again when each executes.
    pub(crate) fn pending_command_snapshot(&self, rules: Option<&RuleSet>) -> Vec<CommandEnvelope> {
        let frame = self.session.binary_frame as i32;
        self.command_queue
            .synchronized_records()
            .filter(|command| !command.record().is_processed())
            .chain(self.command_queue.local_records())
            .filter_map(|command| {
                let ahead = command.record().frame_stamp().wrapping_sub(frame).max(0) as u64;
                self.decode_native_command_record(
                    command.record(),
                    rules,
                    self.session.tick.saturating_add(ahead),
                )
            })
            .collect()
    }

    /// Consume the terminal edge raised by one executed EXIT command.
    pub(crate) fn take_executed_exit_owner(&mut self) -> Option<InternedId> {
        self.executed_exit_owner.take()
    }

    /// The single-player body of `Process_Command_Queues_For_Frame`
    /// (`0x00647260`; modes 0 and 5 take `0x006474DC`): transfer the OutList
    /// to the DoList unchanged, then dispatch every due record in HouseClass
    /// registration order to `EventClass::Execute`. A house is scanned only
    /// when `IsHuman || PlayerControl`. Each house's MEGAMISSION records are
    /// staged and spread as a group before they execute
    /// ([`Self::adjust_staged_megamission_destinations`]). A record decodes
    /// when it executes, against the state earlier records left. Returns
    /// `(executed_commands, spawned_entities)`; a sale or undeploy only starts
    /// the Selling mission (`Simulation::visit_building_down`).
    ///
    /// Planning orders published by Logic are flushed to the DoList first
    /// (`0x0055DD9B`), and those a plan commit publishes during the scan after
    /// it (`0x0055DE45`), as `0x00637550` does.
    pub(crate) fn dispatch_due_command_queue(
        &mut self,
        rules: Option<&RuleSet>,
        overlay_registry: Option<&crate::rules::overlay_types::OverlayTypeRegistry>,
        frame_effects: FrameEffects<'_>,
    ) -> (usize, bool) {
        self.flush_staged_planning_records();
        let mut queue = std::mem::take(&mut self.command_queue);
        queue.transfer_single_player();
        let houses: Vec<_> = self
            .session
            .house_order
            .iter()
            .enumerate()
            .filter_map(|(index, &owner)| {
                Some(CommandDispatchHouse::new(
                    owner,
                    i8::try_from(index).ok()?,
                    self.houses
                        .get(&owner)
                        .is_some_and(|house| house.event_dispatch_eligible()),
                ))
            })
            .collect();
        let current_frame = self.session.binary_frame as i32;
        let execute_tick = self.session.tick.saturating_add(1);
        let mut executed_commands = 0usize;
        let mut spawned_entities = false;
        // The adjust and execute callbacks never run at once; both reach the
        // simulation through this one cell.
        let sim = std::cell::RefCell::new(&mut *self);
        queue.dispatch_due_offline(
            current_frame,
            &houses,
            |_, staged| {
                sim.borrow()
                    .adjust_staged_megamission_destinations(staged, rules, overlay_registry)
            },
            |_, command, _| {
                let mut sim = sim.borrow_mut();
                executed_commands += 1;
                #[cfg(test)]
                sim.dispatched_records_test_trace
                    .push(command.record().clone());
                let Some(envelope) =
                    sim.decode_native_command_record(command.record(), rules, execute_tick)
                else {
                    return;
                };
                let (_, spawned) =
                    sim.apply_one_due_command(&envelope, rules, overlay_registry, frame_effects);
                spawned_entities |= spawned;
            },
        );
        // No receiver issues into the queue: plan commits stage their orders.
        let issued_during_scan = std::mem::replace(&mut self.command_queue, queue);
        debug_assert_eq!(
            issued_during_scan,
            Default::default(),
            "a receiver issued into the command queue during its scan"
        );
        self.flush_staged_planning_records();
        (executed_commands, spawned_entities)
    }

    /// VERA-only: one command and its spawn-refresh report: a Deploy whose MCV left
    /// the store or carries the `TerminalLifetime` dying mark counts as a spawn.
    fn apply_one_due_command(
        &mut self,
        cmd: &CommandEnvelope,
        rules: Option<&RuleSet>,
        overlay_registry: Option<&crate::rules::overlay_types::OverlayTypeRegistry>,
        frame_effects: FrameEffects<'_>,
    ) -> (bool, bool) {
        // Scatter's found-cell arm immediately runs the locomotor. Its
        // authoritative bridge flag can change before the ordinary movement
        // phase, so that phase must rebuild its derived navigation view too.
        let scatter_bridge_before = match cmd.payload {
            Command::Scatter { entity_id } => self
                .substrate
                .entities
                .get(entity_id)
                .map(|entity| (entity_id, entity.on_bridge())),
            _ => None,
        };
        let cmd_owner_str = self.interner.resolve(cmd.owner).to_string();
        let applied = self.apply_command_with_overlays(
            &cmd_owner_str,
            &cmd.payload,
            rules,
            overlay_registry,
            frame_effects,
        );
        let placed_building_owner = self.successful_non_wall_placement_owner(cmd, applied, rules);
        let synchronous_deploy = applied
            && matches!(cmd.payload, Command::DeployMcv { entity_id }
            if self.substrate.entities.get(entity_id).is_none_or(|e| e.death.is_dying()));
        let spawned_entity = synchronous_deploy
            || placed_building_owner.is_some()
            || applied && matches!(cmd.payload, Command::PlaceProducedMobile { .. })
            || applied && matches!(cmd.payload, Command::LaunchSuperWeapon { .. })
            || scatter_bridge_before.is_some_and(|(id, before)| {
                self.substrate
                    .entities
                    .get(id)
                    .is_some_and(|entity| entity.on_bridge() != before)
            });
        (applied, spawned_entity)
    }

    pub(super) fn successful_non_wall_placement_owner(
        &self,
        command: &CommandEnvelope,
        applied: bool,
        rules: Option<&RuleSet>,
    ) -> Option<InternedId> {
        let Command::PlaceReadyBuilding { type_id, .. } = command.payload else {
            return None;
        };
        (applied
            && rules
                .and_then(|rules| rules.object(self.interner.resolve(type_id)))
                .is_some_and(|object| !object.wall))
        .then_some(command.owner)
    }

    /// The run key of `0x0064CDA0`'s caller over one staged MEGAMISSION record:
    /// action, target and destination tokens (`0x0064CB0A..0x0064CB60`), planning
    /// records included. `0x0064CDA0` returns at once unless the destination
    /// token is a Cell (`0x0064CDCA..0x0064CDDC`), so a Move onto an object, an
    /// Area Guard (its post is the target token) and an attack-move with an
    /// object destination have no key: each ends a run and is never spread.
    fn formation_key(record: &CommandRecord) -> Option<((u8, i32, u8), u16, u16)> {
        if record.opcode() != MEGAMISSION_OPCODE {
            return None;
        }
        let p = record.payload();
        if p[16] != 11 {
            return None;
        }
        let destination = i32::from_le_bytes(p[12..16].try_into().ok()?);
        let MegaMissionTarget::Cell { x, y } = MegaMissionTarget::from_token(destination, 11)?
        else {
            return None;
        };
        Some((
            (p[5], i32::from_le_bytes(p[7..11].try_into().ok()?), p[11]),
            x as u16,
            y as u16,
        ))
    }

    /// The staged record's source object (an Abstract token).
    fn formation_entity_id(record: &CommandRecord) -> Option<u64> {
        let p = record.payload();
        if record.opcode() != MEGAMISSION_OPCODE || p[4] != 0x34 {
            return None;
        }
        u64::try_from(i32::from_le_bytes(p[0..4].try_into().ok()?)).ok()
    }

    fn group_member_target(
        &self,
        rules: &RuleSet,
        terrain: &ResolvedTerrainGrid,
        zones: &ZoneGrid,
        clicked_target: (i16, i16),
        entity_id: u64,
    ) -> GroupMemberTarget {
        let entity = self
            .substrate
            .entities
            .get(entity_id)
            .expect("a group member stays live while its run is staged");
        let movement_zone = rules
            .object(self.interner.resolve(entity.type_ref()))
            .map_or_else(Default::default, |object| object.movement_zone);
        let cells = NativeCellQuery::canonical(terrain);
        GroupMemberTarget {
            entity_id,
            movement_zone,
            height: spread_height(terrain, cells.lookup(clicked_target)),
            zone: zones.get_zone_id_native(
                terrain,
                (clicked_target.0 as u16, clicked_target.1 as u16),
                movement_zone,
                true,
            ),
        }
    }

    /// One candidate's gates for one member, as `0x0064CDA0` reads them:
    /// - the candidate's cell (`0x0064D4D6`) and `IsCellInPlayfield(candidate,
    ///   1)` (`0x0064D4E9`);
    /// - its zone (`0x0056D230` with the cell's own bridge flag, `0x0064D51B`),
    ///   compared with the target's as the native DWORD (`0x0064D537`);
    /// - the member's own `Can_Enter_Cell` (`vt+0x1AC` at `0x0064D52F`) with
    ///   no direction, the target's height and no source cell;
    /// - the height band ([`within_spread_band`], `0x0064D53D..0x0064D592`).
    ///
    /// A `None` zone is an inactive-deck walk that native never finishes; it
    /// compares equal only to another `None`. A member whose `Can_Enter_Cell`
    /// VERA cannot read (a retired receiver, a missing type or overlay registry)
    /// refuses the candidate with 7 and a warning; native has no such outcome.
    fn group_destination_candidate_facts(
        &self,
        rules: &RuleSet,
        registry: Option<&crate::rules::overlay_types::OverlayTypeRegistry>,
        terrain: &ResolvedTerrainGrid,
        zones: &ZoneGrid,
        target: &GroupMemberTarget,
        candidate: (i16, i16),
    ) -> group_destination::CandidateFacts {
        let cells = NativeCellQuery::canonical(terrain);
        let cell = cells.lookup(candidate);
        if !crate::sim::cell_rect::cell_is_in_playfield_height_aware(
            (i32::from(candidate.0), i32::from(candidate.1)),
            self.playfield_bounds,
            Some(terrain),
        ) {
            return group_destination::CandidateFacts::outside_playfield();
        }
        let id = target.entity_id;
        let bridge = terrain.native_cell_flags(cell) & BRIDGE_FLAG_STRUCTURAL != 0;
        let zone = zones.get_zone_id_native(
            terrain,
            (candidate.0 as u16, candidate.1 as u16),
            target.movement_zone,
            bridge,
        );
        let args = InfantryEntryArgs {
            direction: -1,
            height: target.height,
            previous_cell: None,
        };
        let can_enter_code = self
            .mover_can_enter(
                id,
                candidate,
                args,
                crate::sim::movement::infantry_entry::EntryQueryMode::CheckLocomotor,
                rules,
                registry,
            )
            .unwrap_or_else(|error| {
                log::warn!("group destination entry for {id}: {error}");
                7
            });
        group_destination::CandidateFacts {
            in_playfield: true,
            same_zone: zone == target.zone,
            height_band_ok: within_spread_band(target.height, spread_height(terrain, cell)),
            can_enter_code,
        }
    }

    /// Adjust consecutive same-target movement runs after their house's
    /// non-megamission scan, immediately before staged command execution.
    /// The group spread `0x0064CDA0` per run (`0x0064CB0A..0x0064CB60`): each Foot
    /// member's coordinates (Location) and source cell.
    pub(super) fn adjust_staged_megamission_destinations(
        &self,
        commands: &mut [SynchronizedCommand],
        rules: Option<&RuleSet>,
        registry: Option<&crate::rules::overlay_types::OverlayTypeRegistry>,
    ) {
        let (Some(rules), Some(terrain), Some(zones)) = (
            rules,
            self.resolved_terrain.as_ref(),
            self.zone_grid.as_ref(),
        ) else {
            return;
        };
        let mut run_start = 0;
        while run_start < commands.len() {
            let Some(key) = Self::formation_key(commands[run_start].record()) else {
                run_start += 1;
                continue;
            };
            let mut run_end = run_start + 1;
            while run_end < commands.len()
                && Self::formation_key(commands[run_end].record()) == Some(key)
            {
                run_end += 1;
            }
            if run_end - run_start > 1 {
                let mut members = Vec::new();
                for (command_index, command) in commands[run_start..run_end].iter().enumerate() {
                    let Some(entity_id) = Self::formation_entity_id(command.record()) else {
                        continue;
                    };
                    let Some(entity) = self.substrate.entities.get(entity_id) else {
                        continue;
                    };
                    if !matches!(
                        entity.category(),
                        EntityCategory::Unit | EntityCategory::Infantry | EntityCategory::Aircraft
                    ) || entity.low_bridge_tube_state.is_some()
                    {
                        continue;
                    }
                    let coord_z = crate::sim::movement::ground_pose::object_world_z_leptons(
                        entity,
                        Some(terrain),
                    );
                    members.push(group_destination::GroupDestinationMember {
                        command_index,
                        entity_id,
                        coord: [
                            i32::from(entity.position.rx())
                                .wrapping_mul(256)
                                .wrapping_add(entity.position.sub_x().to_num::<i32>()),
                            i32::from(entity.position.ry())
                                .wrapping_mul(256)
                                .wrapping_add(entity.position.sub_y().to_num::<i32>()),
                            coord_z,
                        ],
                        source_cell: (entity.position.rx() as i16, entity.position.ry() as i16),
                    });
                }
                let clicked_target = (key.1 as i16, key.2 as i16);
                let mut member_target: Option<GroupMemberTarget> = None;
                let assignments = group_destination::distribute_group_destinations(
                    clicked_target,
                    &members,
                    |member, candidate| {
                        if member_target
                            .as_ref()
                            .is_none_or(|target| target.entity_id != member.entity_id)
                        {
                            member_target = Some(self.group_member_target(
                                rules,
                                terrain,
                                zones,
                                clicked_target,
                                member.entity_id,
                            ));
                        }
                        let target = member_target.as_ref().expect("read for this member");
                        self.group_destination_candidate_facts(
                            rules, registry, terrain, zones, target, candidate,
                        )
                    },
                );
                for assignment in assignments {
                    let Some(command) = commands.get_mut(run_start + assignment.command_index)
                    else {
                        continue;
                    };
                    let destination = MegaMissionTarget::Cell {
                        x: assignment.destination.0,
                        y: assignment.destination.1,
                    }
                    .token()
                    .expect("Cell target token");
                    command.payload_mut()[12..16].copy_from_slice(&destination.0.to_le_bytes());
                }
            }
            run_start = run_end;
        }
    }
}

/// The target facts `0x0064CDA0` reads once per probed member.
struct GroupMemberTarget {
    entity_id: u64,
    movement_zone: MovementZone,
    height: i32,
    zone: Option<u32>,
}

/// The spread's height of a Cell: its signed level (`+0x11B`), plus
/// `BRIDGE_DECK_HEIGHT_LEVELS` on the bridge flag (`+0x140 & 0x100`). The
/// target's is EBX (`0x0064D2AA..0x0064D2DF`), which Can_Enter_Cell receives;
/// a candidate's is measured against it (`0x0064D55F..0x0064D592`).
/// Native executions: tools/spatial_oracle/group_spread_gates.{py,json,meta.json}.
fn spread_height(terrain: &ResolvedTerrainGrid, cell: NativeCellIdentity) -> i32 {
    let level = i32::from(terrain.native_cell_ground_fields(cell).0 as i8);
    if terrain.native_cell_flags(cell) & BRIDGE_FLAG_STRUCTURAL != 0 {
        level + BRIDGE_DECK_HEIGHT_LEVELS
    } else {
        level
    }
}

/// A candidate more than two levels from the target's height ends the
/// member's probes (`JG` at `0x0064D575` and `0x0064D592`).
fn within_spread_band(target_height: i32, candidate_height: i32) -> bool {
    (candidate_height - target_height).abs() <= 2
}

#[cfg(test)]
#[path = "group_spread_gate_tests.rs"]
mod group_spread_gate_tests;
