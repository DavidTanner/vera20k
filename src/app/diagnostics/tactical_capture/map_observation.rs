//! Chosen-map controller within the existing hidden tactical capture lifecycle.
//! The launch is the production session DTO; this diagnostic owns no gameplay.

use super::super::integrity::{SealedJsonFile, parse_strict_json, read_stable_regular_bytes};
use super::super::manifest::{PublishFault, publish_transaction};
use super::*;
use crate::app::diagnostics::state::{MAP_PRESENTATION_CLOCK_POLICY, MAP_PRESENTATION_INTERVAL_MS};
use crate::app::presentation::render::GameRenderTimes;
use crate::skirmish_launch::{LaunchStartPosition, PreFillHouseRoster, SkirmishLaunchSession};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

const PROFILE_V1: &str = "vera20k.map-observation-profile.v1";
const PROFILE_V2: &str = "vera20k.map-observation-profile.v2";
const CHILD_SCHEMA: &str = "vera20k.map-observation.v5";
const OBSERVATION_POLICY: &str = "map-ordinary-command-observation-v2";
const MAX_COMMANDS: usize = 1024;
const MAX_OBSERVED_OWNERS: usize = 30;
const MAX_TERRAIN_CELLS: usize = 256;
const MAX_OBSERVATION_SAMPLES: usize = 100_000;
const MAX_RECEIPT_BYTES: usize = 128 * 1024 * 1024;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct MapScheduledCommand {
    issue_after_step: u32,
    owner: String,
    #[serde(deserialize_with = "deserialize_command")]
    payload: Command,
}

// Reuse Command's one serde schema, but reject fields that its permissive
// enum deserializer would otherwise discard. No second command parser here.
fn deserialize_command<'de, D>(deserializer: D) -> std::result::Result<Command, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = Value::deserialize(deserializer)?;
    let command: Command =
        serde_json::from_value(value.clone()).map_err(serde::de::Error::custom)?;
    let canonical = serde_json::to_value(&command).map_err(serde::de::Error::custom)?;
    if value != canonical {
        return Err(serde::de::Error::custom(
            "command payload has unrecognized or noncanonical fields",
        ));
    }
    Ok(command)
}

fn deserialize_present<'de, D, T>(deserializer: D) -> std::result::Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    T::deserialize(deserializer).map(Some)
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct MapCaptureProfile {
    pub(crate) schema_version: String,
    pub(crate) launch: SkirmishLaunchSession,
    pub(crate) seed: u32,
    pub(crate) input_delay_ticks: u32,
    pub(crate) ticks: u32,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) timeout_seconds: u32,
    // Option retains field presence in the sealed profile receipt: old v1
    // profiles serialize byte-equivalent JSON values without new defaults.
    #[serde(
        default,
        deserialize_with = "deserialize_present",
        skip_serializing_if = "Option::is_none"
    )]
    commands: Option<Vec<MapScheduledCommand>>,
    #[serde(
        default,
        deserialize_with = "deserialize_present",
        skip_serializing_if = "Option::is_none"
    )]
    observe_owners: Option<Vec<String>>,
    #[serde(
        default,
        deserialize_with = "deserialize_present",
        skip_serializing_if = "Option::is_none"
    )]
    camera_cell: Option<[u16; 2]>,
    #[serde(
        default,
        deserialize_with = "deserialize_present",
        skip_serializing_if = "Option::is_none"
    )]
    terrain_cells: Option<Vec<[u16; 2]>>,
}

impl MapCaptureProfile {
    pub(crate) fn load(path: &Path) -> Result<SealedJsonFile<Self>> {
        let (bytes, digest) = read_stable_regular_bytes(path, "map observation profile")?;
        let value: Self = parse_strict_json(&bytes, "map observation profile")?;
        value.validate()?;
        Ok(SealedJsonFile {
            path: path.to_path_buf(),
            byte_length: digest.byte_length,
            sha256: digest.sha256,
            bytes,
            value,
        })
    }

    fn validate(&self) -> Result<()> {
        ensure!(
            matches!(self.schema_version.as_str(), PROFILE_V1 | PROFILE_V2),
            "unsupported map observation schema"
        );
        if self.schema_version == PROFILE_V1 {
            ensure!(
                self.commands.is_none()
                    && self.observe_owners.is_none()
                    && self.camera_cell.is_none()
                    && self.terrain_cells.is_none(),
                "map observation profile v1 cannot declare v2 extension fields"
            );
        }
        ensure!(
            (640..=4096).contains(&self.width) && (480..=4096).contains(&self.height),
            "capture extent must be 640..4096 by 480..4096"
        );
        ensure!(self.ticks <= 100_000, "capture tick budget exceeds 100000");
        ensure!(
            (1..=900).contains(&self.timeout_seconds),
            "timeout must be 1..900 seconds"
        );
        ensure!(
            self.commands().len() <= MAX_COMMANDS,
            "too many scheduled commands"
        );
        let mut previous = 0;
        for command in self.commands() {
            ensure!(
                command.issue_after_step >= previous && command.issue_after_step < self.ticks,
                "commands must be ordered by issue_after_step before the final step"
            );
            ensure!(!command.owner.is_empty(), "command owner is empty");
            ensure!(
                matches!(
                    command.payload,
                    Command::Move { .. }
                        | Command::Stop { .. }
                        | Command::Attack { .. }
                        | Command::ForceAttack { .. }
                        | Command::Guard { .. }
                        | Command::DeployMcv { .. }
                        | Command::ForceAttackCell { .. }
                        | Command::QueueProduction { .. }
                        | Command::PlaceReadyBuilding { .. }
                        | Command::CaptureBuilding { .. }
                        | Command::ToggleRepair { .. }
                ),
                "command is outside the map observation's ordinary order coverage"
            );
            previous = command.issue_after_step;
        }
        let owners = self.observe_owners();
        ensure!(
            owners.len() <= MAX_OBSERVED_OWNERS,
            "too many observed owners"
        );
        let mut unique_owners = BTreeSet::new();
        for owner in owners {
            ensure!(
                !owner.is_empty() && unique_owners.insert(owner),
                "empty or duplicate observed owner"
            );
        }
        let cells = self.terrain_cells();
        ensure!(
            cells.len() <= MAX_TERRAIN_CELLS,
            "too many observed terrain cells"
        );
        ensure!(
            cells.iter().collect::<BTreeSet<_>>().len() == cells.len(),
            "duplicate observed terrain cell"
        );
        ensure!(
            matches!(
                crate::match_bootstrap::classify_startup_session(&self.launch),
                StartupSessionClassification::AcceptedExplicitFixedBattle(_)
            ),
            "map observation requires an explicit fixed Battle launch"
        );
        ensure!(
            self.launch.pre_fill_house_roster
                == PreFillHouseRoster::from_compact_skirmish(self.launch.opponents.len()),
            "map observation requires the ordinary one-human compact skirmish roster"
        );
        let mut starts = std::collections::BTreeSet::new();
        let mut colors = std::collections::BTreeSet::new();
        for (start, color) in std::iter::once((
            self.launch.local.start_position,
            self.launch.local.color_index,
        ))
        .chain(
            self.launch
                .opponents
                .iter()
                .map(|slot| (slot.start_position, slot.color_index)),
        ) {
            let LaunchStartPosition::Position(start) = start else {
                bail!("automatic start is unsupported")
            };
            ensure!(
                start < 8 && starts.insert(start),
                "invalid or duplicate explicit start"
            );
            ensure!(
                color < 8 && colors.insert(color),
                "invalid or duplicate house color"
            );
        }
        Ok(())
    }

