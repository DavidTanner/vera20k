//! Original-executed Next/Previous4AA2B0/4AA380 and Execute536610/536A80.
//! Inputs, native results and execution boundaries are retained together in
//! tools/input_oracle/selection_navigation.{py,json,meta.json,md}.
//!
//! These fixtures supply existing objects and retained display order. They do
//! not establish object construction, display registration, RobotOffline,
//! power/planning mode lifecycle, window redraw or final camera raster parity.
//! Voice/RNG comparisons live with the audio owners.

use super::super::{SelectionMutation, resolve_selection_mutation, selected_stable_ids_in_order};
use super::{ObjectDirection, next_object};
use crate::app::input::{camera, entity_pick};
use crate::app::types::TypeSelectInputState;
use crate::map::entities::EntityCategory;
use crate::rules::{ini_parser::IniFile, ruleset::RuleSet};
use crate::sim::command::Command;
use crate::sim::components::{DriveCoord, Health};
use crate::sim::game_entity::{BunkerLink, GameEntity};
use crate::sim::house_state::HouseState;
use crate::sim::movement::ground_pose;
use crate::sim::world::{Simulation, display_layers::DisplayLayer};
use crate::ui::sidebar::gadget_flash::SidebarGadgetState;
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;

#[derive(Deserialize)]
struct Corpus {
    schema: u32,
    search_cases: Vec<SearchCase>,
    command_histories: Vec<CommandHistory>,
}

#[derive(Deserialize)]
struct Actor {
    id: u64,
    kind: String,
    health: i32,
    limbo: bool,
    alive: bool,
    in_playfield: bool,
    discovered: bool,
    selectable: bool,
    owner: String,
    slave: bool,
    robot_offline: bool,
    bunker: bool,
    docked: bool,
    cell_building: bool,
    locomotor_swap: bool,
    virtual_1d4: bool,
    mission_only: bool,
    techno_cast: bool,
    coord: [i32; 3],
}

#[derive(Default, Deserialize)]
struct HouseControls {
    campaign: bool,
    other_human: bool,
    other_control: bool,
}

#[derive(Deserialize)]
struct Inputs {
    actors: Vec<Actor>,
    layers: [Vec<Option<u64>>; 5],
    #[serde(default)]
    house: HouseControls,
}

#[derive(Deserialize)]
struct SearchCase {
    id: String,
    #[serde(flatten)]
    inputs: Inputs,
    anchor: Option<u64>,
    direction: String,
    candidate: Option<u64>,
    visits: Vec<Visit>,
    rng_unchanged: bool,
}

#[derive(Deserialize)]
struct Visit {
    id: u64,
    gate: String,
    accepted: bool,
}

#[derive(Default, Deserialize)]
#[serde(default)]
struct Modes {
    repair: bool,
    sell: bool,
    power: bool,
    planning: bool,
}

#[derive(Deserialize)]
struct CommandHistory {
    id: String,
    #[serde(flatten)]
    inputs: Inputs,
    selected: Vec<u64>,
    directions: Vec<String>,
    follow: Option<u64>,
    #[serde(default)]
    modes: Modes,
    #[serde(default)]
    placement: bool,
    steps: Vec<CommandStep>,
}

#[derive(Deserialize)]
struct CommandStep {
    direction: String,
    selected: Vec<u64>,
    selected_flags: BTreeMap<String, bool>,
    follow: Option<u64>,
    follow_enabled: bool,
    modes: Modes,
    placement: bool,
    selection_mode: u32,
    selection_across_map: bool,
    center_coord: Option<[i32; 3]>,
    redraw: Vec<u32>,
    call_order: Vec<String>,
}

fn corpus() -> Corpus {
    let corpus: Corpus = serde_json::from_str(crate::test_fixture::text(
        "tools/input_oracle/selection_navigation.json",
    ))
    .expect("checked original selection-navigation corpus");
    assert_eq!(corpus.schema, 1);
    corpus
}

fn direction(value: &str) -> ObjectDirection {
    match value {
        "next" => ObjectDirection::Next,
        "previous" => ObjectDirection::Previous,
        other => panic!("unexpected native direction {other}"),
    }
}

