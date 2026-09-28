//! A house's object counts, as its multiplayer defeat gate reads them.
//!
//! Native owner: `HouseClass`. Two independent sets of counters, each with its
//! own writers:
//! - Tracking (`HouseClass::Add_Tracking @ 0x004FF700`, `Remove_Tracking @
//!   0x004FF550`): added when the Techno is constructed (the class
//!   constructors and InitFromType, `0x007355EA`, `0x00517CD4`, `0x00414068`,
//!   `0x00442C62`), removed by its destructor at the pending-delete drain,
//!   moved by `TechnoClass::ChangeOwner` (`0x007015DE`, `0x007015E6`).
//!   `Insignificant=` and `DontScore=` types are never tracked. A building
//!   counts in `+0x2F0` unless it is a 1x1 undeployer (vtable `+0x80`,
//!   `0x00465D40`) or undeploys into a `ResourceGatherer=` (then `+0x2E8`,
//!   with the units); a Unit counts per type in `+0x5514`.
//! - On the map (`HouseClass::Added_To_Game @ 0x00502A80` from
//!   `TechnoClass::Unlimbo` `0x006F6D8F`, `Removed_From_Game @ 0x005025F0`
//!   from `TechnoClass::Limbo` `0x006F6BD1`, both from ChangeOwner
//!   `0x0070159D`/`0x0070178E`): per-type counters whose totals the normal
//!   game reads (`+0x5564` units, `+0x5578` infantry, `+0x558C` aircraft) and
//!   `+0x5550` per building type. `DontScore=` skips them, except that a
//!   Unit is added without the test (`0x00502CF9`) but removed with it, and
//!   an infantry survivor flagged `+0x6D9` is not added (`0x00502C3C`) but is
//!   removed.
//!
//! Added_To_Game and Removed_From_Game also count, before their per-class
//! arms and with no DontScore test, the house's objects whose type is a
//! `ResourceGatherer=` (`+0x158`, `0x00502A95..0x00502A9F` and
//! `0x00502606..0x00502610`). The computer reads it when it decides whether
//! a lost refinery node may be rebuilt (`sim::ai_base_building`).
//!
//! Their Aircraft, Infantry and Unit arms first add, and subtract, the
//! object's value, its type's `Cost_Of` (`vt+0x84`, `0x00711F00`) for this
//! house at that moment, again with no DontScore test: infantry to `+0x160A8`
//! unless `ConsideredAircraft=` (`+0xD96`), a unit to `+0x160AC` unless
//! `ConsideredAircraft=` or `Spawns=` (`+0xD58`), everything else of the
//! three to the air total `+0x160B0` (`0x00502B90..0x00502CE1`,
//! `0x005028D0..0x00502A15`); buildings have no value arm. The price follows
//! the house's FactoryPlants, so an object that leaves after they changed
//! takes out a different amount than it put in, and the totals drift as
//! natively. An enemy computer reads them as the house's forces
//! (`sim::ai_base_defense`).
//!
//! The gate (`HouseClass::Update @ 0x004F8E86..0x004F8F82`) reads only these:
//! a short game keeps a house alive while `+0x2F0 > 0` or its tracked
//! `BaseUnit=` types sum above zero; a normal game while `+0x2F0` plus the
//! on-map unit, infantry and aircraft totals and the on-map count of
//! `[AI] BuildRefinery=`'s third type is not zero. The other counters those
//! functions write (`+0x2E8`, `+0x2EC`, `+0x2F4`, `+0x2F8`, the owned-type
//! sets) have no reader in this mechanism and are not kept.
//!
//! Evidence: `tools/spatial_oracle/house_tracking.py` runs the original
//! Add_Tracking and Remove_Tracking (36 cases);
//! `tools/spatial_oracle/house_defeat_gate.py` runs the gate block with the
//! original counter readers (21 cases). Added_To_Game and Removed_From_Game
//! are read, not executed.
//!
//! RESIDUAL: the survivor flag `+0x6D9` is not written. SpawnSurvivors sets it
//! (`0x00443111..0x00443127`), before the survivor's Unlimbo, on a `Nominal=`
//! survivor (the Technician) of a building with Buildup art (Init_Managers sets
//! `+0x6E9` when the type has a Buildup shape, `0x00442CAA..0x00442CCF`); the
//! sale crew sets it after a successful Unlimbo (`0x0044A733..0x0044A747`;
//! VERA has no sale crew). Nothing clears it. Added_To_Game skips a flagged
//! infantry on every Unlimbo while Removed_From_Game still decrements it on
//! every Limbo, so each time a flagged Technician leaves the map (dies,
//! garrisons, boards) native `+0x5578` drops by one for good. Trigger: a
//! Technician survivor of an armed building (the 15% crew roll) leaving the
//! map in a normal (non-short) game. Effect: native's sum can reach zero while
//! objects stand (the house is defeated and they are blown up) or stay
//! negative when nothing is left (the house is never defeated); VERA does
//! neither.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::map::entities::EntityCategory;
use crate::rules::object_type::ObjectType;
use crate::rules::ruleset::HouseCostFactors;
use crate::sim::game_entity::GameEntity;
use crate::sim::intern::InternedId;