    fn commands(&self) -> &[MapScheduledCommand] {
        self.commands.as_deref().unwrap_or_default()
    }

    fn observe_owners(&self) -> &[String] {
        self.observe_owners.as_deref().unwrap_or_default()
    }

    fn terrain_cells(&self) -> &[[u16; 2]] {
        self.terrain_cells.as_deref().unwrap_or_default()
    }
}

#[derive(Default)]
pub(super) struct MapObservation {
    pub(super) initial: Option<Value>,
    inputs: Option<Value>,
    loaded_session: Option<Value>,
    rule_types: Vec<Value>,
    draws: Vec<MapDrawTime>,
    commands: Vec<MapCommandReceipt>,
    frames: Vec<MapFrameObservation>,
    observed_ids: BTreeSet<u64>,
    sample_count: usize,
}

#[derive(Debug, Serialize)]
struct MapCommandReceipt {
    ordinal: usize,
    issue_after_step: u32,
    issued_simulation_tick: u64,
    envelope_execute_tick: u64,
    owner: String,
    payload: Command,
}

#[derive(Debug, Serialize)]
struct MapFrameObservation {
    completed_steps: u64,
    simulation_tick: u64,
    binary_frame: u32,
    total_simulation_ms: u64,
    actors: Vec<Value>,
    missing_actor_ids: Vec<u64>,
    terrain: Vec<Value>,
}

#[derive(Debug, Serialize)]
struct MapDrawTime {
    completed_steps: u64,
    radar_ms: u64,
    tooltip_ms: u64,
    message_ms: u64,
}

impl MapObservation {
    fn pending_command<'a>(
        &self,
        profile: &'a MapCaptureProfile,
        completed_steps: u64,
    ) -> Result<Option<&'a MapScheduledCommand>> {
        let Some(command) = profile.commands().get(self.commands.len()) else {
            return Ok(None);
        };
        ensure!(
            u64::from(command.issue_after_step) >= completed_steps,
            "scheduled command missed its issue step"
        );
        Ok((u64::from(command.issue_after_step) == completed_steps).then_some(command))
    }

    fn observe_frame(&mut self, frame: MapFrameObservation, ids: BTreeSet<u64>) -> Result<()> {
        ensure!(
            frame.completed_steps == self.frames.len() as u64
                && frame.simulation_tick == frame.completed_steps
                && u64::from(frame.binary_frame) == frame.completed_steps,
            "actor observation skipped or repeated a committed frame"
        );
        let count = self
            .sample_count
            .checked_add(frame.actors.len())
            .and_then(|count| {
                frame.actors.iter().try_fold(count, |count, actor| {
                    count.checked_add(
                        actor["building"]["animation_slots"]
                            .as_array()
                            .map_or(0, Vec::len),
                    )
                })
            })
            .and_then(|count| count.checked_add(frame.missing_actor_ids.len()))
            .and_then(|count| count.checked_add(frame.terrain.len()))
            .context("actor observation sample count overflow")?;
        ensure!(
            count <= MAX_OBSERVATION_SAMPLES,
            "observation exceeds sample budget"
        );
        self.sample_count = count;
        self.observed_ids = ids;
        self.frames.push(frame);
        Ok(())
    }

    fn observe_draw(
        &mut self,
        requested: u32,
        completed_steps: u64,
        times: GameRenderTimes,
    ) -> Result<()> {
        ensure!(self.initial.is_some(), "map draw precedes accepted L0");
        ensure!(
            self.draws.len() < requested.max(1) as usize,
            "extra map draw"
        );
        let expected_step = if requested == 0 {
            0
        } else {
            self.draws.len() as u64 + 1
        };
        ensure!(
            completed_steps == expected_step,
            "map draw skipped or repeated a committed step"
        );
        let expected_ms = expected_step
            .checked_mul(MAP_PRESENTATION_INTERVAL_MS)
            .context("map presentation time overflow")?;
        let message_ms = times
            .message_ms
            .context("map draw omitted message expiry update")?;
        ensure!(
            times.radar_ms == expected_ms
                && times.tooltip_ms == expected_ms
                && message_ms == expected_ms,
            "map draw consumed inconsistent presentation clocks"
        );
        self.draws.push(MapDrawTime {
            completed_steps,
            radar_ms: times.radar_ms,
            tooltip_ms: times.tooltip_ms,
            message_ms,
        });
        Ok(())
    }
}

