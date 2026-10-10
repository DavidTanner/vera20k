//! Original growth-before-spread and spread driver histories through the app's
//! authoritative frame entry. Native bodies/inputs: spatial_oracle/ore_queue.md.
use super::*;
use crate::sim::ore_growth::OreGrowthConfig;
use crate::sim::ore_growth::queue_oracle_tests::{
    SuppliedFixture, compare_cells, compare_dummy, compare_overlay_state, compare_state, corpus,
    prepare_native_dummy, supplied_fixture,
};
use serde_json::{Value, json};

fn world(
    row: &Value,
) -> (
    Simulation,
    RuleSet,
    crate::rules::overlay_types::OverlayTypeRegistry,
) {
    let SuppliedFixture {
        registry,
        rules,
        grid,
        terrain,
        state,
        rng,
        entities,
        occupancy,
        interner,
        trees,
        sources,
    } = supplied_fixture(row);
    let mut sim = super::refinery_dock_oracle_tests::world_with(
        &rules,
        &crate::rules::ini_parser::IniFile::from_str("[Clear]\nBuildable=yes\n"),
        |base| *base = terrain,
    );
    sim.overlay_grid = Some(grid);
    sim.production.ore_growth_state = state;
    sim.scenario_rng = rng;
    sim.substrate.entities = entities;
    sim.substrate.occupancy = occupancy;
    sim.interner = interner;
    sim.intern_rule_type_ids(&rules);
    sim.resolve_type_handles(&rules);
    sim.production.terrain_object_cells = trees;
    sim.production.tiberium_spawning_terrain_cells = sources;
    let input = &row["input"];
    sim.native_unique_ids = Some(
        crate::sim::native_identity::NativeUniqueIdCursor::test_at_current_value(
            input["scenario_serial"].as_u64().unwrap_or(0) as u32,
        ),
    );
    let width = input["size"][0].as_i64().unwrap() as i32;
    let height = input["size"][1].as_i64().unwrap() as i32;
    sim.playfield_bounds = Some(
        crate::map::playfield::PlayfieldBounds::from_normalized_local_size(
            width, 0, 0, width, height,
        ),
    );
    sim.playfield_size_height = Some(height);
    sim.production.ore_growth_config = OreGrowthConfig {
        grows: input["grows"] == true,
        spreads: input["spreads"] == true,
        tiberium_grows_flag: input["fast_growth"] == true,
    };
    prepare_native_dummy(sim.resolved_terrain.as_ref().unwrap());
    (sim, rules, registry)
}

