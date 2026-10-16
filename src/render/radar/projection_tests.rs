use super::*;
use crate::map::resolved_terrain::{ResolvedTerrainCell, ResolvedTerrainGrid};
use crate::map::terrain::{TerrainGrid, build_terrain_grid_from_resolved};
use crate::render::radar::helpers::OverlayClassification;
use std::collections::{BTreeMap, HashMap};

const SIZE_WIDTH: u16 = 80;
const SIZE_HEIGHT: u16 = 60;
const GRID_SIDE: u16 = 201;

fn wide_bounds() -> PlayfieldBounds {
    PlayfieldBounds::from_normalized_local_size(80, 2, 2, 73, 54)
}

fn shrunken_bounds() -> PlayfieldBounds {
    PlayfieldBounds::from_normalized_local_size(80, 10, 10, 50, 40)
}

fn fixture() -> (TerrainGrid, ResolvedTerrainGrid) {
    let special = BTreeMap::from([
        // raw_x=-1: native clipping retains only this cell's right half.
        ((12, 88), ([20, 30, 40], [100, 120, 140])),
        // raw_x=raw_w-1: native clipping retains only the left half.
        ((85, 15), ([160, 180, 200], [30, 40, 50])),
        // raw_y=109 is the final source row. Active CW 0x0E7F truncates its
        // generated center to y=104; the row still must reach the sampler.
        ((97, 97), ([200, 20, 20], [20, 200, 20])),
    ]);
    let cells = (0..GRID_SIDE)
        .flat_map(|ry| {
            let special = &special;
            (0..GRID_SIDE).map(move |rx| {
                let (left, right) = special
                    .get(&(rx, ry))
                    .copied()
                    .unwrap_or(([2, 2, 2], [2, 2, 2]));
                flat_cell(rx, ry, left, right)
            })
        })
        .collect();
    let allocated = (0..GRID_SIDE)
        .flat_map(|ry| {
            (0..GRID_SIDE)
                .filter_map(move |rx| native_size_diamond_cell(rx, ry).then_some((rx, ry)))
        })
        .collect::<Vec<_>>();
    assert_eq!(
        allocated.len(),
        SIZE_HEIGHT as usize * (2 * SIZE_WIDTH as usize - 1)
    );
    let mut resolved = ResolvedTerrainGrid::from_cells(GRID_SIDE, GRID_SIDE, cells);
    resolved.test_set_native_allocated_cells(&allocated);
    let grid = build_terrain_grid_from_resolved(&resolved, None, None);
    assert_eq!(grid.cells.len(), allocated.len());
    (grid, resolved)
}

fn native_size_diamond_cell(rx: u16, ry: u16) -> bool {
    let x = i32::from(rx);
    let y = i32::from(ry);
    let width = i32::from(SIZE_WIDTH);
    let height = i32::from(SIZE_HEIGHT);
    width < x + y && x - y < width && y - x < width && x + y <= width + 2 * height
}