impl TacticalCaptureSession {
    fn map_state(&self) -> Result<&MapObservation> {
        match &self.controller {
            CaptureController::Map(map) => Ok(map),
            _ => bail!("map controller is absent"),
        }
    }

    fn map_state_mut(&mut self) -> Result<&mut MapObservation> {
        match &mut self.controller {
            CaptureController::Map(map) => Ok(map),
            _ => bail!("map controller is absent"),
        }
    }

    pub(super) fn prepare_map_observation(&mut self, state: &mut AppState) -> Result<()> {
        let profile = &self
            .request
            .map_profile()
            .context("map profile missing")?
            .value;
        ensure!(
            state.renderer.gpu.config.width == profile.width
                && state.renderer.gpu.config.height == profile.height,
            "map observation surface extent differs from request"
        );
        ensure!(
            matches!(
                state.renderer.gpu.config.format,
                wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb
            ) && state
                .renderer
                .gpu
                .config
                .usage
                .contains(wgpu::TextureUsages::COPY_SRC),
            "map observation requires BGRA8 COPY_SRC surface"
        );
        ensure!(
            state.match_state.configured_input_delay_ticks == u64::from(profile.input_delay_ticks),
            "configured input delay differs from observation profile"
        );
        let config = state
            .platform
            .game_config
            .as_ref()
            .context("map observation requires config.toml")?;
        ensure!(
            !config.graphics.upscale && state.renderer.upscale_pass.is_none(),
            "map observation requires native-resolution rendering (upscale=false)"
        );
        let cwd = std::env::current_dir()?;
        let inputs = json!({
            "config": artifact(&cwd.join("config.toml"), "config.toml")?,
            "executable": artifact(&std::env::current_exe()?, "capture executable")?,
        });
        self.map_state_mut()?.inputs = Some(inputs);
        state.diag.use_map_presentation_clock()?;
        state.match_state.input.cursor_x = 0.0;
        state.match_state.input.cursor_y = 0.0;
        let now_ms =
            crate::app::match_runtime::sim_tick::monotonic_frame_pacer_ms(state, Instant::now());
        state.platform.frame_pacer.reanchor(now_ms);
        Ok(())
    }

    pub(super) fn drive_map_observation(&mut self, state: &mut AppState) -> Result<()> {
        if self.map_state()?.initial.is_none() {
            self.failure_stage = "rust-l0".to_owned();
            let profile = &self
                .request
                .map_profile()
                .context("map profile missing")?
                .value;
            let startup = state
                .match_state
                .startup
                .startup()
                .context("accepted startup absent")?;
            let receipt = state
                .match_state
                .startup
                .receipt()
                .context("Rust L0 receipt absent")?;
            ensure!(
                crate::match_bootstrap::accepted_tick_is_admitted(Some(startup), Some(receipt)),
                "loaded startup does not admit ticks"
            );
            ensure!(
                receipt.session.launch_session() == &profile.launch
                    && receipt.seed == profile.seed
                    && receipt.seed_source == MatchSeedSource::Controlled
                    && receipt.seed_authority_certifying
                    && receipt.tick == 0
                    && receipt.total_sim_ms == 0
                    && receipt.binary_frame == 0,
                "Rust L0 differs from requested fixed Battle launch"
            );
            ensure!(
                state.match_state.local_player_owner() == Some(profile.launch.player_name.as_str()),
                "loaded local owner differs from requested launch"
            );
            validate_loaded_resources(state)?;
            let sim = &state
                .match_state
                .sim_runtime
                .as_ref()
                .context("simulation absent")?
                .simulation;
            ensure!(
                sim.session.tick == 0
                    && sim.session.total_sim_ms == 0
                    && sim.session.binary_frame == 0
                    && sim.session.seed == u64::from(profile.seed)
                    && sim.input_delay_ticks == u64::from(profile.input_delay_ticks),
                "live simulation is not the requested tick-zero state"
            );
            validate_game_options(sim, &profile.launch)?;
            let slots: Vec<_> = sim
                .session
                .start_slot_houses
                .iter()
                .map(|(slot, house_id)| {
                    let house = sim.houses.get(house_id);
                    json!({"slot": slot, "house": sim.interner.resolve(*house_id),
                    "waypoint": sim.session.mp_start_waypoints.get(slot),
                    "country": house.and_then(|h| h.country).map(|c| sim.interner.resolve(c)),
                    "human": house.map(|h| h.is_human), "difficulty": house.map(|h| h.difficulty)})
                })
                .collect();
            for start in std::iter::once(profile.launch.local.start_position).chain(
                profile
                    .launch
                    .opponents
                    .iter()
                    .map(|slot| slot.start_position),
            ) {
                let LaunchStartPosition::Position(index) = start else {
                    bail!("unresolved start")
                };
                ensure!(
                    sim.session
                        .mp_start_waypoints
                        .contains_key(&u32::from(index))
                        && sim
                            .session
                            .start_slot_houses
                            .contains_key(&u32::from(index)),
                    "requested start {index} was not installed in the loaded map"
                );
            }
            let loaded_session = json!({"map_name": sim.session.map_name, "theater": sim.session.theater,
                "options": sim.session.game_options, "start_slots": slots, "map_waypoints": sim.session.mp_start_waypoints});
            // Simulation::intern_rule_type_ids owns these handles. Observation
            // reads the rules-owned names and installed handles without adding
            // an interner entry or translating a production command.
            let rules = state.rules().context("loaded rules absent")?;
            let mut rule_types = Vec::new();
            for (category, names) in [
                ("Infantry", &rules.infantry_ids),
                ("Unit", &rules.vehicle_ids),
                ("Aircraft", &rules.aircraft_ids),
                ("Structure", &rules.building_ids),
            ] {
                for name in names {
                    let handle = sim
                        .interner
                        .get(name)
                        .with_context(|| format!("rule type {name:?} was not preinterned"))?;
                    rule_types.push(json!({"type_id": name, "interned_id": handle.index(),
                        "category": category}));
                }
            }
            // Owner strings must name real loaded Houses before the ordinary
            // input owner is allowed to intern a command receiver.
            for owner in profile.observe_owners().iter().map(String::as_str).chain(
                profile
                    .commands()
                    .iter()
                    .map(|command| command.owner.as_str()),
            ) {
                ensure!(
                    sim.interner
                        .get(owner)
                        .is_some_and(|id| sim.houses.contains_key(&id)),
                    "observation owner {owner:?} is absent from the loaded Houses"
                );
            }
            let camera_cell = profile.camera_cell;
            let source = state
                .match_state
                .loaded_map_source
                .as_ref()
                .context("loaded source absent")?;
            ensure!(
                matches!(
                    source,
                    crate::map::source::LoadedMapSource::Loose { .. }
                        | crate::map::source::LoadedMapSource::Mix { .. }
                ),
                "map observation requires consumed loose or MIX map bytes"
            );
            self.map_source_evidence = Some(serde_json::to_value(source)?);
            self.map_state_mut()?.initial = Some(self.map_fingerprint(state)?);
            self.map_state_mut()?.loaded_session = Some(loaded_session);
            self.map_state_mut()?.rule_types = rule_types;
            if let Some([rx, ry]) = camera_cell {
                crate::app::input::camera::center_camera_on_cell(state, rx, ry);
            }
            self.record_map_frame(state)?;
            self.post_l0_started_at = Some(Instant::now());
            self.failure_stage = "exact-steps".to_owned();
        }
        let requested = self
            .request
            .map_profile()
            .context("map profile missing")?
            .value
            .ticks as usize;
        ensure!(
            self.exact_step_receipts.len() <= requested,
            "map observation exceeded tick budget"
        );
        ensure!(
            self.map_state()?.draws.len() == self.exact_step_receipts.len(),
            "previous map step has no completed draw"
        );
        if self.exact_step_receipts.len() < requested {
            self.issue_map_commands(state)?;
            self.advance_exact_step(state)?;
            self.record_map_frame(state)?;
        }
        if self.exact_step_receipts.len() == requested {
            self.capture_requested = true;
            self.failure_stage = "final-render".to_owned();
        }
        Ok(())
    }

