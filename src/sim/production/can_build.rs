//! `HouseClass::CanBuild @ 0x004F7870` as the computer asks it, and
//! `ObjectTypeClass::FindFactory @ 0x005F7900` (vtable `+0x94` of every
//! TechnoType class).
//!
//! The computer's unit choosers (`sim::ai_unit_choice`) ask CanBuild with
//! neither flag; FindFactory asks it with both, which skips to the build
//! limit. The chooser dispatch (`sim::ai_base_building`) and the AI trigger
//! factory check (`sim::ai_team_creation`) ask FindFactory. The player's
//! sidebar keeps its own options (`production_tech::build_option_for_owner`),
//! so the prerequisite arm a human house runs is not ported here.
//!
//! Evidence: instruction reading of both bodies; the oracle
//! `tools/ai_team_oracle.py` runs the choosers with CanBuild's answers
//! hooked.
//!
//! RESIDUALS:
//! - The house's secret-lab types (`HouseClass+0x114`, each building's
//!   `0x00459840`) are not kept, so a type a captured Secret Lab grants goes
//!   through the ordinary gates instead of straight to the build limit.
//!   Trigger: a computer house owning a Tech Secret Lab whose TaskForces
//!   need its type. Effect: the type is refused where its TechLevel or
//!   `RequiredHouses=` would refuse it. Frequency: rare.
//! - The stolen-tech bytes (`+0x2BC..+0x2BE`) and masks (`+0x2C4..+0x2D0`)
//!   are never set (spy infiltration is not ported), so a
//!   `RequiresStolen*Tech=` type is always refused and `RequiredHouses=`
//!   admits the house's own country only.
//! - A build limit below 1 counts what the house has produced of the type
//!   (`+0x55A0`, `+0x55B4`, `+0x55C8`, `+0x55DC`), which is not kept: VERA
//!   counts none. Dormant: retail sets no such limit.
//! - A human house's prerequisite arm is not ported; the player's sidebar
//!   decides with its own checks (`production_tech::build_option_for_owner`,
//!   which also reads `BuildLimit=` differently: 0 as no limit, a negative
//!   limit as its magnitude over the owned and queued objects). Trigger: a
//!   future caller asking CanBuild for a human house, which gets No; a map
//!   setting `BuildLimit=` to 0 or below, where the sidebar and the computer
//!   disagree. Frequency: no caller; retail sets no such limit.
//! - An infantry type with `VehicleThief=` also counts the house's units
//!   that hold one of it (`UnitClass+0x338`); VERA has no hijacking, so the
//!   count adds none.

use crate::map::entities::EntityCategory;
use crate::rules::object_type::{FactoryType, ObjectCategory, ObjectType};
use crate::rules::ruleset::RuleSet;
use crate::sim::intern::InternedId;
use crate::sim::mission::{MissionId, MissionType};
use crate::sim::world::Simulation;

/// `HouseClass::CanBuild`'s answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CanBuild {
    /// 0.
    No,
    /// 1.
    Yes,
    /// -1: the house is at the type's build limit.
    AtLimit,
}

