//! History-sensitive navigation belongs to the live hash. Native ordinary
//! load deliberately clears/rebuilds hierarchy at67E8CD ->581F50; it is not a
//! byte-for-byte rollback. These flat fixtures bound the Rust restore helper.

use super::Simulation;
use crate::map::resolved_terrain::{ResolvedTerrainGrid, test_flat_cell};
use crate::sim::pathfinding::PathGrid;
use crate::sim::pathfinding::zone_hierarchy::ZoneRecord;
use crate::sim::pathfinding::zone_incremental::repair_zone_hierarchy_around_cell;
use crate::sim::pathfinding::zone_map::ZoneGrid;
use std::hash::Hasher;

// Supplied branch-test bounds, as in zone_map_tests::repair_test_bounds. They
// admit the retained flat cells through the native mode1 query; None rejects.
fn bounds() -> crate::map::playfield::PlayfieldBounds {
    crate::map::playfield::PlayfieldBounds {
        base: 0,
        off_fc: -1_000,
        off_100: -1_000,
        off_104: 2_000,
        off_108: 2_000,
    }
}

fn terrain() -> ResolvedTerrainGrid {
    ResolvedTerrainGrid::from_cells(
        16,
        16,
        (0..16)
            .flat_map(|y| (0..16).map(move |x| test_flat_cell(x, y)))
            .collect(),
    )
}

fn world(terrain: ResolvedTerrainGrid) -> Simulation {
    let path = PathGrid::from_resolved_terrain(&terrain);
    let zones = ZoneGrid::build_with_native_map_context(&path, &terrain, &[], None, Some(bounds()));
    let mut sim = Simulation::new();
    sim.playfield_bounds = Some(bounds());
    sim.install_resolved_terrain_for_new_map(terrain);
    sim.zone_grid = Some(zones);
    sim
}

fn navigation_hash(sim: &Simulation) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    sim.zone_grid
        .as_ref()
        .unwrap()
        .fold_navigation_history(&mut hasher);
    hasher.finish()
}

#[test]
fn identical_source_and_houses_with_different_retained_threat_seed_hash_differently() {
    let mut a = world(terrain());
    let mut b = world(terrain());
    assert_eq!(a.state_hash(), b.state_hash());
    let zones = b.zone_grid.as_mut().unwrap();
    let mut hierarchy = zones
        .hierarchy_for(crate::rules::locomotor_type::MovementZone::Normal)
        .unwrap()
        .clone();
    let graph = &mut hierarchy.levels_mut()[1];
    let zone = graph.zone_at(4, 4);
    let record = graph.record(zone).unwrap();
    let seed = if record.native_threat_index() == Some(131) {
        (4, 0)
    } else {
        (0, 0)
    };
    graph.set_record(ZoneRecord::from_seed(
        zone,
        record.parent,
        record.zone_type,
        seed,
    ));
    zones.replace_hierarchy(hierarchy);
    assert_ne!(
        a.state_hash(),
        b.state_hash(),
        "585F40 now reads a different House grid cell"
    );
    // Removing just this owner proves every other retained source/House field
    // in the world hash stayed identical; no second authority was introduced.
    a.zone_grid = None;
    b.zone_grid = None;
    assert_eq!(a.state_hash(), b.state_hash());
}

#[test]
fn ordinary_snapshot_rebuild_discards_local_hierarchy_history() {
    use crate::sim::snapshot::GameSnapshot;
    let terrain = terrain();
    let canonical = world(terrain.clone());
    let mut live = world(terrain.clone());
    let slots = live
        .zone_grid
        .as_ref()
        .unwrap()
        .hierarchy_for(crate::rules::locomotor_type::MovementZone::Normal)
        .unwrap()
        .level(1)
        .unwrap()
        .record_slot_count();
    repair_zone_hierarchy_around_cell(
        live.zone_grid.as_mut().unwrap(),
        (5, 5),
        Some(bounds()),
        &terrain,
        &[],
    );
    assert!(
        live.zone_grid
            .as_ref()
            .unwrap()
            .hierarchy_for(crate::rules::locomotor_type::MovementZone::Normal)
            .unwrap()
            .level(1)
            .unwrap()
            .record_slot_count()
            > slots
    );
    assert_ne!(navigation_hash(&live), navigation_hash(&canonical));
    let bytes = GameSnapshot::save(&live, 0, 0, "flat-navigation-history", 0);
    let mut restored = GameSnapshot::load(&bytes).unwrap().sim;
    let saved_base = bincode::serialize(live.zone_grid.as_ref().unwrap()).unwrap();
    let pending = restored.zone_grid.as_ref().expect("saved base navigation");
    assert!(pending.is_native_load_pending());
    assert_eq!(bincode::serialize(pending).unwrap(), saved_base);
    restored.install_resolved_terrain_for_new_map(terrain);
    assert!(restored.rebuild_dynamic_navigation(&crate::sim::runtime::SimResources::empty().rules));
    assert!(
        !restored
            .zone_grid
            .as_ref()
            .unwrap()
            .is_native_load_pending()
    );
    assert_eq!(
        bincode::serialize(restored.zone_grid.as_ref().unwrap()).unwrap(),
        saved_base
    );
    assert_eq!(navigation_hash(&restored), navigation_hash(&canonical));
    assert_ne!(navigation_hash(&restored), navigation_hash(&live));
}
