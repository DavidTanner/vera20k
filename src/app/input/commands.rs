//! Build and production commands — queuing builds, placing buildings, owner management.
//!
//! Split from the presentation render path. Part of the app layer — may depend on everything.

use std::collections::HashMap;

use crate::app::AppState;
use crate::map::entities::EntityCategory;
use crate::sim::command::{Command, CommandEnvelope, CommandRecord};
use crate::sim::intern::InternedId;
use crate::sim::production;

#[cfg(test)]
#[path = "ground_command_native_tests.rs"]
mod ground_command_native_tests;

/// Default owner name when no playable house is found.
const DEFAULT_OWNER: &str = "Americans";

/// Intern a string (owner name or type id) against the live simulation's
/// interner, returning its InternedId. Returns the default ID when there is
/// no simulation yet.
fn intern_in_sim(state: &mut AppState, s: &str) -> InternedId {
    state
        .match_state
        .sim_runtime
        .as_mut()
        .map(|rt| &mut rt.simulation)
        .map(|sim| sim.interner.intern(s))
        .unwrap_or_default()
}

/// Resolve the local owner and return the owned String.
///
/// Read-only: identity comes from the live scenario's saved current House
/// (or the sandbox heuristic when absent). Command issue never rewrites it —
/// the old per-action override write made "who am I" drift with whatever
/// the heuristic last returned, a lockstep hazard.
fn resolve_owner(state: &mut AppState) -> String {
    preferred_local_owner(state).unwrap_or_else(|| DEFAULT_OWNER.to_string())
}

pub(crate) fn queue_build_by_type(state: &mut AppState, type_id: &str) {
    let owner: String = resolve_owner(state);
    let type_interned = intern_in_sim(state, type_id);
    schedule_command(
        state,
        &owner,
        Command::QueueProduction {
            type_id: type_interned,
        },
    );
    log::info!(
        "Build command queued: owner={} type={} issue_frame=current",
        owner,
        type_id
    );
}

pub(crate) fn suspend_build(state: &mut AppState, category: production::ProductionCategory) {
    let owner: String = resolve_owner(state);
    schedule_command(state, &owner, Command::SuspendProduction { category });
    log::info!(
        "Build hold command queued: owner={} category={} issue_frame=current",
        owner,
        category.label()
    );
}

pub(crate) fn cycle_active_producer(
    state: &mut AppState,
    category: production::ProductionCategory,
) {
    let owner: String = resolve_owner(state);
    schedule_command(state, &owner, Command::CycleProducerFocus { category });
    log::info!(
        "Producer focus cycle queued: owner={} category={} issue_frame=current",
        owner,
        category.label()
    );
}

pub(crate) fn cancel_build_by_type(state: &mut AppState, type_id: &str, all: bool) {
    let owner: String = resolve_owner(state);
    let type_interned = intern_in_sim(state, type_id);
    schedule_command(
        state,
        &owner,
        Command::CancelProductionByType {
            type_id: type_interned,
            all,
        },
    );
    log::info!(
        "Build cancel-by-type queued: owner={} type={} all={} issue_frame=current",
        owner,
        type_id,
        all
    );
}

/// Stable id of the visible object selected by the tactical object picker.
fn visible_object_under_point(state: &AppState, world_x: f32, world_y: f32) -> Option<u64> {
    let sim = state
        .match_state
        .sim_runtime
        .as_ref()
        .map(|rt| &rt.simulation)?;
    let owner = preferred_local_owner(state)?;
    crate::app::input::entity_pick::hover_target_at_point(
        sim,
        world_x,
        world_y,
        &owner,
        state.match_state.sandbox_full_visibility,
        state.rules(),
        state.height_map(),
        crate::app::match_runtime::sim_tick::tactical_bridge_cells(sim),
    )
    .map(|hover| hover.stable_id)
}

fn own_building_id(state: &AppState, stable_id: u64) -> Option<u64> {
    let sim = state
        .match_state
        .sim_runtime
        .as_ref()
        .map(|rt| &rt.simulation)?;
    let owner = preferred_local_owner(state)?;
    let entity = sim.entities().get(stable_id)?;
    (entity.category() == EntityCategory::Structure
        && sim
            .interner
            .resolve(entity.owner())
            .eq_ignore_ascii_case(&owner))
    .then_some(stable_id)
}

/// The local player's own building under the given world point when the
/// repair cursor takes it: `DisplayClass::DetermineAction 0x006927CD..
/// 0x0069280D` needs its owner to be the local player (`IsHumanPlayer`) and
/// asks the building's `Can_Repair` (`production::can_repair_building`).
/// Shared by the repair cursor feedback (`input::cursor`) and the repair
/// click handler below so the two never disagree about the target.
pub(crate) fn own_repairable_building_under_point(
    state: &AppState,
    world_x: f32,
    world_y: f32,
) -> Option<u64> {
    own_repairable_building_id(state, visible_object_under_point(state, world_x, world_y)?)
}

fn own_repairable_building_id(state: &AppState, stable_id: u64) -> Option<u64> {
    let id = own_building_id(state, stable_id)?;
    let sim = &state.match_state.sim_runtime.as_ref()?.simulation;
    crate::sim::production::can_repair_building(sim, state.rules()?, id).then_some(id)
}

/// The local player's own building under the given world point when the sell
/// cursor takes it: `DisplayClass::DetermineAction 0x006929F2` asks the
/// building's `CanSell` (`production::can_sell_building`).
pub(crate) fn own_sellable_building_under_point(
    state: &AppState,
    world_x: f32,
    world_y: f32,
) -> Option<u64> {
    own_sellable_building_id(state, visible_object_under_point(state, world_x, world_y)?)
}

fn own_sellable_building_id(state: &AppState, stable_id: u64) -> Option<u64> {
    let id = own_building_id(state, stable_id)?;
    let sim = &state.match_state.sim_runtime.as_ref()?.simulation;
    crate::sim::production::can_sell_building(sim, state.rules()?, id).then_some(id)
}

pub(crate) fn sell_wall_under_cursor_is_eligible(state: &AppState) -> bool {
    let (world_x, world_y) = crate::app::match_runtime::sim_tick::screen_point_to_world(
        state,
        state.match_state.input.cursor_x,
        state.match_state.input.cursor_y,
    );
    if visible_object_under_point(state, world_x, world_y).is_some() {
        return false;
    }
    let (rx, ry) = crate::app::match_runtime::sim_tick::screen_point_to_world_cell(
        state,
        state.match_state.input.cursor_x,
        state.match_state.input.cursor_y,
    );
    let Some(owner) = preferred_local_owner(state) else {
        return false;
    };
    match (
        state
            .match_state
            .sim_runtime
            .as_ref()
            .map(|rt| &rt.simulation),
        state.overlay_registry(),
    ) {
        (Some(sim), Some(overlays)) => sell_wall_command_for_cell(
            sim,
            overlays,
            &owner,
            rx,
            ry,
            state.match_state.sandbox_full_visibility,
            false,
        )
        .is_some(),
        _ => false,
    }
}