    fn issue_map_commands(&mut self, state: &mut AppState) -> Result<()> {
        let completed_steps = self.exact_step_receipts.len() as u64;
        loop {
            let profile = &self
                .request
                .map_profile()
                .context("map profile missing")?
                .value;
            let Some(command) = self
                .map_state()?
                .pending_command(profile, completed_steps)?
                .cloned()
            else {
                return Ok(());
            };
            let issued_tick = state
                .match_state
                .sim_runtime
                .as_ref()
                .context("command simulation absent")?
                .simulation
                .session
                .tick;
            ensure!(
                issued_tick == completed_steps,
                "command issue is outside exact-step boundary"
            );
            // The sole ordinary input producer owns envelope encoding, stamping
            // and queuing. This diagnostic does not add the input-delay setting
            // or bypass ordinary House/actor admission in the next frame.
            let execute_tick = crate::app::input::commands::try_schedule_command(
                state,
                &command.owner,
                command.payload.clone(),
            )
            .context("ordinary command producer refused scheduled input")?;
            ensure!(
                execute_tick == issued_tick,
                "ordinary producer changed the issue stamp"
            );
            let map = self.map_state_mut()?;
            map.commands.push(MapCommandReceipt {
                ordinal: map.commands.len(),
                issue_after_step: command.issue_after_step,
                issued_simulation_tick: issued_tick,
                envelope_execute_tick: execute_tick,
                owner: command.owner,
                payload: command.payload,
            });
        }
    }

