//! Production views, economy queries and completed mobile delivery.
//!
//! `tick_production()` dispatches completed factory output after the frame's
//! charge sweep. Factory-held identity and accounting settle in factory_lifecycle.

use std::collections::BTreeMap;

use crate::rules::ruleset::RuleSet;
use crate::sim::intern::InternedId;
use crate::sim::world::Simulation;

use super::PRODUCTION_STEPS;
use super::factory_lifecycle;
use super::production_spawn::{
    ProductionDeliveryKind, ProductionSpawnSelection, find_helipad_for_aircraft,
    find_spawn_selection_for_owner_with_type, mark_war_factory_spawn_contact,
    unlimbo_held_naval_unit,
};
use super::production_tech::{owner_matches_build_identity, production_category_for_object};
use super::production_types::*;

pub fn credits_for_owner(sim: &Simulation, owner: &str) -> i32 {
    sim.interner
        .get(owner)
        .and_then(|id| sim.houses.get(&id))
        .map(|h| h.economy.credits)
        .unwrap_or(STARTING_CREDITS)
}

pub fn power_balance_for_owner(sim: &Simulation, _rules: &RuleSet, owner: &str) -> (i32, i32) {
    // Read from cached PowerState (health-scaled output, full-rated drain).
    // Updated each tick by power_system::tick_power_states().
    let Some(owner_id) = sim.interner.get(owner) else {
        return (0, 0);
    };
    sim.power_states
        .get(&owner_id)
        .map(|state| (state.total_output, state.total_drain))
        .unwrap_or((0, 0))
}

/// Sum of |Power=| from TypeClass for ALL owned buildings (including under
/// construction). Used by the sidebar power bar fill curve.
pub fn theoretical_power_for_owner(sim: &Simulation, owner: &str) -> i32 {
    let Some(owner_id) = sim.interner.get(owner) else {
        return 0;
    };
    sim.power_states
        .get(&owner_id)
        .map(|state| state.theoretical_total_power)
        .unwrap_or(0)
}

pub(in crate::sim) fn credits_entry_for_owner<'a>(
    sim: &'a mut Simulation,
    owner: &str,
) -> &'a mut i32 {
    let key = sim.interner.intern(owner);
    // Ensure house entry exists (auto-create with defaults if missing).
    // is_human defaults to true: in real games the app loading path seeds every house
    // with its actual flag, so the only callers that hit this fallback are
    // tests / edge cases that never declared a player. Defaulting to human
    // keeps those paths from accidentally activating AI-only behavior
    // (e.g., AIVirtualPurifiers credit bonus in the deposit path).
    if !sim.houses.contains_key(&key) {
        sim.houses.insert(
            key,
            crate::sim::house_state::HouseState::new(key, 0, None, true, STARTING_CREDITS, 10),
        );
    }
    &mut sim.houses.get_mut(&key).unwrap().economy.credits
}

/// Build a production list across supported sidebar categories for an owner.
///
/// In RA2, only items the player has unlocked via the tech tree are shown
/// ([`BuildOption::visible_in_sidebar`]).
pub fn build_options_for_owner(sim: &Simulation, rules: &RuleSet, owner: &str) -> Vec<BuildOption> {
    let options: Vec<BuildOption> =
        super::production_tech::all_build_options_for_owner(sim, rules, owner);

    // Diagnostic: log reason breakdown when nothing is buildable.
    let enabled_count = options.iter().filter(|o| o.enabled).count();
    if enabled_count == 0 && sim.session.tick % 90 == 0 {
        let mut reason_counts: BTreeMap<&str, usize> = BTreeMap::new();
        for opt in &options {
            let key = match &opt.reason {
                Some(BuildDisabledReason::UnbuildableTechLevel) => "UnbuildableTechLevel",
                Some(BuildDisabledReason::WrongOwner) => "WrongOwner",
                Some(BuildDisabledReason::WrongHouse) => "WrongHouse",
                Some(BuildDisabledReason::ForbiddenHouse) => "ForbiddenHouse",
                Some(BuildDisabledReason::RequiresStolenTech) => "RequiresStolenTech",
                Some(BuildDisabledReason::MissingPrerequisite(_)) => "MissingPrerequisite",
                Some(BuildDisabledReason::NoFactory) => "NoFactory",
                Some(BuildDisabledReason::AtBuildLimit) => "AtBuildLimit",
                None => "Enabled",
            };
            *reason_counts.entry(key).or_default() += 1;
        }
        log::warn!(
            "[BUILD-DIAG] owner='{}' tick={} total_items={} reasons={:?}",
            owner,
            sim.session.tick,
            options.len(),
            reason_counts
        );
        // Log owned structures and their factory status.
        for e in sim.substrate.entities.values() {
            if sim.interner.resolve(e.owner()).eq_ignore_ascii_case(owner)
                && e.category == crate::map::entities::EntityCategory::Structure
            {
                let ts = sim.interner.resolve(e.type_ref());
                log::warn!(
                    "[BUILD-DIAG]   structure '{}' building_up={} factory_type={:?}",
                    ts,
                    e.building_up(),
                    rules.factory_type(ts)
                );
            }
        }
        // Log a few sample failures to show the exact reason per item.
        for opt in options.iter().filter(|o| !o.enabled).take(5) {
            let type_str = sim.interner.resolve(opt.type_id);
            log::warn!(
                "[BUILD-DIAG]   sample: '{}' reason={:?}",
                type_str,
                opt.reason
            );
        }
    }

    let visible: Vec<BuildOption> = options
        .into_iter()
        .filter(BuildOption::visible_in_sidebar)
        .collect();
    dedupe_visible_build_options(visible, sim, rules, owner, &sim.interner)
}

