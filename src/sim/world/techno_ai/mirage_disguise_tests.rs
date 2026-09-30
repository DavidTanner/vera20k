//! The Mirage's still test is its locomotor's Is_Moving.

use super::*;
use crate::rules::ini_parser::IniFile;
use crate::rules::locomotor_type::LocomotorKind;
use crate::sim::components::{DriveCoord, MovementTarget};
use crate::sim::game_entity::GameEntity;
use crate::sim::movement::locomotor::LocomotorState;

fn mirage(order: bool, destination: Option<DriveCoord>) -> (Simulation, RuleSet) {
    let mut rules = RuleSet::from_ini(&IniFile::from_str(
        "[VehicleTypes]\n0=MGTK\n[MGTK]\nStrength=400\nSpeed=6\nDisguiseWhenStill=yes\n",
    ))
    .expect("rules");
    rules.general.default_mirage_disguises = vec!["TREE01".to_string()];
    let mut sim = Simulation::new();
    let mut entity = GameEntity::test_default(1, "MGTK", "Americans", 5, 5);
    entity.category = EntityCategory::Unit;
    entity.locomotor = Some(LocomotorState::for_test_kind(LocomotorKind::Drive));
    entity.drive_locomotion = Some(crate::sim::components::DriveLocomotionRuntime {
        destination,
        ..Default::default()
    });
    entity.movement_target = order.then(MovementTarget::default);
    sim.substrate.entities.insert(entity);
    sim.interner = crate::sim::intern::test_interner();
    (sim, rules)
}

fn disguised(sim: &Simulation) -> bool {
    sim.substrate
        .entities
        .get(1)
        .and_then(|entity| entity.disguise.as_ref())
        .is_some_and(|disguise| disguise.disguised)
}

/// `UnitClass::UpdateDisguise @ 0x007468C0` asks the locomotor's Is_Moving
/// (ILocomotion+0x10 at `0x007468F4`), which reads the Drive's own destination
/// and head, not the owner's order. A Mirage holding an order its Drive has
/// not taken up is still, so it takes a disguise. No production path is known
/// to leave a Mirage in that state; this pins the owner's answer.
#[test]
fn a_mirage_whose_drive_has_not_started_its_order_disguises() {
    let (mut sim, rules) = mirage(true, None);
    techno_common_pre(&mut sim, 1, Some(&rules), None);
    assert!(disguised(&sim));
}

/// A Mirage whose Drive has a destination is moving, so UpdateDisguise takes
/// its clear arm (Is_Moving at `0x0074693D`, then ClearDisguise at
/// `0x00746AFF`) even without an order.
#[test]
fn a_disguised_mirage_whose_drive_has_a_destination_drops_its_disguise() {
    let (mut sim, rules) = mirage(false, Some(DriveCoord::cell(9, 5, 0)));
    let tree = sim.interner.intern("TREE01");
    let entity = sim.substrate.entities.get_mut(1).expect("mirage");
    entity
        .disguise
        .get_or_insert_with(Default::default)
        .acquire(0, Some(tree), None);
    assert!(disguised(&sim));
    techno_common_pre(&mut sim, 1, Some(&rules), None);
    assert!(!disguised(&sim));
}