fn sell_wall_command_for_cell(
    sim: &crate::sim::world::Simulation,
    overlays: &crate::rules::overlay_types::OverlayTypeRegistry,
    local_owner: &str,
    rx: u16,
    ry: u16,
    ignore_visibility: bool,
    object_under_cursor: bool,
) -> Option<Command> {
    if object_under_cursor || (rx, ry) == (0, 0) {
        return None;
    }
    let local_owner_id = sim.interner.get(local_owner)?;
    if crate::app::presentation::instances::cell_visibility_for_local_owner(
        Some(local_owner_id),
        Some(&sim.fog),
        rx,
        ry,
        ignore_visibility,
        sim.resolved_terrain.as_ref(),
    ) != crate::app::presentation::instances::CellVisibilityState::Visible
    {
        return None;
    }
    let grid = sim.overlay_grid.as_ref()?;
    if rx >= grid.width() || ry >= grid.height() {
        return None;
    }
    let cell = grid.cell(rx, ry);
    let overlay_id = cell.overlay_id?;
    if !overlays.flags(overlay_id).is_some_and(|flags| flags.wall) {
        return None;
    }
    let wall_owner = cell.wall_owner?;
    let wall_owner_house = sim.houses.get(&wall_owner)?;
    let owner_is_human_player = if sim.session.game_mode_nonzero {
        wall_owner == local_owner_id
    } else {
        wall_owner_house.is_human || wall_owner_house.player_control
    };
    owner_is_human_player.then_some(Command::SellWallAtCell {
        x: rx as i16,
        y: ry as i16,
    })
}

/// Handle a tactical left-click while the sidebar Repair or Sell cursor mode is
/// active. Issues `ToggleRepair` / `SellBuilding` on the own building under the
/// cursor. The mode stays active afterwards — gamemd repair/sell modes are
/// sticky: the player keeps repairing/selling until right-click, Esc, or a
/// second click on the sidebar button clears the mode. Returns `true` when a
/// repair/sell mode was active (the click is consumed by the mode regardless of
/// whether it landed on a building), `false` when neither mode is on.
pub(crate) fn try_repair_sell_mode_click(state: &mut AppState) -> bool {
    let repair = state
        .match_state
        .match_presentation
        .sidebar_gadget_state
        .repair_mode_on;
    let sell = state
        .match_state
        .match_presentation
        .sidebar_gadget_state
        .sell_mode_on;
    if !repair && !sell {
        return false;
    }
    let (world_x, world_y) = crate::app::match_runtime::sim_tick::screen_point_to_world(
        state,
        state.match_state.input.cursor_x,
        state.match_state.input.cursor_y,
    );
    let object_under_cursor = visible_object_under_point(state, world_x, world_y);
    let building = object_under_cursor.and_then(|id| {
        if repair {
            own_repairable_building_id(state, id)
        } else {
            own_sellable_building_id(state, id)
        }
    });
    if let Some(entity_id) = building {
        let owner: String =
            preferred_local_owner(state).unwrap_or_else(|| DEFAULT_OWNER.to_string());
        let payload = if repair {
            Command::ToggleRepair { entity_id }
        } else {
            Command::SellBuilding { entity_id }
        };
        schedule_command(state, &owner, payload);
    } else if sell && object_under_cursor.is_none() {
        let (rx, ry) = crate::app::match_runtime::sim_tick::screen_point_to_world_cell(
            state,
            state.match_state.input.cursor_x,
            state.match_state.input.cursor_y,
        );
        let Some(owner) = preferred_local_owner(state) else {
            return true;
        };
        let payload = match (
            state
                .match_state
                .sim_runtime
                .as_ref()
                .map(|rt| &rt.simulation),
            state.overlay_registry(),
        ) {
            (Some(sim), Some(overlays)) => sell_wall_command_for_cell(
                sim,
                overlays,
                &owner,
                rx,
                ry,
                state.match_state.sandbox_full_visibility,
                false,
            ),
            _ => None,
        };
        if let Some(payload) = payload {
            schedule_command(state, &owner, payload);
        }
    }
    true
}

pub(crate) fn place_ready_building_at_cursor(state: &mut AppState, type_id: &str) {
    // The original producer refuses a null current product and uses the
    // issuer captured by ManualPlace, rather than choosing a House again.
    let Some(issuer) = state.match_state.input.building_placement_issuer() else {
        return;
    };
    let Some(owner) = state
        .match_state
        .sim_runtime
        .as_ref()
        .map(|runtime| runtime.simulation.interner.resolve(issuer).to_string())
    else {
        return;
    };
    // Use the preview's stored (rx, ry) so the placed building exactly matches
    // the cells the player saw, avoiding any cursor-movement drift between frames.
    let (rx, ry) =
        if let Some(preview) = state.match_state.input.building_placement_preview.as_ref() {
            log::info!(
                "Click placement: using preview ({},{}) size={}x{} type={}",
                preview.rx,
                preview.ry,
                preview.width,
                preview.height,
                type_id,
            );
            (preview.rx, preview.ry)
        } else {
            crate::app::match_runtime::sim_tick::screen_point_to_world_cell(
                state,
                state.match_state.input.cursor_x,
                state.match_state.input.cursor_y,
            )
        };
    if let Some(preview) = state.match_state.input.building_placement_preview.as_ref()
        && !preview.valid
    {
        if let Some(reason) = &preview.reason {
            log::warn!(
                "Ready building placement rejected locally: owner={} type={} cell=({}, {}) reason={}",
                owner,
                type_id,
                rx,
                ry,
                reason.label()
            );
        }
        // `DisplayClass::BandBox_LeftUp 0x004ABAAC/0x004ABABA`: a
        // placement click whose pending building is not placeable
        // (`+0x1180` clear, `BuildingClass 0x00452670` refused the cell)
        // or not adjacent (`+0x1181` clear) goes to `0x004ABC76..
        // 0x004ABC80 PlayEVA("EVA_CannotDeployHere", -1)` on the clicking
        // machine, before any event is queued.
        crate::app::input::dispatch::push_local_eva(
            state,
            crate::app::input::sidebar_eva::EVA_CANNOT_DEPLOY_HERE,
        );
        return;
    }
    let type_interned = intern_in_sim(state, type_id);
    schedule_command(
        state,
        &owner,
        Command::PlaceReadyBuilding {
            type_id: type_interned,
            rx,
            ry,
        },
    );
    // Clear placement mode immediately so the foundation preview stops following
    // the cursor after the order is issued.
    state.match_state.input.submit_building_placement();
    log::info!(
        "Ready building placement queued: owner={} type={} cell=({}, {}) execute_tick=current",
        owner,
        type_id,
        rx,
        ry
    );
}