fn dedupe_visible_build_options(
    options: Vec<BuildOption>,
    sim: &Simulation,
    rules: &RuleSet,
    owner: &str,
    interner: &crate::sim::intern::StringInterner,
) -> Vec<BuildOption> {
    let mut deduped: Vec<BuildOption> = Vec::new();
    let mut seen: BTreeMap<(ProductionCategory, String), usize> = BTreeMap::new();

    for option in options {
        let Some(key) = build_option_sidebar_key(rules, &option, interner) else {
            deduped.push(option);
            continue;
        };

        let seen_key = (option.queue_category, key);
        if let Some(existing_idx) = seen.get(&seen_key).copied() {
            let existing = &deduped[existing_idx];
            if prefers_sidebar_variant(sim, rules, owner, &option, existing, interner) {
                deduped[existing_idx] = option;
            }
            continue;
        }

        seen.insert(seen_key, deduped.len());
        deduped.push(option);
    }

    deduped
}

fn build_option_sidebar_key(
    rules: &RuleSet,
    option: &BuildOption,
    interner: &crate::sim::intern::StringInterner,
) -> Option<String> {
    let type_str = interner.resolve(option.type_id);
    let obj = rules.object(type_str)?;
    let image_key = if obj.image.trim().is_empty() {
        obj.id.to_ascii_uppercase()
    } else {
        obj.image.to_ascii_uppercase()
    };
    Some(format!("{}:{image_key}", option.object_category as u8))
}

fn prefers_sidebar_variant(
    sim: &Simulation,
    rules: &RuleSet,
    owner: &str,
    candidate: &BuildOption,
    existing: &BuildOption,
    interner: &crate::sim::intern::StringInterner,
) -> bool {
    sidebar_variant_rank(sim, rules, owner, candidate, interner)
        > sidebar_variant_rank(sim, rules, owner, existing, interner)
}

fn sidebar_variant_rank(
    sim: &Simulation,
    rules: &RuleSet,
    owner: &str,
    option: &BuildOption,
    interner: &crate::sim::intern::StringInterner,
) -> (u8, u16, u8) {
    let type_str = interner.resolve(option.type_id);
    let Some(obj) = rules.object(type_str) else {
        return (0, 0, 0);
    };

    let required_house_match = obj
        .required_houses
        .iter()
        .any(|house| owner_matches_build_identity(sim, owner, house));
    let owner_specificity = u16::MAX.saturating_sub(obj.owner.len() as u16);
    let enabled = option.enabled as u8;

    (required_house_match as u8, owner_specificity, enabled)
}

/// True if this owner has at least one buildable production option — useful
/// for picking a likely local player house in UI code.
pub fn has_build_option_for_owner(sim: &Simulation, rules: &RuleSet, owner: &str) -> bool {
    super::production_tech::all_build_options_for_owner(sim, rules, owner)
        .iter()
        .any(|o| o.enabled)
}

/// Advance production timers and spawn completed items.
pub fn tick_production(sim: &mut Simulation, rules: &RuleSet) -> bool {
    tick_production_with_overlay_registry(sim, rules, None)
}