fn fixture(inputs: &Inputs, selected: &[u64]) -> (Simulation, RuleSet) {
    // Per-actor type names keep the native type Selectable byte independent.
    // These are explicit source inputs to the production INI reader, not
    // asserted retail rosters or a second implementation of the predicates.
    let mut ini = String::new();
    for (section, kind) in [
        ("InfantryTypes", "infantry"),
        ("VehicleTypes", "unit"),
        ("BuildingTypes", "building"),
    ] {
        writeln!(ini, "[{section}]").unwrap();
        for (index, actor) in inputs.actors.iter().filter(|a| a.kind == kind).enumerate() {
            writeln!(ini, "{index}=ACTOR{}", actor.id).unwrap();
        }
        if kind == "building" {
            writeln!(ini, "helper=DOCKBUILDING").unwrap();
        }
    }
    for actor in &inputs.actors {
        writeln!(
            ini,
            "[ACTOR{}]\nStrength=100\nSelectable={}\nPrimary=GUN",
            actor.id,
            if actor.selectable { "yes" } else { "no" }
        )
        .unwrap();
    }
    ini.push_str("[DOCKBUILDING]\nStrength=100\nFoundation=1x1\n[GUN]\nDamage=10\n");
    let rules = RuleSet::from_ini(&IniFile::from_str(&ini)).expect("native control types");
    let mut sim = Simulation::new();
    let local = sim.interner.intern("local");
    let other = sim.interner.intern("other");
    sim.session.current_house = Some(local);
    sim.session.game_mode_nonzero = !inputs.house.campaign;
    sim.session.house_order = vec![local, other];
    sim.houses
        .insert(local, HouseState::new(local, 0, None, true, 0, 10));
    let mut other_house = HouseState::new(other, 1, None, inputs.house.other_human, 0, 10);
    other_house.player_control = inputs.house.other_control;
    sim.houses.insert(other, other_house);
    for actor in &inputs.actors {
        assert!(!actor.robot_offline && actor.techno_cast);
        let category = match actor.kind.as_str() {
            "infantry" => EntityCategory::Infantry,
            "unit" => EntityCategory::Unit,
            "building" => EntityCategory::Structure,
            other => panic!("unexpected native class {other}"),
        };
        let owner = match actor.owner.as_str() {
            "local" => local,
            "other" => other,
            other => panic!("unexpected native owner {other}"),
        };
        let type_ref = sim.interner.intern(&format!("ACTOR{}", actor.id));
        let mut entity = GameEntity::new_at_frame_zero_for_test(
            actor.id,
            0,
            0,
            0,
            0,
            owner,
            Health {
                current: actor.health,
            },
            type_ref,
            category,
            0,
            5,
            false,
        );
        entity.lifecycle.object_alive = actor.alive;
        entity.lifecycle.in_limbo = actor.limbo;
        entity.in_playfield = actor.in_playfield;
        entity.discovery.discovered_by_current_house = actor.discovered;
        entity.selected = selected.contains(&actor.id);
        entity.foot_locomotor_swap_active = actor.locomotor_swap;
        if actor.mission_only {
            entity.mark_mission_only();
        }
        if actor.virtual_1d4 {
            entity.temporal = crate::sim::temporal::TemporalState::warped_by_for_test(999);
        }
        if actor.slave {
            entity.slave = crate::sim::slave_manager::SlaveLink::for_test(Some(999), Vec::new());
        }
        if actor.bunker {
            entity.bunker_link = BunkerLink::Installed(999);
        }
        if actor.docked {
            entity.dock_entered_with = Some(999);
        }
        let [x, y, z] = actor.coord;
        ground_pose::put_location(&mut entity.position, DriveCoord { x, y, z });
        if actor.cell_building {
            let building_type = sim.interner.intern("DOCKBUILDING");
            let mut building = GameEntity::new_at_frame_zero_for_test(
                1000 + actor.id,
                entity.position.rx,
                entity.position.ry,
                0,
                0,
                owner,
                Health { current: 100 },
                building_type,
                EntityCategory::Structure,
                0,
                0,
                false,
            );
            building.lifecycle.object_alive = true;
            building.lifecycle.in_limbo = false;
            sim.entities_mut().insert(building);
        }
        sim.entities_mut().insert(entity);
    }
    // Retained layer state is an input, independent of storage ID order and
    // current sort keys. Use its existing persisted representation. Native
    // NULL holes do no work; the Rust owner forbids ID zero and stores only
    // members, so compact those holes without changing encounter order.
    let layers = inputs
        .layers
        .each_ref()
        .map(|ids| ids.iter().flatten().copied().collect::<Vec<_>>());
    let mut state = serde_json::to_value(&sim).expect("serialize selection fixture");
    state["substrate"]["display"] = serde_json::to_value(&layers).unwrap();
    let sim: Simulation = serde_json::from_value(state).expect("retained display state");
    for (index, ids) in layers.iter().enumerate() {
        assert_eq!(
            sim.display_layers()
                .members(DisplayLayer::from_index(index as u8).unwrap()),
            ids
        );
    }
    (sim, rules)
}