/// `HouseClass::CanBuild @ 0x004F7870` for house `owner` and type `obj`.
///
/// With `skip_tech` (the first flag), only the build limit is asked
/// ([`build_limit`]). Otherwise, in order (`0x004F7889..0x004F7BE5`):
/// - `TechnoType+0xC98` refuses; only the constructor writes it, with 0
///   (`0x007113CC`);
/// - a `PrerequisiteOverride=` BuildingType the house has on the map
///   (`+0x5550`) skips to the build limit;
/// - `TechLevel=-1` refuses;
/// - `RequiresStolenThirdTech=`, `RequiresStolenSovietTech=` or
///   `RequiresStolenAlliedTech=` (`+0xD9B`, `+0xD9C`, `+0xD9D`) refuses
///   unless the house stole that tech (module residual);
/// - `RequiredHouses=` (`+0xDA0`) without the house's bit refuses;
/// - `ForbiddenHouses=` (`+0xDA4`) with it refuses;
/// - with super weapons off (`0xA8B263`), a BuildingType whose
///   `SuperWeapon=` is `DisableableFromShell=` refuses unless `[General]
///   BuildTech=` lists it;
/// - a TechLevel above the house's (`+0x1D4`, signed) refuses;
/// - a house no human controls is answered Yes: neither its prerequisites
///   nor its build limit are asked.
///
/// A human house reaches the prerequisite arm (`0x004F7BF1..0x004F7F9E`),
/// which is not ported: callers ask for computer houses only.
pub(crate) fn can_build(
    sim: &Simulation,
    rules: &RuleSet,
    owner: InternedId,
    obj: &ObjectType,
    skip_tech: bool,
    count_in_production: bool,
) -> CanBuild {
    if skip_tech {
        return build_limit(sim, owner, obj, count_in_production);
    }
    let Some(house) = sim.houses.get(&owner) else {
        return CanBuild::No;
    };
    let override_on_map = obj.prerequisite_override.iter().any(|type_id| {
        sim.interner.get(type_id).is_some_and(|type_id| {
            house
                .tracking
                .active_count(EntityCategory::Structure, type_id)
                > 0
        })
    });
    if override_on_map {
        return build_limit(sim, owner, obj, count_in_production);
    }
    if obj.tech_level == -1
        || obj.requires_stolen_third_tech
        || obj.requires_stolen_soviet_tech
        || obj.requires_stolen_allied_tech
    {
        return CanBuild::No;
    }
    let house_bit = crate::sim::ai_buildable::house_country_bit(
        rules,
        sim.interner.resolve(house.house_type_id()),
    );
    let house_mask = |tokens: &[String]| crate::sim::ai_buildable::house_token_mask(tokens, rules);
    if !obj.required_houses.is_empty() && house_mask(&obj.required_houses) & house_bit == 0 {
        return CanBuild::No;
    }
    if !obj.forbidden_houses.is_empty() && house_mask(&obj.forbidden_houses) & house_bit != 0 {
        return CanBuild::No;
    }
    if !sim.session.game_options.super_weapons
        && obj.category == ObjectCategory::Building
        && let Some(super_weapon) = obj.super_weapon.as_deref()
        && !rules
            .build_tech_types
            .iter()
            .any(|type_id| type_id.eq_ignore_ascii_case(&obj.id))
        && rules
            .super_weapon(super_weapon)
            .is_some_and(|super_weapon| super_weapon.disableable_from_shell)
    {
        return CanBuild::No;
    }
    if obj.tech_level > house.tech_level {
        return CanBuild::No;
    }
    if !house.is_controlled_by_human(sim.session.game_mode_nonzero) {
        return CanBuild::Yes;
    }
    // A human house's prerequisite arm (`0x004F7BF1..`, taken at
    // `0x004F7B94`) is not ported (module residual): no caller asks for one.
    CanBuild::No
}

/// CanBuild's build limit (`0x004F7F9E..0x004F8357`): a type at or over
/// its positive `BuildLimit=` by the house's tracked count
/// (`HouseTracking::owned_count`) is at its limit, unless
/// `count_in_production` and one of the house's factories holds one
/// (`FactoryClass::Get_Object @ 0x004CA160`), which is Yes. For infantry the
/// factory scan runs only when the count equals the limit exactly.
fn build_limit(
    sim: &Simulation,
    owner: InternedId,
    obj: &ObjectType,
    count_in_production: bool,
) -> CanBuild {
    let limit = obj.build_limit;
    // `limit < 1` compares `|limit|` with the produced count, which VERA
    // does not keep (module residual): zero refuses, a negative limit
    // passes. Only aircraft, buildings and units answer Yes at once.
    if limit < 1 {
        if limit == 0 {
            return CanBuild::No;
        }
        if obj.category != ObjectCategory::Infantry {
            return CanBuild::Yes;
        }
    }
    let Some(type_id) = sim.interner.get(&obj.id) else {
        return CanBuild::Yes;
    };
    let owned = sim.houses.get(&owner).map_or(0, |house| {
        house
            .tracking
            .owned_count(EntityCategory::from(obj.category), type_id)
    });
    if limit < 1 || owned < limit {
        return CanBuild::Yes;
    }
    if !count_in_production || (obj.category == ObjectCategory::Infantry && owned != limit) {
        return CanBuild::AtLimit;
    }
    let in_production =
        sim.production
            .factories
            .iter_insertion_ordered()
            .into_iter()
            .any(|factory| {
                factory.owner == owner
                    && factory.object.as_ref().is_some_and(|object| {
                        object.type_id == type_id && object.entity_id.is_some()
                    })
            });
    if in_production {
        CanBuild::Yes
    } else {
        CanBuild::AtLimit
    }
}

