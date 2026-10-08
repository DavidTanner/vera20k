//! Tech tree, build options, factory matching, and spawn cell logic.
//!
//! Determines what a player can build based on owned structures, prerequisites,
//! faction ownership, and available factories. Also handles spawn cell selection
//! for newly produced units.

use crate::map::entities::EntityCategory;
use crate::rules::foundation::foundation_dimensions;
use crate::rules::object_type::{BuildCategory, FactoryType, ObjectCategory};
use crate::rules::ruleset::RuleSet;
use crate::sim::entity_store::EntityStore;
use crate::sim::world::Simulation;

use super::production_types::*;

/// Whether `owner` may build `type_id` now, and why not. No money check: a
/// build starts without the funds and stalls its steps instead
/// (`SelectClass::Action @ 0x006AAD00` and `HouseClass::Begin_Production
/// @ 0x004FA350` test none).
pub(super) fn build_option_for_owner(
    sim: &Simulation,
    rules: &RuleSet,
    owner: &str,
    type_id: &str,
) -> Option<BuildOption> {
    let obj = rules.object(type_id)?;
    let queue_category = production_category_for_object(obj);

    // `HouseClass::CanBuild @ 0x004F7870` compares the type's TechLevel
    // (`TechnoTypeClass+0x634`) with the house's (`HouseClass+0x1D4`).
    let house_tech_level =
        crate::sim::house_state::house_state_for_owner(&sim.houses, owner, &sim.interner)
            .map(|house| house.tech_level);
    let mut reason: Option<BuildDisabledReason> = None;
    if obj.tech_level < 0 || house_tech_level.is_none_or(|level| obj.tech_level > level) {
        reason = Some(BuildDisabledReason::UnbuildableTechLevel);
    } else if !obj.owner.is_empty() && !owner_matches_any_build_identity(sim, owner, &obj.owner) {
        reason = Some(BuildDisabledReason::WrongOwner);
    } else if !obj.required_houses.is_empty()
        && !owner_matches_any_build_identity(sim, owner, &obj.required_houses)
    {
        reason = Some(BuildDisabledReason::WrongHouse);
    } else if !obj.forbidden_houses.is_empty()
        && owner_matches_any_build_identity(sim, owner, &obj.forbidden_houses)
    {
        reason = Some(BuildDisabledReason::ForbiddenHouse);
    } else if obj.requires_stolen_allied_tech
        || obj.requires_stolen_soviet_tech
        || obj.requires_stolen_third_tech
    {
        // Spy infiltration not yet implemented — always block stolen-tech units.
        reason = Some(BuildDisabledReason::RequiresStolenTech);
    } else {
        // PrerequisiteOverride: if owner has ANY override building, skip normal prereqs.
        let override_satisfied = !obj.prerequisite_override.is_empty()
            && has_any_override_building(sim, owner, &obj.prerequisite_override);
        if !override_satisfied {
            if let Some(missing) = first_missing_prereq(sim, rules, owner, &obj.prerequisite) {
                reason = Some(BuildDisabledReason::MissingPrerequisite(missing));
            }
        }
    }
    if reason.is_none()
        && !has_factory_for_owner(
            &sim.substrate.entities,
            rules,
            owner,
            queue_category,
            &sim.interner,
        )
    {
        reason = Some(BuildDisabledReason::NoFactory);
    }
    // BuildLimit check: count owned entities + queued + ready-for-placement.
    if reason.is_none()
        && let Some(limit) = effective_build_limit(obj.build_limit)
        && count_owned_and_queued(sim, owner, &obj.id) >= limit
    {
        reason = Some(BuildDisabledReason::AtBuildLimit);
    }
    let type_interned = sim.interner.get(type_id).unwrap_or_default();
    let cost = match sim.interner.get(owner) {
        Some(owner_id) => sim.cost_of(owner_id, obj, rules),
        None => rules.cost_of(obj, None),
    };
    Some(BuildOption {
        type_id: type_interned,
        display_name: obj.name.clone().unwrap_or_else(|| obj.id.clone()),
        cost,
        object_category: obj.category,
        queue_category,
        enabled: reason.is_none(),
        reason,
    })
}

