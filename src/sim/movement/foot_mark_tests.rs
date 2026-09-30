use super::*;
use crate::rules::jumpjet_params::JumpjetParams;
use crate::rules::locomotor_type::LocomotorKind;
use crate::sim::game_entity::GameEntity;
use crate::sim::movement::locomotor::LocomotorState;
use crate::util::fixed_math::SimFixed;

fn fixture(category: EntityCategory, kind: LocomotorKind) -> Simulation {
    let mut sim = Simulation::new();
    let mut entity = GameEntity::test_default(1, "ACTOR", "Americans", 4, 4);
    entity.category = category;
    entity.lifecycle.in_limbo = false;
    entity.lifecycle.cell_marked = false;
    entity.foot_occupation_enabled = true;
    entity.locomotor = Some(LocomotorState::for_test_kind(kind));
    entity.position.exact_z_leptons = Some(0);
    sim.substrate.entities.insert(entity);
    sim.interner = crate::sim::intern::test_interner();
    sim
}

/// A Jumpjet cruising at its `JumpjetHeight=`: `0x0054B8D0` answers Top
/// while it is marked, Ground once +0x74 is clear.
fn cruising_jumpjet() -> Simulation {
    let mut sim = fixture(EntityCategory::Unit, LocomotorKind::Jumpjet);
    let entity = sim.substrate.entities.get_mut(1).unwrap();
    let locomotor = entity.locomotor.as_mut().unwrap();
    locomotor
        .jumpjet_runtime_mut()
        .expect("jumpjet runtime")
        .link(&JumpjetParams {
            turn_rate: 4,
            speed: SimFixed::from_num(14),
            climb: 5.0,
            crash: 5.0,
            height: 500,
            accel: 2.0,
            wobbles: 0.15,
            deviation: 40,
            no_wobbles: false,
        });
    locomotor.altitude = SimFixed::from_num(500);
    entity.position.exact_z_leptons = Some(500);
    sim
}

/// Add/RemoveContent return for Infantry (`0x0047E9EA` / `0x0047EAFE`)
/// before the Foot enable and the object's vt+0xF0/+0xF4: Unit `0x007441B0`
/// writes 0x20, Aircraft's ObjectClass `0x005F60A0` writes 0x40.
#[test]
fn mark_writes_the_unit_and_aircraft_raw_bits_and_never_an_infantrymans() {
    for (category, kind, expected_bits) in [
        (EntityCategory::Unit, LocomotorKind::Drive, 0x20),
        (EntityCategory::Aircraft, LocomotorKind::Fly, 0x40),
        (EntityCategory::Infantry, LocomotorKind::Walk, 0x00),
    ] {
        let mut sim = fixture(category, kind);
        sim.foot_mark_put(1, None, None);
        assert!(sim.substrate.occupancy.contains_entity(4, 4, 1));
        assert_eq!(
            sim.substrate.raw_cell_occupation.ground_bits(4, 4),
            expected_bits,
            "{category:?}"
        );
        sim.foot_mark_put(1, None, None);
        assert_eq!(
            sim.substrate
                .occupancy
                .count_on_layer(4, 4, MovementLayer::Ground),
            1
        );
        sim.foot_mark_remove(1, None, None);
        assert!(!sim.substrate.occupancy.contains_entity(4, 4, 1));
        assert_eq!(sim.substrate.raw_cell_occupation.ground_bits(4, 4), 0);
        assert!(!sim.substrate.entities.get(1).unwrap().lifecycle.cell_marked);
    }
}

/// Mark toggles +0x74 for an airborne Fly object, but `0x004CFCF0` answers
/// Top above height 0, so neither Pick_Up nor Place_Down runs
/// (`0x004D37A9..0x004D37AC`).
#[test]
fn an_airborne_fly_object_is_marked_without_joining_its_cell() {
    let mut sim = fixture(EntityCategory::Aircraft, LocomotorKind::Fly);
    sim.substrate
        .entities
        .get_mut(1)
        .unwrap()
        .position
        .exact_z_leptons = Some(600);
    assert!(sim.foot_mark_put(1, None, None));
    assert!(sim.substrate.entities.get(1).unwrap().lifecycle.cell_marked);
    assert!(!sim.substrate.occupancy.contains_entity(4, 4, 1));
    assert_eq!(sim.substrate.raw_cell_occupation.ground_bits(4, 4), 0);
    assert!(sim.foot_mark_remove(1, None, None));
    assert!(!sim.substrate.entities.get(1).unwrap().lifecycle.cell_marked);
}