/// Advance production timers and spawn completed items with optional native
/// tiberium context for harvester-side reduction/reseed.
pub fn tick_production_with_overlay_registry(
    sim: &mut Simulation,
    rules: &RuleSet,
    overlay_registry: Option<&crate::map::overlay_types::OverlayTypeRegistry>,
) -> bool {
    // P5d: the registry is the queue-of-record + completion authority. Collect the
    // (owner, category) keys whose active build has completed (progress == 54, object held),
    // in construction (insertion_seq) order — the SAME order step_all charged in and the
    // hash folds. The registry advances (StartNextQueued) on a
    // successful delivery (C7), not on completion alone.
    let completed_keys = sim.production.factory_shadow.completed_keys();
    if completed_keys.is_empty() {
        return false;
    }

    let mut spawned_any = false;
    for (owner_id, queue_category) in completed_keys {
        let owner_str = sim.interner.resolve(owner_id).to_string();
        // The completed-held active object's type.
        let Some(done_type) = sim
            .production
            .factory_shadow
            .view(owner_id, queue_category)
            .and_then(|v| v.object.map(|o| o.type_id))
        else {
            continue;
        };
        let done_type_str = sim.interner.resolve(done_type).to_string();
        let produced_category = rules.object(&done_type_str).map(|o| o.category);
        factory_lifecycle::publish_completion(sim, rules, owner_id, queue_category);
        if produced_category == Some(crate::rules::object_type::ObjectCategory::Building) {
            continue;
        }
        let is_vehicle =
            produced_category == Some(crate::rules::object_type::ObjectCategory::Vehicle);
        // Aircraft use helipad spawn path; other units use exit cell path.
        let is_aircraft =
            produced_category == Some(crate::rules::object_type::ObjectCategory::Aircraft);
        let (selection, airfield) = if is_aircraft {
            let Some((airfield, rx, ry)) = find_helipad_for_aircraft(sim, rules, &owner_str) else {
                // No free helipad — refund.
                factory_lifecycle::refund_failed_delivery(sim, rules, owner_id, queue_category);
                continue;
            };
            let selection = ProductionSpawnSelection {
                producer_id: airfield,
                cell: (rx, ry),
                delivery: ProductionDeliveryKind::Standard,
            };
            (selection, Some(airfield))
        } else {
            let is_naval: bool = rules.object(&done_type_str).map_or(false, |o| o.naval);
            let spawn_selection = produced_category.and_then(|cat| {
                find_spawn_selection_for_owner_with_type(
                    sim,
                    rules,
                    &owner_str,
                    Some(&done_type_str),
                    cat,
                    is_naval,
                )
            });
            let Some(selection) = spawn_selection else {
                if is_vehicle {
                    continue;
                }
                factory_lifecycle::refund_failed_delivery(sim, rules, owner_id, queue_category);
                continue;
            };
            (selection, None)
        };

        let Some(stable_id) = factory_lifecycle::active_entity_id(sim, owner_id, queue_category)
        else {
            debug_assert!(
                false,
                "active Factory object must own its StartProduction entity before delivery"
            );
            continue;
        };

        let delivered = deliver_produced_object(
            sim,
            rules,
            owner_id,
            &done_type_str,
            stable_id,
            selection,
            airfield,
            overlay_registry,
        );
        if delivered.is_some() {
            spawned_any = true;
            factory_lifecycle::release_delivered_mobile(sim, rules, owner_id, queue_category);
        } else {
            if is_vehicle {
                continue;
            }
            factory_lifecycle::refund_failed_delivery(sim, rules, owner_id, queue_category);
        }
    }

    // P5d: drop any factory left idle (delivered + empty queue) — replaces the
    // `queues_by_owner.retain` prune.
    sim.production.factory_shadow.prune_all_idle();
    spawned_any
}