#[test]
fn ore_rung_publishes_native_cell_attributes_before_live_object_turns() {
    let corpus = corpus();
    let row = corpus["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["input"]["name"] == "spread_driver_original_retail_reader_values")
        .unwrap();
    let (mut sim, rules, registry) = world(row);
    let name = row["input"]["name"].as_str().unwrap();
    sim.session.binary_frame = row["input"]["frame"].as_i64().unwrap() as u32;
    sim.tick_ore_growth_rungs(&rules, Some(&registry));
    compare_overlay_state(&sim, &row["steps"][0]["state"], name);
    compare_dummy(
        sim.resolved_terrain.as_ref().unwrap(),
        &row["steps"][0]["state"],
        name,
    );
    compare_cells(
        sim.overlay_grid.as_ref().unwrap(),
        sim.resolved_terrain.as_ref().unwrap(),
        &row["cells"],
        name,
    );
    let marked = sim.overlay_grid.as_ref().unwrap().pending_dirty_cells();
    assert!(
        !marked.is_empty(),
        "inline projection must retain render updates"
    );
    for &(x, y) in marked {
        let cell = sim.resolved_terrain.as_ref().unwrap().cell(x, y).unwrap();
        assert!(
            sim.path_grid
                .as_ref()
                .unwrap()
                .resolved_cell_is_current(cell, false)
        );
        for (&speed_type, costs) in &sim.terrain_costs {
            assert_eq!(
                costs.ground_cost_at(x, y),
                cell.speed_costs
                    .cost_for_speed_type(speed_type)
                    .unwrap_or(0),
                "{name}: new ore's movement row is visible before object AI"
            );
        }
    }
}

#[test]
fn app_frames_match_original_ore_driver_histories_and_rng_continuation() {
    let corpus = corpus();
    let rows: Vec<_> = corpus["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| {
            matches!(
                row["input"]["entry"].as_str().unwrap(),
                "spread_driver" | "growth_then_spread_driver"
            )
        })
        .collect();
    assert_eq!(rows.len(), 21, "original driver histories");
    for row in rows {
        let input = &row["input"];
        let name = input["name"].as_str().unwrap();
        let capacity = row["capacity"].as_u64().unwrap() as u32;
        let (mut sim, rules, registry) = world(row);
        compare_state(
            &sim.production.ore_growth_state,
            &sim.scenario_rng,
            &row["before"],
            capacity,
            name,
        );
        let mut total_draws = 0;
        for (index, step) in row["steps"].as_array().unwrap().iter().enumerate() {
            // Native frame N is consumed before MainTick increments it. Gaps
            // in supplied histories are fixture inputs, not simulated frames.
            let frame = step["frame"].as_i64().unwrap() as u32;
            sim.session.binary_frame = frame;
            let (output, draws) = crate::sim::rng::trace_draws(|| {
                sim.advance_app_frame(
                    &[],
                    Some(&rules),
                    Some(&registry),
                    67,
                    TickLane::Ordinary,
                    None,
                 crate::sim::world::FrameEffects::default(),)
                .unwrap()
            });
            assert!(output.tick.frame_committed, "{name}: app frame {index}");
            assert_eq!(sim.session.binary_frame, frame.wrapping_add(1));
            total_draws += draws.len();
            assert_eq!(
                json!(draws.len()),
                step["draw_count"],
                "{name}: step {index} draws"
            );
            compare_state(
                &sim.production.ore_growth_state,
                &sim.scenario_rng,
                &step["state"],
                capacity,
                &format!("{name}: step {index}"),
            );
            compare_dummy(sim.resolved_terrain.as_ref().unwrap(), &step["state"], name);
            compare_cells(
                sim.overlay_grid.as_ref().unwrap(),
                sim.resolved_terrain.as_ref().unwrap(),
                &step["cells"],
                name,
            );
            assert_eq!(
                json!(sim.native_unique_ids.as_ref().unwrap().current_raw()),
                step["state"]["scenario_serial"],
                "{name}: app constructor IDs follow the native ore history",
            );
            assert_eq!(
                sim.load_objects.queue_count(),
                0,
                "{name}: app tail drains dead Overlays"
            );
        }
        assert_eq!(json!(total_draws), row["draw_count"], "{name}: total draws");
        let next: Vec<_> = (0..4).map(|_| sim.scenario_rng.next_u32()).collect();
        assert_eq!(json!(next), row["next_random"], "{name}: RNG continuation");
    }
}

#[test]
fn app_ore_constructor_terminal_cleanup_matches_original_deferred_drain() {
    let corpus = corpus();
    let rows: Vec<_> = corpus["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| row["input"]["drain_deferred"] == true)
        .collect();
    assert!(!rows.is_empty(), "original deferred-lifecycle controls");
    for row in rows {
        let name = row["input"]["name"].as_str().unwrap();
        let (mut sim, rules, registry) = world(row);
        sim.session.binary_frame = row["input"]["frame"].as_i64().unwrap() as u32;
        let (output, draws) = crate::sim::rng::trace_draws(|| {
            sim.advance_app_frame(
                &[],
                Some(&rules),
                Some(&registry),
                67,
                TickLane::Ordinary,
                None,
             crate::sim::world::FrameEffects::default(),)
            .unwrap()
        });
        assert!(output.tick.frame_committed, "{name}: admitted app frame");
        compare_overlay_state(&sim, &row["after_drains"][0]["state"], name);
        compare_cells(
            sim.overlay_grid.as_ref().unwrap(),
            sim.resolved_terrain.as_ref().unwrap(),
            &row["after_drains"][0]["cells"],
            name,
        );
        assert_eq!(
            json!(draws.len()),
            row["draw_count"],
            "{name}: constructor path RNG"
        );
        let next: Vec<_> = (0..4).map(|_| sim.scenario_rng.next_u32()).collect();
        assert_eq!(
            json!(next),
            row["next_random"],
            "{name}: constructor RNG continuation"
        );
    }
}