/// P6 revalidation classifier: re-check an active/queued build's eligibility AFTER enqueue,
/// so a build whose prerequisites / producing factory were lost is disposed of. Reproduces
/// gamemd's `FindFactory(1,0,1)` gate (the embedded `HouseClass::CanBuild` scan across the
/// owner's candidate factory buildings): a tech-tree / owner / factory-presence failure ->
/// `PermanentlyBlocked` (abandon); a build-limit "busy" is NOT a mid-build abandon ->
/// `Buildable` (keep charging).
///
/// gamemd makes this check in three places. `HouseClass::Update_Factory_Queue @ 0x00509140`
/// drops each queued build that fails it (`0x005091B0..0x005091EF`) and abandons a failing
/// active one (`0x0050921C`); a factory building runs it for its own `Factory=` kind only
/// (`0x00445DFA..0x00445E14`) when it goes offline or online, into or out of limbo, is read
/// from a map, or at `0x00449267`/`0x00449284`. The local player's strip abandons a cameo
/// whose type `CanBuild(type, 0, 1)` refuses, through ABANDON / ABANDON_ALL events
/// (`StripClass::Recalculate @ 0x006AA600`, `0x006AA781`). A dying factory building abandons
/// its own factory (`BuildingClass::Detach_All`, see `FactoryRegistry::plan_revalidation`).
///
/// Residual: VERA revalidates every house's builds every tick. Trigger: a build loses a
/// prerequisite. Effect: a human player's abandon lands earlier than gamemd's events do; a
/// computer house that loses it through a building other than a factory (a Battle Lab)
/// keeps building in gamemd until a factory building of that kind changes state, and VERA
/// abandons and refunds at once (instruction reading; the computer paths are untraced).
/// Frequency: occasional.
///
/// Residual: the pass also holds an active build that only offline factories could build
/// (`FindFactory(1,1,1)` fails: `Suspend(0)` at `0x0050924D`, lifted at `0x00509283`).
/// VERA has no such hold. Its only offline factory is one in a temporal warp
/// (`GameEntity::building_online`), whose start runs no update (`0x004521C0`). GoOffline's
/// callers are not ported: the power toggle event (`0x004C6D9A`), a trigger action
/// (`0x006DDFB9`) and a map's powered-down building (`0x0044FD23`). Trigger: a building
/// event while every factory of the kind is offline. Effect: VERA keeps building.
/// Frequency: rare.
pub(in crate::sim) fn revalidate_eligibility(
    sim: &Simulation,
    rules: &RuleSet,
    owner: &str,
    type_id: &str,
) -> super::factory::BuildEligibility {
    use super::factory::BuildEligibility;
    match build_option_for_owner(sim, rules, owner, type_id) {
        None => BuildEligibility::PermanentlyBlocked,
        // A build-limit "busy" is not an abandon in gamemd — keep building.
        Some(opt) if matches!(opt.reason, None | Some(BuildDisabledReason::AtBuildLimit)) => {
            BuildEligibility::Buildable
        }
        // Tech-tree / owner / factory loss -> CanBuild fails -> abandon.
        Some(_) => BuildEligibility::PermanentlyBlocked,
    }
}

/// Every rules type's build option for `owner`, by category.
pub(super) fn all_build_options_for_owner(
    sim: &Simulation,
    rules: &RuleSet,
    owner: &str,
) -> Vec<BuildOption> {
    let mut out: Vec<BuildOption> = Vec::new();
    for id in &rules.building_ids {
        if let Some(opt) = build_option_for_owner(sim, rules, owner, id) {
            out.push(opt);
        }
    }
    for id in &rules.infantry_ids {
        if let Some(opt) = build_option_for_owner(sim, rules, owner, id) {
            out.push(opt);
        }
    }
    for id in &rules.vehicle_ids {
        if let Some(opt) = build_option_for_owner(sim, rules, owner, id) {
            out.push(opt);
        }
    }
    for id in &rules.aircraft_ids {
        if let Some(opt) = build_option_for_owner(sim, rules, owner, id) {
            out.push(opt);
        }
    }
    out.sort_by_key(|opt| opt.queue_category);
    out
}

pub(super) fn owner_matches_any_build_identity(
    sim: &Simulation,
    owner: &str,
    candidates: &[String],
) -> bool {
    candidates
        .iter()
        .any(|candidate| owner_matches_build_identity(sim, owner, candidate))
}