/// Unlimbo house `owner_id`'s held produced object `stable_id` (of type
/// `type_name`) at `selection`, docked at `airfield` for an aircraft, then
/// its arrival: the war factory contact, the airfield pad, the player's
/// "unit ready", the Slave Miner's hunt and the rally move. `Some` with the
/// object when it was placed; otherwise it stays held in limbo.
///
/// The player's queue (above) and a computer's factory building
/// (`production::factory_ai`) both deliver through it.
#[allow(clippy::too_many_arguments)]
pub(super) fn deliver_produced_object(
    sim: &mut Simulation,
    rules: &RuleSet,
    owner_id: InternedId,
    type_name: &str,
    stable_id: u64,
    selection: ProductionSpawnSelection,
    airfield: Option<u64>,
    overlay_registry: Option<&crate::map::overlay_types::OverlayTypeRegistry>,
) -> Option<u64> {
    let (rx, ry) = selection.cell;
    // ExitObject443C81 writes this byte before class dispatch, even if a later
    // Unlimbo fails. It is the existing Techno+3D5 owner, not a new flag.
    let is_unit = sim.substrate.entities.get_mut(stable_id).map(|product| {
        product.in_playfield = true;
        product.category == crate::map::entities::EntityCategory::Unit
    })?;
    let land_factory = is_unit
        && sim
            .substrate
            .entities
            .get(selection.producer_id)
            .is_some_and(|producer| {
                super::production_spawn::exact_land_vehicle_exit_factory(
                    rules,
                    sim.interner.resolve(producer.type_ref()),
                ) && producer.mission.effective().known()
                    != Some(crate::sim::mission::MissionType::Unload)
            });
    let spawned = if land_factory {
        let producer = sim.substrate.entities.get(selection.producer_id)?;
        let coord = crate::sim::movement::configured_building_exit_coordinate(
            crate::sim::movement::ground_pose::position_world_coord(&producer.position),
            rules.object(sim.interner.resolve(producer.type_ref()))?,
        )?;
        sim.with_object_placement_scope(|sim| {
            let spawned = sim.reveal_constructed_object_at_coord_with_overlay_context(
                stable_id,
                coord,
                64,
                crate::sim::world::PlacementEvidence::EvaluateMark,
                rules,
                overlay_registry,
            )?;
            // The native successful suffix44459F..4445C9 re-marks around the
            // second coordinate write. Use the shared Mark/SetLocation owners.
            let context = crate::sim::world::UninitContext::new(Some(rules), overlay_registry);
            sim.unmark_entity_remove(spawned, context);
            crate::sim::movement::ground_pose::foot_set_location(
                &mut sim.substrate.entities,
                spawned,
                coord,
                Some(rules),
                &sim.interner,
            );
            sim.mark_entity_put(spawned, context);
            mark_war_factory_spawn_contact(sim, rules, selection.producer_id, spawned);
            // RESIDUAL:4445F0 queues producer Unload16, whose whole factory
            // animation/ForceTrack/alternate-producer lifecycle is unported.
            // Retain the existing delivery scheduling adapter until that
            // separate mechanism owns its exit/cleanup and timer/RNG cadence.
            Some(spawned)
        })
    } else {
        let spawned = match selection.delivery {
            ProductionDeliveryKind::NavalUnit => {
                unlimbo_held_naval_unit(sim, rules, stable_id, (rx, ry), overlay_registry)
            }
            ProductionDeliveryKind::Standard => {
                let z = sim.terrain_cell_level(rx, ry).unwrap_or(0);
                sim.reveal_constructed_object_at_height_with_overlay_context(
                    stable_id,
                    rx,
                    ry,
                    64,
                    z,
                    crate::sim::world::PlacementEvidence::EvaluateMark,
                    rules,
                    overlay_registry,
                )
            }
        }?;
        mark_war_factory_spawn_contact(sim, rules, selection.producer_id, spawned);
        Some(spawned)
    }?;
    // Aircraft spawned on helipad: reserve dock slot then set
    // DockedIdle carrying the assigned pad index.
    if let Some(af_id) = airfield {
        let max_slots = sim
            .substrate
            .entities
            .get(af_id)
            .and_then(|af| {
                let af_type = sim.interner.resolve(af.type_ref());
                let af_obj = rules.object(af_type)?;
                Some(af_obj.dock_contact_capacity())
            })
            .unwrap_or(1);
        let assigned_pad = sim
            .reserve_airfield_pad(af_id, spawned, max_slots)
            .unwrap_or(0); // Fresh spawn on a single-pad helipad always wins pad 0.
        if let Some(entity) = sim.substrate.entities.get_mut(spawned) {
            entity.aircraft_mission = Some(crate::sim::aircraft::AircraftMission::DockedIdle {
                airfield_id: af_id,
                pad_index: assigned_pad,
            });
        }
    }
    // `HouseClass::Place_Production 0x004FB5C6..0x004FB644`: for a
    // human-controlled house (`this == PlayerPtr` in MP, `+0x1EC ||
    // +0x1ED` in campaign) `CreateRadarEvent(6, object cell)` gates
    // `EVA_UnitReady` — type 6 dedupes within 2 cells, so two units
    // leaving one factory in quick succession give one line. The app
    // filters the owner to the local player and admits the event on
    // that client's radar.
    if sim
        .houses
        .get(&owner_id)
        .is_some_and(|house| house.is_controlled_by_human(sim.session.game_mode_nonzero))
    {
        sim.sound_events
            .push(crate::sim::world::SimSoundEvent::UnitComplete {
                owner: owner_id,
                radar: crate::sim::radar::RadarEventRequest::new(
                    crate::sim::radar::RadarEventType::UnitReady,
                    rx,
                    ry,
                ),
            });
    }
    let stable_id = spawned;
    // A Slave Miner leaving its war factory starts its hunt instead of
    // taking the rally point (`sim::slave_manager`).
    let hunting = matches!(selection.delivery, ProductionDeliveryKind::Standard)
        && sim.slave_master_leaves_factory(stable_id, rules);
    // Auto-move newly produced unit to rally point (if set).
    // Skip for aircraft docked on helipad — they wait for orders.
    if airfield.is_none() && !hunting {
        // `ExitObject_Main @ 0x00443C60` reads the factory's own
        // ArchiveTarget (`+0x218`, the rally point) for the object
        // leaving it; the naval arm reads it after Unlimbo
        // (`0x0044441A`).
        //
        // Residual (instruction reading; not ported): the non-naval
        // arms also copy it into the leaving object's own archive
        // (`0x0044498E`, `0x00444492`), which the Unit's exit arms
        // consume: a human house's unit drives to it
        // (`0x0073AAA1..0x0073AABB`), a computer house's
        // WeaponsFactory unit instead takes the cell of HouseClass
        // `0x00500200`, archives it and queues AreaGuard
        // (`0x0073A9DE..0x0073AA9C`); `0x00500200` draws Scenario
        // `RandomRanged(1, 4)` (`0x0050023B`) when the unit's vt+0x2D4,
        // +0x2D8 and +0x2DC sum is nonzero. VERA gives the rally move
        // here and writes no unit archive. Trigger: every produced
        // unit. Effect: a computer house's units stay at the factory
        // instead of spreading to posts; a unit's archive reads None
        // where native holds the rally cell (a harvester may also
        // take the Harvest exit arm, `0x0073AAE6`, unchecked); one
        // Scenario draw per armed computer war-factory unit is
        // missing. Frequency: every build. Downstream: the Scenario
        // RNG stream after computer unit production.
        let rally = sim
            .substrate
            .entities
            .get(selection.producer_id)
            .and_then(|producer| producer.rally_cell());
        let naval_rally = matches!(selection.delivery, ProductionDeliveryKind::NavalUnit)
            .then_some(rally)
            .flatten();
        if let Some((tx, ty)) = naval_rally {
            if let Some(entity) = sim.substrate.entities.get_mut(stable_id) {
                // BuildingClass::ExitObject_Main @ 0x0044442B calls
                // virtual Assign_Destination(target, 1) before its
                // deferred Queue_Mission(Move, 0). NavCom is the owner
                // destination; immediate A* is only a Rust executor.
                crate::sim::mission::concrete_effects::represented_assign_destination_mode_one(
                    entity,
                    Some(crate::sim::components::NavTargetRef::cell(tx, ty)),
                );
            }
            let _ = sim.mission_queue_exact(
                stable_id,
                crate::sim::mission::MissionId::from_known(crate::sim::mission::MissionType::Move),
                0,
                sim.session.binary_frame,
                &crate::sim::mission::authority::EntityReadyInputProvider,
            );
        }
        // Without a published grid the rally move and its restore are skipped.
        if let Some((tx, ty)) = rally
            && sim.path_grid().is_some()
        {
            let obj = rules.object(type_name);
            // The rally move is an ordinary move order, so a unit that leaves
            // the factory already promoted (InitialVeteran, cloning) drives to
            // the rally point at its FASTER speed.
            let speed = match sim.substrate.entities.get(stable_id) {
                Some(e) => crate::sim::movement::order_speed(e, obj, Some(rules), &sim.houses),
                None => crate::util::fixed_math::ra2_speed_to_leptons_per_second(
                    obj.map_or(4, |o| o.speed),
                ),
            };
            let speed_type = sim
                .substrate
                .entities
                .get(stable_id)
                .and_then(|e| e.locomotor.as_ref())
                .map(|l| l.speed_type);
            let _ = sim.issue_ground_move(
                crate::sim::world::GroundMove {
                    entity_id: stable_id,
                    target: (tx, ty),
                    speed,
                    queue: false,
                    speed_type,
                    owner_blocks: false,
                    object_destination: None,
                },
                Some(rules),
            );
            if naval_rally.is_some()
                && let Some(entity) = sim.substrate.entities.get_mut(stable_id)
            {
                // A Ship's setter publishes the rally unchanged; the
                // command-time adapter of the remaining locomotors
                // (Hover) may redirect its endpoint. Restore the
                // producer rally so that A* never owns NavCom.
                crate::sim::mission::concrete_effects::represented_assign_destination_mode_one(
                    entity,
                    Some(crate::sim::components::NavTargetRef::cell(tx, ty)),
                );
            }
        }
        if matches!(selection.delivery, ProductionDeliveryKind::NavalUnit)
            && let Some(entity) = sim.substrate.entities.get_mut(stable_id)
        {
            // Native +0x124/+0x1B4/+0x124 success tail writes the
            // selected CellClass centre after rally/mission assignment.
            entity.position.sub_x = crate::util::lepton::CELL_CENTER_LEPTON;
            entity.position.sub_y = crate::util::lepton::CELL_CENTER_LEPTON;
        }
    }
    Some(stable_id)
}