/// The type facts the counters test, fixed when the Techno is constructed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TrackingFacts {
    /// `ObjectType+0x232` Insignificant.
    pub insignificant: bool,
    /// A building tracked with the units: a 1x1 undeployer, or one that
    /// undeploys into a `ResourceGatherer=` (a deployed Slave Miner).
    pub unit_like_building: bool,
    /// `TechnoType+0x5EC`, `ResourceGatherer=` (ReadINI `0x007143DF`).
    pub resource_gatherer: bool,
    /// The value arm of the on-map writers; none for a building.
    #[serde(default)]
    pub force_value: Option<ForceValueFacts>,
}

/// The house total an object's value joins.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ForceKind {
    /// `HouseClass+0x160A8`.
    Infantry,
    /// `HouseClass+0x160AC`.
    Vehicles,
    /// `HouseClass+0x160B0`.
    Air,
}

/// What the on-map writers price: the total and the type's `Cost_Of` inputs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ForceValueFacts {
    pub kind: ForceKind,
    /// `TechnoType+0x610`, the `Cost=` that `Cost_Of` scales (read through
    /// virtual `+0xAC`, `0x00711EB0`).
    pub cost: i32,
    /// [`ObjectType::factor_slot`].
    pub factor_slot: u8,
}

impl ForceValueFacts {
    /// The value arm an object of class `category` and type `ty` takes (see
    /// the module doc).
    pub(crate) fn of(category: EntityCategory, ty: &ObjectType) -> Option<Self> {
        let kind = match category {
            EntityCategory::Aircraft => ForceKind::Air,
            EntityCategory::Infantry if ty.considered_aircraft => ForceKind::Air,
            EntityCategory::Infantry => ForceKind::Infantry,
            EntityCategory::Unit if ty.considered_aircraft || ty.spawns.is_some() => ForceKind::Air,
            EntityCategory::Unit => ForceKind::Vehicles,
            EntityCategory::Structure => return None,
        };
        Some(Self {
            kind,
            cost: ty.cost,
            factor_slot: ty.factor_slot() as u8,
        })
    }
}

/// `HouseClass+0x160A8`, `+0x160AC` and `+0x160B0`, which the constructor
/// zeroes (`0x004F5DB4..0x004F5DC0`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ForceValues {
    pub infantry: i32,
    pub vehicles: i32,
    pub air: i32,
}

impl ForceValues {
    fn total(&mut self, kind: ForceKind) -> &mut i32 {
        match kind {
            ForceKind::Infantry => &mut self.infantry,
            ForceKind::Vehicles => &mut self.vehicles,
            ForceKind::Air => &mut self.air,
        }
    }
}