pub(super) fn owner_matches_build_identity(sim: &Simulation, owner: &str, candidate: &str) -> bool {
    if candidate.eq_ignore_ascii_case(owner) {
        return true;
    }
    sim.interner
        .get(owner)
        .and_then(|owner_id| sim.houses.get(&owner_id))
        .and_then(|house| house.country)
        .is_some_and(|country| candidate.eq_ignore_ascii_case(sim.interner.resolve(country)))
}

/// Check if the owner has ANY completed structure from the PrerequisiteOverride list.
fn has_any_override_building(sim: &Simulation, owner: &str, overrides: &[String]) -> bool {
    sim.substrate.entities.values().any(|e| {
        !e.dying
            && !e.lifecycle.in_limbo
            && sim.interner.resolve(e.owner()).eq_ignore_ascii_case(owner)
            && e.category == EntityCategory::Structure
            && !e.building_up()
            && overrides
                .iter()
                .any(|ov| ov.eq_ignore_ascii_case(sim.interner.resolve(e.type_ref())))
    })
}

/// Interpret BuildLimit value. Returns None if no limit applies (the
/// constructor's `0x7FFFFFFF`, or 0).
fn effective_build_limit(build_limit: i32) -> Option<u32> {
    if build_limit == 0 || build_limit == i32::MAX {
        return None;
    }
    Some(build_limit.unsigned_abs())
}

/// Count owned entities + queued items + ready-for-placement of this type for an owner.
fn count_owned_and_queued(sim: &Simulation, owner: &str, type_id: &str) -> u32 {
    let owner_id = sim.interner.get(owner);
    let type_interned = sim.interner.get(type_id);

    let owned = match (owner_id, type_interned) {
        (Some(oid), Some(tid)) => sim
            .substrate
            .entities
            .values()
            .filter(|e| !e.dying && e.owner() == oid && e.type_ref() == tid)
            .count() as u32,
        _ => 0,
    };

    // P5d: count from the registry queue-of-record — the active build (head) + the FIFO
    // tail, across the owner's factories (was the per-`BuildQueueItem` `queues_by_owner` scan).
    let queued = match (owner_id, type_interned) {
        (Some(oid), Some(tid)) => sim
            .production
            .factories
            .iter_insertion_ordered()
            .iter()
            .filter(|f| f.owner == oid)
            .map(|f| {
                // StartProduction materializes the active object into EntityStore,
                // so `owned` above already counts it. Only a restored/malformed
                // active head missing its swizzled identity needs the registry
                // fallback; queued tails remain unconstructed and count here.
                let active = f
                    .object
                    .as_ref()
                    .map_or(0, |o| u32::from(o.type_id == tid && o.entity_id.is_none()));
                let tail = f.queue.iter().filter(|e| e.type_id == tid).count() as u32;
                active + tail
            })
            .sum(),
        _ => 0,
    };

    let ready = owner_id
        .and_then(|oid| sim.production.ready_by_owner.get(&oid))
        .map(|ready| {
            ready
                .iter()
                .filter(|&&tid| type_interned.map_or(false, |expected| tid == expected))
                .count() as u32
        })
        .unwrap_or(0);

    let materialized_ready = match (owner_id, type_interned) {
        (Some(oid), Some(tid)) => {
            sim.production
                .factories
                .iter_insertion_ordered()
                .iter()
                .filter(|factory| {
                    factory.owner == oid
                        && factory.progress >= super::factory::PRODUCTION_STEPS
                        && factory.object.as_ref().is_some_and(|object| {
                            object.type_id == tid && object.entity_id.is_some()
                        })
                })
                .count() as u32
        }
        _ => 0,
    };

    owned + queued + ready.saturating_sub(materialized_ready)
}

fn first_missing_prereq(
    sim: &Simulation,
    rules: &RuleSet,
    owner: &str,
    prereqs: &[String],
) -> Option<String> {
    for p in prereqs {
        if p.is_empty() {
            continue;
        }
        // Only structures satisfy prerequisites — units/infantry/aircraft don't count.
        let ok = sim.substrate.entities.values().any(|e| {
            !e.dying
                && !e.lifecycle.in_limbo
                && sim.interner.resolve(e.owner()).eq_ignore_ascii_case(owner)
                && e.category == EntityCategory::Structure
                && !e.building_up()
                && structure_satisfies_prerequisite(rules, sim.interner.resolve(e.type_ref()), p)
        });
        if !ok {
            return Some(p.clone());
        }
    }
    None
}