    fn record_map_frame(&mut self, state: &AppState) -> Result<()> {
        let profile = &self
            .request
            .map_profile()
            .context("map profile missing")?
            .value;
        let runtime = state
            .match_state
            .sim_runtime
            .as_ref()
            .context("observation simulation absent")?;
        let sim = &runtime.simulation;
        let grid = runtime.view().resolved_terrain();
        // This derived diagnostic index retains every observed stable handle.
        // Captured objects continue to report their actual new owner; destroyed
        // objects retain a missing row rather than disappearing from history.
        let mut ids = self.map_state()?.observed_ids.clone();
        let mut actors = Vec::new();
        if !profile.observe_owners().is_empty() {
            for (id, entity) in sim.entities().iter_sorted() {
                let owner = sim.interner.resolve(entity.owner());
                if !ids.contains(&id)
                    && !profile.observe_owners().iter().any(|watch| watch == owner)
                {
                    continue;
                }
                ids.insert(id);
                let coord =
                    crate::sim::movement::ground_pose::position_world_coord(&entity.position);
                let timer = entity.mission.dispatch_timer();
                let foot = if entity.category != crate::map::entities::EntityCategory::Structure {
                    let (navigation_leptons, navigation_unavailable) =
                        match sim.foot_navigation_coordinate(id) {
                            Ok(coord) => (Some([coord.x, coord.y, coord.z]), None),
                            Err(cause) => (None, Some(cause)),
                        };
                    Some(json!({
                        "retarget_after_stop_688": entity.foot_retarget_after_stop(),
                        "firing_sequence_latch_68d": entity.mission_leaf.foot_firing_sequence_latch(),
                        "infantry_doing": entity.mission_leaf.as_infantry().map(|leaf| leaf.doing()),
                        "navigation_leptons": navigation_leptons,
                        "navigation_unavailable": navigation_unavailable,
                    }))
                } else {
                    None
                };
                let unit = entity.mission_leaf.as_unit().map(|leaf| {
                    let animation = entity.deploy_anim().map(|anim_id| {
                        json!({"stable_id": anim_id, "live": sim.anim(anim_id).map(|anim| {
                            json!({"type_id": sim.interner.resolve(anim.type_id),
                                "frame": anim.runtime.current_frame, "owner_entity": anim.owner_entity})
                        })})
                    });
                    json!({"deployed_6e0": leaf.deployed(),
                        "deploying_6e1": leaf.deploy_begin_active(),
                        "undeploying_6e2": leaf.deploy_reverse_active(),
                        "landing_for_deploy_134": entity.landing_for_deploy(),
                        "deploy_anim_130": animation, "stage_f8": entity.native_stage().value(),
                        "body_counter_538": entity.body_frame_counter})
                });
                let building = if entity.category == crate::map::entities::EntityCategory::Structure
                {
                    let slots: Vec<_> = entity
                        .building_anim_slots
                        .iter()
                        .enumerate()
                        .filter_map(|(slot, anim_id)| {
                            anim_id.map(|anim_id| {
                                let animation = sim.anim(anim_id).map(|anim| {
                                    json!({"stable_id": anim.stable_id,
                                        "native_id": anim.native_unique_id,
                                        "type_id": sim.interner.resolve(anim.type_id),
                                        "interned_type_id": anim.type_id.index(),
                                        "physical_leptons": [anim.world_coord.x, anim.world_coord.y, anim.world_coord.z],
                                        "in_logic_vector": anim.in_logic_vector,
                                        "owner_entity": anim.owner_entity,
                                        "building_slot": anim.building_slot,
                                        "runtime": anim.runtime})
                                });
                                json!({"slot": slot, "anim_id": anim_id, "animation": animation})
                            })
                        })
                        .collect();
                    Some(json!({"body_state": entity.building_body_state(),
                        "queued_body_state": entity.queued_building_body_state(),
                        "construction_control": entity.building_construction_control(),
                        "stage": entity.native_stage(), "ready_latch": entity.building_ready_latch(),
                        "actually_placed": entity.building_actually_placed,
                        "last_operational": entity.building_last_operational,
                        "animation_slots": slots}))
                } else {
                    None
                };
                actors.push(json!({
                    "stable_id": id, "owner": owner,
                    "type_id": sim.interner.resolve(entity.type_ref()), "category": entity.category,
                    "cell": [entity.position.rx, entity.position.ry],
                    "physical_leptons": [coord.x, coord.y, coord.z], "on_bridge": entity.on_bridge,
                    "health": entity.health.current, "active": entity.is_active(),
                    "in_limbo": entity.lifecycle.in_limbo, "dying": entity.dying,
                    "mission": {"current": entity.mission.current().raw(),
                        "queued": entity.mission.queued().raw(), "suspended": entity.mission.suspended().raw(),
                        "effective": entity.mission.effective().raw(), "handler_state": entity.mission.handler_state(),
                        "start_frame": entity.mission.mission_start_frame(), "ai_counter": entity.mission.ai_counter(),
                        "dispatch_timer": {"start_frame": timer.start_frame(), "delay": timer.delay()}},
                    "target": entity.attack_target.as_ref().map(|target| target.target),
                    "archive": entity.archive_target(), "nav": entity.navigation.nav_com, "foot": foot,
                    "building": building, "unit": unit,
                }));
            }
        }
        let missing_actor_ids = ids
            .iter()
            .copied()
            .filter(|id| !sim.entities().contains(*id))
            .collect();
        let terrain = profile.terrain_cells().iter().map(|&[rx, ry]| {
            // Immutable real-cell indexing only. A diagnostic lookup must not
            // stamp canonical Dummy or evaluate gameplay height/zone queries.
            let cell = grid.and_then(|grid| grid.cell(rx, ry));
            let at = (rx, ry);
            let overlay = sim.overlay_grid.as_ref().map(|grid| grid.cell(rx, ry));
            let terrain_object = sim.production.terrain_object_cells.get(&at)
                .and_then(|id| sim.production.terrain_objects.get(id));
            let animation = sim.production.terrain_animations.get(&at);
            json!({"cell": [rx, ry], "allocated": cell.is_some(),
                "overlay": overlay.map(|overlay| json!({"id": overlay.overlay_id, "density": overlay.overlay_data})),
                "terrain_object": terrain_object.map(|object| json!({"name": sim.interner.resolve(object.type_ref),
                    "frame": animation.map(|animation| animation.current_frame()),
                    "active": animation.map(|animation| animation.is_active())})),
                "final_tile_index": cell.map(|cell| cell.final_tile_index),
                "final_sub_tile": cell.map(|cell| cell.final_sub_tile),
                "presentation_tile": cell.and_then(|cell| grid.map(|grid| grid.presentation_tile(cell))),
                "level": cell.map(|cell| cell.level), "slope": cell.map(|cell| cell.slope_type),
                "raw_bridge_flags": cell.map(|cell| cell.bridge_facts.raw_flags),
                "bridge_state": cell.map(|cell| cell.bridge_facts.state_byte),
                "has_deck": cell.map(|cell| cell.has_bridge_deck), "deck_level": cell.map(|cell| cell.bridge_deck_level),
                "walkable": cell.map(|cell| cell.bridge_walkable), "transition": cell.map(|cell| cell.bridge_transition)})
        }).collect();
        let frame = MapFrameObservation {
            completed_steps: self.exact_step_receipts.len() as u64,
            simulation_tick: sim.session.tick,
            binary_frame: sim.session.binary_frame,
            total_simulation_ms: sim.session.total_sim_ms,
            actors,
            missing_actor_ids,
            terrain,
        };
        self.map_state_mut()?.observe_frame(frame, ids)
    }

    pub(super) fn observe_map_draw(
        &mut self,
        state: &AppState,
        output: &GameRenderOutput,
    ) -> Result<()> {
        let requested = self
            .request
            .map_profile()
            .context("map profile missing")?
            .value
            .ticks;
        let sim = &state
            .match_state
            .sim_runtime
            .as_ref()
            .context("map simulation absent")?
            .simulation;
        ensure!(
            sim.session.tick == self.exact_step_receipts.len() as u64
                && u64::from(sim.session.binary_frame) == sim.session.tick,
            "map draw differs from committed exact-step receipts"
        );
        ensure!(
            state.diagnostic_presentation_ms() == Some(output.times.radar_ms),
            "map diagnostic presentation policy is not active"
        );
        self.map_state_mut()?
            .observe_draw(requested, sim.session.tick, output.times)
    }

