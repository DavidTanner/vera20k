//! Original enqueue/rebuild and connected caller goldens, not a Rust model.
//! Scope, fixture seams and reproduction: tools/spatial_oracle/ore_queue.md.
use super::*;
use crate::rules::ini_parser::IniFile;
use crate::rules::ruleset::RuleSet;
use crate::sim::entity_store::EntityStore;
use crate::sim::intern::StringInterner;
use crate::sim::movement::locomotor::MovementLayer;
use crate::sim::occupancy::{CellListInsertion, OccupancyGrid};
use crate::sim::tiberium::{ReduceTiberiumContext, reduce_tiberium};
use serde_json::{Value, json};
use std::collections::BTreeMap;

fn at(value: &Value) -> (u16, u16) {
    (
        value[0].as_u64().unwrap() as u16,
        value[1].as_u64().unwrap() as u16,
    )
}

fn types(input: &Value) -> (OverlayTypeRegistry, RuleSet) {
    let text = crate::sim::tiberium::test_support::tiberium_rules_text().replace(
        "[Tiberiums]\n0=Riparius\n1=Cruentus\n",
        "[Tiberiums]\n0=Riparius\n1=Cruentus\n2=Vinifera\n3=Aboreus\n",
    );
    let mut overrides = String::new();
    for (i, name) in ["Riparius", "Cruentus", "Vinifera", "Aboreus"]
        .iter()
        .enumerate()
    {
        overrides.push_str(&format!(
            "\n[{name}]\nImage={}\nGrowthPercentage={}\nSpreadPercentage={}\n",
            i + 1,
            input["growth_percentages"][i],
            input["spread_percentages"][i]
        ));
    }
    let mut ini = IniFile::from_str(&text);
    ini.merge(&IniFile::from_str(&overrides));
    (
        OverlayTypeRegistry::from_ini(&ini, None),
        RuleSet::from_ini_with_fixed_art_for_test(&ini, &IniFile::from_str("")).unwrap(),
    )
}

fn seed_queue(
    input: &Value,
    class: usize,
    name: &str,
    capacity: u32,
) -> (NativeTiberiumQueue, BTreeSet<(u16, u16)>) {
    let selected = class == input["receiver"].as_u64().unwrap() as usize
        && name == input["queue"].as_str().unwrap();
    let count = if selected {
        input["pre_count"].as_u64().unwrap() as usize
    } else if class == input["receiver"].as_u64().unwrap() as usize {
        input["other_pre_counts"][name].as_u64().unwrap_or(1) as usize
    } else {
        1
    };
    let cell = at(&input["pre_cell"]);
    let entries = vec![
        NativeTiberiumQueueEntry {
            rx: cell.0,
            ry: cell.1,
            priority_bits: input["pre_priority_bits"].as_u64().unwrap() as u32
        };
        count
    ];
    let mut heap = vec![0];
    if selected {
        heap.extend(
            input["pre_heap_indices"]
                .as_array()
                .unwrap()
                .iter()
                .map(|i| i.as_u64().unwrap() as u32),
        );
    } else {
        heap.push(0);
    }
    let bitmap = if selected {
        input["pre_bitmap"]
            .as_array()
            .unwrap()
            .iter()
            .map(at)
            .collect()
    } else {
        BTreeSet::from([(2, 3)])
    };
    (
        NativeTiberiumQueue {
            entries,
            heap,
            capacity,
        },
        bitmap,
    )
}

fn queue_state(queue: &NativeTiberiumQueue, bitmap: &BTreeSet<(u16, u16)>) -> Value {
    json!({"array_count": queue.array_len(),
        "entries": queue.entries.iter().map(|e| json!({"cell":[e.rx,e.ry],"priority_bits":e.priority_bits})).collect::<Vec<_>>(),
        "heap_indices": &queue.heap[1..],
        "bitmap_cells": bitmap.iter().map(|&(x,y)| [x,y]).collect::<Vec<_>>()})
}