pub(super) fn production_category_for_object(
    obj: &crate::rules::object_type::ObjectType,
) -> ProductionCategory {
    match obj.category {
        ObjectCategory::Infantry => ProductionCategory::Infantry,
        ObjectCategory::Vehicle if obj.naval => ProductionCategory::Ship,
        ObjectCategory::Vehicle => ProductionCategory::Vehicle,
        ObjectCategory::Aircraft => ProductionCategory::Aircraft,
        ObjectCategory::Building => match obj.build_cat {
            Some(BuildCategory::Combat) => ProductionCategory::Defense,
            _ => ProductionCategory::Building,
        },
    }
}

pub(super) fn supports_live_production(obj: &crate::rules::object_type::ObjectType) -> bool {
    matches!(
        production_category_for_object(obj),
        ProductionCategory::Building
            | ProductionCategory::Defense
            | ProductionCategory::Infantry
            | ProductionCategory::Vehicle
            | ProductionCategory::Aircraft
            | ProductionCategory::Ship
    )
}

pub(super) fn has_factory_for_owner(
    entities: &EntityStore,
    rules: &RuleSet,
    owner: &str,
    category: ProductionCategory,
    interner: &crate::sim::intern::StringInterner,
) -> bool {
    entities.values().any(|e| {
        !e.dying
            && !e.lifecycle.in_limbo
            && interner.resolve(e.owner()).eq_ignore_ascii_case(owner)
            && e.category == EntityCategory::Structure
            && !e.building_up()
            && is_production_factory(rules, interner.resolve(e.type_ref()), category)
    })
}

/// Check if a structure is a production factory for the given category.
///
/// Uses the data-driven Factory= key from rules.ini via RuleSet.factory_map.
/// A building with `Factory=InfantryType` produces infantry, `Factory=UnitType`
/// produces vehicles, etc. Buildings without Factory= are never factories.
pub(super) fn is_production_factory(
    rules: &RuleSet,
    structure_id: &str,
    category: ProductionCategory,
) -> bool {
    let Some(factory_type) = rules.factory_type(structure_id) else {
        return false;
    };
    match category {
        ProductionCategory::Infantry => factory_type == FactoryType::InfantryType,
        ProductionCategory::Vehicle => {
            factory_type == FactoryType::UnitType
                && rules
                    .object(structure_id)
                    .is_some_and(|object| !object.naval)
        }
        ProductionCategory::Ship => {
            factory_type == FactoryType::UnitType
                && rules
                    .object(structure_id)
                    .is_some_and(|object| object.naval)
        }
        ProductionCategory::Aircraft => factory_type == FactoryType::AircraftType,
        ProductionCategory::Building | ProductionCategory::Defense => {
            factory_type == FactoryType::BuildingType
        }
    }
}

/// The owner's live factories for `category`, as the House factory counters
/// hold them for `Time_To_Build`'s MultipleFactory loop (`0x00500910`): a
/// factory counts from `BuildingClass::Unlimbo` (`0x00440D13`, through
/// `0x004FFA50`) until Limbo (`0x00445D8E`, through `0x004FF980`), so a factory
/// still in its build-up animation counts. VERA drops a dying factory at once;
/// whether gamemd's Limbo lags a building's death is untraced.
pub(in crate::sim::production) fn matching_factory_count_for_owner(
    entities: &EntityStore,
    rules: &RuleSet,
    owner: &str,
    category: ProductionCategory,
    interner: &crate::sim::intern::StringInterner,
) -> u32 {
    entities
        .values()
        .filter(|e| {
            !e.dying
                && !e.lifecycle.in_limbo
                && interner.resolve(e.owner()).eq_ignore_ascii_case(owner)
                && e.category == EntityCategory::Structure
                && is_production_factory(rules, interner.resolve(e.type_ref()), category)
        })
        .count() as u32
}