/// Schedule `Command::LaunchSuperWeapon` at the current cursor cell.
///
/// `section` is the SW INI section name (e.g., "LightningStormSpecial").
///
/// Returns early when the cursor is over the sidebar or minimap — this
/// matters for the release of the arming click itself, which lands on the
/// cameo. On a real tactical-map click it schedules the command, as
/// `DisplayClass::BandBox_LeftUp @ 0x004AB9B0` queues the SPECIAL_PLACE event
/// (`0x12`), and keeps the selection: Launch's tail clears or moves it
/// (`match_runtime::super_selection`). The sim's ClickFire admits the Super;
/// UI does not duplicate it. The event names the first type whose `Action=`
/// is the click's action (`SuperWeaponTypeClass::FindFirstOfAction`); retail
/// types' actions are unique, so that is the selected one.
pub(crate) fn launch_super_weapon_at_cursor(state: &mut AppState, section: &str) {
    // Guard: arming click's RELEASE lands on the cameo. Don't fire the SW
    // at a bogus off-map cell behind the sidebar panel; leave the mode
    // armed so the next real map click fires.
    if crate::app::presentation::sidebar_render::is_cursor_over_minimap(state)
        || crate::app::input::cursor::current_sidebar_view_hit(state)
    {
        return;
    }

    let owner: String = resolve_owner(state);
    let sw_type_id = intern_in_sim(state, section);
    let (rx, ry) = crate::app::match_runtime::sim_tick::screen_point_to_world_cell(
        state,
        state.match_state.input.cursor_x,
        state.match_state.input.cursor_y,
    );
    schedule_command(
        state,
        &owner,
        Command::LaunchSuperWeapon {
            sw_type_id,
            target_rx: rx,
            target_ry: ry,
        },
    );
    log::info!(
        "SuperWeapon launch queued: owner={} section={} cell=({}, {}) issue_frame=current",
        owner,
        section,
        rx,
        ry
    );
}

pub(crate) fn place_starter_base_for_local_owner(state: &mut AppState) {
    let owner: String = resolve_owner(state);
    let (Some(sim), Some(rules)) = (
        state
            .match_state
            .sim_runtime
            .as_ref()
            .map(|rt| &rt.simulation),
        state.rules(),
    ) else {
        return;
    };
    let opening = [
        pick_building_for_owner(rules, &owner, &["GAPOWR", "NAPOWR", "YAPOWR"]),
        pick_building_for_owner(
            rules,
            &owner,
            &["GAPILE", "NAHAND", "YABRCK", "NABRCK", "GABARR"],
        ),
        pick_building_for_owner(rules, &owner, &["GAREFN", "NAREFN", "YAREFN", "GAOREP"]),
    ];
    let build_options = production::build_options_for_owner(sim, rules, &owner);
    let queueable: Vec<String> = opening
        .into_iter()
        .flatten()
        .filter(|type_id| {
            build_options.iter().any(|opt| {
                let opt_str = sim.interner.resolve(opt.type_id);
                opt_str.eq_ignore_ascii_case(type_id) && opt.enabled
            })
        })
        .collect();
    let mut queued = 0u32;
    for type_id in queueable {
        let type_interned = intern_in_sim(state, &type_id);
        schedule_command(
            state,
            &owner,
            Command::QueueProduction {
                type_id: type_interned,
            },
        );
        queued += 1;
    }
    if queued > 0 {
        log::info!(
            "Starter opening queued: owner={} count={} issue_frame=current",
            owner,
            queued
        );
    } else {
        log::warn!(
            "Starter opening queue failed: owner={} (no compatible first-build sequence available)",
            owner
        );
    }
}

pub(crate) fn spawn_test_units_for_local_owner(state: &mut AppState) {
    let owner: String = resolve_owner(state);
    let sw: f32 = state.render_width() as f32;
    let sh: f32 = state.render_height() as f32;
    let (mut base_rx, mut base_ry) =
        crate::app::match_runtime::sim_tick::screen_point_to_world_cell(state, sw * 0.5, sh * 0.5);
    let path_grid = state
        .match_state
        .sim_runtime
        .as_ref()
        .map(|rt| &rt.simulation)
        .and_then(crate::sim::world::Simulation::path_grid_snapshot);
    let Some(rt) = state.match_state.sim_runtime.as_mut() else {
        return;
    };
    let resources = &rt.resources;
    let sim = &mut rt.simulation;
    let rules = &resources.rules;
    if let Some(grid) = path_grid.as_deref() {
        (base_rx, base_ry) =
            crate::app::match_runtime::sim_tick::clamp_cell_to_grid(grid, (base_rx, base_ry));
    }

    let mut debug_types: Vec<String> = {
        let options = production::build_options_for_owner(sim, rules, &owner);
        let mut selected: Vec<String> = options
            .iter()
            .filter(|o| {
                o.enabled
                    && o.object_category == crate::rules::object_type::ObjectCategory::Infantry
            })
            .take(3)
            .map(|o| sim.interner.resolve(o.type_id).to_string())
            .collect();
        if selected.len() < 3 {
            let vehicles = options
                .iter()
                .filter(|o| {
                    o.enabled
                        && o.object_category == crate::rules::object_type::ObjectCategory::Vehicle
                })
                .take(3 - selected.len())
                .map(|o| sim.interner.resolve(o.type_id).to_string());
            selected.extend(vehicles);
        }
        selected
    };
    if debug_types.is_empty() {
        debug_types = vec!["HTNK".to_string(), "MTNK".to_string(), "E1".to_string()];
    }

    let mut spawned: u32 = 0;
    let mut first_spawn: Option<(u16, u16)> = None;
    for (i, type_id) in debug_types.iter().enumerate() {
        let mut desired = (
            base_rx.saturating_add(2 + i as u16 * 2),
            base_ry.saturating_add(2),
        );
        if let Some(grid) = path_grid.as_deref() {
            desired = crate::app::match_runtime::sim_tick::clamp_cell_to_grid(grid, desired);
        }
        let spawn_cell = path_grid
            .as_deref()
            .and_then(|g| {
                crate::app::match_runtime::sim_tick::nearest_walkable_cell(g, desired, 16)
            })
            .unwrap_or(desired);
        if sim
            .spawn_object_with_overlay_registry(
                type_id,
                &owner,
                spawn_cell.0,
                spawn_cell.1,
                64,
                rules,
                &resources.overlay_registry,
            )
            .is_some()
        {
            if first_spawn.is_none() {
                first_spawn = Some(spawn_cell);
            }
            let name = rules
                .object(type_id)
                .and_then(|o| o.name.clone())
                .unwrap_or_else(|| type_id.clone());
            log::info!("Spawn test unit: {} ({})", name, type_id);
            spawned += 1;
        }
    }
    if spawned > 0 {
        crate::app::match_runtime::sim_tick::refresh_entity_atlases(state);
        if let Some((rx, ry)) = first_spawn {
            crate::app::input::camera::center_camera_on_cell(state, rx, ry);
        }
    }
    log::info!(
        "Spawn test units: owner={} spawned={} at ({},{}) types={:?}",
        owner,
        spawned,
        base_rx,
        base_ry,
        debug_types
    );
}