#[test]
fn next_previous_search_and_candidate_gates_match_original_execution() {
    let corpus = corpus();
    assert_eq!(corpus.search_cases.len(), 94);
    let mut excluded = BTreeSet::new();
    let mut compared = 0;
    for row in &corpus.search_cases {
        if row
            .inputs
            .actors
            .iter()
            .any(|a| a.robot_offline || !a.techno_cast)
        {
            excluded.insert(row.id.as_str());
            continue;
        }
        let (sim, rules) = fixture(&row.inputs, &[]);
        assert_eq!(
            next_object(&sim, Some(&rules), row.anchor, direction(&row.direction)),
            row.candidate,
            "{}: retained display search",
            row.id
        );
        for visit in &row.visits {
            let entity = sim.entities().get(visit.id).unwrap();
            let accepted = match visit.gate.as_str() {
                "local_gate" => {
                    entity_pick::is_local_player_selectable_object(&sim, Some(&rules), entity)
                }
                "dynamic_gate" => {
                    entity_pick::selection_dynamic_prefix(
                        entity,
                        sim.entities(),
                        Some(&rules),
                        Some(&sim.interner),
                    ) && sim.object_is_selectable(visit.id, Some(&rules))
                }
                other => panic!("unexpected native gate {other}"),
            };
            assert_eq!(
                accepted, visit.accepted,
                "{}: {} for {}",
                row.id, visit.gate, visit.id
            );
        }
        assert!(row.rng_unchanged, "{}: native search RNG boundary", row.id);
        compared += 1;
    }
    assert_eq!(compared, 86);
    // RobotOffline's producer/lifecycle has no owner yet. The raw cast-bit
    // controls deliberately supply a malformed Techno, which a typed Rust
    // GameEntity cannot represent. Neither is silently mapped to another bit.
    assert_eq!(
        excluded,
        BTreeSet::from([
            "infantry_robot_offline_next",
            "infantry_robot_offline_previous",
            "unit_robot_offline_next",
            "unit_robot_offline_previous",
            "infantry_no_techno_cast_next",
            "infantry_no_techno_cast_previous",
            "unit_no_techno_cast_next",
            "unit_no_techno_cast_previous",
        ])
    );
}