pub fn producer_candidates_for_owner_category(
    entities: &EntityStore,
    rules: &RuleSet,
    owner: &str,
    category: ProductionCategory,
    require_matching_factory: bool,
    interner: &crate::sim::intern::StringInterner,
) -> Vec<(u64, u16, u16, String)> {
    let mut preferred_factories: Vec<(u64, u16, u16, String)> = Vec::new();
    for e in entities.values() {
        // A Dying factory corpse must not be selected as a unit's producer/exit.
        if e.dying {
            continue;
        }
        if e.lifecycle.in_limbo {
            continue;
        }
        if !interner.resolve(e.owner()).eq_ignore_ascii_case(owner) {
            continue;
        }
        if e.category != EntityCategory::Structure {
            continue;
        }
        if e.building_up() {
            continue;
        }
        let type_ref_str = interner.resolve(e.type_ref());
        let is_match = is_production_factory(rules, type_ref_str, category);
        if require_matching_factory && !is_match {
            continue;
        }
        if !require_matching_factory || is_match {
            preferred_factories.push((
                e.stable_id(),
                e.position.rx,
                e.position.ry,
                type_ref_str.to_string(),
            ));
        }
    }
    preferred_factories.sort_by(|a, b| a.0.cmp(&b.0));
    preferred_factories
}

pub fn is_matching_factory(
    rules: &RuleSet,
    structure_id: &str,
    produced_category: ObjectCategory,
) -> bool {
    match produced_category {
        ObjectCategory::Infantry => {
            is_production_factory(rules, structure_id, ProductionCategory::Infantry)
        }
        ObjectCategory::Vehicle => {
            is_production_factory(rules, structure_id, ProductionCategory::Vehicle)
                || is_production_factory(rules, structure_id, ProductionCategory::Ship)
        }
        ObjectCategory::Aircraft => {
            is_production_factory(rules, structure_id, ProductionCategory::Aircraft)
        }
        ObjectCategory::Building => {
            is_production_factory(rules, structure_id, ProductionCategory::Building)
        }
    }
}

pub fn structure_satisfies_prerequisite(rules: &RuleSet, structure_id: &str, prereq: &str) -> bool {
    // Direct match: the structure ID is exactly the prerequisite.
    if structure_id.eq_ignore_ascii_case(prereq) {
        return true;
    }
    // Alias match: look up the prerequisite in [General] PrerequisiteXxx groups.
    // e.g. prereq="POWER" → check if structure_id is in PrerequisitePower list.
    if let Some(group) = rules.prerequisite_group(prereq) {
        let sid_upper: String = structure_id.to_ascii_uppercase();
        return group.iter().any(|id| *id == sid_upper);
    }
    false
}

/// Returns the base foundation cells for normal building occupancy.
///
/// gamemd keeps these cells separate from `AddOccupy`/`RemoveOccupy`, which only
/// adjust hidden occupancy counters behind `CanHideThings`.
pub fn building_base_foundation_cells(
    origin_rx: u16,
    origin_ry: u16,
    foundation: &str,
) -> Vec<(u16, u16)> {
    use std::collections::BTreeSet;
    let (w, h) = foundation_dimensions(foundation);
    let mut cells: BTreeSet<(u16, u16)> = BTreeSet::new();

    for dx in 0..w {
        for dy in 0..h {
            let rx = origin_rx as i32 + dx as i32;
            let ry = origin_ry as i32 + dy as i32;
            if rx >= 0 && rx <= u16::MAX as i32 && ry >= 0 && ry <= u16::MAX as i32 {
                cells.insert((rx as u16, ry as u16));
            }
        }
    }

    cells.into_iter().collect()
}