#[test]
fn projection_raw_fill_keeps_clipped_edges_and_final_source_row() {
    let (grid, resolved) = fixture();
    let projection = MinimapPlayfieldProjection::derive(
        &grid,
        Some(&resolved),
        &[],
        &HashMap::new(),
        "TEMPERATE",
        Some(wide_bounds()),
        None,
    );
    let geometry = projection.native_radar_surface.expect("native surface");
    assert_eq!(geometry.raw_size(), (146, 110));
    assert_eq!(geometry.generated_size(), (140, 105));

    let surface = projection
        .native_radar_terrain
        .as_ref()
        .expect("native terrain");
    let raw = surface.raw_rgb();
    assert_eq!(raw[15 * 146], [10, 15, 20], "left clip keeps right");
    assert_eq!(raw[15 * 146 + 145], [80, 90, 100], "right clip keeps left");
    assert_eq!(geometry.cell_to_raw_pixel((97, 97)), (75, 109));
    assert_eq!(geometry.cell_to_surface_pixel((97, 97)).1, 104);
    assert_eq!(
        &raw[109 * 146 + 75..109 * 146 + 77],
        &[[100, 10, 10], [100, 10, 10]]
    );

    // `Math__ftol @ 0x007C5F00` runs under live CW 0x0E7F (truncate).
    // Therefore the critic's proposed 109 -> 105 boundary is excluded: in
    // this 146x110 fixture every cell whose raw footprint intersects the
    // source has an in-bounds generated center. Raw-fill admission remains
    // independent of that incidental fact, exactly like 0x00654EA0.
    for cell in &grid.cells {
        let origin = geometry.cell_to_raw_pixel((cell.rx, cell.ry));
        let raw_intersects = origin.1 >= 0
            && origin.1 < geometry.raw_size().1
            && origin.0 < geometry.raw_size().0
            && origin.0 + 1 >= 0;
        if raw_intersects {
            let center = geometry.cell_to_surface_pixel((cell.rx, cell.ry));
            assert!(
                center.0 >= 0
                    && center.0 < geometry.generated_size().0
                    && center.1 >= 0
                    && center.1 < geometry.generated_size().1,
                "raw-intersecting cell ({},{}) projected outside at {center:?}",
                cell.rx,
                cell.ry,
            );
        }
    }

    let expected_raw = reference_raw(&grid, geometry);
    assert_eq!(raw, expected_raw);
    // Numeric generation is checked against executed original6547C0 in
    // terrain_tests; this test owns projection admission and raw source fill.
}

#[test]
fn action40_projection_rebuild_changes_geometry_and_raw_source_without_stale_data() {
    let (grid, resolved) = fixture();
    let initial = MinimapPlayfieldProjection::derive(
        &grid,
        Some(&resolved),
        &[],
        &HashMap::new(),
        "TEMPERATE",
        Some(wide_bounds()),
        None,
    );
    let rebuilt = MinimapPlayfieldProjection::derive(
        &grid,
        Some(&resolved),
        &[],
        &HashMap::new(),
        "TEMPERATE",
        Some(shrunken_bounds()),
        None,
    );
    let initial_geometry = initial.native_radar_surface.expect("initial surface");
    let rebuilt_geometry = rebuilt.native_radar_surface.expect("rebuilt surface");
    assert_eq!(initial_geometry.raw_size(), (146, 110));
    assert_eq!(rebuilt_geometry.raw_size(), (100, 82));

    let initial = initial.native_radar_terrain.expect("initial terrain");
    let rebuilt = rebuilt.native_radar_terrain.expect("rebuilt terrain");
    assert_ne!(initial.raw_rgb(), rebuilt.raw_rgb());
    let expected = reference_raw(&grid, rebuilt_geometry);
    assert_eq!(rebuilt.raw_rgb(), expected);
}