fn compare_state(
    state: &OreGrowthState,
    rng: &SimRng,
    expected: &Value,
    capacity: u32,
    name: &str,
) {
    assert_eq!(
        state.native_tiberium.classes.len(),
        4,
        "{name}: class count"
    );
    for (i, class) in state.native_tiberium.classes.iter().enumerate() {
        let native = &expected["classes"][i];
        assert_eq!(native["type_id"], i, "{name}: class identity");
        for (kind, queue, bitmap) in [
            ("growth", &class.growth, &class.growth_bitmap),
            ("spread", &class.spread, &class.spread_bitmap),
        ] {
            assert_eq!(queue.capacity(), capacity, "{name}: {i} {kind} capacity");
            let mut expected_queue = native[kind].clone();
            expected_queue
                .as_object_mut()
                .unwrap()
                .remove("bitmap_indices");
            assert_eq!(
                queue_state(queue, bitmap),
                expected_queue,
                "{name}: class {i} {kind}"
            );
        }
        assert_eq!(
            json!([
                class.growth_timer.start_frame(),
                0,
                class.growth_timer.duration()
            ]),
            native["growth_timer"],
            "{name}: {i} growth timer"
        );
        assert_eq!(
            json!([
                class.spread_timer.start_frame(),
                0,
                class.spread_timer.duration()
            ]),
            native["spread_timer"],
            "{name}: {i} spread timer"
        );
    }
    let view = rng.logical_view();
    assert_eq!(
        json!([view.index_a, view.index_b]),
        expected["rng_indices"],
        "{name}: RNG indices"
    );
    let hex = rng.native_state_hex();
    let bytes: Vec<_> = hex
        .as_bytes()
        .chunks_exact(2)
        .map(|b| u8::from_str_radix(std::str::from_utf8(b).unwrap(), 16).unwrap())
        .collect();
    assert_eq!(
        crate::util::sha256::sha256_hex(&bytes),
        expected["rng_sha256"].as_str().unwrap(),
        "{name}: full RNG state"
    );
}