pub(crate) fn cycle_local_owner(state: &mut AppState) {
    // Debug cycling is available only without a scenario current House.
    // It cannot repoint ordinary saved identity through a presentation choice.
    if state.match_state.local_player_owner().is_some() {
        log::info!("Cycle owner ignored: local player is pinned for this match");
        return;
    }
    let mut owners = collect_playable_owners(state);
    if owners.is_empty() {
        return;
    }
    let current = preferred_local_owner(state);
    let next_idx = current
        .as_ref()
        .and_then(|c| owners.iter().position(|o| o.eq_ignore_ascii_case(c)))
        .map(|idx| (idx + 1) % owners.len())
        .unwrap_or(0);
    // Move out of Vec instead of cloning, then clone once for the override.
    let next = owners.swap_remove(next_idx);
    state.match_state.local_owner_override = Some(next.clone());
    state.match_state.input.targeting_mode = None;
    log::info!("Local owner switched to {}", next);
}

pub(crate) fn preferred_local_owner_name(state: &AppState) -> Option<String> {
    preferred_local_owner(state)
}

pub(crate) fn preferred_local_owner(state: &AppState) -> Option<String> {
    let sim = &state.match_state.sim_runtime.as_ref()?.simulation;
    preferred_local_owner_for_sim(
        sim,
        state.rules(),
        &state.match_state.match_presentation.house_roster,
        state.match_state.local_owner_override.as_deref(),
    )
}

fn preferred_local_owner_for_sim(
    sim: &crate::sim::world::Simulation,
    rules: Option<&crate::rules::ruleset::RuleSet>,
    roster: &crate::map::houses::HouseRoster,
    sandbox_override: Option<&str>,
) -> Option<String> {
    // Normal command and presentation consumers share saved native identity.
    // App debug preferences and selection apply only to identity-less sandboxes.
    if let Some(owner) = crate::app::match_runtime::state::scenario_local_owner(sim) {
        return Some(owner.to_string());
    }
    // Sandbox fallback: prefer owner of selected unit first.
    for entity in sim.entities().values() {
        let owner_str = sim.interner.resolve(entity.owner());
        if entity.selection.is_selected() && is_playable_house_name(owner_str) {
            return Some(owner_str.to_string());
        }
    }

    // Then explicit local override set by debug actions.
    if let Some(owner) = sandbox_override
        && is_playable_house_name(owner)
    {
        return Some(owner.to_string());
    }

    // Prefer owners that currently have structures.
    let mut structure_counts: HashMap<String, usize> = HashMap::new();
    for entity in sim.entities().values() {
        let owner_str = sim.interner.resolve(entity.owner());
        if entity.category() == EntityCategory::Structure && is_playable_house_name(owner_str) {
            *structure_counts.entry(owner_str.to_string()).or_insert(0) += 1;
        }
    }
    if !structure_counts.is_empty() {
        let mut ranked: Vec<(usize, String)> = structure_counts
            .into_iter()
            .filter_map(|(owner, count)| {
                let strict_buildable = rules.is_some_and(|rules| {
                    production::has_build_option_for_owner(sim, rules, &owner)
                });
                strict_buildable.then_some((count, owner))
            })
            .collect();
        ranked.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
        if let Some((_, owner)) = ranked.first() {
            return Some(owner.to_string());
        }
    }

    // Next fallback: playable houses from map config.
    let houses = collect_playable_owners_from(roster, Some(sim));
    if let Some(owner) = houses.first() {
        return Some(owner.to_string());
    }

    // Last fallback: any playable owner present in entity store.
    let mut owners: Vec<String> = sim
        .entities()
        .values()
        .map(|e| sim.interner.resolve(e.owner()).to_string())
        .filter(|o| is_playable_house_name(o))
        .collect();
    owners.sort();
    owners.dedup();
    owners.first().cloned()
}

pub(crate) fn collect_playable_owners(state: &AppState) -> Vec<String> {
    collect_playable_owners_from(
        &state.match_state.match_presentation.house_roster,
        state
            .match_state
            .sim_runtime
            .as_ref()
            .map(|rt| &rt.simulation),
    )
}

fn collect_playable_owners_from(
    roster: &crate::map::houses::HouseRoster,
    simulation: Option<&crate::sim::world::Simulation>,
) -> Vec<String> {
    let mut owners: Vec<String> = roster
        .houses
        .iter()
        .filter(|house| is_playable_house_name(&house.name))
        .filter(|house| house.player_control != Some(false))
        .map(|house| house.name.clone())
        .collect();
    if let Some(sim) = simulation {
        for entity in sim.entities().values() {
            let owner_str = sim.interner.resolve(entity.owner());
            if is_playable_house_name(owner_str) {
                owners.push(owner_str.to_string());
            }
        }
    }
    owners.sort();
    owners.dedup();
    owners
}

fn pick_building_for_owner(
    rules: &crate::rules::ruleset::RuleSet,
    owner: &str,
    candidates: &[&str],
) -> Option<String> {
    for id in candidates {
        let Some(obj) = rules.object(id) else {
            continue;
        };
        if obj.category != crate::rules::object_type::ObjectCategory::Building {
            continue;
        }
        if !obj.owner.is_empty() && !obj.owner.iter().any(|o| o.eq_ignore_ascii_case(owner)) {
            continue;
        }
        return Some((*id).to_string());
    }
    for id in candidates {
        let Some(obj) = rules.object(id) else {
            continue;
        };
        if obj.category == crate::rules::object_type::ObjectCategory::Building {
            return Some((*id).to_string());
        }
    }
    None
}

pub(super) fn schedule_command_in_sim(
    sim: &mut crate::sim::world::Simulation,
    rules: Option<&crate::rules::ruleset::RuleSet>,
    owner: &str,
    payload: Command,
) -> Option<u64> {
    // The ordinary client producer is behind the native54F200/6930A0 lock
    // gates. Escape's modal remains usable: its EXIT record has its existing
    // authoritative owner and is not a tactical order.
    if sim.session.input_locked() && !matches!(&payload, Command::ExitMatch) {
        return None;
    }
    let execute_tick = sim.session.tick;
    let owner_id = sim.interner.intern(owner);
    // The Stop, deploy, sell and repair keys and clicks, the superweapon
    // targeting click, the sidebar and EXIT (`encode_local_command_records`).
    // On this route the deploy family comes from the deploy key
    // (`Selection::IssueDeployCommand 0x00730AF0`), which sends DEPLOY; the
    // click route ([`encode_local_order`]) sends the self-click's MEGAMISSION
    // Unload. A record that does not decode now is not issued.
    let records = sim.encode_local_command_records(
        owner_id,
        &payload,
        rules,
        crate::sim::world::LocalProducer::Key,
    )?;
    if records.is_empty()
        || records.iter().any(|record| {
            sim.decode_native_command_record(record, rules, execute_tick)
                .is_none()
        })
    {
        return None;
    }
    for record in records {
        sim.issue_command_record(record);
    }
    Some(execute_tick)
}

