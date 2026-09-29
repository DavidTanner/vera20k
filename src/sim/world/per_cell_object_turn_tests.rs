//! The object turn's `Per_Cell_Process(2)` callers run the whole per-cell
//! owner (`movement/per_cell.rs`), not a part of it.

use super::*;
use crate::rules::ini_parser::IniFile;
use crate::rules::locomotor_type::LocomotorKind;
use crate::sim::game_entity::GameEntity;
use crate::sim::movement::locomotor::LocomotorState;
use crate::sim::movement::parachute_descent::ParachuteDescentState;
use crate::util::fixed_math::SimFixed;

/// Object AI `0x005F3F8D` calls `Per_Cell_Process(2)` when a fall grounds,
/// in the cell it fell through too; the Foot body's Techno tail then
/// promotes the object into the playfield (`0x006F511A`). The copy this
/// replaced ran only the neighbour update, and a Walk owner never reaches
/// the cell-change stand-in.
#[test]
fn a_parachute_landing_in_its_own_cell_runs_the_foot_body() {
    let rules = RuleSet::from_ini(&IniFile::from_str(
        "[InfantryTypes]\n0=E1\n\n[E1]\nStrength=125\nSpeed=4\n",
    ))
    .unwrap();
    let mut sim = Simulation::with_seed(0);
    sim.fog.width = 32;
    sim.fog.height = 32;
    sim.playfield_bounds = Some(crate::sim::cell_rect::PlayfieldBounds {
        base: 0,
        off_fc: -100,
        off_100: -100,
        off_104: 200,
        off_108: 200,
    });
    let mut entity = GameEntity::test_default(1, "E1", "Americans", 5, 5);
    entity.owner = sim.intern("Americans");
    entity.type_ref = sim.intern("E1");
    entity.locomotor = Some(LocomotorState::for_test_kind(LocomotorKind::Walk));
    entity.parachute_state = Some(ParachuteDescentState {
        rate: -3,
        altitude: SimFixed::from_num(2),
    });
    sim.substrate.entities.insert(entity);
    sim.substrate.next_stable_object_id = 2;
    assert!(matches!(
        sim.reveal(1),
        super::super::RevealOutcome::Revealed { .. }
    ));
    sim.substrate.entities.get_mut(1).unwrap().in_playfield = false;

    sim.advance_live_object_turn(1, Some(&rules), techno_ai::ObjectAiCtx::default())
        .unwrap();

    let entity = sim.substrate.entities.get(1).unwrap();
    assert!(
        entity.parachute_state.is_none(),
        "the fall grounded this turn"
    );
    assert_eq!((entity.position.rx, entity.position.ry), (5, 5));
    assert!(entity.in_playfield, "the landing's Techno tail promotes");
}
