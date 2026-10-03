//! Joined paid GAPILE -> two GI output through the ordinary master-frame owner.
//!
//! Native local goldens: original ExitObject443C60 -> Infantry51DFF0,
//! InfantryIdle51CBA0, the next live InfantryAI51BAB0 -> Scatter51D0D0,
//! Walk75AC80 and automatic InfantryPerCell519630 -> radio8/25/3. The fixture
//! is mechanically selected from corrected original-byte observations with
//! the registered Walk CRT and Foot EMPTY startup prerequisites executed.
//! Rally adds original SetRally443860 and the later InfantryMovement520F40 ->
//! InfantryIdle51CBA0 -> Foot4D82B0 Archive consumption/destination handoff.
//!
//! This is structural production integration with native local comparisons.
//! P2 excludes whole producer/HouseAI and unrelated admitted actor AI, whereas
//! this test runs the canonical whole tick. Absolute P2 frames, the particular
//! Scatter destination and complete RNG streams are therefore not compared.
//! Corrected native coverage follows both rally GIs through Archive consumption,
//! actual paid Walk, final Guard and cleared navigation/Walk/radio state.

use super::Simulation;
use crate::map::overlay_types::OverlayTypeRegistry;
use crate::map::resolved_terrain::ResolvedTerrainGrid;
use crate::rules::locomotor_type::LocomotorKind;
use crate::rules::retail_ini_fixture::retail_battle_rules_for_map;
use crate::rules::ruleset::RuleSet;
use crate::rules::terrain_rules::TerrainRules;
use crate::sim::bridge_state::BridgeRuntimeState;
use crate::sim::combat::TargetKind;
use crate::sim::command::{Command, CommandEnvelope};
use crate::sim::components::NavTargetRef;
use crate::sim::house_state::{HouseDifficulty, HouseState};
use crate::sim::movement::ground_pose;
use crate::sim::overlay_grid::OverlayGrid;
use crate::sim::production::ProductionCategory;
use crate::sim::radio::{self, TransmitRecord};
use serde_json::{Value, json};

const TICK_MS: u32 = 67;

fn corpus() -> Value {
    let data: Value =
        serde_json::from_str(include_str!("fixtures/factory_infantry_local_native.json")).unwrap();
    assert_eq!(data["schema_version"], 2);
    assert_eq!(
        data["native_sha256"],
        "1cdd1180e49024fbda8ad568caac2e86e856063ff67ab38f62b7d2c7bb84298c"
    );
    assert_eq!(data["products"].as_array().unwrap().len(), 2);
    assert_eq!(data["rally"]["products"].as_array().unwrap().len(), 2);
    data
}

fn fixture_cell(value: &Value) -> (u16, u16) {
    (
        value[0].as_u64().unwrap().try_into().unwrap(),
        value[1].as_u64().unwrap().try_into().unwrap(),
    )
}

fn assert_native_fields(actual: &Value, expected: &Value, context: &str) {
    for (field, value) in expected.as_object().unwrap() {
        assert_eq!(
            &actual[field], value,
            "{context}: native local field {field}; actual state={actual}; expected state={expected}"
        );
    }
}

/// Supplied clear33x33/diamond16 map from the native fixture's local ground
/// prior. The physical map/theater startup is outside this test; path/zone,
/// terrain-cost and occupation updates still use their production owners.
fn install_ground(sim: &mut Simulation, rules: &RuleSet, terrain_rules: &TerrainRules) {
    let clear = terrain_rules.semantics_for_land_type(0).unwrap();
    let cells = (0..33)
        .flat_map(|y| {
            (0..33).map(move |x| {
                let mut cell = super::lifecycle_tests::common_raw_terrain_cell(x, y, 0, false);
                cell.terrain_class = clear.terrain_class;
                cell.base_terrain_class = clear.terrain_class;
                cell.speed_costs = clear.speed_costs;
                cell.base_speed_costs = clear.speed_costs;
                // The retained native class plane has an outside border7.
                if x == 0 || x == 32 || y == 0 || y == 32 {
                    cell.outside_playfield = true;
                    cell.zone_type = 7;
                    cell.ground_walk_blocked = true;
                }
                cell
            })
        })
        .collect();
    let terrain = ResolvedTerrainGrid::from_cells(33, 33, cells);
    sim.playfield_bounds =
        Some(crate::map::playfield::PlayfieldBounds::from_normalized_local_size(16, 0, 0, 16, 16));
    sim.playfield_size_height = Some(16);
    sim.session.map_width = 33;
    sim.session.map_height = 33;
    sim.overlay_grid = Some(OverlayGrid::new(33, 33));
    sim.bridge_state = Some(BridgeRuntimeState::from_resolved_terrain_with_map_size(
        &terrain,
        true,
        rules.bridge_rules.strength,
        (16, 16),
    ));
    sim.install_resolved_terrain_for_new_map(terrain);
    assert!(sim.rebuild_dynamic_navigation(rules));
}