/// Mark(UP) clears +0x74 (`0x005F5913`) before the layer query, which a
/// cruising Jumpjet then answers as Ground: its Pick_Up clears the 0x20 bit
/// under it. Mark(DOWN) sets +0x74 first, so the same Jumpjet answers Top
/// and joins nothing.
#[test]
fn a_cruising_jumpjet_clears_the_bit_under_it_on_remove_and_joins_nothing_on_put() {
    let mut sim = cruising_jumpjet();
    sim.substrate
        .entities
        .get_mut(1)
        .unwrap()
        .lifecycle
        .cell_marked = true;
    sim.substrate.raw_cell_occupation.mark_ground(4, 4, 0x20);
    assert!(sim.foot_mark_remove(1, None, None));
    assert_eq!(sim.substrate.raw_cell_occupation.ground_bits(4, 4), 0);

    assert!(sim.foot_mark_put(1, None, None));
    assert!(sim.substrate.entities.get(1).unwrap().lifecycle.cell_marked);
    assert!(!sim.substrate.occupancy.contains_entity(4, 4, 1));
    assert_eq!(sim.substrate.raw_cell_occupation.ground_bits(4, 4), 0);
}

/// `ObjectClass::Mark` refuses a Limbo object (`0x005F5854..0x005F585C`).
#[test]
fn a_limbo_object_is_refused() {
    let mut sim = fixture(EntityCategory::Unit, LocomotorKind::Drive);
    sim.substrate
        .entities
        .get_mut(1)
        .unwrap()
        .lifecycle
        .in_limbo = true;
    assert!(!sim.foot_mark_put(1, None, None));
    assert!(!sim.substrate.entities.get(1).unwrap().lifecycle.cell_marked);
    assert!(!sim.substrate.occupancy.contains_entity(4, 4, 1));
}

/// Place_Down skips a Location outside the allocated cells
/// (`0x00568471..0x0056848E`); Mark still records +0x74.
#[test]
fn a_location_outside_the_map_is_marked_without_a_cell() {
    let mut sim = fixture(EntityCategory::Unit, LocomotorKind::Drive);
    let cells = (0..4)
        .flat_map(|y| {
            (0..4).map(move |x| crate::sim::world::common_raw_test_terrain_cell(x, y, 0, false))
        })
        .collect();
    sim.resolved_terrain =
        Some(crate::map::resolved_terrain::ResolvedTerrainGrid::from_cells(4, 4, cells));
    assert!(sim.foot_mark_put(1, None, None));
    assert!(sim.substrate.entities.get(1).unwrap().lifecycle.cell_marked);
    assert!(!sim.substrate.occupancy.contains_entity(4, 4, 1));
    assert_eq!(sim.substrate.raw_cell_occupation.ground_bits(4, 4), 0);
}

#[test]
fn put_receiver_observes_mark_and_link_before_raw_and_can_change_live_enable() {
    let mut sim = fixture(EntityCategory::Unit, LocomotorKind::Drive);
    let mut called = false;
    sim.foot_mark_put_observed(1, None, None, &mut |sim, id| {
        called = true;
        assert!(
            sim.substrate
                .entities
                .get(id)
                .unwrap()
                .lifecycle
                .cell_marked
        );
        assert!(sim.substrate.occupancy.contains_entity(4, 4, id));
        assert_eq!(sim.substrate.raw_cell_occupation.ground_bits(4, 4), 0);
        sim.substrate
            .entities
            .get_mut(id)
            .unwrap()
            .foot_occupation_enabled = false;
    });
    assert!(called);
    assert_eq!(sim.substrate.raw_cell_occupation.ground_bits(4, 4), 0);
    assert!(sim.substrate.occupancy.contains_entity(4, 4, 1));
    sim.foot_mark_remove(1, None, None);
    assert!(!sim.substrate.occupancy.contains_entity(4, 4, 1));
}