#[test]
fn enqueue_rebuilds_and_callers_match_native_queue_arrays_heaps_bitmaps_timers_and_rng() {
    let corpus: Value =
        serde_json::from_str(include_str!("../../../tools/spatial_oracle/ore_queue.json")).unwrap();
    let rows = corpus["cases"].as_array().unwrap();
    assert_eq!(rows.len(), 60, "native case coverage");
    for row in rows {
        let input = &row["input"];
        let name = input["name"].as_str().unwrap();
        let rect = at(&input["size"]);
        let capacity = row["capacity"].as_u64().unwrap() as u32;
        assert_eq!(
            native_tiberium_queue_capacity(rect),
            capacity,
            "{name}: map capacity"
        );
        let (registry, rules) = types(input);
        let types = &rules.tiberium_types;
        for (i, ty) in types.types().iter().enumerate() {
            assert_eq!(
                ty.growth_percentage_bits,
                input["growth_percentages"][i].as_f64().unwrap().to_bits(),
                "{name}: growth reader"
            );
            assert_eq!(
                ty.spread_percentage_bits,
                input["spread_percentages"][i].as_f64().unwrap().to_bits(),
                "{name}: spread reader"
            );
        }
        let mut grid = OverlayGrid::new(33, 33);
        let mut terrain = crate::map::resolved_terrain::test_grid(33, 33, |x, y| {
            crate::map::resolved_terrain::test_loader_clear_cell(x, y)
        });
        let mut occupancy = OccupancyGrid::new();
        for c in input["cells"].as_array().unwrap() {
            let cell = at(&c["cell"]);
            let kind = c["type_id"].as_u64().unwrap();
            let variant = c["variant"].as_u64().unwrap();
            let overlay = registry
                .id_for_name(&format!(
                    "{}{:02}",
                    if kind == 0 { "TIB" } else { "GEM" },
                    variant + 1
                ))
                .unwrap();
            grid.place_overlay(
                cell.0,
                cell.1,
                overlay,
                c["density"].as_u64().unwrap() as u8,
            );
            terrain.cell_mut(cell.0, cell.1).unwrap().slope_type =
                c["slope"].as_u64().unwrap() as u8;
            if c["occupied"] == true {
                occupancy.add(
                    cell.0,
                    cell.1,
                    100,
                    MovementLayer::Ground,
                    None,
                    CellListInsertion::PrependNonBuilding,
                );
            }
        }
        let mut state = OreGrowthState::new(33, 33);
        state.reset_native_tiberium_classes_for_rect(rect, 4, 0);
        for i in 0..4 {
            let (growth, growth_bitmap) = seed_queue(input, i, "growth", capacity);
            let (spread, spread_bitmap) = seed_queue(input, i, "spread", capacity);
            let class = &mut state.native_tiberium.classes[i];
            class.growth = growth;
            class.growth_bitmap = growth_bitmap;
            class.spread = spread;
            class.spread_bitmap = spread_bitmap;
            class.growth_timer = CdTimer::from_raw(21 + i as i32, 300 + i as i32);
            class.spread_timer = CdTimer::from_raw(11 + i as i32, 200 + i as i32);
        }
        let mut rng = SimRng::new(input["seed"].as_u64().unwrap());
        if let Some(raw) = input["next_raw"].as_u64() {
            let view = rng.logical_view();
            let (a, b) = (view.index_a as usize, view.index_b as usize);
            let word = raw as u32 ^ view.words[b];
            let mut saved = serde_json::to_value(&rng).unwrap();
            saved["state"][a] = json!(word);
            rng = serde_json::from_value(saved).unwrap();
        }
        compare_state(&state, &rng, &row["before"], capacity, name);
        let receiver = TiberiumTypeId(input["receiver"].as_u64().unwrap() as u8);
        let cell = at(&input["target"]);
        let frame = input["frame"].as_i64().unwrap() as u32;
        let sources = BTreeSet::new();
        let trees = BTreeMap::new();
        let view = Some(NativeCellObjectView::new(&occupancy, &trees));
        let entities = EntityStore::new();
        let interner = StringInterner::new();
        let objects =
            TiberiumPlacementObjectContext::new(&entities, &occupancy, &rules, &interner, &trees);
        let (_, draws) = crate::sim::rng::trace_draws(|| match input["entry"].as_str().unwrap() {
            "enqueue" if input["queue"] == "growth" => {
                state.add_native_growth_queue_cell(
                    &grid,
                    &registry,
                    types,
                    Some(&terrain),
                    input["grows"] == true,
                    receiver,
                    cell.0,
                    cell.1,
                    frame,
                    &mut rng,
                );
            }
            "enqueue" => {
                state.add_native_spread_queue_cell(
                    receiver,
                    &grid,
                    &registry,
                    types,
                    Some(&terrain),
                    &sources,
                    view,
                    cell.0,
                    cell.1,
                    frame,
                    input["spreads"] == true,
                    &mut rng,
                );
            }
            "reduce" => {
                let result = reduce_tiberium(
                    &mut ReduceTiberiumContext {
                        overlay_grid: Some(&mut grid),
                        ore_growth_state: &mut state,
                        overlay_registry: Some(&registry),
                        tiberium_types: Some(types),
                        resolved_terrain: Some(&mut terrain),
                        source_object_cells: Some(&sources),
                        live_objects: view,
                        rng: Some(&mut rng),
                        binary_frame: frame,
                        spread_enabled: input["spreads"] == true,
                        radar_dirty_cells: None,
                        radar_dirty_generation: None,
                        tactical_dirty_cells: None,
                    },
                    cell,
                    input["amount"].as_i64().unwrap_or(1) as i32,
                );
                assert_eq!(
                    json!(result.removed_amount),
                    row["returned"],
                    "{name}: reduced amount"
                );
            }
            "growth_processor" => {
                state.process_native_growth_for_type_with_placement(
                    receiver,
                    &mut grid,
                    &registry,
                    types,
                    Some(&terrain),
                    &sources,
                    Some(objects),
                    &mut rng,
                    frame,
                    input["grows"] == true,
                    input["spreads"] == true,
                    None,
                    None,
                    None,
                );
            }
            other => panic!("unhandled native entry {other}"),
        });
        assert_eq!(json!(draws.len()), row["draw_count"], "{name}: RNG draws");
        compare_state(&state, &rng, &row["state"], capacity, name);
        for c in row["cells"].as_array().unwrap() {
            let cell = at(&c["cell"]);
            let overlay = grid.cell(cell.0, cell.1);
            assert_eq!(
                json!(overlay.overlay_id.map_or(-1, i32::from)),
                c["overlay"],
                "{name}: cell {cell:?} overlay"
            );
            assert_eq!(
                json!(overlay.overlay_data),
                c["density"],
                "{name}: cell {cell:?} density"
            );
        }
        let next: Vec<_> = (0..4).map(|_| rng.next_u32()).collect();
        assert_eq!(json!(next), row["next_random"], "{name}: RNG continuation");
    }
}
