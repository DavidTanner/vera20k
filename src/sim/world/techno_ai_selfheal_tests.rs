//! Tech Hospital and Machine Shop self-heal (issue #1219).
//!
//! Pins the four house counters and both `AI_Update` arms against the retail
//! bodies read with `tools/native_inspect`:
//!
//! - `BuildingClass::OnConstructionComplete` adds `InfantryGainSelfHeal`
//!   (`+0x1564`) to house `+0x164` and `UnitsGainSelfHeal` (`+0x1568`) to house
//!   `+0x168` (`0x00446382..0x004463B4`).
//! - `BuildingClass::Limbo` subtracts both and clamps at zero
//!   (`0x004459AE..0x004459CA`).
//! - `BuildingClass::ChangeOwner` moves both, gated on the building's own
//!   `+0x6E4` placed byte (`0x00448AC8..0x00448B04`).
//! - `TechnoClass::AI_Update` heals the non-organic arm every
//!   `SelfHealUnitFrames` (`Rules+0x38`) by `SelfHealUnitAmount` (`+0x3C`)
//!   times the house count, and the `Organic=` arm every
//!   `SelfHealInfantryFrames` (`Rules+0x30`) by `SelfHealInfantryAmount`
//!   (`+0x34`) times its count, both clamped at full strength.

use super::*;
use crate::map::playfield::PlayfieldBounds;
use crate::rules::ini_parser::IniFile;
use crate::sim::house_state::HouseState;
use crate::sim::intern::InternedId;

/// Stock shapes: a `[CAMACH]`-style machine shop and a `[CATHOSP]`-style
/// hospital carry the gain keys; the four `[General]` keys hold the retail
/// values. `[E1]` is an organic infantry and `[MTNK]` a non-organic vehicle.
fn rules() -> RuleSet {
    RuleSet::from_ini(&IniFile::from_str(
        "[General]\nRepairRate=.016\nSelfHealInfantryFrames=50\nSelfHealInfantryAmount=20\n\
         SelfHealUnitFrames=75\nSelfHealUnitAmount=5\n\
         [InfantryTypes]\n0=E1\n\
         [VehicleTypes]\n0=MTNK\n\
         [BuildingTypes]\n0=GACNST\n1=CAHOSP\n2=CAMACH\n\
         [E1]\nStrength=125\nSpeed=4\nCost=100\n\
         [MTNK]\nStrength=300\nSpeed=7\nCost=700\nROT=5\n\
         [GACNST]\nStrength=1000\nCost=2000\nFoundation=4x4\n\
         [CAHOSP]\nStrength=800\nCost=1000\nFoundation=2x2\nInfantryGainSelfHeal=1\n\
         [CAMACH]\nStrength=800\nCost=1000\nFoundation=2x2\nUnitsGainSelfHeal=1\n",
    ))
    .expect("self-heal rules")
}

fn scene() -> (Simulation, RuleSet, InternedId) {
    let rules = rules();
    let mut sim = Simulation::with_seed(0x5E1F_4EA1);
    sim.fog.width = 64;
    sim.fog.height = 64;
    sim.playfield_bounds = Some(PlayfieldBounds::from_normalized_local_size(
        64, 2, 2, 56, 52,
    ));
    sim.intern_rule_type_ids(&rules);
    let owner = sim.interner.intern("Soviet");
    sim.houses
        .insert(owner, HouseState::new(owner, 0, None, false, 0, 10));
    (sim, rules, owner)
}

/// A cell that admits a structure, taken from the fixture's playfield.
fn free_cell(sim: &Simulation) -> (u16, u16) {
    let bounds = sim.playfield_bounds.unwrap();
    (8u16..56)
        .flat_map(|ry| (8u16..56).map(move |rx| (rx, ry)))
        .find(|&(rx, ry)| bounds.contains_height_aware_packed(rx.into(), ry.into(), 0, 0))
        .expect("interior mode-one cell")
}

fn spawn(sim: &mut Simulation, rules: &RuleSet, type_id: &str) -> u64 {
    let (rx, ry) = free_cell(sim);
    sim.spawn_object_at_height(type_id, "Soviet", rx, ry, 0, 0, rules)
        .expect("spawn")
}

#[test]
fn a_completed_hospital_grants_its_infantry_count_and_limbo_takes_it_back() {
    let (mut sim, rules, owner) = scene();
    let hospital = spawn(&mut sim, &rules, "CAHOSP");
    // The spawn ran the production opening, so the grant is already applied by
    // `grand_opening` — this is the live path, not a synthetic call.
    assert_eq!(
        sim.houses[&owner].self_heal_infantry, 1,
        "0x00446392..0x00446398 adds InfantryGainSelfHeal to house +0x164"
    );
    assert_eq!(
        sim.houses[&owner].self_heal_units, 0,
        "a hospital grants no unit self-heal"
    );

    let second = spawn(&mut sim, &rules, "CAHOSP");
    assert_eq!(
        sim.houses[&owner].self_heal_infantry, 2,
        "each completed hospital adds its own count"
    );

    sim.remove_house_self_heal(hospital, &rules);
    assert_eq!(sim.houses[&owner].self_heal_infantry, 1);
    sim.remove_house_self_heal(second, &rules);
    assert_eq!(sim.houses[&owner].self_heal_infantry, 0);
}

