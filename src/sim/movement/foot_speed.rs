//! Shared Foot speed inputs and the existing deterministic fraction projection.
use crate::rules::object_type::ObjectType;
use crate::rules::ruleset::RuleSet;
use crate::sim::game_entity::GameEntity;
use crate::sim::intern::StringInterner;
use crate::sim::type_handle_table::TypeHandleTable;
use crate::util::fixed_math::{SIM_ZERO, SimFixed};

/// The speed a move order stamps into `MovementTarget::speed`: the adjusted
/// type speed of `FootClass::GetCurrentSpeed @ 0x004DB1A0` (stages 1 and 2),
/// in leptons/second. Every order, resume, scatter and rally calls this.
///
/// The stamp is VERA's; native keeps no order speed and re-queries the getter
/// (or a Fly/Jumpjet/Rocket locomotor its own speed) each Process frame. The
/// track, walk and Fly steps already re-query live, so the stamp's remaining
/// production reader is the legacy pass lane.
///
/// No minimum: none of the getter's truncations (`0x004DB1DB`, `0x004DB200`,
/// `0x004DB213`) clamp, so a `Speed=0` type stamps 0. Every retail mover that
/// authors `Speed=` reads at least 1. A rules-less fixture without a type moves
/// as `Speed=4`.
pub(crate) fn order_speed(
    entity: &GameEntity,
    object: Option<&ObjectType>,
    rules: Option<&RuleSet>,
) -> SimFixed {
    crate::sim::combat::veterancy::entity_mover_speed_leptons_per_second(
        entity,
        object,
        object.map_or(4, |object| object.speed),
        rules.map_or(1.0, |rules| rules.general.veteran_speed),
    )
}

/// Resolve live type/veterancy speed. A MovementTarget speed is a path cache,
/// not the type authority. Like gamemd's getter (`0x004DB1A0`), nothing about
/// the other units ordered with this one enters it.
pub(super) fn adjusted_speed(
    entity: &GameEntity,
    object: Option<&ObjectType>,
    veteran_speed: f64,
) -> SimFixed {
    object
        .map(|object| {
            crate::sim::combat::veterancy::entity_mover_speed_leptons_per_second(
                entity,
                Some(object),
                object.speed,
                veteran_speed,
            )
        })
        .or_else(|| entity.movement_target.as_ref().map(|target| target.speed))
        .unwrap_or(SIM_ZERO)
}

/// Foot4DB1A0: truncate adjusted type speed, then apply Foot+578 and truncate.
/// Rust's adjusted input is in leptons/second; native consumes a 15 Hz integer.
/// This shared fixed-point projection retains live crate and FASTER inputs. Native
/// house factors and CTF halving remain required getter-input work;
/// track_speed_native's isolated corpus is not their production implementation.
pub(crate) fn owner_current_speed_from_fraction(
    adjusted_speed_per_second: SimFixed,
    current_speed_fraction: SimFixed,
) -> i32 {
    let adjusted_type_speed = (adjusted_speed_per_second / SimFixed::from_num(15)).to_num::<i32>();
    (SimFixed::from_num(adjusted_type_speed) * current_speed_fraction).to_num::<i32>()
}

/// What the live GetCurrentSpeed reads beyond the owner: its type, through
/// the precomputed handle table, and `VeteranSpeed`. A query that runs every
/// frame for every Foot must not resolve the type by name.
#[derive(Clone, Copy)]
pub(crate) struct SpeedRules<'a> {
    rules: &'a RuleSet,
    interner: &'a StringInterner,
    types: &'a TypeHandleTable,
}

impl<'a> SpeedRules<'a> {
    pub(crate) fn new(
        rules: &'a RuleSet,
        interner: &'a StringInterner,
        types: &'a TypeHandleTable,
    ) -> Self {
        Self {
            rules,
            interner,
            types,
        }
    }

    /// Foot4DB1A0 for `entity`, live.
    pub(crate) fn owner_current_speed(self, entity: &GameEntity) -> i32 {
        owner_current_speed(
            entity,
            self.types
                .object(self.interner, entity.type_ref(), self.rules),
            self.rules.general.veteran_speed,
        )
    }
}

/// Foot4DB1A0 for the live owner outside a locomotor's own step: its adjusted
/// type speed and its applied fraction (Foot+578), through the same shared
/// projection the movers use. FireAt's lead reads it (`0x0070BD4C`).
pub(crate) fn owner_current_speed(
    entity: &GameEntity,
    object: Option<&ObjectType>,
    veteran_speed: f64,
) -> i32 {
    owner_current_speed_from_fraction(
        adjusted_speed(entity, object, veteran_speed),
        entity.foot_speed.applied_fraction,
    )
}

#[cfg(test)]
impl crate::sim::world::Simulation {
    /// An entity's live GetCurrentSpeed, for tests that observe a step's
    /// speed budget.
    pub(crate) fn current_speed_for_test(&self, id: u64, rules: &RuleSet) -> i32 {
        let entity = self.substrate.entities.get(id).expect("live entity");
        owner_current_speed(
            entity,
            self.object_type(entity.type_ref(), rules),
            rules.general.veteran_speed,
        )
    }
}

#[cfg(test)]
mod tests {
    /// The retail movers `order_speed`'s missing minimum can reach: every
    /// registered infantry, vehicle and aircraft type whose `Speed=` reads
    /// below 1 through the production reader. They are exactly the five
    /// registry entries that author no `Speed=` (the Visceroids come from
    /// `[General]`; `[DeathDummy]` authors only a weapon), so no authored
    /// retail mover saw the former 25-lepton floor or `.max(1)` clamps.
    #[test]
    fn retail_movers_below_speed_one_author_no_speed() {
        let Some(ini) = crate::rules::retail_ini_fixture::retail_ini("rulesmd.ini") else {
            return;
        };
        let rules = crate::rules::ruleset::RuleSet::from_ini(&ini).expect("retail rules parse");
        let slow: Vec<&str> = rules
            .infantry_ids
            .iter()
            .chain(&rules.vehicle_ids)
            .chain(&rules.aircraft_ids)
            .filter(|id| rules.object(id).is_none_or(|object| object.speed < 1))
            .map(String::as_str)
            .collect();
        assert_eq!(
            slow,
            ["DeathDummy", "YDUM", "VISC_LRG", "VISC_SML", "APACHE"]
        );
        assert!(slow.iter().all(|id| {
            ini.section(id)
                .is_none_or(|section| !section.is_present("Speed"))
        }));
    }
}