    pub(super) fn map_fingerprint(&self, state: &AppState) -> Result<Value> {
        let sim = &state
            .match_state
            .sim_runtime
            .as_ref()
            .context("simulation absent")?
            .simulation;
        Ok(json!(FinalFingerprint {
            simulation_tick: sim.session.tick,
            total_simulation_ms: sim.session.total_sim_ms,
            binary_frame: sim.session.binary_frame,
            deterministic_state_hash: sim.state_hash()
        }))
    }

    pub(super) fn map_render_readiness(
        &self,
        state: &AppState,
        output: &GameRenderOutput,
    ) -> Result<(bool, Value)> {
        validate_loaded_resources(state)?;
        let unit_atlas = state
            .match_state
            .match_presentation
            .unit_atlas
            .as_ref()
            .context("unit atlas absent at final map render")?
            .statistics()?;
        let ready = output.sidebar_view.is_some()
            && !state.match_state.paused()
            && !state.match_state.match_presentation.show_save_load_panel
            && !state.main_menu_dialog_open()
            && !state.diag.debug_show_pathgrid
            && !state.diag.debug_unit_inspector
            && !state.diag.debug_show_cell_grid
            && !state.diag.debug_show_heightmap
            && !state.match_state.match_presentation.show_hotkey_help
            && self.focus_violations == 0
            && self.input_violations == 0;
        let static_default_cursor =
            crate::app::presentation::ui_overlays::static_default_cursor(state);
        let camera_input_idle = crate::app::input::camera::camera_input_idle(state);
        ensure!(
            ready && static_default_cursor && camera_input_idle,
            "map observation requires every draw ready with a static cursor and idle camera input"
        );
        Ok((
            ready,
            json!({"ready": ready, "sidebar_view_present": output.sidebar_view.is_some(),
                "instance_counts": super::super::evidence::RenderInstanceCountEvidence::from_counts(output.instance_counts)?,
                "internal_extent": [state.render_width(), state.render_height()],
                "surface_extent": [state.renderer.gpu.config.width, state.renderer.gpu.config.height],
                "ui_scale": state.match_state.match_presentation.ui_scale,
                "gpu": super::super::evidence::GpuAdapterEvidence::from_observation(state.renderer.gpu.capture_adapter_observation()),
                "unit_atlas": unit_atlas,
                "neutral_input": {"static_default_cursor": static_default_cursor, "camera_input_idle": camera_input_idle},
                "camera": {"requested_cell": self.request.map_profile().context("map profile missing")?.value.camera_cell,
                    "top_left": [state.match_state.input.camera_x, state.match_state.input.camera_y],
                    "zoom": state.match_state.input.zoom_level},
            }),
        ))
    }

    pub(super) fn publish_map_observation(
        &self,
        state: &AppState,
        format: wgpu::TextureFormat,
        pixels: &[u8],
    ) -> Result<()> {
        let profile = self.request.map_profile().context("map profile missing")?;
        ensure!(
            self.exact_step_receipts.len() == profile.value.ticks as usize,
            "incomplete exact-step sequence"
        );
        let frame = FrameArtifact::from_bgra(
            self.request.width(),
            self.request.height(),
            format!("{format:?}"),
            pixels,
        )?;
        let map = self.map_state()?;
        ensure!(
            map.draws.len() == profile.value.ticks.max(1) as usize,
            "incomplete map draw schedule"
        );
        ensure!(
            map.frames.len() == profile.value.ticks as usize + 1
                && map.commands.len() == profile.value.commands().len(),
            "incomplete actor/command observation transcript"
        );
        let mut render = self
            .last_render_evidence
            .clone()
            .context("map render evidence absent")?;
        // Serialize the actual transcript only once, avoiding quadratic work
        // across the up-to-100000-step capture route.
        render["presentation_clock"] = json!({"policy": MAP_PRESENTATION_CLOCK_POLICY,
            "origin_ms": 0, "interval_ms": MAP_PRESENTATION_INTERVAL_MS, "draws": map.draws});
        let manifest = json!({
            "schema_version": CHILD_SCHEMA, "status": "COMPLETE",
            "profile": {"sha256": profile.sha256, "request": profile.value},
            "contract": {"sha256": self.request.sealed_contract().sha256},
            "inputs": self.map_state()?.inputs, "map_source": self.map_source_evidence,
            "startup": self.startup_evidence, "initial": self.map_state()?.initial,
            "loaded_session": self.map_state()?.loaded_session,
            "final": self.map_fingerprint(state)?, "exact_step_count": self.exact_step_receipts.len(),
            "first_exact_step": self.exact_step_receipts.first(), "last_exact_step": self.exact_step_receipts.last(),
            "frame": frame, "render": render,
            "observations": {"policy": OBSERVATION_POLICY, "owners": profile.value.observe_owners(),
                "rule_types": map.rule_types, "commands": map.commands, "frames": map.frames},
            "lifecycle": {"window_hidden": state.platform.window.is_visible() == Some(false),
                "window_focused": state.platform.window.has_focus(), "focus_violations": self.focus_violations,
                "input_violations": self.input_violations},
            "native_comparator": "NONE", "parity_certification": "NONE",
            "evidence_limitations": ["Production loading, exact stepping and GPU readback only; no native pixel or gameplay equivalence is established.",
                "Radar and timed HUD presentation consume the recorded diagnostic exact-step clock; ordinary gameplay clocks are unchanged. Audio, menus, animated input and scenario exit are outside this comparison."],
        });
        ensure!(
            serde_json::to_vec_pretty(&manifest)?.len() < MAX_RECEIPT_BYTES,
            "map observation manifest exceeds the 128 MiB receipt budget"
        );
        publish_transaction(
            self.request.output_dir(),
            &manifest,
            Some(pixels),
            PublishFault::None,
        )
    }