#[test]
fn cell_get_radar_color_precedence_feeds_raw_surface_before_weighted_generation() {
    let (mut grid, mut resolved) = fixture();
    resolved
        .cell_mut(50, 50)
        .expect("terrain object cell")
        .terrain_object_occupation = Some(0);
    resolved
        .cell_mut(50, 51)
        .expect("structural bridge cell")
        .bridge_facts
        .raw_flags = crate::map::bridge_facts::BRIDGE_FLAG_STRUCTURAL;
    resolved.test_set_radar_color_valid(51, 50, false);
    resolved.test_set_radar_color_valid(52, 50, true);
    let black = grid
        .cells
        .iter_mut()
        .find(|cell| (cell.rx, cell.ry) == (52, 50))
        .expect("valid black TMP cell");
    black.radar_left = [0; 3];
    black.radar_right = [99; 3];
    let overlays = [
        MinimapOverlayDatum {
            rx: 50,
            ry: 50,
            classification: OverlayClassification::Wall,
            source: MinimapCellRadarSource::Overlay {
                overlay_id: 10,
                frame: 4,
                is_tiberium: false,
                has_cell_anim: false,
                has_tiberium_type: false,
            },
        },
        MinimapOverlayDatum {
            rx: 50,
            ry: 51,
            classification: OverlayClassification::Wall,
            source: MinimapCellRadarSource::Overlay {
                overlay_id: 10,
                frame: 4,
                is_tiberium: false,
                has_cell_anim: false,
                has_tiberium_type: false,
            },
        },
        MinimapOverlayDatum {
            rx: 97,
            ry: 97,
            classification: OverlayClassification::Wall,
            source: MinimapCellRadarSource::Overlay {
                overlay_id: 10,
                frame: 4,
                is_tiberium: false,
                has_cell_anim: false,
                has_tiberium_type: false,
            },
        },
        MinimapOverlayDatum {
            rx: 90,
            ry: 90,
            classification: OverlayClassification::Ore,
            source: MinimapCellRadarSource::Overlay {
                overlay_id: 102,
                frame: 11,
                is_tiberium: true,
                has_cell_anim: false,
                has_tiberium_type: true,
            },
        },
        MinimapOverlayDatum {
            rx: 85,
            ry: 85,
            classification: OverlayClassification::Ore,
            source: MinimapCellRadarSource::Overlay {
                overlay_id: 103,
                frame: 11,
                is_tiberium: true,
                has_cell_anim: true,
                has_tiberium_type: true,
            },
        },
        MinimapOverlayDatum {
            rx: 80,
            ry: 80,
            classification: OverlayClassification::Ore,
            source: MinimapCellRadarSource::Overlay {
                overlay_id: 104,
                frame: 11,
                is_tiberium: true,
                has_cell_anim: true,
                has_tiberium_type: true,
            },
        },
    ];
    let colors = HashMap::from([
        ((24, 0), [5, 6, 7]),
        ((10, 4), [11, 12, 13]),
        ((102, 11), [169, 155, 61]),
        ((103, 11), [31, 41, 59]),
    ]);
    let projection = MinimapPlayfieldProjection::derive(
        &grid,
        Some(&resolved),
        &overlays,
        &colors,
        "TEMPERATE",
        Some(wide_bounds()),
        None,
    );
    let geometry = projection.native_radar_surface.expect("surface");
    let terrain = projection.native_radar_terrain.expect("terrain");
    let raw = terrain.raw_rgb().to_vec();
    let pair = |cell| {
        let (x, y) = geometry.cell_to_raw_pixel(cell);
        [raw[(y * 146 + x) as usize], raw[(y * 146 + x + 1) as usize]]
    };

    assert_eq!(pair((50, 50)), [[200, 200, 160]; 2], "TerrainClass wins");
    assert_eq!(pair((50, 51)), [[5, 6, 7]; 2], "flag 0x100 uses BRIDGE1 f0");
    assert_eq!(
        pair((97, 97)),
        [[11, 12, 13]; 2],
        "ordinary overlay wins TMP"
    );
    assert_eq!(
        pair((90, 90)),
        [[170, 170, 130]; 2],
        "stock no-CellAnim tiberium ignores its own SHP header",
    );
    assert_eq!(pair((85, 85)), [[31, 41, 59]; 2], "CellAnim header wins");
    assert_eq!(pair((80, 80)), [[0, 0, 0]; 2], "missing CellAnim is black");
    assert_eq!(
        pair((51, 50)),
        [[60, 60, 60]; 2],
        "missing subimage fallback"
    );
    assert_eq!(pair((52, 50)), [[0, 0, 0]; 2], "valid black remains black");
}

fn reference_raw(grid: &TerrainGrid, geometry: NativeRadarSurfaceGeometry) -> Vec<[u8; 3]> {
    let size = geometry.raw_size();
    let mut raw = vec![[0; 3]; (size.0 * size.1) as usize];
    for cell in &grid.cells {
        let (left, right) = radar_colors_for_cell(cell, true, 1.0);
        let origin = geometry.cell_to_raw_pixel((cell.rx, cell.ry));
        for (dx, color) in [(0, left), (1, right)] {
            let x = origin.0 + dx;
            let y = origin.1;
            if x >= 0 && x < size.0 && y >= 0 && y < size.1 {
                raw[(y * size.0 + x) as usize] = color;
            }
        }
    }
    raw
}

fn flat_cell(rx: u16, ry: u16, radar_left: [u8; 3], radar_right: [u8; 3]) -> ResolvedTerrainCell {
    ResolvedTerrainCell {
        radar_left,
        radar_right,
        ..crate::map::resolved_terrain::test_smudge_cell(rx, ry)
    }
}