#[test]
fn execute_histories_match_selection_follow_modes_placement_and_camera() {
    let corpus = corpus();
    assert_eq!(corpus.command_histories.len(), 20);
    let mut unrepresented_modes = BTreeSet::new();
    let mut compared_steps = 0;
    for row in &corpus.command_histories {
        let (mut sim, rules) = fixture(&row.inputs, &row.selected);
        let mut ordered = row.selected.clone();
        let mut pending = false;
        let mut follow = row.follow;
        let mut modes = SidebarGadgetState {
            repair_mode_on: row.modes.repair,
            sell_mode_on: row.modes.sell,
            ..Default::default()
        };
        if row.modes.power || row.modes.planning {
            unrepresented_modes.insert(row.id.as_str());
        }
        // Native fixture input B0FE54=3, B0FE58=1. Reach that state through
        // the existing mode owner, then use its ordinary-selection reset.
        let mut type_select = TypeSelectInputState::default();
        type_select.finish_health_navigation(true);
        type_select.across_map = true;
        assert_eq!(row.directions.len(), row.steps.len());
        for (index, (requested, expected)) in row.directions.iter().zip(&row.steps).enumerate() {
            assert_eq!(requested, &expected.direction);
            modes.disable_selection_modes(row.placement);
            let current = selected_stable_ids_in_order(Some(&sim), Some(&rules), &ordered, pending);
            let candidate = next_object(
                &sim,
                Some(&rules),
                current.first().copied(),
                direction(requested),
            );
            let mut center_coord = None;
            if let Some(id) = candidate {
                let effects = resolve_selection_mutation(
                    &sim,
                    Some(&rules),
                    current,
                    follow,
                    row.placement,
                    &SelectionMutation {
                        clear: true,
                        select: vec![id],
                        ..Default::default()
                    },
                );
                assert!(effects.native_selection_mode_reset);
                assert_eq!(
                    effects.successful_adds, expected.selected,
                    "{} step {index}: actual Select admission",
                    row.id
                );
                ordered = effects.ordered;
                follow = effects.follow_target;
                pending = true;
                type_select.reset_scope();
                let coords: Vec<_> = ordered
                    .iter()
                    .map(|id| {
                        let entity = sim.entities().get(*id).unwrap();
                        let coord =
                            ground_pose::object_location(entity, sim.resolved_terrain.as_ref());
                        (coord.x, coord.y, coord.z)
                    })
                    .collect();
                center_coord =
                    camera::selection_view_centre_leptons(&coords).map(|(x, y, z)| [x, y, z]);
            }
            assert_eq!(
                ordered, expected.selected,
                "{} step {index}: immediate membership",
                row.id
            );
            assert_eq!(
                follow, expected.follow,
                "{} step {index}: Follow cleanup",
                row.id
            );
            assert_eq!(
                follow.is_some(),
                expected.follow_enabled,
                "{} step {index}: Follow enable",
                row.id
            );
            assert_eq!(
                modes.repair_mode_on, expected.modes.repair,
                "{} step {index}: Repair",
                row.id
            );
            assert_eq!(
                modes.sell_mode_on, expected.modes.sell,
                "{} step {index}: Sell",
                row.id
            );
            if !row.modes.power && !row.modes.planning {
                assert!(!expected.modes.power && !expected.modes.planning);
            }
            assert_eq!(
                row.placement, expected.placement,
                "{} step {index}: placement retained",
                row.id
            );
            assert_eq!(
                type_select.health_navigation_continues(),
                expected.selection_mode == 3,
                "{} step {index}: selection mode",
                row.id
            );
            assert_eq!(
                type_select.across_map, expected.selection_across_map,
                "{} step {index}: selection scope",
                row.id
            );
            assert_eq!(
                center_coord, expected.center_coord,
                "{} step {index}: camera coordinate",
                row.id
            );
            // Check native platform handoff reachability, without claiming a
            // window redraw or a camera projection from this headless seam.
            assert_eq!(
                candidate.is_some(),
                expected
                    .call_order
                    .iter()
                    .any(|call| call == "unselect_all"),
                "{} step {index}: UnselectAll caller",
                row.id
            );
            assert_eq!(
                candidate.is_some(),
                !expected.redraw.is_empty(),
                "{} step {index}: redraw handoff",
                row.id
            );
            for actor in &row.inputs.actors {
                assert_eq!(
                    ordered.contains(&actor.id),
                    expected.selected_flags[&actor.id.to_string()],
                    "{} step {index}: pending bit for {}",
                    row.id,
                    actor.id
                );
                assert_eq!(
                    sim.entities().get(actor.id).unwrap().selected,
                    row.selected.contains(&actor.id),
                    "{} step {index}: sim must remain uncommitted",
                    row.id
                );
            }
            compared_steps += 1;
        }
        // Only after the whole key sequence, commit the final local snapshot
        // through the production command receiver and its shared Select gate.
        assert!(sim.apply_command(
            "local",
            &Command::Select {
                entity_ids: ordered.clone(),
                additive: false
            },
            Some(&rules)
        ));
        let expected = row.steps.last().unwrap();
        for actor in &row.inputs.actors {
            assert_eq!(
                sim.entities().get(actor.id).unwrap().selected,
                expected.selected_flags[&actor.id.to_string()],
                "{}: committed bit for {}",
                row.id,
                actor.id
            );
        }
        assert_eq!(
            selected_stable_ids_in_order(Some(&sim), Some(&rules), &ordered, false),
            expected.selected,
            "{}: post-commit ledger",
            row.id
        );
    }
    assert_eq!(compared_steps, 31);
    // Selection, Follow, placement and camera still compare in these four
    // histories. Only power/planning mode state lacks a production owner.
    assert_eq!(
        unrepresented_modes,
        BTreeSet::from([
            "cancel_power",
            "cancel_planning",
            "placement_preserves_power_refuses_selection",
            "placement_preserves_planning_refuses_selection",
        ])
    );
}