#[test]
fn limbo_clamps_a_shortfall_at_zero() {
    let (mut sim, rules, owner) = scene();
    let shop = spawn(&mut sim, &rules, "CAMACH");
    assert_eq!(sim.houses[&owner].self_heal_units, 1);

    // A capture or scenario path can leave the counter short of the type's
    // count; 0x004459C2 tests the result and 0x004459CA raises it to zero.
    sim.remove_house_self_heal(shop, &rules);
    assert_eq!(sim.houses[&owner].self_heal_units, 0);
    sim.remove_house_self_heal(shop, &rules);
    assert_eq!(
        sim.houses[&owner].self_heal_units, 0,
        "clamped, not negative"
    );
}

#[test]
fn change_owner_moves_the_counts_and_requires_the_placed_byte() {
    let (mut sim, rules, owner) = scene();
    let new_owner = sim.interner.intern("Americans");
    sim.houses
        .insert(new_owner, HouseState::new(new_owner, 0, None, false, 0, 10));
    let hospital = spawn(&mut sim, &rules, "CAHOSP");
    assert_eq!(sim.houses[&owner].self_heal_infantry, 1);

    // `+0x6E4` clear: every arm returns before touching a counter.
    sim.substrate
        .entities
        .get_mut(hospital)
        .unwrap()
        .building_actually_placed = false;
    sim.transfer_house_self_heal(hospital, owner, new_owner, &rules);
    assert_eq!(
        sim.houses[&owner].self_heal_infantry, 1,
        "still the old house"
    );
    assert_eq!(sim.houses[&new_owner].self_heal_infantry, 0);

    // Placed: the old owner loses it (clamped) and the new owner gains it.
    sim.substrate
        .entities
        .get_mut(hospital)
        .unwrap()
        .building_actually_placed = true;
    sim.transfer_house_self_heal(hospital, owner, new_owner, &rules);
    assert_eq!(sim.houses[&owner].self_heal_infantry, 0);
    assert_eq!(sim.houses[&new_owner].self_heal_infantry, 1);
}

#[test]
fn a_machine_shop_heals_non_organic_units_on_the_unit_cadence() {
    let (mut sim, rules, owner) = scene();
    let tank = spawn(&mut sim, &rules, "MTNK");
    sim.substrate.entities.get_mut(tank).unwrap().health.current = 100;
    sim.houses.get_mut(&owner).unwrap().self_heal_units = 1;

    sim.session.binary_frame = 74;
    house_self_heal_step(&mut sim, tank, &rules);
    assert_eq!(
        sim.substrate.entities.get(tank).unwrap().health.current,
        100,
        "off-cadence frames do nothing (0x006FA7EE tests the remainder)"
    );

    sim.session.binary_frame = 75;
    house_self_heal_step(&mut sim, tank, &rules);
    assert_eq!(
        sim.substrate.entities.get(tank).unwrap().health.current,
        105,
        "SelfHealUnitAmount(5) x the house count on a SelfHealUnitFrames frame"
    );

    // A second shop doubles both the count and the step.
    sim.houses.get_mut(&owner).unwrap().self_heal_units = 2;
    sim.session.binary_frame = 150;
    house_self_heal_step(&mut sim, tank, &rules);
    assert_eq!(
        sim.substrate.entities.get(tank).unwrap().health.current,
        115
    );
}

#[test]
fn a_hospital_heals_organic_infantry_on_the_infantry_cadence() {
    let (mut sim, rules, owner) = scene();
    let infantry = spawn(&mut sim, &rules, "E1");
    sim.substrate
        .entities
        .get_mut(infantry)
        .unwrap()
        .health
        .current = 100;
    sim.houses.get_mut(&owner).unwrap().self_heal_infantry = 1;

    sim.session.binary_frame = 75;
    house_self_heal_step(&mut sim, infantry, &rules);
    assert_eq!(
        sim.substrate.entities.get(infantry).unwrap().health.current,
        100,
        "the 75-frame unit cadence does not drive the organic arm"
    );

    sim.session.binary_frame = 50;
    house_self_heal_step(&mut sim, infantry, &rules);
    assert_eq!(
        sim.substrate.entities.get(infantry).unwrap().health.current,
        120,
        "SelfHealInfantryAmount(20) on a SelfHealInfantryFrames frame"
    );
}