/// Original ObjectType5F7900, scanned in House+6C insertion order. A first
/// eligible primary wins; without one, return the last eligible building.
/// Human PLACE4FB1DA retains this transient identity through its radio/release
/// path. Existing null-test callers consume this same result.
pub(crate) fn find_factory(
    sim: &Simulation,
    rules: &RuleSet,
    owner: InternedId,
    obj: &ObjectType,
    skip_busy_unit_radio: bool,
    require_online: bool,
    require_can_build: bool,
) -> Option<u64> {
    let house = sim.houses.get(&owner)?;
    let factory_type = match obj.category {
        ObjectCategory::Infantry => FactoryType::InfantryType,
        ObjectCategory::Vehicle => FactoryType::UnitType,
        ObjectCategory::Aircraft => FactoryType::AircraftType,
        ObjectCategory::Building => FactoryType::BuildingType,
    };
    let game_mode_nonzero = sim.session.game_mode_nonzero;
    let ownable = get_ownable(obj, rules, game_mode_nonzero);
    let selling = MissionId::from_known(MissionType::Selling);
    let naval_unit = obj.category == ObjectCategory::Vehicle && obj.naval;
    let primary_category = super::production_tech::production_category_for_object(obj);
    let primary = sim.production.primary_factory(owner, primary_category);
    let mut candidate = None;
    for &id in house.base_projection.buildings() {
        let Some(building) = sim.substrate.entities.get(id) else {
            continue;
        };
        let Some(building_type) = sim.object_type(building.type_ref(), rules) else {
            continue;
        };
        if !building.lifecycle.in_limbo
            && building_type.factory == Some(factory_type)
            && (!require_online || building.building_online())
            && building.mission.effective() != selling
            && building.mission.queued() != selling
            && (!require_can_build
                || can_build(sim, rules, building.owner(), obj, true, true) == CanBuild::Yes)
            && ownable & get_ownable(building_type, rules, game_mode_nonzero) != 0
            && building_type.naval == naval_unit
            // ObjectType5F79F4: arg1 only skips the UnitType radio gate.
            // Wrapper5F5C20 supplies (arg1,arg2,false,actualHouse); House
            // PLACE first tries (0,1), then Unit alone retries (1,1).
            && (skip_busy_unit_radio || obj.category != ObjectCategory::Vehicle
                || building.radio_contacts.is_empty()
                || building.radio_contacts.first_free().is_some())
        {
            candidate = Some(id);
            if primary == Some(id) {
                break;
            }
        }
    }
    candidate
}

/// Building448070, called from admitted Unlimbo4411B1 and ChangeOwner448C69:
/// preserve an existing live non-limbo primary of matching Factory/Naval;
/// otherwise make this building primary. This is the lifecycle producer, not
/// the fallback choice returned by FindFactory.
pub(crate) fn initialize_factory_primary(sim: &mut Simulation, id: u64, rules: &RuleSet) {
    let Some(building) = sim.substrate.entities.get(id) else {
        return;
    };
    if building.category != EntityCategory::Structure || building.lifecycle.in_limbo {
        return;
    }
    let Some(ty) = sim.object_type(building.type_ref(), rules) else {
        return;
    };
    let Some(factory) = ty.factory else {
        return;
    };
    let category = match factory {
        FactoryType::InfantryType => super::ProductionCategory::Infantry,
        FactoryType::UnitType if ty.naval => super::ProductionCategory::Ship,
        FactoryType::UnitType => super::ProductionCategory::Vehicle,
        FactoryType::AircraftType => super::ProductionCategory::Aircraft,
        FactoryType::BuildingType => super::ProductionCategory::Building,
    };
    let owner = building.owner();
    let current = sim.production.primary_factory(owner, category);
    let keep = current
        .and_then(|primary| sim.substrate.entities.get(primary))
        .is_some_and(|candidate| {
            !candidate.lifecycle.in_limbo
                && candidate.owner() == owner
                && sim
                    .object_type(candidate.type_ref(), rules)
                    .is_some_and(|candidate_type| {
                        candidate_type.factory == Some(factory) && candidate_type.naval == ty.naval
                    })
        });
    if !keep {
        sim.production.set_primary_factory(owner, category, id);
    }
}

/// `TechnoTypeClass::Get_Ownable @ 0x00711EC0` (vtable `+0x70` of every
/// TechnoType class): every house for a `DoubleOwned=` type outside a
/// campaign, else the `Owner=` houses (`+0x6CC`).
fn get_ownable(obj: &ObjectType, rules: &RuleSet, game_mode_nonzero: bool) -> u32 {
    if obj.double_owned && game_mode_nonzero {
        return 0x7FFF_FFFF;
    }
    crate::sim::ai_buildable::house_token_mask(&obj.owner, rules)
}
