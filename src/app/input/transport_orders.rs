//! Deploy-key / self-click admission for passenger transports.
//!
//! In gamemd a vehicle or aircraft that carries passengers unloads through
//! its Unload mission: `UnitClass::Mission_Unload @ 0x0073D630` takes its
//! transport branch on `Type+0x5E0 Passengers > 0` and the Aircraft Unload
//! slot `0x004151E0` serves the Nighthawk. The Deploy key sends it DEPLOY and
//! a self-click a MEGAMISSION Unload; both input sites ask this one predicate
//! which objects are loaded transports, so they cannot disagree.

use crate::map::entities::EntityCategory;
use crate::rules::object_type::ObjectType;
use crate::sim::game_entity::GameEntity;

/// Whether the entity is a vehicle/aircraft transport with passengers aboard.
pub(crate) fn is_loaded_transport(entity: &GameEntity, obj: Option<&ObjectType>) -> bool {
    matches!(
        entity.category(),
        EntityCategory::Unit | EntityCategory::Aircraft
    ) && obj.is_some_and(|o| o.passengers > 0)
        && entity.passenger_role.cargo().is_some_and(|c| !c.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::ini_parser::IniFile;
    use crate::rules::ruleset::RuleSet;
    use crate::sim::passenger::{PassengerCargo, PassengerRole};

    fn rules() -> RuleSet {
        let ini = IniFile::from_str(
            "[InfantryTypes]\n0=E1\n[VehicleTypes]\n0=BFRT\n1=MTNK\n[AircraftTypes]\n0=SHAD\n\
             [BuildingTypes]\n\n[E1]\nStrength=125\nSize=1\n\n[BFRT]\nStrength=600\n\
             Passengers=5\nOpenTopped=yes\n\n[MTNK]\nStrength=300\n\n[SHAD]\nStrength=200\n\
             Passengers=5\nLandable=yes\n",
        );
        RuleSet::from_ini(&ini).expect("rules")
    }

    fn entity(id: u64, type_id: &str, category: EntityCategory, loaded: bool) -> GameEntity {
        let mut e = GameEntity::test_default(id, type_id, "Americans", 10, 10);
        e.set_category_for_test(category);
        let mut cargo = PassengerCargo::new(5, 0);
        if loaded {
            cargo.board(99, 1);
        }
        e.passenger_role = PassengerRole::Transport { cargo };
        e
    }

    #[test]
    fn loaded_apc_and_aircraft_are_loaded_transports() {
        let rules = rules();
        let bfrt = entity(7, "BFRT", EntityCategory::Unit, true);
        assert!(is_loaded_transport(&bfrt, rules.object("BFRT")));
        let shad = entity(8, "SHAD", EntityCategory::Aircraft, true);
        assert!(is_loaded_transport(&shad, rules.object("SHAD")));
    }

    #[test]
    fn empty_holds_and_non_transports_are_not_loaded_transports() {
        let rules = rules();
        let empty = entity(7, "BFRT", EntityCategory::Unit, false);
        assert!(!is_loaded_transport(&empty, rules.object("BFRT")));
        let tank = entity(9, "MTNK", EntityCategory::Unit, true);
        assert!(!is_loaded_transport(&tank, rules.object("MTNK")));
        let mut building = entity(10, "BFRT", EntityCategory::Structure, true);
        building.set_category_for_test(EntityCategory::Structure);
        assert!(!is_loaded_transport(&building, rules.object("BFRT")));
    }
}
