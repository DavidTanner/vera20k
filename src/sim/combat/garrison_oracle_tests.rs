//! Replays of original occupied-Building queries. The corpus and its sidecar
//! bound these claims to GetWeapon, GetCurrentWeapon, GetWeaponRange,
//! IsOccupied, HalfFoundation and the occupied Greatest_Threat bound slice.

use super::combat_weapon::{
    current_weapon, get_weapon, half_foundation, is_occupied, weapon_range,
};
use crate::map::entities::EntityCategory;
use crate::rules::foundation::FOUNDATION_TABLE;
use crate::rules::ini_parser::IniFile;
use crate::rules::object_type::ObjectType;
use crate::rules::ruleset::RuleSet;
use crate::sim::entity_store::EntityStore;
use crate::sim::game_entity::GameEntity;
use crate::sim::intern::{StringInterner, test_interner};
use crate::sim::passenger::{PassengerCargo, PassengerRole};
use serde_json::Value;
use std::fmt::Write;

/// Supply the native corpus's type links and live state through existing
/// fixture owners. Expected outputs remain in the executed native corpus.
pub(super) fn for_each_native_case(
    mut check: impl FnMut(&Value, &GameEntity, &ObjectType, &EntityStore, &RuleSet, &StringInterner),
) {
    let corpus: Value = serde_json::from_str(crate::test_fixture::text(
        "tools/garrison_oracle/weapon_range.json",
    ))
    .unwrap();
    for case in corpus["cases"].as_array().unwrap() {
        let input = &case["input"];
        let occupants = input["occupants"].as_array().unwrap();
        let turreted = input["building_turret_count"].as_i64().unwrap() > 0;
        let mut ini = format!(
            "[CombatDamage]\nOccupyWeaponRange={}\n[BuildingTypes]\n0=HOUSE\n[InfantryTypes]\n",
            input["occupy_range"]
        );
        for index in 0..occupants.len() {
            writeln!(ini, "{index}=OCC{index}").unwrap();
        }
        ini.push_str("[WeaponTypes]\n");
        let ranges = corpus["weapon_range_leptons"].as_object().unwrap();
        for (index, name) in ranges.keys().enumerate() {
            writeln!(ini, "{index}={name}").unwrap();
        }
        writeln!(
            ini,
            "[HOUSE]\nCanBeOccupied={}\nCanOccupyFire={}\nTurretCount={}\nWeaponCount=2",
            input["can_be_occupied"], input["can_occupy_fire"], input["building_turret_count"]
        )
        .unwrap();
        let slots = if turreted {
            ["Weapon1", "Weapon2", "EliteWeapon1", "EliteWeapon2"]
        } else {
            ["Primary", "Secondary", "ElitePrimary", "EliteSecondary"]
        };
        let keys = ["primary", "secondary", "elite_primary", "elite_secondary"];
        for (key, slot) in keys.iter().zip(slots) {
            if let Some(name) = input["building_weapons"][key].as_str() {
                writeln!(ini, "{slot}={name}").unwrap();
            }
        }
        for (index, occupant) in occupants.iter().enumerate() {
            writeln!(ini, "[OCC{index}]").unwrap();
            for (key, slot) in [
                ("primary", "Primary"),
                ("secondary", "Secondary"),
                ("elite_primary", "ElitePrimary"),
                ("elite_secondary", "EliteSecondary"),
                ("occupy_weapon", "OccupyWeapon"),
                ("elite_occupy_weapon", "EliteOccupyWeapon"),
            ] {
                if let Some(name) = occupant[key].as_str() {
                    writeln!(ini, "{slot}={name}").unwrap();
                }
            }
        }
        for (name, range) in ranges {
            writeln!(ini, "[{name}]\nRange={}", range.as_i64().unwrap() / 256).unwrap();
        }
        let foundation = FOUNDATION_TABLE[input["foundation_id"].as_u64().unwrap() as usize];
        let art = IniFile::from_str(&format!("[HOUSE]\nFoundation={}\n", foundation.name));
        let rules =
            RuleSet::from_ini_with_fixed_art_for_test(&IniFile::from_str(&ini), &art).unwrap();
        let object = rules.object("HOUSE").unwrap();
        let mut building = GameEntity::test_default_of_category(
            1,
            "HOUSE",
            "Test",
            10,
            10,
            EntityCategory::Structure,
        );
        building.foundation.clone_from(&object.foundation);
        building.set_veterancy_rank((input["building_veterancy"].as_f64().unwrap() * 100.0) as u16);
        building.set_gunner_selection_for_test(
            input["current_weapon_number"].as_i64().unwrap() as i32,
            0,
        );
        let mut cargo = PassengerCargo::new(5, 0);
        let mut entities = EntityStore::new();
        // Cargo's writer prepends: reverse the supplied native array when boarding.
        for (index, occupant) in occupants.iter().enumerate().rev() {
            let id = index as u64 + 2;
            let mut entity = GameEntity::test_default_of_category(
                id,
                &format!("OCC{index}"),
                "Test",
                10,
                10,
                EntityCategory::Infantry,
            );
            entity.set_veterancy_rank((occupant["veterancy"].as_f64().unwrap() * 100.0) as u16);
            assert!(cargo.board(id, 1));
            entities.insert(entity);
        }
        cargo.garrison_fire_index = input["fire_index"].as_u64().unwrap() as u8;
        building.passenger_role = PassengerRole::Transport { cargo };
        check(case, &building, object, &entities, &rules, &test_interner());
    }
}

#[test]
fn occupied_weapon_and_foundation_queries_match_original_instructions() {
    let corpus: Value = serde_json::from_str(crate::test_fixture::text(
        "tools/garrison_oracle/weapon_range.json",
    ))
    .unwrap();
    for row in corpus["foundations"].as_array().unwrap() {
        let foundation = FOUNDATION_TABLE[row["foundation_id"].as_u64().unwrap() as usize];
        assert_eq!(u64::from(foundation.width), row["width"].as_u64().unwrap());
        assert_eq!(
            u64::from(foundation.height),
            row["height"].as_u64().unwrap()
        );
    }
    for_each_native_case(|case, building, object, entities, rules, interner| {
        let name = case["input"]["name"].as_str().unwrap();
        assert_eq!(
            is_occupied(building, object),
            case["occupied"].as_bool().unwrap(),
            "{name}"
        );
        assert_eq!(
            i64::from(half_foundation(object)),
            case["half_foundation"].as_i64().unwrap(),
            "{name}"
        );
        for (index, expected) in corpus["weapon_indices"]
            .as_array()
            .unwrap()
            .iter()
            .zip(case["weapon_ids"].as_array().unwrap())
        {
            assert_eq!(
                get_weapon(
                    building,
                    object,
                    index.as_i64().unwrap() as i32,
                    entities,
                    rules,
                    interner
                )
                .map(|weapon| weapon.id.as_str()),
                expected.as_str(),
                "{name}: slot {index}"
            );
        }
        assert_eq!(
            current_weapon(building, object, entities, rules, interner)
                .map(|weapon| weapon.id.as_str()),
            case["current_weapon_id"].as_str(),
            "{name}: current"
        );
        for (index, expected) in case["weapon_ranges"].as_array().unwrap().iter().enumerate() {
            assert_eq!(
                i64::from(weapon_range(
                    building,
                    object,
                    index as i32,
                    entities,
                    rules,
                    interner
                )),
                expected.as_i64().unwrap(),
                "{name}: range {index}"
            );
        }
    });
}