/// Move-click producer for a click on a Cell. Each receiver resolves its own
/// Cell ([`Simulation::cell_click_destination`]) before event encoding;
/// synchronized/replay/internal Move payloads are already resolved. Original
/// initialized GI and MTNK Shift0x10 still execute that resolver and emit
/// ordinary MegaMission (111-byte Event)/Move2 -> class(Cell,1); neither
/// appends a queued waypoint (Infantry51AA40, Unit741970). Native
/// input/event/true-RET controls: walk_first_path.json `gi_reissue`.
///
/// [`Simulation::cell_click_destination`]: crate::sim::world::Simulation::cell_click_destination
pub(crate) fn ordinary_cell_move_goal(
    sim: &crate::sim::world::Simulation,
    rules: &crate::rules::ruleset::RuleSet,
    registry: Option<&crate::rules::overlay_types::OverlayTypeRegistry>,
    viewer: InternedId,
    entity_id: u64,
    clicked: (u16, u16),
    shift: bool,
) -> Option<(u16, u16)> {
    sim.cell_click_destination(viewer, entity_id, clicked, shift, rules, registry)
        .unwrap_or_else(|cause| {
            log::warn!("Cell click input: {cause}");
            None
        })
}

/// Move-click producer for a click over an object: the object, or a Cell
/// near it ([`Simulation::object_move_click`]).
///
/// [`Simulation::object_move_click`]: crate::sim::world::Simulation::object_move_click
pub(crate) fn object_move_destination(
    sim: &crate::sim::world::Simulation,
    rules: &crate::rules::ruleset::RuleSet,
    viewer: InternedId,
    entity_id: u64,
    target_id: u64,
    action: crate::sim::world::MoveAction,
    force_move_key: bool,
) -> Option<crate::sim::combat::TargetKind> {
    sim.object_move_click(viewer, entity_id, target_id, action, force_move_key, rules)
        .unwrap_or_else(|cause| {
            log::warn!("Move click over an object: {cause}");
            None
        })
}

/// Encode a local click order as its native records, each paired with what
/// it decodes to against the current state. The caller issues the records
/// ([`issue_local_order_records`]); the pairs' decoded side only serves the
/// click's own choices (voice, selection order). The tail decodes each record
/// again when it executes. An order with a record that does not decode now
/// yields nothing.
///
/// Active YR routes a cell click through `ClickedAction` (`0x004D7D50`), and an
/// object click through `0x004D74E0` (Building `0x00443410`), to
/// ClickedMission `0x006FFBE0` and `EventClass__BuildMegaMissionEnvelope`
/// (`0x004C6860`). Guard key730D60 reaches the same producer. Every order that
/// event carries (Move, Attack, AttackMove, AreaGuard, Enter, Capture,
/// Sabotage, Harvest, Unload) is one MEGAMISSION: an Enter order decodes to
/// `Command::Enter`, an Attack to `Command::ForceAttack`, and Unload to the
/// class's receiver. A self-click's DeployMcv and VERA's bunker eject click
/// send the Foot self-click's Unload. A rally click sends one ARCHIVE per
/// factory, and an undeploy order ARCHIVE and SELL
/// (`Simulation::encode_local_command_records`).
pub(crate) fn encode_local_order(
    sim: &crate::sim::world::Simulation,
    rules: Option<&crate::rules::ruleset::RuleSet>,
    envelope: CommandEnvelope,
) -> Vec<(CommandRecord, CommandEnvelope)> {
    let CommandEnvelope {
        owner,
        execute_tick,
        payload,
    } = envelope;
    sim.encode_local_command_records(
        owner,
        &payload,
        rules,
        crate::sim::world::LocalProducer::Click,
    )
    .unwrap_or_default()
    .into_iter()
    .map(|record| {
        let decoded = sim.decode_native_command_record(&record, rules, execute_tick)?;
        Some((record, decoded))
    })
    .collect::<Option<Vec<_>>>()
    .unwrap_or_default()
}

/// What a click order's records decode to now ([`encode_local_order`]).
#[cfg(test)]
pub(crate) fn decoded_local_order(
    sim: &crate::sim::world::Simulation,
    rules: Option<&crate::rules::ruleset::RuleSet>,
    envelope: CommandEnvelope,
) -> Vec<CommandEnvelope> {
    encode_local_order(sim, rules, envelope)
        .into_iter()
        .map(|(_, decoded)| decoded)
        .collect()
}

/// Issue a click's encoded records into the OutList in order (`LocalEvent`).
pub(crate) fn issue_local_order_records(
    sim: &mut crate::sim::world::Simulation,
    orders: impl IntoIterator<Item = (CommandRecord, CommandEnvelope)>,
) {
    for (record, _) in orders {
        sim.issue_command_record(record);
    }
}

/// Issue one ordinary command's records and return the raw issue ordinal.
///
/// Tactical certification records this ordinal. Offline the records carry the
/// current frame and the next tail dispatches them; network transfer would
/// overwrite the stamp with its negotiated ahead frame.
pub(crate) fn try_schedule_command(
    state: &mut AppState,
    owner: &str,
    payload: Command,
) -> Option<u64> {
    let rt = state.match_state.sim_runtime.as_mut()?;
    schedule_command_in_sim(
        &mut rt.simulation,
        Some(&rt.resources.rules),
        owner,
        payload,
    )
}

/// Preserve the existing unit-returning command boundary for ordinary callers.
pub(crate) fn schedule_command(state: &mut AppState, owner: &str, payload: Command) {
    let _ = try_schedule_command(state, owner, payload);
}

pub(crate) fn is_playable_house_name(name: &str) -> bool {
    let up = name.to_ascii_uppercase();
    !matches!(
        up.as_str(),
        "NEUTRAL" | "SPECIAL" | "CIVILIAN" | "GOODGUY" | "BADGUY" | "JP"
    )
}

#[cfg(test)]
mod tests {
    use super::{schedule_command_in_sim, sell_wall_command_for_cell};

    use crate::map::entities::EntityCategory;
    use crate::rules::ini_parser::IniFile;
    use crate::sim::combat::Health;
    use crate::sim::command::{Command, CommandEnvelope};
    use crate::sim::game_entity::GameEntity;
    use crate::sim::house_state::HouseState;
    use crate::sim::world::Simulation;