/// The counters the defeat gate reads, and the on-map gatherer count (see
/// the module doc).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HouseTracking {
    /// `HouseClass+0x2F0`.
    buildings: i32,
    /// `HouseClass+0x5514`, the tracked count of each UnitType.
    unit_types: BTreeMap<InternedId, i32>,
    /// `HouseClass+0x5564` total.
    active_units: i32,
    /// `HouseClass+0x5578` total.
    active_infantry: i32,
    /// `HouseClass+0x558C` total.
    active_aircraft: i32,
    /// `HouseClass+0x5550`, the on-map count of each BuildingType.
    active_building_types: BTreeMap<InternedId, i32>,
    /// `HouseClass+0x158`, the on-map objects whose type is a
    /// `ResourceGatherer=`.
    #[serde(default)]
    resource_gatherers: i32,
    /// The value totals of the house's forces on the map.
    #[serde(default)]
    force_values: ForceValues,
}

impl HouseTracking {
    /// `HouseClass::Add_Tracking @ 0x004FF700`.
    pub(crate) fn add_tracking(&mut self, entity: &GameEntity) {
        self.track(entity, 1);
    }

    /// `HouseClass::Remove_Tracking @ 0x004FF550`.
    pub(crate) fn remove_tracking(&mut self, entity: &GameEntity) {
        self.track(entity, -1);
    }

    fn track(&mut self, entity: &GameEntity, delta: i32) {
        let facts = entity.tracking_facts;
        if facts.insignificant || entity.dont_score {
            return;
        }
        match entity.category {
            EntityCategory::Unit => {
                let count = self.unit_types.entry(entity.type_ref()).or_default();
                *count = count.wrapping_add(delta);
            }
            EntityCategory::Structure if !facts.unit_like_building => {
                self.buildings = self.buildings.wrapping_add(delta);
            }
            _ => {}
        }
    }

    /// Fold the defeat counters as the derived hash of this struct did before
    /// the gatherer count joined it (schema `HouseDefeatTracking`).
    pub(crate) fn hash_defeat_counters(&self, hasher: &mut impl std::hash::Hasher) {
        use std::hash::Hash;
        self.buildings.hash(hasher);
        self.unit_types.hash(hasher);
        self.active_units.hash(hasher);
        self.active_infantry.hash(hasher);
        self.active_aircraft.hash(hasher);
        self.active_building_types.hash(hasher);
    }

    /// `HouseClass+0x158`.
    pub(crate) const fn resource_gatherers(&self) -> i32 {
        self.resource_gatherers
    }

    /// `HouseClass+0x160A8`, `+0x160AC` and `+0x160B0`.
    pub(crate) const fn force_values(&self) -> ForceValues {
        self.force_values
    }

    /// `HouseClass+0x5550`'s count for one BuildingType (`0x0049FAE0`).
    pub(crate) fn active_building_count(&self, building: InternedId) -> i32 {
        self.active_building_types
            .get(&building)
            .copied()
            .unwrap_or(0)
    }

    /// `HouseClass::Added_To_Game @ 0x00502A80`, pricing with this house's
    /// `factors`.
    pub(crate) fn added_to_game(&mut self, entity: &GameEntity, factors: &HouseCostFactors) {
        if entity.tracking_facts.resource_gatherer {
            self.resource_gatherers = self.resource_gatherers.wrapping_add(1);
        }
        if let Some(value) = entity.tracking_facts.force_value {
            let total = self.force_values.total(value.kind);
            *total = total.wrapping_add(factors.adjust(value.cost, value.factor_slot.into()));
        }
        let dont_score = entity.dont_score;
        match entity.category {
            // The Unit case (increment at `0x00502CF9`) has no DontScore test.
            EntityCategory::Unit => self.active_units = self.active_units.wrapping_add(1),
            EntityCategory::Aircraft if !dont_score => {
                self.active_aircraft = self.active_aircraft.wrapping_add(1);
            }
            EntityCategory::Structure if !dont_score => {
                let count = self
                    .active_building_types
                    .entry(entity.type_ref())
                    .or_default();
                *count = count.wrapping_add(1);
            }
            EntityCategory::Infantry if !dont_score => {
                self.active_infantry = self.active_infantry.wrapping_add(1);
            }
            _ => {}
        }
    }

