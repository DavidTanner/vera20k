//! War-factory exit radio-contact transient break.
//!
//! A newborn land vehicle from a war factory holds a live radio contact with its
//! producer so it can drive across the factory footprint (the NumberImpassableRows
//! row-skip read in `cell_entry::decide_live_vehicle_building_entry`). The existing
//! footprint sweep schedules the original RequestClearance8 teardown, whose
//! shared radio owners break both ends. Despawn / limbo cleanup
//! (`clear_radio_contacts_for`) remains the safety net. sim/ only — depends on
//! map/entities, rules, and sim::{entity_store,intern,movement::locomotor,occupancy}.

use crate::map::entities::EntityCategory;
use crate::rules::ruleset::RuleSet;
use crate::sim::movement::locomotor::MovementLayer;
use crate::sim::world::Simulation;

use super::production_spawn::exact_land_vehicle_exit_factory;

/// Break each war-factory exit contact whose vehicle has cleared the factory
/// footprint. Runs once per tick, right after ground movement.
///
/// Existing sweep gates (all must hold):
/// - the mover is a vehicle carrying a dock-entered flag (`+0x418`) toward a producer;
/// - that producer is a WeaponsFactory land-vehicle exit factory (the refinery's
///   dock-entered flag points at a refinery -> skipped, so its lifecycle is intact);
/// - the mover's current cell has no `Structure` occupant (footprint cleared).
///
/// Native Unit739EC0 reason2 reaches8 after mission, NavCom and first-ground-
/// Building gates73A7D2..73A930; this tick-level sweep does not port that whole
/// per-cell dispatcher. Source: unit_unlimbo's factory_exit_radio receipt.
pub fn tick_war_factory_exit_contacts(sim: &mut Simulation, rules: &RuleSet) {
    // Pass 1 (immutable reads): decide which mover contacts to break.
    // `entities.values()` iterates in stable_id order -> deterministic.
    let to_break: Vec<u64> = {
        let ents = &sim.substrate.entities;
        ents.values()
            .filter_map(|mover| {
                // Skip Dying corpses (mover or its producer) awaiting the
                // end-of-tick drain — don't compute contact breaks against them.
                if mover.dying || mover.category != EntityCategory::Unit {
                    return None;
                }
                let producer_id = mover.dock_entered_with?;
                let producer = ents.get(producer_id)?;
                if producer.dying || producer.category != EntityCategory::Structure {
                    return None;
                }
                if !exact_land_vehicle_exit_factory(
                    rules,
                    sim.interner.resolve(producer.type_ref()),
                ) {
                    return None;
                }
                let on_footprint = sim
                    .substrate
                    .occupancy
                    .get(mover.position.rx, mover.position.ry)
                    .is_some_and(|cell| {
                        cell.blockers(MovementLayer::Ground).any(|id| {
                            ents.get(id)
                                .is_some_and(|o| o.category == EntityCategory::Structure)
                        })
                    });
                if on_footprint {
                    return None;
                }
                Some(mover.stable_id())
            })
            .collect()
    };

    // UnitPerCell73A93D transmits8 to Contacts[0], not to the tether
    // projection. Building43CDDD -> Techno6F4C29 sends0x19 then0x03 BACK to the
    // Unit. The synchronous radio owners clear both sparse slots and tethers.
    // Native receipt: anytown_damage/unit_unlimbo factory_exit_radio controls.
    // The tick-level footprint sweep remains the existing scheduling adapter;
    // the original Unit per-cell dispatcher is a separate migration.
    for mover_id in to_break {
        crate::sim::radio::transmit_to_contact(
            sim,
            mover_id,
            crate::sim::radio::RadioMessage::RequestClearance,
            Some(rules),
        );
    }
}