/// Cells that block static grid movement, given the base foundation and whether
/// the building has a bib (`Bib=yes` in rules.ini).
///
/// For non-bib buildings, this is just the base foundation. For `Bib=yes`
/// buildings, the east-edge column of the foundation is excluded: those cells
/// are unit-passable in the original engine via the per-occupant-chain bib
/// relaxation in `Can_Enter_Cell` (probes the east neighbor; if it isn't part
/// of the same building, the building stops blocking the cell).
///
/// "East edge" = any cell in `base_foundation` whose east neighbor `(x+1, y)`
/// is outside the base foundation.
///
/// `NumberImpassableRows` is deliberately not applied here. That field is a
/// live `UnitClass::Can_Enter_Cell` object-list skip (`0x0073F57C..5A2` radio
/// contact, `0x0073F74B..774` UnitRepair/Bunker through `0x00458A00`), which
/// the Unit Find_Path search reaches through `Simulation::foot_can_enter`; it
/// is not static terrain data.
pub fn building_movement_blocking_cells(
    base_foundation: &[(u16, u16)],
    has_bib: bool,
) -> Vec<(u16, u16)> {
    use std::collections::BTreeSet;
    if !has_bib {
        return base_foundation.to_vec();
    }
    let set: BTreeSet<(u16, u16)> = base_foundation.iter().copied().collect();
    base_foundation
        .iter()
        .copied()
        .filter(|&(x, y)| x.checked_add(1).is_some_and(|nx| set.contains(&(nx, y))))
        .collect()
}

#[cfg(test)]
mod footprint_tests {
    use super::*;

    #[test]
    fn rectangle_only_4x3() {
        let cells = building_base_foundation_cells(10, 20, "4x3");
        assert_eq!(cells.len(), 12);
        assert!(cells.contains(&(10, 20)));
        assert!(cells.contains(&(13, 22)));
    }

    #[test]
    fn garefn_base_foundation_ignores_hidden_occupy_modifiers() {
        // GAREFN: Foundation=4x3, AddOccupy1=-1,0, AddOccupy2=-1,-1, RemoveOccupy1=3,1
        let cells = building_base_foundation_cells(10, 20, "4x3");
        assert_eq!(cells.len(), 12);
        assert!(cells.contains(&(13, 21)));
        assert!(!cells.contains(&(9, 19)));
        assert!(!cells.contains(&(9, 20)));
    }

    #[test]
    fn narefn_base_foundation_keeps_dock_pad_despite_remove_occupy() {
        let cells = building_base_foundation_cells(10, 20, "4x3");
        assert_eq!(cells.len(), 12);
        assert!(cells.contains(&(13, 21)));
        assert!(!cells.contains(&(8, 20)));
        assert!(!cells.contains(&(8, 19)));
        assert!(!cells.contains(&(8, 18)));
    }

    #[test]
    fn movement_blocking_no_bib_keeps_full_footprint() {
        let footprint = building_base_foundation_cells(10, 20, "4x3");
        let blocking = building_movement_blocking_cells(&footprint, false);
        assert_eq!(blocking.len(), footprint.len());
    }

    #[test]
    fn movement_blocking_with_bib_drops_east_edge_rectangle() {
        // Plain 4x3 with bib → east column (x = 13) drops.
        let footprint = building_base_foundation_cells(10, 20, "4x3");
        let blocking = building_movement_blocking_cells(&footprint, true);
        // 12 base cells - 3 east-edge column (13, 20), (13, 21), (13, 22) = 9.
        assert_eq!(blocking.len(), 9);
        assert!(!blocking.contains(&(13, 20)));
        assert!(!blocking.contains(&(13, 21)));
        assert!(!blocking.contains(&(13, 22)));
        assert!(blocking.contains(&(12, 20)));
        assert!(blocking.contains(&(10, 22)));
    }

    #[test]
    fn movement_blocking_with_bib_garefn_uses_base_foundation_topology() {
        let footprint = building_base_foundation_cells(10, 20, "4x3");
        let blocking = building_movement_blocking_cells(&footprint, true);
        assert_eq!(blocking.len(), 9);
        assert!(!blocking.contains(&(9, 19)));
        assert!(!blocking.contains(&(9, 20)));
        assert!(blocking.contains(&(12, 21)));
        assert!(!blocking.contains(&(13, 20)));
        assert!(!blocking.contains(&(13, 21)));
        assert!(blocking.contains(&(10, 20)));
    }

    #[test]
    fn static_movement_blocking_keeps_number_rows_out_of_the_grid() {
        let footprint = building_base_foundation_cells(10, 20, "4x3");
        let blocking = building_movement_blocking_cells(&footprint, false);
        assert_eq!(blocking.len(), 12);
        assert!(blocking.contains(&(12, 21)));
        assert!(blocking.contains(&(13, 21)));
    }
}