    /// `HouseClass::Removed_From_Game @ 0x005025F0`, pricing with this
    /// house's `factors`.
    pub(crate) fn removed_from_game(&mut self, entity: &GameEntity, factors: &HouseCostFactors) {
        if entity.tracking_facts.resource_gatherer {
            self.resource_gatherers = self.resource_gatherers.wrapping_sub(1);
        }
        if let Some(value) = entity.tracking_facts.force_value {
            let total = self.force_values.total(value.kind);
            *total = total.wrapping_sub(factors.adjust(value.cost, value.factor_slot.into()));
        }
        if entity.dont_score {
            return;
        }
        match entity.category {
            EntityCategory::Unit => self.active_units = self.active_units.wrapping_sub(1),
            EntityCategory::Aircraft => {
                self.active_aircraft = self.active_aircraft.wrapping_sub(1);
            }
            EntityCategory::Structure => {
                let count = self
                    .active_building_types
                    .entry(entity.type_ref())
                    .or_default();
                *count = count.wrapping_sub(1);
            }
            EntityCategory::Infantry => {
                self.active_infantry = self.active_infantry.wrapping_sub(1);
            }
        }
    }

    /// The short game's test (`0x004F8EC6..0x004F8F1D`): alive while
    /// `+0x2F0 > 0` or the tracked counts of `BaseUnit=` entries 1, 2 and 0
    /// sum above zero.
    pub(crate) fn short_game_alive(&self, base_units: &[Option<InternedId>; 3]) -> bool {
        let tracked = |slot: usize| {
            base_units[slot].map_or(0, |unit| self.unit_types.get(&unit).copied().unwrap_or(0))
        };
        let base = tracked(1).wrapping_add(tracked(2)).wrapping_add(tracked(0));
        self.buildings > 0 || base > 0
    }

    /// The normal game's test (`0x004F8F21..0x004F8F77`): alive while
    /// `+0x2F0` plus the on-map totals and the on-map count of `[AI]
    /// BuildRefinery=`'s third type is not zero.
    pub(crate) fn normal_game_alive(&self, build_refinery_2: Option<InternedId>) -> bool {
        let refinery = build_refinery_2.map_or(0, |refinery| {
            self.active_building_types
                .get(&refinery)
                .copied()
                .unwrap_or(0)
        });
        self.buildings
            .wrapping_add(self.active_units)
            .wrapping_add(self.active_infantry)
            .wrapping_add(self.active_aircraft)
            .wrapping_add(refinery)
            != 0
    }
}

#[cfg(test)]
impl HouseTracking {
    /// `+0x2F0`.
    pub(crate) fn buildings_for_test(&self) -> i32 {
        self.buildings
    }

    /// Stand in for tracked buildings a fixture places without the lifecycle.
    pub(crate) fn set_buildings_for_test(&mut self, buildings: i32) {
        self.buildings = buildings;
    }

    /// The tracked Units of every type.
    pub(crate) fn units_for_test(&self) -> i32 {
        self.unit_types.values().sum()
    }

    /// Stand in for on-map units a fixture places without the lifecycle.
    pub(crate) fn set_active_units_for_test(&mut self, units: i32) {
        self.active_units = units;
    }

    /// The on-map unit, infantry and aircraft totals.
    pub(crate) fn active_for_test(&self) -> (i32, i32, i32) {
        (
            self.active_units,
            self.active_infantry,
            self.active_aircraft,
        )
    }

    /// Set every counter the gate reads.
    pub(crate) fn set_for_test(
        &mut self,
        buildings: i32,
        unit_types: &[(InternedId, i32)],
        active: (i32, i32, i32),
        active_building_types: &[(InternedId, i32)],
    ) {
        self.buildings = buildings;
        self.unit_types = unit_types.iter().copied().collect();
        (
            self.active_units,
            self.active_infantry,
            self.active_aircraft,
        ) = active;
        self.active_building_types = active_building_types.iter().copied().collect();
    }
}

#[cfg(test)]
#[path = "house_tracking_tests.rs"]
mod tests;