#[test]
fn each_arm_ignores_the_other_classes_and_an_empty_counter() {
    let (mut sim, rules, owner) = scene();
    let infantry = spawn(&mut sim, &rules, "E1");
    let tank = spawn(&mut sim, &rules, "MTNK");
    for id in [infantry, tank] {
        sim.substrate.entities.get_mut(id).unwrap().health.current = 50;
    }

    // Both counters stocked: each object takes exactly its own arm's amount.
    sim.houses.get_mut(&owner).unwrap().self_heal_infantry = 1;
    sim.houses.get_mut(&owner).unwrap().self_heal_units = 1;
    sim.session.binary_frame = 150; // divisible by both 50 and 75
    house_self_heal_step(&mut sim, infantry, &rules);
    house_self_heal_step(&mut sim, tank, &rules);
    assert_eq!(
        sim.substrate.entities.get(infantry).unwrap().health.current,
        70,
        "the organic object takes only the infantry arm"
    );
    assert_eq!(
        sim.substrate.entities.get(tank).unwrap().health.current,
        55,
        "the non-organic object takes only the unit arm"
    );

    // No ability: neither object heals.
    sim.houses.get_mut(&owner).unwrap().self_heal_infantry = 0;
    sim.houses.get_mut(&owner).unwrap().self_heal_units = 0;
    sim.substrate
        .entities
        .get_mut(infantry)
        .unwrap()
        .health
        .current = 50;
    sim.substrate.entities.get_mut(tank).unwrap().health.current = 50;
    sim.session.binary_frame = 300;
    house_self_heal_step(&mut sim, infantry, &rules);
    house_self_heal_step(&mut sim, tank, &rules);
    assert_eq!(
        sim.substrate.entities.get(infantry).unwrap().health.current,
        50
    );
    assert_eq!(sim.substrate.entities.get(tank).unwrap().health.current, 50);
}

#[test]
fn the_heal_stops_at_full_strength() {
    let (mut sim, rules, owner) = scene();
    let tank = spawn(&mut sim, &rules, "MTNK");
    let strength = rules.object("MTNK").unwrap().strength;
    sim.substrate.entities.get_mut(tank).unwrap().health.current = strength - 3;
    sim.houses.get_mut(&owner).unwrap().self_heal_units = 2;

    sim.session.binary_frame = 75;
    house_self_heal_step(&mut sim, tank, &rules);
    assert_eq!(
        sim.substrate.entities.get(tank).unwrap().health.current,
        strength,
        "0x6FA852 clamps the added step to Strength - Health"
    );

    sim.session.binary_frame = 150;
    house_self_heal_step(&mut sim, tank, &rules);
    assert_eq!(
        sim.substrate.entities.get(tank).unwrap().health.current,
        strength,
        "a full-strength object never overshoots"
    );
}

#[test]
fn the_production_ai_stage_consumes_the_hospital_count() {
    let (mut sim, rules, owner) = scene();
    // Spawning the hospital runs the real opening, so the house ability comes
    // from the production lifecycle rather than a hand-set counter.
    spawn(&mut sim, &rules, "CAHOSP");
    assert_eq!(sim.houses[&owner].self_heal_infantry, 1);

    let infantry = spawn(&mut sim, &rules, "E1");
    sim.substrate
        .entities
        .get_mut(infantry)
        .unwrap()
        .health
        .current = 40;

    // `TechnoClass::AI_Update`'s own position: the house pulse is inside
    // `techno_common_steps`, which `object_ai_stage` drives.
    sim.session.binary_frame = 50;
    sim.object_ai_stage(Some(&rules));
    assert_eq!(
        sim.substrate.entities.get(infantry).unwrap().health.current,
        60,
        "the production AI stage applies SelfHealInfantryAmount x the count"
    );

    // Losing the hospital stops the pulse without any further bookkeeping.
    let hospital = sim
        .substrate
        .entities
        .values()
        .find(|entity| entity.category == crate::map::entities::EntityCategory::Structure)
        .map(|entity| entity.stable_id)
        .expect("hospital");
    sim.remove_house_self_heal(hospital, &rules);
    assert_eq!(sim.houses[&owner].self_heal_infantry, 0);
    sim.session.binary_frame = 100;
    sim.object_ai_stage(Some(&rules));
    assert_eq!(
        sim.substrate.entities.get(infantry).unwrap().health.current,
        60,
        "no ability, no pulse"
    );
}

#[test]
fn an_absent_self_heal_frame_key_cannot_divide_by_zero() {
    let (mut sim, _rules, owner) = scene();
    let rules = RuleSet::from_ini(&IniFile::from_str(
        "[General]\nRepairRate=.016\n\
         [InfantryTypes]\n0=E1\n\
         [BuildingTypes]\n0=CAHOSP\n\
         [E1]\nStrength=125\nSpeed=4\nCost=100\n\
         [CAHOSP]\nStrength=800\nCost=1000\nFoundation=2x2\nInfantryGainSelfHeal=1\n",
    ))
    .expect("rules without the SelfHeal keys");
    assert_eq!(rules.general.self_heal_infantry_frames, 0);
    let hospital = spawn(&mut sim, &rules, "CAHOSP");
    sim.grant_house_self_heal(hospital, &rules);
    sim.houses.get_mut(&owner).unwrap().self_heal_infantry = 1;
    let infantry = spawn(&mut sim, &rules, "E1");
    sim.substrate
        .entities
        .get_mut(infantry)
        .unwrap()
        .health
        .current = 10;
    sim.session.binary_frame = 0;
    // The native IDIV would fault on a zero period; the port declines instead of
    // panicking, which is the documented difference from native.
    house_self_heal_step(&mut sim, infantry, &rules);
    assert_eq!(
        sim.substrate.entities.get(infantry).unwrap().health.current,
        10
    );
}
