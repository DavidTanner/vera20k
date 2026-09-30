//! Original Object GetHeight/low/high queries from retained physical XYZ.
use super::*;
use crate::map::resolved_terrain::{ResolvedTerrainGrid, test_flat_cell};
use crate::sim::game_entity::GameEntity;
use serde::Deserialize;

#[derive(Deserialize)]
struct Input {
    name: String,
    marked: u8,
    on_bridge: u8,
    xyz: [i32; 3],
    cell_level: u8,
}
#[derive(Deserialize)]
struct Row {
    input: Input,
    height: i32,
    low_flying: u8,
    high_flying: u8,
    dummy_xy: [i16; 2],
}
#[derive(Deserialize)]
struct Corpus {
    native_sha256: String,
    rows: Vec<Row>,
}

#[test]
fn native_object_flight_queries_use_live_ground_bridge_and_mark() {
    let corpus: Corpus = serde_json::from_str(include_str!(
        "../../../tools/spatial_oracle/object_flight_height.json"
    ))
    .unwrap();
    assert_eq!(
        corpus.native_sha256,
        "1cdd1180e49024fbda8ad568caac2e86e856063ff67ab38f62b7d2c7bb84298c"
    );
    assert_eq!(corpus.rows.len(), 24);
    for row in corpus.rows {
        let mut cells: Vec<_> = (0..25)
            .flat_map(|y| (0..25).map(move |x| test_flat_cell(x, y)))
            .collect();
        cells[20 * 25 + 10].level = row.input.cell_level;
        cells[20 * 25 + 10].bridge_facts.raw_flags = 0x100;
        let mut terrain = ResolvedTerrainGrid::from_cells(25, 25, cells);
        terrain.test_set_native_allocated_cells(&[(10, 20)]);
        let dummy = terrain.shared_cell_dummy();
        dummy.stamp_coord(111, -222);
        let [x, y, z] = row.input.xyz;
        let mut entity =
            GameEntity::test_default(1, "FV", "Americans", (x / 256) as u16, (y / 256) as u16);
        entity.position.sub_x = SimFixed::from_num(x % 256);
        entity.position.sub_y = SimFixed::from_num(y % 256);
        entity.position.exact_z_leptons = Some(z);
        entity.on_bridge = row.input.on_bridge != 0;
        entity.lifecycle.cell_marked = row.input.marked != 0;
        assert_eq!(
            current_fly_height(&entity, Some(&terrain)),
            row.height,
            "{} height",
            row.input.name
        );
        assert_eq!(
            is_low_flying(&entity, Some(&terrain), None),
            row.low_flying != 0,
            "{} low",
            row.input.name
        );
        assert_eq!(
            is_high_flying(&entity, Some(&terrain), None),
            row.high_flying != 0,
            "{} high",
            row.input.name
        );
        assert_eq!(
            dummy.snapshot().coord,
            (i32::from(row.dummy_xy[0]), i32::from(row.dummy_xy[1])),
            "{} dummy",
            row.input.name
        );
    }
}

/// Paradrop (`0x005F5940`) raises the falling byte (`0x005F5965`), and each
/// frame `ObjectClass::AI`'s falling block writes the Location Z
/// (`0x005F3F2C..0x005F3F60`). So GetHeight (`0x005F5F40`) is the descent
/// height, and IsInAir (`0x005F6B90`) holds while it is at least two levels.
/// VERA keeps that height in the descent state, without an exact Z.
#[test]
fn a_descending_paratrooper_reads_its_descent_height() {
    let cells: Vec<_> = (0..25)
        .flat_map(|y| (0..25).map(move |x| test_flat_cell(x, y)))
        .collect();
    let terrain = ResolvedTerrainGrid::from_cells(25, 25, cells);
    let mut entity = GameEntity::test_default(1, "E1", "Americans", 10, 10);
    entity.lifecycle.cell_marked = true;
    entity.parachute_state = Some(
        crate::sim::movement::parachute_descent::ParachuteDescentState {
            rate: -3,
            altitude: SimFixed::from_num(1200),
        },
    );
    assert_eq!(current_fly_height(&entity, Some(&terrain)), 1200);
    assert!(is_high_flying(&entity, Some(&terrain), None));

    entity.parachute_state.as_mut().unwrap().altitude = SimFixed::from_num(207);
    assert_eq!(current_fly_height(&entity, Some(&terrain)), 207);
    assert!(is_low_flying(&entity, Some(&terrain), None));
}