    #[test]
    fn cinematic_lock_rejects_tactical_commands_but_keeps_escape_options_usable() {
        let mut sim = Simulation::new();
        let local = sim.interner.intern("Local");
        let kind = sim.interner.intern("FixtureType");
        sim.houses
            .insert(local, HouseState::new(local, 0, None, true, 0, 10));
        sim.session.house_order.push(local);
        sim.session.set_input_locked(true);
        let before_hash = sim.state_hash();
        let before_names = sim.interner.len();
        for payload in [
            Command::Stop { entity_id: 11 },
            Command::SellBuilding { entity_id: 12 },
            Command::ToggleRepair { entity_id: 13 },
            Command::PlaceReadyBuilding {
                type_id: kind,
                rx: 20,
                ry: 21,
            },
            Command::LaunchSuperWeapon {
                sw_type_id: kind,
                target_rx: 22,
                target_ry: 23,
            },
        ] {
            assert_eq!(
                schedule_command_in_sim(&mut sim, None, "Unknown", payload),
                None
            );
        }
        assert!(sim.pending_command_snapshot(None).is_empty());
        assert_eq!(sim.interner.len(), before_names);
        assert_eq!(sim.state_hash(), before_hash);

        // Offline Options stores its speed directly, outside the command path.
        assert!(sim.apply_local_game_speed(local, 4));
        assert_eq!(
            schedule_command_in_sim(&mut sim, None, "Local", Command::ExitMatch),
            Some(0)
        );
        assert_eq!(sim.pending_command_snapshot(None).len(), 1);
        sim.session.set_input_locked(false);
        let mut entity = GameEntity::new_at_frame_zero_for_test(
            11,
            1,
            1,
            0,
            0,
            local,
            Health::new(100),
            kind,
            EntityCategory::Unit,
            0,
            5,
            false,
        );
        entity.lifecycle.set_in_limbo_for_test(false);
        sim.substrate.entities.insert(entity);
        assert_eq!(
            schedule_command_in_sim(&mut sim, None, "Local", Command::Stop { entity_id: 11 }),
            Some(0)
        );
        assert_eq!(sim.pending_command_snapshot(None).len(), 2);
    }

    /// Every production command is queued as what its native record decodes
    /// to (`Simulation::encode_production_record`), and needs the match's
    /// rules to resolve the record's type index.
    #[test]
    fn production_issue_queues_what_its_native_record_decodes_to() {
        let rules = crate::rules::ruleset::RuleSet::from_ini(&IniFile::from_str(
            "[Countries]\n0=Americans\n[InfantryTypes]\n0=E1\n[VehicleTypes]\n\
             [AircraftTypes]\n[BuildingTypes]\n0=GAPOWR\n\
             [E1]\nOwner=Americans\n[GAPOWR]\nOwner=Americans\n",
        ))
        .unwrap();
        let mut sim = Simulation::new();
        sim.intern_rule_type_ids(&rules);
        let local = sim.interner.intern("Local");
        sim.houses
            .insert(local, HouseState::new(local, 0, None, true, 0, 10));
        sim.session.house_order.push(local);
        let e1 = sim.interner.get("E1").unwrap();
        let gapowr = sim.interner.get("GAPOWR").unwrap();
        let issued = [
            Command::QueueProduction { type_id: e1 },
            Command::CancelProductionByType {
                type_id: e1,
                all: true,
            },
            Command::PlaceReadyBuilding {
                type_id: gapowr,
                rx: 12,
                ry: 34,
            },
            Command::PlaceProducedMobile {
                category: crate::sim::production::ProductionCategory::Infantry,
            },
        ];
        for payload in &issued {
            assert_eq!(
                schedule_command_in_sim(&mut sim, None, "Local", payload.clone()),
                None,
                "{payload:?} without rules"
            );
            assert_eq!(
                schedule_command_in_sim(&mut sim, Some(&rules), "Local", payload.clone()),
                Some(0),
                "{payload:?}"
            );
        }
        // SUSPEND names the factory's build; with none there is no record.
        assert_eq!(
            schedule_command_in_sim(
                &mut sim,
                Some(&rules),
                "Local",
                Command::SuspendProduction {
                    category: crate::sim::production::ProductionCategory::Infantry,
                },
            ),
            None
        );
        let queued: Vec<_> = sim
            .pending_command_snapshot(Some(&rules))
            .into_iter()
            .map(|envelope| envelope.payload)
            .collect();
        assert_eq!(queued, issued);
    }

    /// The deploy key's family and the undeploy order queue what their
    /// native records decode to: DEPLOY on an Aircraft decodes to nothing
    /// (`0x004C7781`), and the undeploy order is ARCHIVE then SELL.
    #[test]
    fn single_event_issue_queues_what_its_native_records_decode_to() {
        use crate::sim::world::single_event_record_tests::{record_world, spawn};
        let (mut sim, rules, _) = record_world();
        let gi = spawn(&mut sim, &rules, "GI", 10, 10);
        let orca = spawn(&mut sim, &rules, "ORCA", 12, 10);
        let yard = spawn(&mut sim, &rules, "GACNST", 20, 20);
        let issue = |sim: &mut Simulation, payload| {
            schedule_command_in_sim(sim, Some(&rules), "Americans", payload)
        };
        assert_eq!(
            issue(&mut sim, Command::ToggleInfantryDeploy { entity_id: gi }),
            Some(0)
        );
        assert_eq!(
            issue(&mut sim, Command::UnloadPassengers { transport_id: orca }),
            None
        );
        assert_eq!(
            issue(&mut sim, Command::UndeployBuilding { entity_id: yard }),
            Some(0)
        );
        let queued: Vec<_> = sim
            .pending_command_snapshot(None)
            .into_iter()
            .map(|envelope| envelope.payload)
            .collect();
        let [
            Command::ToggleInfantryDeploy { entity_id },
            Command::SetRally { producer_ids, .. },
            Command::SellBuilding { entity_id: sold },
        ] = queued.as_slice()
        else {
            panic!("{queued:?}");
        };
        assert_eq!(
            (*entity_id, producer_ids.as_slice(), *sold),
            (gi, &[yard][..], yard)
        );
    }

    /// Offline Options speed is not an event (`0x004E1E54`, modes 0 and 5):
    /// the scheduler issues nothing for it.
    #[test]
    fn offline_game_speed_is_never_scheduled() {
        let mut sim = Simulation::new();
        let local = sim.interner.intern("Local");
        sim.houses
            .insert(local, HouseState::new(local, 0, None, true, 0, 10));
        sim.session.house_order.push(local);
        assert_eq!(
            schedule_command_in_sim(&mut sim, None, "Local", Command::SetGameSpeed { speed: 4 }),
            None
        );
        assert!(sim.pending_command_snapshot(None).is_empty());
        assert_eq!(sim.session.game_options.game_speed, 1);
    }