#[test]
fn snapshot_restores_singleton_anchor_and_retained_layer_order_for_next_key() {
    use crate::sim::snapshot::GameSnapshot;

    let corpus = corpus();
    let row = corpus
        .command_histories
        .iter()
        .find(|row| row.id == "consecutive_next_previous")
        .unwrap();
    let (mut sim, rules) = fixture(&row.inputs, &row.selected);
    // Supply a valid monotonic allocator/Logic state for the authored IDs so
    // the production restore transaction can validate the fixture normally.
    for _ in 0..row
        .inputs
        .actors
        .iter()
        .map(|actor| actor.id)
        .max()
        .unwrap()
    {
        sim.allocate_stable_id();
    }
    for actor in &row.inputs.actors {
        sim.register_live_object(actor.id);
    }
    sim.session.map_name = row.id.clone();
    let current = selected_stable_ids_in_order(Some(&sim), Some(&rules), &row.selected, false);
    let candidate = next_object(
        &sim,
        Some(&rules),
        current.first().copied(),
        direction(&row.directions[0]),
    )
    .unwrap();
    let first = resolve_selection_mutation(
        &sim,
        Some(&rules),
        current,
        row.follow,
        false,
        &SelectionMutation {
            clear: true,
            select: vec![candidate],
            ..Default::default()
        },
    );
    assert_eq!(first.ordered, row.steps[0].selected);
    assert_eq!(first.ordered.len(), 1);
    assert_eq!(first.follow_target, row.steps[0].follow);
    assert!(sim.apply_command(
        "local",
        &Command::Select {
            entity_ids: first.ordered,
            additive: false,
        },
        Some(&rules),
    ));

    let bytes = GameSnapshot::save_validated(&sim, 1, 2, "N/M continuation", 0);
    let mut restored = GameSnapshot::load_validated(&bytes, 1, 2, &row.id)
        .expect("current VERA snapshot and matching content identity")
        .sim;
    restored
        .restore_after_snapshot_load()
        .expect("ordinary fixture references and retained membership");
    for index in 0..5 {
        let layer = DisplayLayer::from_index(index).unwrap();
        assert_eq!(
            restored.display_layers().members(layer),
            sim.display_layers().members(layer)
        );
    }
    for actor in &row.inputs.actors {
        assert_eq!(
            restored.entities().get(actor.id).unwrap().selected,
            row.steps[0].selected_flags[&actor.id.to_string()]
        );
    }

    // This supplies the empty-cache/nonpending state produced by the app's
    // reset_for_world_replacement. It does not execute that whole AppState
    // wrapper or claim comparison with native save-stream bytes.
    let current = selected_stable_ids_in_order(Some(&restored), Some(&rules), &[], false);
    assert_eq!(current, row.steps[0].selected);
    let candidate = next_object(
        &restored,
        Some(&rules),
        current.first().copied(),
        direction(&row.directions[1]),
    )
    .unwrap();
    let second = resolve_selection_mutation(
        &restored,
        Some(&rules),
        current,
        first.follow_target,
        false,
        &SelectionMutation {
            clear: true,
            select: vec![candidate],
            ..Default::default()
        },
    );
    assert_eq!(second.ordered, row.steps[1].selected);
    assert_eq!(second.successful_adds, row.steps[1].selected);
    assert_eq!(second.follow_target, row.steps[1].follow);
}