fn local_state(sim: &Simulation, id: u64) -> Value {
    let entity = sim.substrate.entities.get(id).unwrap();
    let p = ground_pose::position_world_coord(&entity.position);
    let walk = entity.locomotor.as_ref().unwrap();
    json!({
        "location": [p.x, p.y, p.z],
        "cell": [entity.position.rx, entity.position.ry],
        "health": entity.health.current,
        "mission": entity.mission.current().raw(),
        "queued": entity.mission.queued().raw(),
        "nav_is_set": entity.navigation.nav_com.is_some(),
        "archive_is_set": entity.archive_target().is_some(),
        "tether": entity.dock_entered_with.is_some(),
        "contact_count": entity.radio_contacts.len(),
        "doing": entity.mission_leaf.as_infantry().unwrap().doing(),
        "idle_entry_latch": entity.mission_leaf.foot_idle_entry_latch(),
        "paid_head_is_set": walk.step_head().is_some(),
        "walk_is_moving": walk.walk_is_moving().unwrap(),
        "walk_destination_is_set": walk.walk_destination().is_some(),
        "paid_head": walk.step_head().map(|c| [c.x, c.y, c.z]),
        "walk_destination": walk.walk_destination().map(|c| [c.x, c.y, c.z]),
        "nav_target": entity.navigation.nav_com,
        "archive_target": entity.archive_target(),
        "stage": entity.native_stage().value(),
        "stage_rate": entity.native_stage().rate(),
    })
}

fn pair_radio(log: &[TransmitRecord], producer: u64, product: u64) -> Vec<Value> {
    let participant = |id| {
        if id == producer {
            "producer"
        } else {
            assert_eq!(id, product);
            "product"
        }
    };
    log.iter()
        .filter(|e| {
            (e.sender_sid == producer && e.target_sid == product)
                || (e.sender_sid == product && e.target_sid == producer)
        })
        .map(|e| {
            json!([
                participant(e.sender_sid),
                participant(e.target_sid),
                e.msg,
                e.reply.unwrap()
            ])
        })
        .collect()
}