    #[test]
    fn gsi_01_04_abort_waits_for_one_due_exit_dispatch_before_terminal_edge() {
        let mut sim = Simulation::new();
        let local = sim.interner.intern("Local");
        sim.houses
            .insert(local, HouseState::new(local, 0, None, true, 0, 10));
        sim.session.house_order = vec![local];
        sim.session.tick = 41;
        sim.session.binary_frame = 73;

        assert_eq!(
            schedule_command_in_sim(&mut sim, None, "Local", Command::ExitMatch),
            Some(41)
        );
        assert_eq!(sim.pending_command_snapshot(None).len(), 1);
        assert_eq!(
            sim.pending_command_snapshot(None)[0].payload,
            Command::ExitMatch
        );
        assert!(!sim.quit_requested, "confirmation only queues EXIT");
        assert_eq!(
            sim.take_executed_exit_owner(),
            None,
            "confirmation cannot trigger app teardown"
        );

        let result = sim.advance_tick(&[], None, None, None, 33);
        assert_eq!(result.executed_commands, 1);
        assert!(
            !result.frame_committed,
            "EXIT terminates its dispatch frame"
        );
        assert!(sim.quit_requested);
        assert_eq!(
            sim.take_executed_exit_owner(),
            Some(local),
            "the app cascade receives exactly one executed edge"
        );
        assert_eq!(
            crate::app::match_runtime::scenario_exit::arbitrate_executed_exit(false),
            crate::app::match_runtime::scenario_exit::ExecutedExitDisposition::Abort,
            "standalone EXIT keeps the abort route"
        );
        assert_eq!(
            sim.take_executed_exit_owner(),
            None,
            "repeated drains cannot duplicate the cascade trigger"
        );
    }

    #[test]
    fn gsi_01_04_same_frame_outcome_expiry_preempts_due_exit_after_consuming_edge() {
        let mut sim = Simulation::new();
        let local = sim.interner.intern("Local");
        let enemy = sim.interner.intern("Enemy");
        let mut local_house = HouseState::new(local, 0, None, true, 0, 10);
        let mut enemy_house = HouseState::new(enemy, 1, None, false, 0, 10);
        local_house.tracking.set_buildings_for_test(1);
        enemy_house.tracking.set_buildings_for_test(1);
        sim.houses.insert(local, local_house);
        sim.houses.insert(enemy, enemy_house);
        sim.session.house_order = vec![local, enemy];
        sim.session.game_options.short_game = false;
        sim.session.tick = 41;
        sim.session.binary_frame = 42;
        sim.session.current_house = Some(local);
        assert!(
            sim.houses
                .get_mut(&local)
                .expect("local house")
                .accept_win_result(41, 1, false)
        );
        assert_eq!(
            schedule_command_in_sim(&mut sim, None, "Local", Command::ExitMatch),
            Some(41)
        );

        let result = sim.advance_tick(&[], None, None, None, 33);
        let local_outcome_exit_ready = sim.houses[&local]
            .outcome()
            .is_some_and(|outcome| outcome.exit_ready);

        assert!(!result.frame_committed);
        assert!(local_outcome_exit_ready, "House expiry happened this frame");
        assert_eq!(sim.take_executed_exit_owner(), Some(local));
        assert_eq!(
            crate::app::match_runtime::scenario_exit::arbitrate_executed_exit(
                local_outcome_exit_ready
            ),
            crate::app::match_runtime::scenario_exit::ExecutedExitDisposition::Outcome,
            "Main_Game's victory/loss route has priority over simultaneous EXIT"
        );
        assert_eq!(
            sim.take_executed_exit_owner(),
            None,
            "the suppressed abort edge is still consumed exactly once"
        );
    }

    #[test]
    fn gsi_16_01_local_scheduler_sends_ordinary_moves_through_their_bytes() {
        let mut sim = Simulation::new();
        let local = sim.interner.intern("Local");
        let type_ref = sim.interner.intern("TESTUNIT");
        sim.houses
            .insert(local, HouseState::new(local, 0, None, false, 0, 10));
        sim.session.house_order = vec![local];
        sim.session.tick = 123;
        sim.session.binary_frame = 77;
        let mut entity = GameEntity::new_at_frame_zero_for_test(
            42,
            1,
            1,
            0,
            0,
            local,
            Health::new(100),
            type_ref,
            EntityCategory::Unit,
            0,
            5,
            false,
        );
        entity.lifecycle.set_in_limbo_for_test(false);
        sim.substrate.entities.insert(entity);

        // A cell the Megamission record cannot hold shows the ordinary Move
        // takes its bytes: the codec refuses it and nothing is queued.
        assert_eq!(
            schedule_command_in_sim(
                &mut sim,
                None,
                "Local",
                Command::Move {
                    entity_id: 42,
                    target_rx: u16::MAX,
                    target_ry: 12,
                },
            ),
            None
        );
        assert!(sim.pending_command_snapshot(None).is_empty());

        assert_eq!(
            schedule_command_in_sim(
                &mut sim,
                None,
                "Local",
                Command::Move {
                    entity_id: 42,
                    target_rx: 34,
                    target_ry: 12,
                },
            ),
            Some(123)
        );
        assert_eq!(
            sim.pending_command_snapshot(None)[0],
            CommandEnvelope::new(
                local,
                123,
                Command::Move {
                    entity_id: 42,
                    target_rx: 34,
                    target_ry: 12,
                }
            ),
            "an ordinary Move keeps its cell through its Megamission bytes"
        );

        schedule_command_in_sim(
            &mut sim,
            None,
            "Local",
            Command::MoveToObject {
                entity_id: 42,
                target_id: 7,
            },
        )
        .unwrap();
        assert_eq!(
            sim.pending_command_snapshot(None)[1].payload,
            Command::MoveToObject {
                entity_id: 42,
                target_id: 7,
            },
            "a Move onto an object keeps it through its Megamission bytes"
        );
    }

    /// A registered house and one live unit, `id`, for the scheduler tests.
    fn scheduler_fixture(owner: &str, id: u64) -> Simulation {
        let mut sim = Simulation::new();
        let house = sim.interner.intern(owner);
        let type_ref = sim.interner.intern("TESTUNIT");
        sim.houses
            .insert(house, HouseState::new(house, 0, None, true, 0, 10));
        sim.session.house_order = vec![house];
        let mut entity = GameEntity::new_at_frame_zero_for_test(
            id,
            1,
            1,
            0,
            0,
            house,
            Health::new(100),
            type_ref,
            EntityCategory::Unit,
            0,
            5,
            false,
        );
        entity.lifecycle.set_in_limbo_for_test(false);
        sim.substrate.entities.insert(entity);
        sim
    }