/// Build a queue snapshot for one owner, including progress metadata for UI.
///
/// Projects the player-visible build queue from the registry (the queue-of-record).
/// Per factory: the active build (head) then its FIFO tail, sorted by `(category,
/// stamp)` where stamp is the factory's construction stamp for the head (older than
/// every entry queued behind it) or a tail entry's `enqueue_order`.
///
/// `state` is DERIVED: head -> Paused if held by the user (`manual`), else Done if
/// complete-held (`progress >= PRODUCTION_STEPS`, the blocked-exit case that persists
/// across ticks), else Building (a cash stall included); tail -> Queued. `progress` is
/// the factory's step count; a queued tail item has not started.
pub fn queue_view_for_owner(sim: &Simulation, rules: &RuleSet, owner: &str) -> Vec<QueueItemView> {
    let Some(owner_id) = sim.interner.get(owner) else {
        return Vec::new();
    };
    // (category, stamp, type_id, state, progress)
    let mut items: Vec<(ProductionCategory, u64, InternedId, BuildQueueState, u16)> = Vec::new();
    for f in sim.production.factory_shadow.iter_insertion_ordered() {
        if f.owner != owner_id {
            continue;
        }
        if let Some(obj) = f.object.as_ref() {
            let state = if f.manual {
                BuildQueueState::Paused
            } else if f.progress >= PRODUCTION_STEPS {
                BuildQueueState::Done
            } else {
                BuildQueueState::Building
            };
            items.push((
                f.category,
                f.insertion_seq,
                obj.type_id,
                state,
                f.progress.min(PRODUCTION_STEPS),
            ));
        }
        for e in &f.queue {
            items.push((
                f.category,
                e.enqueue_order,
                e.type_id,
                BuildQueueState::Queued,
                0,
            ));
        }
    }
    items.sort_by_key(|&(category, stamp, ..)| (category, stamp));
    items
        .into_iter()
        .map(|(queue_category, _stamp, type_id, state, progress)| {
            let type_str = sim.interner.resolve(type_id);
            let display_name = rules
                .object(type_str)
                .and_then(|obj| obj.name.clone())
                .unwrap_or_else(|| type_str.to_string());
            QueueItemView {
                type_id,
                display_name,
                queue_category,
                state,
                progress,
            }
        })
        .collect()
}

pub fn ready_buildings_for_owner(
    sim: &Simulation,
    rules: &RuleSet,
    owner: &str,
) -> Vec<ReadyBuildingView> {
    let owner_id = sim.interner.get(owner);
    let ready = owner_id.and_then(|id| sim.production.ready_by_owner.get(&id));
    ready
        .map(|ready| {
            ready
                .iter()
                .filter_map(|&type_id| {
                    let type_str = sim.interner.resolve(type_id);
                    let obj = rules.object(type_str)?;
                    Some(ReadyBuildingView {
                        type_id,
                        display_name: obj.name.clone().unwrap_or_else(|| type_str.to_string()),
                        queue_category: production_category_for_object(obj),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}