    pub(super) fn publish_map_failure(&self, error: &str) -> Result<()> {
        let profile = self.request.map_profile().context("map profile missing")?;
        let map = self.map_state()?;
        let manifest = json!({"schema_version": CHILD_SCHEMA, "status": "FAILED",
            "profile": {"sha256": profile.sha256, "request": profile.value},
            "contract": {"sha256": self.request.sealed_contract().sha256},
            "failure": {"stage": self.failure_stage, "message": error}, "frame": null,
            "exact_step_count": self.exact_step_receipts.len(), "map_source": self.map_source_evidence,
            "observations": {"policy": OBSERVATION_POLICY, "owners": profile.value.observe_owners(),
                "rule_types": map.rule_types, "commands": map.commands, "frames": map.frames},
            "native_comparator": "NONE", "parity_certification": "NONE"});
        ensure!(
            serde_json::to_vec_pretty(&manifest)?.len() < MAX_RECEIPT_BYTES,
            "map failure manifest exceeds the 128 MiB receipt budget"
        );
        publish_transaction(
            self.request.output_dir(),
            &manifest,
            None,
            PublishFault::None,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn initialized_map() -> MapObservation {
        MapObservation {
            initial: Some(json!({"tick": 0})),
            ..Default::default()
        }
    }

    fn times(ms: u64) -> GameRenderTimes {
        GameRenderTimes {
            radar_ms: ms,
            tooltip_ms: ms,
            message_ms: Some(ms),
        }
    }

    #[test]
    fn capture_retains_only_actual_draws_in_committed_order() {
        let mut zero = initialized_map();
        zero.observe_draw(0, 0, times(0)).unwrap();
        assert!(zero.observe_draw(0, 0, times(0)).is_err());
        let mut map = initialized_map();
        assert!(map.observe_draw(3, 0, times(0)).is_err());
        map.observe_draw(3, 1, times(22)).unwrap();
        assert!(map.observe_draw(3, 1, times(22)).is_err());
        assert!(map.observe_draw(3, 3, times(66)).is_err());
        map.observe_draw(3, 2, times(44)).unwrap();
        map.observe_draw(3, 3, times(66)).unwrap();
        let evidence = serde_json::to_value(&map.draws).unwrap();
        assert_eq!(evidence[0]["completed_steps"], 1);
        assert_eq!(evidence[2]["radar_ms"], 66);
        assert_eq!(map.draws.len(), 3);
        assert!(map.observe_draw(3, 4, times(88)).is_err());
    }

    #[test]
    fn capture_rejects_missing_hud_update_or_any_wall_clock_sample() {
        assert!(
            MapObservation::default()
                .observe_draw(1, 1, times(22))
                .is_err()
        );
        let mut map = initialized_map();
        for sample in [
            GameRenderTimes {
                radar_ms: 9_876,
                ..times(22)
            },
            GameRenderTimes {
                tooltip_ms: 9_876,
                ..times(22)
            },
            GameRenderTimes {
                message_ms: Some(0),
                ..times(22)
            },
            GameRenderTimes {
                message_ms: None,
                ..times(22)
            },
        ] {
            assert!(map.observe_draw(1, 1, sample).is_err());
            assert!(
                map.draws.is_empty(),
                "rejected observation must not advance schedule"
            );
        }
        map.observe_draw(1, 1, times(22)).unwrap();
    }

    fn example() -> MapCaptureProfile {
        serde_json::from_str(include_str!(
            "../../../../tools/map_observation.example.json"
        ))
        .unwrap()
    }

    #[test]
    fn example_preserves_existing_accepted_launch_and_allows_chosen_map() {
        let mut profile = example();
        profile.validate().unwrap();
        let radar: super::super::super::profile::TacticalCaptureProfile =
            serde_json::from_str(include_str!(
                "../../../../tools/tactical_certification/profiles/soviet-radar-online-v2.json"
            ))
            .unwrap();
        assert_eq!(profile.launch, radar.launch_session());
        profile.launch.selected_map_file = Some("mp01t4.map".to_owned());
        profile.validate().unwrap();
        profile.ticks = 0;
        profile.validate().unwrap();
    }

    #[test]
    fn unresolved_launch_duplicate_slots_and_inconsistent_roster_fail() {
        let mut profile = example();
        profile.launch.local.start_position = LaunchStartPosition::Auto;
        assert!(profile.validate().is_err());
        let mut profile = example();
        profile.launch.opponents[0].color_index = profile.launch.local.color_index;
        assert!(profile.validate().is_err());
        let mut profile = example();
        profile.launch.opponents.clear();
        assert!(profile.validate().is_err());
    }

    #[test]
    fn invalid_budgets_and_unknown_request_fields_fail() {
        let mut profile = example();
        profile.timeout_seconds = 0;
        assert!(profile.validate().is_err());
        profile.timeout_seconds = 180;
        profile.ticks = 100_001;
        assert!(profile.validate().is_err());
        let mut json = serde_json::to_value(example()).unwrap();
        json["launch"]["ignored_option"] = json!(true);
        assert!(serde_json::from_value::<MapCaptureProfile>(json).is_err());
    }

    #[test]
    fn versioned_extension_fields_preserve_presence_and_reject_null_or_ignored_arguments() {
        let original: Value = serde_json::from_str(include_str!(
            "../../../../tools/map_observation.example.json"
        ))
        .unwrap();
        assert_eq!(serde_json::to_value(example()).unwrap(), original);
        let mut legacy = example();
        legacy.commands = Some(Vec::new());
        assert!(legacy.validate().is_err());
        let mut modern = original;
        modern["schema_version"] = json!(PROFILE_V2);
        modern["commands"] = json!([{"issue_after_step": 0, "owner": "Computer1",
            "payload": {"DeployMcv": {"entity_id": 1}}}]);
        modern["observe_owners"] = json!(["Computer1"]);
        let profile: MapCaptureProfile = serde_json::from_value(modern.clone()).unwrap();
        profile.validate().unwrap();
        assert_eq!(serde_json::to_value(profile).unwrap(), modern);
        for key in ["commands", "observe_owners", "camera_cell", "terrain_cells"] {
            let mut invalid = modern.clone();
            invalid[key] = Value::Null;
            assert!(
                serde_json::from_value::<MapCaptureProfile>(invalid).is_err(),
                "{key}"
            );
        }
        for (key, value) in [("ignored", json!(true)), ("entity_id", json!(1.0))] {
            let mut invalid = modern.clone();
            invalid["commands"][0]["payload"]["DeployMcv"][key] = value;
            assert!(
                serde_json::from_value::<MapCaptureProfile>(invalid).is_err(),
                "{key}"
            );
        }
    }

    #[test]
    fn command_schedule_keeps_equal_step_input_order_and_refuses_missed_or_final_steps() {
        let mut profile = example();
        profile.schema_version = PROFILE_V2.to_owned();
        profile.ticks = 3;
        profile.commands = Some(vec![
            MapScheduledCommand {
                issue_after_step: 0,
                owner: "Computer1".to_owned(),
                payload: Command::Stop { entity_id: 1 },
            },
            MapScheduledCommand {
                issue_after_step: 0,
                owner: "Computer1".to_owned(),
                payload: Command::DeployMcv { entity_id: 2 },
            },
            MapScheduledCommand {
                issue_after_step: 2,
                owner: "Computer1".to_owned(),
                payload: Command::ForceAttackCell {
                    attacker_id: 3,
                    target_rx: 87,
                    target_ry: 53,
                },
            },
        ]);
        profile.validate().unwrap();
        let mut map = initialized_map();
        assert!(map.pending_command(&profile, 1).is_err());
        for ordinal in 0..2 {
            let command = map.pending_command(&profile, 0).unwrap().unwrap().clone();
            assert_eq!(command.payload, profile.commands()[ordinal].payload);
            map.commands.push(MapCommandReceipt {
                ordinal,
                issue_after_step: 0,
                issued_simulation_tick: 0,
                envelope_execute_tick: 0,
                owner: command.owner,
                payload: command.payload,
            });
        }
        assert!(map.pending_command(&profile, 1).unwrap().is_none());
        assert_eq!(
            map.pending_command(&profile, 2)
                .unwrap()
                .unwrap()
                .issue_after_step,
            2
        );
        assert!(map.pending_command(&profile, 3).is_err());
        profile.commands.as_mut().unwrap()[2].issue_after_step = 3;
        assert!(profile.validate().is_err());
        profile.commands.as_mut().unwrap()[0].issue_after_step = 1;
        assert!(profile.validate().is_err());
    }

    #[test]
    fn production_profile_reuses_typed_command_serde_without_name_translation() {
        let mut value = serde_json::to_value(example()).unwrap();
        value["schema_version"] = json!(PROFILE_V2);
        value["commands"] = json!([
            {"issue_after_step": 0, "owner": "Computer1",
                "payload": {"QueueProduction": {"type_id": 41}}},
            {"issue_after_step": 1, "owner": "Computer1",
                "payload": {"PlaceReadyBuilding": {"type_id": 41, "rx": 87, "ry": 53}}},
            {"issue_after_step": 2, "owner": "Computer1",
                "payload": {"CaptureBuilding": {"engineer_id": 7, "target_building_id": 9}}},
            {"issue_after_step": 2, "owner": "Computer1",
                "payload": {"ToggleRepair": {"entity_id": 9}}},
        ]);
        let profile: MapCaptureProfile = serde_json::from_value(value.clone()).unwrap();
        profile.validate().unwrap();
        assert_eq!(serde_json::to_value(profile).unwrap(), value);
        for invalid_id in [json!("GAPOWR"), json!(true), json!(1.5), json!(u64::MAX)] {
            let mut invalid = value.clone();
            invalid["commands"][0]["payload"]["QueueProduction"]["type_id"] = invalid_id;
            assert!(serde_json::from_value::<MapCaptureProfile>(invalid).is_err());
        }
        for invalid_id in [json!("ENGINEER"), json!(true), json!(1.5), json!(-1)] {
            let mut invalid = value.clone();
            invalid["commands"][2]["payload"]["CaptureBuilding"]["engineer_id"] = invalid_id;
            assert!(serde_json::from_value::<MapCaptureProfile>(invalid).is_err());
        }
        value["commands"][1]["payload"]["PlaceReadyBuilding"]["ignored"] = json!(true);
        assert!(serde_json::from_value::<MapCaptureProfile>(value).is_err());
    }

    #[test]
    fn observations_require_l0_then_every_committed_frame_and_bound_retained_samples() {
        let frame = |step, tick| MapFrameObservation {
            completed_steps: step,
            simulation_tick: tick,
            binary_frame: tick as u32,
            total_simulation_ms: tick * 22,
            actors: Vec::new(),
            missing_actor_ids: Vec::new(),
            terrain: Vec::new(),
        };
        let mut map = initialized_map();
        assert!(map.observe_frame(frame(1, 1), BTreeSet::new()).is_err());
        map.observe_frame(frame(0, 0), BTreeSet::new()).unwrap();
        assert!(map.observe_frame(frame(0, 0), BTreeSet::new()).is_err());
        assert!(map.observe_frame(frame(1, 2), BTreeSet::new()).is_err());
        map.observe_frame(frame(1, 1), BTreeSet::new()).unwrap();
        let mut large = frame(2, 2);
        large.terrain = vec![Value::Null; MAX_OBSERVATION_SAMPLES + 1];
        assert!(map.observe_frame(large, BTreeSet::new()).is_err());
        assert_eq!(map.frames.len(), 2);
        assert_eq!(map.sample_count, 0);
    }

    #[test]
    fn anytown_discovery_example_is_an_accepted_ordinary_allied_ai_launch() {
        let profile: MapCaptureProfile = serde_json::from_str(include_str!(
            "../../../../tools/map_observation.bridge-response.example.json"
        ))
        .unwrap();
        profile.validate().unwrap();
        assert_eq!(
            profile.launch.selected_map_file.as_deref(),
            Some("XMP03T4.MAP")
        );
        assert_eq!(profile.launch.opponents.len(), 2);
        assert_eq!(
            profile.commands().len(),
            0,
            "discovery must not invent stable actor IDs"
        );
        assert_eq!(profile.ticks, 0);
        assert_eq!(profile.observe_owners(), ["Computer1", "Computer2"]);
    }
}