    #[test]
    fn recorded_scheduler_stamps_the_current_raw_issue_ordinal() {
        let mut sim = scheduler_fixture("Russians", 99);
        sim.session.tick = 41;
        sim.input_delay_ticks = 7;

        let execute_tick = schedule_command_in_sim(
            &mut sim,
            None,
            "Russians",
            Command::Enter {
                entity_id: 99,
                target_id: 5,
            },
        )
        .unwrap();

        assert_eq!(execute_tick, 41);
        assert_eq!(sim.pending_command_snapshot(None).len(), 1);
        assert_eq!(
            sim.pending_command_snapshot(None)[0].execute_tick,
            execute_tick
        );
        assert_eq!(
            sim.interner
                .resolve(sim.pending_command_snapshot(None)[0].owner),
            "Russians"
        );
        assert_eq!(
            sim.pending_command_snapshot(None)[0].payload,
            Command::Enter {
                entity_id: 99,
                target_id: 5,
            }
        );
    }

    #[test]
    fn recorded_scheduler_does_not_apply_offline_input_delay() {
        let mut sim = scheduler_fixture("YuriCountry", 7);
        sim.session.tick = u64::MAX - 1;
        sim.input_delay_ticks = 8;

        let execute_tick = schedule_command_in_sim(
            &mut sim,
            None,
            "YuriCountry",
            Command::Enter {
                entity_id: 7,
                target_id: 5,
            },
        )
        .unwrap();

        assert_eq!(execute_tick, u64::MAX - 1);
        assert_eq!(
            sim.pending_command_snapshot(None)[0].execute_tick,
            u64::MAX - 1
        );
    }

    #[test]
    fn gsi_04_07_wall_sell_scheduler_uses_current_tick_and_signed_cell_payload() {
        let mut sim = Simulation::new();
        sim.session.tick = 23;
        let local = sim.interner.intern("Local");
        let remote = sim.interner.intern("Remote");
        let ai = sim.interner.intern("AI");
        let missing_house = sim.interner.intern("MissingHouse");
        let mut local_house = HouseState::new(local, 0, None, false, 0, 10);
        local_house.player_control = true;
        sim.houses.insert(local, local_house);
        sim.houses
            .insert(remote, HouseState::new(remote, 1, None, true, 0, 10));
        sim.houses
            .insert(ai, HouseState::new(ai, 2, None, false, 0, 10));
        sim.session.house_order = vec![local, remote, ai];
        sim.session.binary_frame = 23;

        let issued = sim
            .encode_sell_wall_at_cell_record(local, 0, 1)
            .expect("registered local house encodes");
        assert_eq!(
            issued.opcode(),
            crate::sim::command::SELL_WALL_AT_CELL_OPCODE
        );
        assert_eq!(issued.house_id(), 0);
        assert_eq!(issued.frame_stamp(), 23);
        assert_eq!(&issued.payload()[..4], &[0, 0, 1, 0]);
        assert_eq!(
            sim.decode_native_command_record(&issued, None, 23).unwrap(),
            CommandEnvelope::new(local, 23, Command::SellWallAtCell { x: 0, y: 1 })
        );

        let overlay_ini = IniFile::from_str(
            "[OverlayTypes]\n0=GAWALL\n1=ORE\n\
             [GAWALL]\nWall=yes\n\
             [ORE]\nWall=no\n",
        );
        let overlays =
            crate::rules::overlay_types::OverlayTypeRegistry::from_ini(&overlay_ini, None);
        sim.overlay_grid = Some(crate::sim::overlay_grid::OverlayGrid::new(4, 4));
        for cell in [(0, 1), (1, 1), (3, 3)] {
            sim.fog.mark_visible_for_owner(local, cell.0, cell.1);
        }

        fn attempt(
            sim: &mut Simulation,
            overlays: &crate::rules::overlay_types::OverlayTypeRegistry,
            cell: (u16, u16),
            object_under_cursor: bool,
        ) -> Option<u64> {
            let payload = sell_wall_command_for_cell(
                sim,
                overlays,
                "Local",
                cell.0,
                cell.1,
                false,
                object_under_cursor,
            )?;
            schedule_command_in_sim(sim, None, "Local", payload)
        }

        assert_eq!(attempt(&mut sim, &overlays, (0, 0), false), None);
        assert_eq!(attempt(&mut sim, &overlays, (3, 3), false), None);

        sim.overlay_grid
            .as_mut()
            .unwrap()
            .place_owned_wall(1, 1, 1, 0, local);
        assert_eq!(attempt(&mut sim, &overlays, (1, 1), false), None);

        sim.overlay_grid
            .as_mut()
            .unwrap()
            .place_owned_wall(2, 2, 0, 0, local);
        assert_eq!(attempt(&mut sim, &overlays, (2, 2), false), None);

        sim.overlay_grid.as_mut().unwrap().place_overlay(1, 1, 0, 0);
        assert_eq!(attempt(&mut sim, &overlays, (1, 1), false), None);
        sim.overlay_grid
            .as_mut()
            .unwrap()
            .place_owned_wall(1, 1, 0, 0, missing_house);
        assert_eq!(attempt(&mut sim, &overlays, (1, 1), false), None);
        sim.overlay_grid
            .as_mut()
            .unwrap()
            .place_owned_wall(1, 1, 0, 0, ai);
        assert_eq!(attempt(&mut sim, &overlays, (1, 1), false), None);

        sim.overlay_grid
            .as_mut()
            .unwrap()
            .place_owned_wall(0, 1, 0, 0, remote);
        assert_eq!(attempt(&mut sim, &overlays, (0, 1), false), Some(23));
        assert_eq!(sim.pending_command_snapshot(None).len(), 1);
        assert_eq!(sim.pending_command_snapshot(None)[0].execute_tick, 23);
        assert_eq!(
            sim.pending_command_snapshot(None)[0].payload,
            Command::SellWallAtCell { x: 0, y: 1 }
        );
        sim.dispatch_due_command_queue(None, None, crate::sim::world::FrameEffects::default());
        assert!(sim.pending_command_snapshot(None).is_empty());

        sim.session.game_mode_nonzero = true;
        sim.overlay_grid
            .as_mut()
            .unwrap()
            .place_owned_wall(1, 1, 0, 0, remote);
        assert_eq!(attempt(&mut sim, &overlays, (1, 1), false), None);
        sim.overlay_grid
            .as_mut()
            .unwrap()
            .place_owned_wall(1, 1, 0, 0, local);
        assert_eq!(attempt(&mut sim, &overlays, (1, 1), true), None);
        assert!(sim.pending_command_snapshot(None).is_empty());

        sim.overlay_grid
            .as_mut()
            .unwrap()
            .place_owned_wall(0, 1, 0, 0, local);
        assert_eq!(attempt(&mut sim, &overlays, (0, 1), false), Some(23));
        assert_eq!(sim.pending_command_snapshot(None).len(), 1);
        assert_eq!(sim.pending_command_snapshot(None)[0].execute_tick, 23);
        assert_eq!(
            sim.pending_command_snapshot(None)[0].payload,
            Command::SellWallAtCell { x: 0, y: 1 }
        );
    }
}

#[cfg(test)]
#[path = "local_owner_tests.rs"]
mod local_owner_tests;