#[derive(Default)]
struct ProductProgress {
    id: u64,
    completed_frame: Option<u32>,
    placed_frame: Option<u32>,
    first_live_frame: Option<u32>,
    release_frame: Option<u32>,
    archive_handoff_frame: Option<u32>,
    settled_frame: Option<u32>,
    changed_position: bool,
    paid_rally_walk: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum OutputRoute {
    NoRally,
    Rally,
    RallyBetweenCompletionAndPlace,
}

#[test]
fn no_rally_two_paid_gi_walk_out_and_release_their_factory_contacts() {
    joined_two_paid_gi(OutputRoute::NoRally);
}

#[test]
fn rally_two_paid_gi_consume_archive_walk_to_rally_and_settle() {
    joined_two_paid_gi(OutputRoute::Rally);
}

/// Building443860 appends SetRally before the next Strip6A8B30 appends PLACE.
/// The already admitted player event must reach the tail first: Exit443C60
/// copies the new Archive to the retained GI instead of its previous value.
#[test]
fn rally_between_factory_completion_and_place_reaches_the_held_gi() {
    joined_two_paid_gi(OutputRoute::RallyBetweenCompletionAndPlace);
}

fn joined_two_paid_gi(route: OutputRoute) {
    let native = corpus();
    let rally_cell =
        (route != OutputRoute::NoRally).then(|| fixture_cell(&native["rally"]["request"]["cell"]));
    let exit_cell = fixture_cell(&native["rally"]["exit_cell"]);
    let native_products = if rally_cell.is_some() {
        &native["rally"]["products"]
    } else {
        &native["products"]
    };
    let Some(retail) = retail_battle_rules_for_map("Hills.mmx") else {
        return;
    };
    let rules = retail.rules;
    let registry = OverlayTypeRegistry::from_ini(&retail.processed_rules, Some(&retail.fixed_art));
    let terrain_rules = TerrainRules::from_ini(&retail.processed_rules);
    let mut sim = Simulation::with_seed(2);
    sim.intern_rule_type_ids(&rules);
    sim.resolve_type_handles(&rules);
    install_ground(&mut sim, &rules, &terrain_rules);
    let owner = sim.interner.intern("Americans");
    // Human/currentHouse/difficulty0 and cash10000 are retained native priors.
    // TechLevel10 is explicit structural eligibility, not a measured P2 field.
    let mut house = HouseState::new(owner, 0, Some(owner), true, 10_000, 10);
    house.difficulty = HouseDifficulty::Hard;
    house.project_country_mults(&rules, &sim.interner);
    sim.houses.insert(owner, house);
    sim.session.house_order.push(owner);
    sim.session.current_house = Some(owner);
    sim.session.game_mode_nonzero = true;
    sim.session.game_options.game_speed = 3;
    let producer = sim
        .spawn_object_at_height_with_overlay_registry(
            "GAPILE",
            "Americans",
            14,
            14,
            64,
            0,
            &rules,
            &registry,
        )
        .expect("retail GAPILE enters through its shared constructor/admission");
    sim.advance_tick(&[], Some(&rules), None, Some(&registry), TICK_MS);
    let p =
        ground_pose::position_world_coord(&sim.substrate.entities.get(producer).unwrap().position);
    assert_eq!(json!([p.x, p.y, p.z]), native["producer_prior"]["location"]);
    assert_eq!(
        json!(sim.substrate.entities.get(producer).unwrap().health.current),
        native["producer_prior"]["health"]
    );
    assert_eq!(
        json!(sim.power_states[&owner].total_output),
        native["producer_prior"]["power"]
    );
    assert_eq!(
        json!(sim.power_states[&owner].total_drain),
        native["producer_prior"]["drain"]
    );
    if route == OutputRoute::Rally
        && let Some((rx, ry)) = rally_cell
    {
        let command = CommandEnvelope::new(
            owner,
            sim.session.tick + 1,
            Command::SetRally {
                rx,
                ry,
                producer_ids: vec![producer],
            },
        );
        sim.advance_tick(&[command], Some(&rules), None, Some(&registry), TICK_MS);
        assert_eq!(
            sim.substrate.entities.get(producer).unwrap().rally_cell(),
            rally_cell,
            "the ordinary player event archives the rally before Begin"
        );
    }
    let e1 = sim.interner.get("E1").unwrap();
    let commands = [
        CommandEnvelope::new(
            owner,
            sim.session.tick + 1,
            Command::QueueProduction { type_id: e1 },
        ),
        CommandEnvelope::new(
            owner,
            sim.session.tick + 1,
            Command::QueueProduction { type_id: e1 },
        ),
    ];
    radio::take_transmit_log();
    sim.advance_tick(&commands, Some(&rules), None, Some(&registry), TICK_MS);
    let head = sim
        .production
        .factory_shadow
        .view(owner, ProductionCategory::Infantry)
        .expect("real production event starts the first GI");
    let first = head.object.unwrap().entity_id.unwrap();
    assert_eq!(
        head.queue.len(),
        1,
        "the second Begin waits as an unconstructed tail"
    );
    assert_eq!(head.progress, 0);
    let mut products = vec![ProductProgress {
        id: first,
        ..Default::default()
    }];

    for _ in 0..10_000 {
        let frame = sim.session.binary_frame;
        if route == OutputRoute::RallyBetweenCompletionAndPlace
            && products[0].completed_frame.is_some()
            && products[0].placed_frame.is_none()
        {
            let (rx, ry) = rally_cell.unwrap();
            sim.queue_command(CommandEnvelope::new(
                owner,
                sim.session.tick + 1,
                Command::SetRally {
                    rx,
                    ry,
                    producer_ids: vec![producer],
                },
            ));
        }
        // The ordinary app drains player ingress before advancing each frame.
        let due = sim.take_due_commands();
        if route == OutputRoute::RallyBetweenCompletionAndPlace {
            let mut output = sim
                .advance_app_frame(
                    &due,
                    Some(&rules),
                    Some(&registry),
                    TICK_MS,
                    super::TickLane::Ordinary,
                    None,
                )
                .expect("the ordinary app frame completes");
            let admitted = output.take_admitted_commands();
            if products[0].completed_frame.is_some() && products[0].placed_frame.is_none() {
                let phase: Value = serde_json::from_str(include_str!(
                    "fixtures/factory_infantry_publication_native.json"
                ))
                .unwrap();
                assert_eq!(phase["native_sha256"], native["native_sha256"]);
                let event_types: Vec<_> = admitted
                    .iter()
                    .map(|command| match command.payload {
                        Command::SetRally { .. } => 0x1E,
                        Command::PlaceProducedMobile { .. } => 0x0B,
                        _ => panic!("unexpected admitted race command: {command:?}"),
                    })
                    .collect();
                let before = &phase["cases"]["before_strip"];
                assert_eq!(json!(event_types), before["executed_event_types"]);
                let state = local_state(&sim, first);
                assert_eq!(state["location"], before["placed"]["position"]);
                for field in [
                    "mission",
                    "queued",
                    "archive_is_set",
                    "nav_is_set",
                    "tether",
                ] {
                    assert_eq!(
                        state[field], before["placed"][field],
                        "native phase field {field}"
                    );
                }
                assert_eq!(state["walk_is_moving"], before["placed"]["walk_moving"]);
                assert_eq!(
                    state["walk_destination"],
                    before["placed"]["walk_destination"]
                );
            }
        } else {
            sim.advance_tick(&due, Some(&rules), None, Some(&registry), TICK_MS);
        }
        let log = radio::take_transmit_log();
        if let Some(view) = sim
            .production
            .factory_shadow
            .view(owner, ProductionCategory::Infantry)
            && let Some(held) = view.object.and_then(|object| object.entity_id)
        {
            if !products.iter().any(|p| p.id == held) {
                assert_eq!(products.len(), 1, "exactly two identities are constructed");
                assert!(
                    !sim.substrate
                        .entities
                        .get(first)
                        .unwrap()
                        .lifecycle
                        .in_limbo,
                    "the FIFO successor constructs only after the first PLACE admits"
                );
                assert!(sim.substrate.entities.get(held).unwrap().lifecycle.in_limbo);
                assert_eq!(view.progress, 0);
                assert!(view.queue.is_empty());
                products.push(ProductProgress {
                    id: held,
                    ..Default::default()
                });
            }
            let progress = products.iter_mut().find(|p| p.id == held).unwrap();
            if view.ready && progress.completed_frame.is_none() {
                progress.completed_frame = Some(frame);
                let entity = sim.substrate.entities.get(held).unwrap();
                assert!(entity.lifecycle.in_limbo && !entity.in_logic_vector);
                assert!(
                    !sim.pending_commands_for_tests().iter().any(|command| {
                        matches!(command.payload, Command::PlaceProducedMobile { .. })
                    }),
                    "Factory completion retains its change flag until the next Strip prefix"
                );
            }
        }
        for (index, progress) in products.iter_mut().enumerate() {
            let entity = sim.substrate.entities.get(progress.id).unwrap();
            if entity.lifecycle.in_limbo {
                assert!(!entity.in_logic_vector && !entity.lifecycle.cell_marked);
                continue;
            }
            let golden = &native_products[index];
            let state = local_state(&sim, progress.id);
            let context = format!("{route:?} GI[{index}]={} frame={frame}", progress.id);
            assert!(entity.lifecycle.cell_marked && entity.in_logic_vector);
            assert!(sim.substrate.occupancy.contains_entity(
                entity.position.rx,
                entity.position.ry,
                progress.id
            ));
            let transmissions = pair_radio(&log, producer, progress.id);
            if progress.placed_frame.is_none() {
                assert_eq!(frame, progress.completed_frame.unwrap() + 1);
                assert_native_fields(&state, &golden["placed"], &format!("{context} PLACE"));
                assert_eq!(
                    json!(entity.body_facing_byte(frame)),
                    golden["unlimbo_facing"]
                );
                assert_eq!(json!(transmissions), golden["delivery_radio"]);
                assert_eq!(
                    sim.substrate
                        .entities
                        .get(producer)
                        .unwrap()
                        .radio_contacts
                        .slot(0),
                    Some(progress.id)
                );
                assert_eq!(entity.radio_contacts.slot(0), Some(producer));
                if let Some((rx, ry)) = rally_cell {
                    // Exit443C60 copies the producer's Archive at44499C.
                    // Unlimbo Idle consumes it; Exit restores that Nav to
                    // Archive at444CE7 before sending the GI to its exit.
                    assert_eq!(entity.archive_target(), Some(TargetKind::Cell(rx, ry)));
                    assert_eq!(
                        entity.navigation.nav_com,
                        Some(NavTargetRef::cell(exit_cell.0, exit_cell.1))
                    );
                }
                if index == 1 {
                    // Native House4FAC21 and Strip6ABBDF release the final
                    // factory during this PLACE, before any later Strip visit.
                    assert!(
                        sim.production
                            .factory_shadow
                            .view(owner, ProductionCategory::Infantry)
                            .is_none()
                    );
                }
                progress.placed_frame = Some(frame);
            } else if progress.first_live_frame.is_none() {
                assert_eq!(frame, progress.placed_frame.unwrap() + 1);
                assert_native_fields(
                    &state,
                    &golden["first_live"],
                    &format!("{context} next live post-Foot output"),
                );
                let walk = entity.locomotor.as_ref().unwrap();
                assert_eq!(walk.active_kind(), LocomotorKind::Walk);
                progress.first_live_frame = Some(frame);
            }
            if state["location"] != golden["placed"]["location"] {
                progress.changed_position = true;
            }
            let release_start = transmissions.iter().position(|row| row[2] == 8);
            if let Some((rx, ry)) = rally_cell
                && progress.release_frame.is_none()
                && release_start.is_none()
            {
                assert_eq!(
                    entity.archive_target(),
                    Some(TargetKind::Cell(rx, ry)),
                    "{context}: rally Archive must survive until actual radio8; state={state}"
                );
                assert_eq!(
                    entity.navigation.nav_com,
                    Some(NavTargetRef::cell(exit_cell.0, exit_cell.1)),
                    "{context}: exit Nav must survive until actual radio8; state={state}"
                );
            }
            if let Some(start) = release_start {
                assert!(
                    progress.release_frame.is_none(),
                    "arrival cleanup happens once"
                );
                assert!(
                    progress.changed_position,
                    "actual paid Walk reaches PerCell"
                );
                assert!(frame > progress.first_live_frame.unwrap());
                assert_eq!(json!(&transmissions[start..]), golden["arrival_radio"]);
                assert!(entity.radio_contacts.is_empty() && entity.dock_entered_with.is_none());
                assert!(
                    sim.substrate
                        .entities
                        .get(producer)
                        .unwrap()
                        .radio_contacts
                        .is_empty()
                );
                progress.release_frame = Some(frame);
                assert_native_fields(
                    &state,
                    &golden["released"],
                    &format!("{context} radio8 release"),
                );
                if let Some((rx, ry)) = rally_cell {
                    // Correct native Walk CRT supplies height104, so paid
                    // completion can Stop before this same visit's Idle
                    // consumes Archive. The historical height0 fixture
                    // skipped that comparison and delayed it a frame.
                    assert!(entity.archive_target().is_none());
                    assert_eq!(entity.navigation.nav_com, Some(NavTargetRef::cell(rx, ry)));
                } else {
                    assert!(
                        entity.archive_target().is_none() && entity.navigation.nav_com.is_none()
                    );
                }
            }
            if let Some((rx, ry)) = rally_cell {
                let walk = entity.locomotor.as_ref().unwrap();
                if progress.release_frame.is_some()
                    && progress.archive_handoff_frame.is_none()
                    && entity.archive_target().is_none()
                {
                    assert!(frame >= progress.release_frame.unwrap());
                    assert_native_fields(
                        &state,
                        &golden["archive_handoff"],
                        &format!("{context} Archive handoff"),
                    );
                    assert_eq!(entity.navigation.nav_com, Some(NavTargetRef::cell(rx, ry)));
                    let destination = walk.walk_destination().unwrap();
                    assert_eq!(
                        json!([destination.x, destination.y, destination.z]),
                        golden["rally_walk_destination"]
                    );
                    assert_eq!(walk.walk_is_moving(), Some(true));
                    assert!(walk.step_head().is_none());
                    progress.archive_handoff_frame = Some(frame);
                }
                if progress
                    .archive_handoff_frame
                    .is_some_and(|handoff| frame > handoff)
                    && entity.navigation.nav_com == Some(NavTargetRef::cell(rx, ry))
                    && walk.step_head().is_some()
                    && state["location"] != golden["archive_handoff"]["location"]
                {
                    progress.paid_rally_walk = true;
                }
            }
            if progress.settled_frame.is_none()
                && progress.release_frame.is_some()
                && rally_cell.is_none_or(|cell| (entity.position.rx, entity.position.ry) == cell)
                && entity.navigation.nav_com.is_none()
                && entity.archive_target().is_none()
                && entity.locomotor.as_ref().unwrap().walk_is_moving() == Some(false)
                && json!(entity.mission.current().raw()) == golden["terminal"]["mission"]
            {
                if rally_cell.is_some() {
                    assert!(
                        progress.archive_handoff_frame.is_some() && progress.paid_rally_walk,
                        "{context}: terminal rally state requires actual handoff/paid Walk; state={state}"
                    );
                }
                // Both products' Guard/cleared Nav, Archive, Walk and radio
                // states are selected from corrected original execution.
                // No-rally Scatter coordinates are RNG-dependent, so only
                // rally asserts the actual player-requested destination cell.
                assert_native_fields(
                    &state,
                    &golden["terminal"],
                    &format!("{context} terminal output"),
                );
                progress.settled_frame = Some(frame);
            }
        }
        if products.len() == 2 && products.iter().all(|p| p.settled_frame.is_some()) {
            break;
        }
    }
    assert_eq!(products.len(), 2);
    assert!(
        products
            .iter()
            .all(|p| p.changed_position && p.release_frame.is_some() && p.settled_frame.is_some()),
        "both paid GI must naturally walk to automatic PerCell/radio cleanup and terminal Guard"
    );
    assert!(products[0].release_frame.unwrap() < products[1].placed_frame.unwrap());
    if rally_cell.is_some() {
        assert!(
            products.iter().all(|p| p.archive_handoff_frame.is_some()
                && p.paid_rally_walk
                && p.settled_frame.is_some()),
            "both GI must consume Archive, walk to the requested rally and settle"
        );
        assert_eq!(
            sim.substrate.entities.get(producer).unwrap().rally_cell(),
            rally_cell,
            "each product consumes its own Archive while the producer retains its rally"
        );
    }
    let factory = sim
        .production
        .factory_shadow
        .view(owner, ProductionCategory::Infantry);
    assert!(factory.is_none_or(|view| view.object.is_none() && view.queue.is_empty()));
    assert!(sim.pending_commands_for_tests().is_empty());
    assert_eq!(
        json!(sim.houses[&owner].economy.credits),
        native["final_wallet"]["credits"]
    );
    assert_eq!(
        json!(sim.houses[&owner].economy.spent_credits),
        native["final_wallet"]["spent"]
    );
}
