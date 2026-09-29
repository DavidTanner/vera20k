//! Integration tests for the engineer-bridge-repair flow + the C4-on-CABHUT
//! collapse path. Engineer entry repairs the bridge and consumes the
//! engineer; C4 on the hut leaves the hut at full HP and collapses the
//! bridge segment via the BridgeRepairHut branch in
//! `apply_c4_damage_to_building`.

use super::*;
use crate::map::bridge_facts::{
    BRIDGE_FLAG_ANCHOR_SELF, BRIDGE_FLAG_DESTROYED_OR_RAMP, BRIDGE_FLAG_DIRECTION_ZERO,
    BRIDGE_FLAG_STRUCTURAL, BridgeAnchorRelation, BridgeRampKind, BridgeRampTile,
    BridgeStampFamily, BridgeStampSlot,
};
use crate::map::entities::EntityCategory;
use crate::map::resolved_terrain::{ResolvedTerrainCell, ResolvedTerrainGrid};
use crate::rng_continuation::MapGenRngContinuation;
use crate::rules::ini_parser::IniFile;
use crate::rules::ruleset::RuleSet;
use crate::sim::bridge_state::{
    AnchorSpan, Axis, BridgeCellRole, BridgeRuntimeCell, BridgeRuntimeState, DamageState, Direction,
};
use crate::sim::command::Command;
use crate::sim::components::{Health, PendingC4Detonation};
use crate::sim::game_entity::GameEntity;
use crate::sim::timer::CdTimer;
use std::collections::BTreeMap;

/// Minimal 20x20 flat terrain so the repair path's `(bs, terrain)` gate
/// succeeds. has_damaged_data=false → the embedded flood-fill clear is a
/// no-op, leaving the repair test focused on damage-state transitions.
fn dummy_resolved_terrain() -> ResolvedTerrainGrid {
    crate::map::resolved_terrain::test_grid(20, 20, |rx, ry| ResolvedTerrainCell {
        ..crate::map::resolved_terrain::test_flat_cell(rx, ry)
    })
}

const BRIDGE_REPAIR_TEST_INI: &str = "[InfantryTypes]\n0=ENGI\n1=GHOST\n\n\
         [VehicleTypes]\n\n\
         [AircraftTypes]\n\n\
         [BuildingTypes]\n0=CABHUT\n\n\
         [ENGI]\nStrength=75\nArmor=none\nSpeed=4\nPrimary=none\nEngineer=yes\n\n\
         [GHOST]\nStrength=125\nArmor=flak\nSpeed=4\nPrimary=none\nC4=yes\n\n\
         [CABHUT]\nStrength=200\nArmor=concrete\nFoundation=1x1\nBridgeRepairHut=yes\n\n\
         [AudioVisual]\nRepairBridgeSound=BridgeRepaired\n\n\
         [CombatDamage]\nC4Warhead=SA\n\n\
         [SA]\nVerses=100%,100%,100%,100%,100%,100%,100%,100%,100%,100%,100%\n";

fn bridge_repair_test_rules() -> RuleSet {
    let ini = IniFile::from_str(BRIDGE_REPAIR_TEST_INI);
    RuleSet::from_ini(&ini).expect("bridge-repair test rules should parse")
}

fn build_sim() -> (Simulation, RuleSet, BTreeMap<(u16, u16), u8>) {
    let mut sim = Simulation::new();
    let rules = bridge_repair_test_rules();
    sim.resolve_type_handles(&rules);
    sim.resolved_terrain = Some(dummy_resolved_terrain());
    (sim, rules, BTreeMap::new())
}

fn build_ordinary_c4_sim(
    overlay: u8,
) -> (
    Simulation,
    RuleSet,
    crate::map::overlay_types::OverlayTypeRegistry,
) {
    use crate::sim::house_state::HouseState;

    let ini = format!(
        "{BRIDGE_REPAIR_TEST_INI}\n[GHOST]\n\
         Locomotor={{4A582744-9839-11d1-B709-00A024DDAFD1}}\n"
    );
    let (mut sim, rules, registry) = super::entry_test_fixture::fixture_with_rules(&ini);
    // A raw ordinary three-cell width, with resident TMP/navigation owners.
    // Overlay families do not imply structural/deck geometry.
    for y in [14, 15, 16] {
        sim.resolved_terrain
            .as_mut()
            .unwrap()
            .cell_mut(17, y)
            .unwrap()
            .bridge_facts
            .overlay_id = Some(overlay);
        sim.overlay_grid
            .as_mut()
            .unwrap()
            .place_overlay(17, y, overlay, 0);
    }
    sim.bridge_state = Some(BridgeRuntimeState::from_resolved_terrain_with_map_size(
        sim.resolved_terrain.as_ref().unwrap(),
        true,
        300,
        (16, 16),
    ));
    for (side, name) in ["Americans", "Soviets"].into_iter().enumerate() {
        let house = sim.interner.intern(name);
        sim.houses.insert(
            house,
            HouseState::new(house, side as u8, None, false, 1000, 10),
        );
        sim.session.house_order.push(house);
    }
    (sim, rules, registry)
}

fn spawn_engineer(sim: &mut Simulation, rx: u16, ry: u16) -> u64 {
    let owner = sim.interner.intern("Americans");
    let ty = sim.interner.intern("ENGI");
    let id = sim.substrate.next_stable_object_id;
    sim.substrate.next_stable_object_id += 1;
    let e = GameEntity::new_at_frame_zero_for_test(
        id,
        rx,
        ry,
        0,
        0,
        owner,
        Health { current: 75 },
        ty,
        EntityCategory::Infantry,
        0,
        5,
        false,
    );
    sim.substrate.entities.insert(e);
    assert!(matches!(sim.reveal(id), RevealOutcome::Revealed { .. }));
    id
}

fn spawn_seal(sim: &mut Simulation, rx: u16, ry: u16) -> u64 {
    let owner = sim.interner.intern("Americans");
    let ty = sim.interner.intern("GHOST");
    let id = sim.substrate.next_stable_object_id;
    sim.substrate.next_stable_object_id += 1;
    let e = GameEntity::new_at_frame_zero_for_test(
        id,
        rx,
        ry,
        0,
        0,
        owner,
        Health { current: 125 },
        ty,
        EntityCategory::Infantry,
        0,
        5,
        false,
    );
    sim.substrate.entities.insert(e);
    assert!(matches!(sim.reveal(id), RevealOutcome::Revealed { .. }));
    id
}

fn spawn_cabhut(sim: &mut Simulation, rx: u16, ry: u16) -> u64 {
    let owner = sim.interner.intern("Soviets");
    let ty = sim.interner.intern("CABHUT");
    let id = sim.substrate.next_stable_object_id;
    sim.substrate.next_stable_object_id += 1;
    let e = GameEntity::new_at_frame_zero_for_test(
        id,
        rx,
        ry,
        0,
        0,
        owner,
        Health { current: 200 },
        ty,
        EntityCategory::Structure,
        0,
        5,
        false,
    );
    sim.substrate.entities.insert(e);
    assert!(matches!(sim.reveal(id), RevealOutcome::Revealed { .. }));
    id
}

const BRIDGE_CELLS: &[(u16, u16)] = &[(10, 9), (10, 10), (10, 11), (10, 12), (10, 13)];

fn seed_destroyed_bridge(sim: &mut Simulation) {
    seed_bridge_with_state(sim, DamageState::Destroyed);
}

fn seed_bridge_with_state(sim: &mut Simulation, state: DamageState) {
    let mut bs = BridgeRuntimeState::default();
    let span = AnchorSpan {
        id: 1,
        anchor: (10, 10),
        cells: [
            Some((10, 10)),
            Some((10, 11)),
            Some((10, 12)),
            Some((10, 13)),
            Some((10, 9)),
            None,
        ],
        axis: Axis::NS,
        direction: Direction::S,
        damage_state: state,
        bridge_group_id: 1,
    };
    bs.test_seed_anchor_span(span);
    let overlay_byte = match state {
        DamageState::Destroyed => 0xE7,
        DamageState::Damaged | DamageState::PartialCollapseA | DamageState::PartialCollapseB => {
            0xD1
        }
        DamageState::Healthy { .. } => 0xCD,
    };
    for &(rx, ry) in BRIDGE_CELLS {
        let role = if (rx, ry) == (10, 10) {
            BridgeCellRole::Anchor
        } else {
            BridgeCellRole::Body
        };
        bs.test_seed_cell(
            rx,
            ry,
            BridgeRuntimeCell {
                deck_present: true,
                destroyable: true,
                deck_level: 0,
                bridge_group_id: Some(1),
                damage_state: state,
                axis: Some(Axis::NS),
                role,
                anchor_span_id: Some(1),
                overlay_byte,
                bridgehead_anchor_class: crate::sim::bridge_state::BridgeheadAnchorClass::Variant0,
            },
        );
    }
    sim.bridge_state = Some(bs);
}

fn seed_hut_fallback_bridgehead_layout(sim: &mut Simulation) {
    let mut bs = BridgeRuntimeState::default();
    let span = AnchorSpan {
        id: 1,
        anchor: (13, 10),
        cells: [Some((13, 10)), None, None, None, None, None],
        axis: Axis::EW,
        direction: Direction::E,
        damage_state: DamageState::Damaged,
        bridge_group_id: 1,
    };
    bs.test_seed_anchor_span(span);
    bs.test_seed_cell(
        12,
        10,
        BridgeRuntimeCell {
            deck_present: false,
            destroyable: true,
            deck_level: 4,
            bridge_group_id: None,
            damage_state: DamageState::Healthy { variant: 0 },
            axis: Some(Axis::EW),
            role: BridgeCellRole::Bridgehead,
            anchor_span_id: None,
            overlay_byte: 0,
            bridgehead_anchor_class: crate::sim::bridge_state::BridgeheadAnchorClass::Variant0,
        },
    );
    bs.test_seed_cell(
        13,
        10,
        BridgeRuntimeCell {
            deck_present: true,
            destroyable: true,
            deck_level: 4,
            bridge_group_id: Some(1),
            damage_state: DamageState::Damaged,
            axis: Some(Axis::EW),
            role: BridgeCellRole::Anchor,
            anchor_span_id: Some(1),
            overlay_byte: 0,
            bridgehead_anchor_class: crate::sim::bridge_state::BridgeheadAnchorClass::Variant0,
        },
    );
    sim.bridge_state = Some(bs);
    let terrain = sim
        .resolved_terrain
        .as_mut()
        .expect("bridge fallback tests require resolved terrain");
    let starter = terrain.cell_mut(12, 10).unwrap();
    starter.bridge_facts.raw_flags = BRIDGE_FLAG_STRUCTURAL;
    starter.bridge_facts.anchor = Some(BridgeAnchorRelation {
        anchor: (13, 10),
        slot: BridgeStampSlot::Forward1,
        family: BridgeStampFamily::Nesw,
        direction: 6,
    });
    let anchor = terrain.cell_mut(13, 10).unwrap();
    anchor.bridge_facts.raw_flags = 0;
    anchor.bridge_facts.ramp_tile = Some(BridgeRampTile {
        kind: BridgeRampKind::Middle1,
        relative_tile_index: 7,
        height_byte: 4,
    });
}

fn seed_hut_pure_bridgehead_fallback_layout(sim: &mut Simulation) {
    let mut bs = BridgeRuntimeState::default();
    let span = AnchorSpan {
        id: 1,
        anchor: (11, 10),
        cells: [Some((11, 10)), None, None, None, None, None],
        axis: Axis::EW,
        direction: Direction::E,
        damage_state: DamageState::Damaged,
        bridge_group_id: 1,
    };
    bs.test_seed_anchor_span(span);
    bs.test_seed_cell(
        11,
        10,
        BridgeRuntimeCell {
            deck_present: true,
            destroyable: true,
            deck_level: 4,
            bridge_group_id: Some(1),
            damage_state: DamageState::Damaged,
            axis: Some(Axis::EW),
            role: BridgeCellRole::Anchor,
            anchor_span_id: Some(1),
            overlay_byte: 0,
            bridgehead_anchor_class: crate::sim::bridge_state::BridgeheadAnchorClass::Variant0,
        },
    );
    sim.bridge_state = Some(bs);

    let terrain = sim
        .resolved_terrain
        .as_mut()
        .expect("bridge fallback tests require resolved terrain");
    terrain.cell_mut(12, 10).unwrap().bridge_facts.raw_flags = BRIDGE_FLAG_DESTROYED_OR_RAMP;
    let anchor = terrain.cell_mut(11, 10).unwrap();
    anchor.bridge_facts.raw_flags = 0;
    anchor.bridge_facts.ramp_tile = Some(BridgeRampTile {
        kind: BridgeRampKind::Middle1,
        relative_tile_index: 7,
        height_byte: 4,
    });
}

fn seed_terminal_overlay_with_fallback_trap(sim: &mut Simulation, overlay_byte: u8) {
    seed_hut_fallback_bridgehead_layout(sim);
    sim.bridge_state.as_mut().unwrap().test_seed_cell(
        10,
        10,
        BridgeRuntimeCell {
            deck_present: true,
            destroyable: true,
            deck_level: 4,
            bridge_group_id: Some(2),
            damage_state: DamageState::Destroyed,
            axis: Some(Axis::EW),
            role: BridgeCellRole::Body,
            anchor_span_id: None,
            overlay_byte,
            bridgehead_anchor_class: crate::sim::bridge_state::BridgeheadAnchorClass::Variant0,
        },
    );
}

fn seed_stock_high_cabhut_no_overlay_fallback_fixture(sim: &mut Simulation) {
    // Derived from stock high-bridge CABHUT/no-overlay placements such as
    // loose:Barrel.mmx and multimd.mix:bridgegap.map.
    seed_hut_fallback_bridgehead_layout(sim);
}

fn seed_stock_low_cabhut_no_overlay_fallback_fixture(sim: &mut Simulation) {
    // Derived from stock low-bridge CABHUT/no-overlay placements such as
    // loose:Carville.mmx, loose:Hills.mmx, and multimd.mix:xcarville.map.
    seed_hut_pure_bridgehead_fallback_layout(sim);
}

fn seed_stock_no_starter_cabhut_no_overlay_fixture(sim: &mut Simulation) {
    // Derived from stock CABHUT/no-overlay placements with no nearby 0x100/0x400
    // fallback starter, including MULTI.MIX:mp24t2.map and multimd.mix:xnorest.map.
    seed_hut_fallback_bridgehead_layout(sim);
    let terrain = sim
        .resolved_terrain
        .as_mut()
        .expect("bridge fallback tests require resolved terrain");
    let starter = terrain.cell_mut(12, 10).unwrap();
    starter.bridge_facts.raw_flags = 0;
    starter.bridge_facts.anchor = None;
}

fn step(sim: &mut Simulation, rules: &RuleSet) -> TickResult {
    let due = sim.take_due_commands();
    sim.advance_tick(&due, Some(rules), None, None, 67)
}

fn step_with_overlay_registry(
    sim: &mut Simulation,
    rules: &RuleSet,
    registry: &crate::map::overlay_types::OverlayTypeRegistry,
) -> TickResult {
    let due = sim.take_due_commands();
    sim.advance_tick(&due, Some(rules), None, Some(registry), 67)
}

fn advance_pending_c4_to_detonation(sim: &mut Simulation, rules: &RuleSet) -> bool {
    let mut bridge_state_changed_seen = false;
    for _ in 0..(rules.c4_delay_ticks as u64 + 1) {
        let result = step(sim, rules);
        bridge_state_changed_seen |= result.bridge_state_changed;
    }
    bridge_state_changed_seen
}

fn advance_until_c4_claim(
    sim: &mut Simulation,
    rules: &RuleSet,
    target_id: u64,
    registry: &crate::map::overlay_types::OverlayTypeRegistry,
) -> u64 {
    // SEAL/Tanya at Speed=4 covers ~10 lep/tick (gamemd-faithful), so a
    // one-cell enter (256 leptons) takes ~26 ticks; 32 leaves headroom.
    for _ in 0..32 {
        step_with_overlay_registry(sim, rules, registry);
        if let Some(pending) = sim
            .substrate
            .entities
            .get(target_id)
            .and_then(|b| b.pending_c4_detonation)
        {
            return pending.timer.start_frame() as u64;
        }
    }
    panic!("C4 plant was not claimed after entering the target building cell");
}

#[test]
fn capture_building_command_accepts_noncapturable_bridge_repair_hut() {
    let (mut sim, rules, _) = build_sim();
    let cabhut = spawn_cabhut(&mut sim, 9, 10);
    let engineer = spawn_engineer(&mut sim, 9, 10);

    let accepted = sim.apply_command(
        "Americans",
        &Command::CaptureBuilding {
            engineer_id: engineer,
            target_building_id: cabhut,
        },
        Some(&rules),
        None,
    );

    assert!(accepted);
    assert_eq!(
        sim.substrate
            .entities
            .get(engineer)
            .and_then(|e| e.capture_target),
        Some(cabhut)
    );
}

/// SEAL with `c4_plant` set, adjacent to a healthy CABHUT, must:
///   - not claim from the adjacent cell,
///   - claim after entering the CABHUT cell,
///   - leave the hut at full HP across the entire C4Delay window,
///   - on timer expiry, route through the BridgeRepairHut branch in
///     `apply_c4_damage_to_building` so the bridge collapses while the
///     hut survives,
///   - propagate `bridge_state_changed` to TickResult so the app rebuilds
///     PathGrid.
///
/// This exercises the ordinary concrete hut sweep
/// (`574000 -> 5749C0 -> 575BA0`) and its raw-overlay publication.
/// Resident damage and complete navigation results are covered by the
/// separate concrete bridge integration tests.
#[test]
fn c4_on_cabhut_collapses_bridge_and_hut_survives() {
    let (mut sim, rules, registry) = build_ordinary_c4_sim(0xD4);
    let cabhut = sim
        .spawn_object_at_height("CABHUT", "Soviets", 15, 15, 0, 0, &rules)
        .expect("hut must be constructed and placed beside the concrete strip");
    let cabhut_max_hp = sim.substrate.entities.get(cabhut).unwrap().health.current;
    let seal = sim
        .spawn_object_at_height("GHOST", "Americans", 16, 15, 0, 0, &rules)
        .expect("SEAL must be constructed with a Walk locomotor in the adjacent cell");
    sim.substrate.entities.get_mut(seal).unwrap().c4_plant =
        Some(crate::sim::components::C4PlantState {
            target_building_id: cabhut,
        });

    // First tick: adjacency only issues the one-cell enter move. It must not
    // claim the marker until the SEAL's current cell resolves to the CABHUT.
    step_with_overlay_registry(&mut sim, &rules, &registry);
    assert!(
        sim.substrate
            .entities
            .get(cabhut)
            .and_then(|b| b.pending_c4_detonation)
            .is_none(),
        "adjacent SEAL must not claim C4 before entering CABHUT"
    );
    let plant_start = advance_until_c4_claim(&mut sim, &rules, cabhut, &registry);

    // Throughout the C4Delay window: hut HP must stay at max — the
    // BridgeRepairHut branch never damages the hut, even before the timer
    // fires. The damaged strip stays unchanged until detonation.
    let delay = rules.c4_delay_ticks as u64;
    let mut bridge_state_changed_seen = false;
    for _ in 0..(delay + 1) {
        if (sim.session.binary_frame as u64) < plant_start + delay {
            for y in [14, 15, 16] {
                assert_eq!(
                    sim.resolved_terrain
                        .as_ref()
                        .unwrap()
                        .cell(17, y)
                        .unwrap()
                        .bridge_facts
                        .overlay_id,
                    Some(0xD4),
                    "damaged concrete strip must remain unchanged before C4Delay expires"
                );
            }
        }
        let result = step_with_overlay_registry(&mut sim, &rules, &registry);
        bridge_state_changed_seen |= result.bridge_state_changed;
        // Hut HP invariant — hold across every tick of the window.
        let cur = sim.substrate.entities.get(cabhut).unwrap().health.current;
        assert_eq!(
            cur, cabhut_max_hp,
            "hut HP must stay at max during C4Delay (plant_start={plant_start}, sim.session.tick={})",
            sim.session.tick
        );
    }

    // After detonation: hut alive, bridge segment Destroyed,
    // bridge_state_changed propagated at least once.
    let hut = sim
        .substrate
        .entities
        .get(cabhut)
        .expect("hut entity must survive the explosion");
    assert_eq!(
        hut.health.current, cabhut_max_hp,
        "hut HP unchanged: BridgeRepairHut branch must skip damage"
    );
    assert!(!hut.dying, "hut must not be marked dying");
    assert!(
        hut.pending_c4_detonation.is_none(),
        "CABHUT pending C4 marker must clear after bridge dispatch"
    );

    // MapClass's concrete hut sweep (574000 -> 5749C0 -> 575BA0)
    // changes the raw overlay authority, without structural runtime cells.
    for y in [14, 15, 16] {
        assert_eq!(
            sim.resolved_terrain
                .as_ref()
                .unwrap()
                .cell(17, y)
                .unwrap()
                .bridge_facts
                .overlay_id,
            Some(0xE7),
            "CABHUT C4 must collapse concrete cell (17, {y})"
        );
    }

    // BR-16: the collapse must feed the minimap radar-dirty channel end-to-end
    // (the same channel the engineer-repair path uses).
    assert!(
        !sim.radar_terrain_dirty_cells.is_empty(),
        "bridge collapse must dirty minimap terrain cells"
    );
    assert!(
        sim.radar_terrain_dirty_cells.contains(&(17, 15)),
        "the collapsed concrete cell (17,15) must be radar-dirty"
    );

    assert!(
        bridge_state_changed_seen,
        "TickResult.bridge_state_changed must fire at least once so the app rebuilds PathGrid"
    );
}

/// `seal`'s C4 charge on `target`, planted this frame.
fn plant_c4(sim: &mut Simulation, rules: &RuleSet, target: u64, seal: u64) {
    sim.substrate
        .entities
        .get_mut(target)
        .unwrap()
        .pending_c4_detonation = Some(PendingC4Detonation {
        timer: CdTimer::started(sim.session.binary_frame as i32, rules.c4_delay_ticks as i32),
        source_entity_id: Some(seal),
    });
}

#[test]
fn c4_on_cabhut_without_bridge_clears_pending_marker() {
    let (mut sim, rules, _) = build_sim();
    let cabhut = spawn_cabhut(&mut sim, 9, 10);
    let seal = spawn_seal(&mut sim, 10, 10);
    let cabhut_max_hp = sim.substrate.entities.get(cabhut).unwrap().health.current;
    sim.bridge_state = Some(BridgeRuntimeState::default());
    plant_c4(&mut sim, &rules, cabhut, seal);

    let mut bridge_state_changed_seen = false;
    for _ in 0..(rules.c4_delay_ticks as u64 + 1) {
        let result = step(&mut sim, &rules);
        bridge_state_changed_seen |= result.bridge_state_changed;
    }

    let hut = sim.substrate.entities.get(cabhut).unwrap();
    assert_eq!(hut.health.current, cabhut_max_hp);
    assert!(!hut.dying);
    assert!(hut.pending_c4_detonation.is_none());
    assert!(
        !bridge_state_changed_seen,
        "no bridge evidence means no bridge-state change"
    );
}

#[test]
fn c4_on_invulnerable_cabhut_still_dispatches_bridge_and_clears_pending() {
    use crate::sim::superweapon::invulnerability::{InvulnKind, InvulnerabilityState};

    let (mut sim, rules, registry) = build_ordinary_c4_sim(0xD4);
    let cabhut = sim
        .spawn_object_at_height("CABHUT", "Soviets", 15, 15, 0, 0, &rules)
        .expect("hut must be constructed and placed beside the concrete strip");
    let seal = sim
        .spawn_object_at_height("GHOST", "Americans", 16, 15, 0, 0, &rules)
        .expect("SEAL must be constructed with a Walk locomotor in the adjacent cell");
    let cabhut_max_hp = sim.substrate.entities.get(cabhut).unwrap().health.current;
    plant_c4(&mut sim, &rules, cabhut, seal);
    sim.substrate
        .entities
        .get_mut(cabhut)
        .unwrap()
        .invulnerability = Some(InvulnerabilityState {
        timer: crate::sim::timer::CdTimer::started(
            sim.session.tick as i32,
            rules.c4_delay_ticks as i32 + 20,
        ),
        kind: InvulnKind::IronCurtain,
    });

    let mut bridge_state_changed_seen = false;
    for _ in 0..(rules.c4_delay_ticks as u64 + 1) {
        let result = step_with_overlay_registry(&mut sim, &rules, &registry);
        bridge_state_changed_seen |= result.bridge_state_changed;
    }

    let hut = sim.substrate.entities.get(cabhut).unwrap();
    assert_eq!(hut.health.current, cabhut_max_hp);
    assert!(!hut.dying);
    assert!(hut.pending_c4_detonation.is_none());
    assert!(bridge_state_changed_seen);
    for y in [14, 15, 16] {
        assert_eq!(
            sim.resolved_terrain
                .as_ref()
                .unwrap()
                .cell(17, y)
                .unwrap()
                .bridge_facts
                .overlay_id,
            Some(0xE7)
        );
    }
}

#[test]
fn c4_on_cabhut_bridgehead_fallback_collapses_bridge() {
    let (mut sim, rules, _) = build_sim();
    let cabhut = spawn_cabhut(&mut sim, 9, 10);
    let seal = spawn_seal(&mut sim, 9, 10);
    let hut_hp = sim.substrate.entities.get(cabhut).unwrap().health.current;
    seed_hut_fallback_bridgehead_layout(&mut sim);
    plant_c4(&mut sim, &rules, cabhut, seal);

    let bridge_state_changed_seen = advance_pending_c4_to_detonation(&mut sim, &rules);

    let hut = sim.substrate.entities.get(cabhut).unwrap();
    assert_eq!(hut.health.current, hut_hp);
    assert!(!hut.dying);
    assert!(hut.pending_c4_detonation.is_none());
    assert!(bridge_state_changed_seen);
    let bs = sim.bridge_state.as_ref().unwrap();
    assert!(matches!(
        bs.cell(13, 10).unwrap().damage_state,
        DamageState::Destroyed
    ));
}

#[test]
fn c4_on_cabhut_pure_bridgehead_fallback_uses_opposite_anchor_offset() {
    let (mut sim, rules, _) = build_sim();
    let cabhut = spawn_cabhut(&mut sim, 9, 10);
    let seal = spawn_seal(&mut sim, 9, 10);
    seed_hut_pure_bridgehead_fallback_layout(&mut sim);
    plant_c4(&mut sim, &rules, cabhut, seal);

    let bridge_state_changed_seen = advance_pending_c4_to_detonation(&mut sim, &rules);

    assert!(
        bridge_state_changed_seen,
        "pure 0x400 starter should resolve anchor two cells opposite the east scan"
    );
    let bs = sim.bridge_state.as_ref().unwrap();
    assert!(matches!(
        bs.cell(11, 10).unwrap().damage_state,
        DamageState::Destroyed
    ));
}

#[test]
fn c4_on_cabhut_fallback_rejects_anchor_or_direction_flags_alone() {
    let (mut sim, rules, _) = build_sim();
    let cabhut = spawn_cabhut(&mut sim, 9, 10);
    let seal = spawn_seal(&mut sim, 9, 10);
    seed_hut_fallback_bridgehead_layout(&mut sim);
    let starter = sim
        .resolved_terrain
        .as_mut()
        .unwrap()
        .cell_mut(12, 10)
        .unwrap();
    starter.bridge_facts.raw_flags = BRIDGE_FLAG_ANCHOR_SELF | BRIDGE_FLAG_DIRECTION_ZERO;
    starter.bridge_facts.anchor = None;
    plant_c4(&mut sim, &rules, cabhut, seal);

    let bridge_state_changed_seen = advance_pending_c4_to_detonation(&mut sim, &rules);

    assert!(
        !bridge_state_changed_seen,
        "0x80/0x800 alone must not trigger CABHUT no-overlay fallback"
    );
    let bs = sim.bridge_state.as_ref().unwrap();
    assert!(matches!(
        bs.cell(13, 10).unwrap().damage_state,
        DamageState::Damaged
    ));
}

#[test]
fn stock_high_cabhut_no_overlay_fallback_collapses_bridge() {
    let (mut sim, rules, _) = build_sim();
    let cabhut = spawn_cabhut(&mut sim, 9, 10);
    let seal = spawn_seal(&mut sim, 9, 10);
    seed_stock_high_cabhut_no_overlay_fallback_fixture(&mut sim);
    plant_c4(&mut sim, &rules, cabhut, seal);

    let bridge_state_changed_seen = advance_pending_c4_to_detonation(&mut sim, &rules);

    assert!(bridge_state_changed_seen);
    assert!(matches!(
        sim.bridge_state
            .as_ref()
            .unwrap()
            .cell(13, 10)
            .unwrap()
            .damage_state,
        DamageState::Destroyed
    ));
}

#[test]
fn stock_low_cabhut_no_overlay_fallback_collapses_bridge() {
    let (mut sim, rules, _) = build_sim();
    let cabhut = spawn_cabhut(&mut sim, 9, 10);
    let seal = spawn_seal(&mut sim, 9, 10);
    seed_stock_low_cabhut_no_overlay_fallback_fixture(&mut sim);
    plant_c4(&mut sim, &rules, cabhut, seal);

    let bridge_state_changed_seen = advance_pending_c4_to_detonation(&mut sim, &rules);

    assert!(bridge_state_changed_seen);
    assert!(matches!(
        sim.bridge_state
            .as_ref()
            .unwrap()
            .cell(11, 10)
            .unwrap()
            .damage_state,
        DamageState::Destroyed
    ));
}

#[test]
fn stock_cabhut_no_overlay_without_starter_is_noop() {
    let (mut sim, rules, _) = build_sim();
    let cabhut = spawn_cabhut(&mut sim, 9, 10);
    let seal = spawn_seal(&mut sim, 9, 10);
    seed_stock_no_starter_cabhut_no_overlay_fixture(&mut sim);
    plant_c4(&mut sim, &rules, cabhut, seal);

    let bridge_state_changed_seen = advance_pending_c4_to_detonation(&mut sim, &rules);

    assert!(!bridge_state_changed_seen);
    assert!(matches!(
        sim.bridge_state
            .as_ref()
            .unwrap()
            .cell(13, 10)
            .unwrap()
            .damage_state,
        DamageState::Damaged
    ));
}

#[test]
fn c4_on_cabhut_low_overlay_collapses_low_bridge() {
    // Original574C20 ->574780 ->575220 reaches the same live ordinary owner
    // as the physical hut corpus. The retired fixture populated only a runtime
    // cache, leaving CellClass overlays empty and providing no Recalc inputs.
    let (mut sim, rules, registry) = build_ordinary_c4_sim(0x4A);
    let cabhut = sim
        .spawn_object_at_height("CABHUT", "Soviets", 15, 15, 0, 0, &rules)
        .expect("hut beside the wooden strip");
    let seal = sim
        .spawn_object_at_height("GHOST", "Americans", 16, 15, 0, 0, &rules)
        .expect("SEAL beside the hut");
    let hut_hp = sim.substrate.entities.get(cabhut).unwrap().health.current;
    plant_c4(&mut sim, &rules, cabhut, seal);

    let mut changed = false;
    for _ in 0..=rules.c4_delay_ticks {
        changed |= step_with_overlay_registry(&mut sim, &rules, &registry).bridge_state_changed;
    }
    let hut = sim.substrate.entities.get(cabhut).unwrap();
    assert_eq!(hut.health.current, hut_hp);
    assert!(!hut.dying);
    assert!(hut.pending_c4_detonation.is_none());
    assert!(changed);
    for y in [14, 15, 16] {
        let cell = sim.resolved_terrain.as_ref().unwrap().cell(17, y).unwrap();
        assert_eq!(cell.bridge_facts.overlay_id, Some(100));
        assert!(!cell.bridge_facts.has_structural_bridge());
        assert_eq!(
            sim.overlay_grid.as_ref().unwrap().cell(17, y).overlay_id,
            Some(100)
        );
    }
}

#[test]
fn c4_on_cabhut_low_terminal_overlay_0x65_uses_overlay_first_scan() {
    let (mut sim, rules, _) = build_sim();
    let cabhut = spawn_cabhut(&mut sim, 9, 10);
    let seal = spawn_seal(&mut sim, 10, 10);
    seed_terminal_overlay_with_fallback_trap(&mut sim, 0x65);
    plant_c4(&mut sim, &rules, cabhut, seal);

    let bridge_state_changed_seen = advance_pending_c4_to_detonation(&mut sim, &rules);

    assert!(
        !bridge_state_changed_seen,
        "terminal overlay scan hit must not fall through to fallback trap"
    );
    assert!(matches!(
        sim.bridge_state
            .as_ref()
            .unwrap()
            .cell(13, 10)
            .unwrap()
            .damage_state,
        DamageState::Damaged
    ));
}

#[test]
fn c4_on_cabhut_high_terminal_overlay_0xe8_uses_overlay_first_scan() {
    let (mut sim, rules, _) = build_sim();
    let cabhut = spawn_cabhut(&mut sim, 9, 10);
    let seal = spawn_seal(&mut sim, 10, 10);
    seed_terminal_overlay_with_fallback_trap(&mut sim, 0xE8);
    plant_c4(&mut sim, &rules, cabhut, seal);

    let bridge_state_changed_seen = advance_pending_c4_to_detonation(&mut sim, &rules);

    assert!(
        !bridge_state_changed_seen,
        "terminal overlay scan hit must not fall through to fallback trap"
    );
    assert!(matches!(
        sim.bridge_state
            .as_ref()
            .unwrap()
            .cell(13, 10)
            .unwrap()
            .damage_state,
        DamageState::Damaged
    ));
}

// ---- G4 damaged-variant lifecycle integration tests ------------------------

/// 20×20 terrain with has_damaged_data=true and a common final_tile_index on
/// every cell. Lets the damaged-variant flood-fill propagate freely across
/// any bridge cells defined in the test BridgeRuntimeState.
fn damaged_data_resolved_terrain(tile_id: i32) -> ResolvedTerrainGrid {
    crate::map::resolved_terrain::test_grid(20, 20, |rx, ry| ResolvedTerrainCell {
        source_tile_index: tile_id,
        final_tile_index: tile_id,
        bridge_facts: crate::map::bridge_facts::BridgeCellFacts {
            raw_flags: if ry == 10 && matches!(rx, 10 | 11) {
                crate::map::bridge_facts::BRIDGE_FLAG_ANCHOR_SELF
            } else {
                0
            },
            ..Default::default()
        },
        has_damaged_data: true,
        ..crate::map::resolved_terrain::test_flat_cell(rx, ry)
    })
}

/// Seed a single NS-anchor body cell at `pos` with the given state. Uses span
/// id derived from the coord so callers can place multiple independent
/// anchors without collisions.
fn seed_isolated_anchor(
    bs: &mut BridgeRuntimeState,
    pos: (u16, u16),
    span_id: u16,
    state: DamageState,
) {
    let span = AnchorSpan {
        id: span_id,
        anchor: pos,
        cells: [Some(pos), None, None, None, None, None],
        axis: Axis::NS,
        direction: Direction::S,
        damage_state: state,
        bridge_group_id: span_id,
    };
    bs.test_seed_anchor_span(span);
    bs.test_seed_cell(
        pos.0,
        pos.1,
        BridgeRuntimeCell {
            deck_present: true,
            destroyable: true,
            deck_level: 0,
            bridge_group_id: Some(span_id),
            damage_state: state,
            axis: Some(Axis::NS),
            role: BridgeCellRole::Anchor,
            anchor_span_id: Some(span_id),
            overlay_byte: 0,
            bridgehead_anchor_class: crate::sim::bridge_state::BridgeheadAnchorClass::Variant0,
        },
    );
}

#[test]
fn g4_damage_path_sets_damaged_variant_at_perpendicular_target() {
    let mut bs = BridgeRuntimeState::default();
    // Seed anchor at (10, 10) and a perpendicular target anchor at (11, 10)
    // (one east — the DamageA perpendicular direction for an NS bridge).
    seed_isolated_anchor(&mut bs, (10, 10), 1, DamageState::Healthy { variant: 0 });
    seed_isolated_anchor(&mut bs, (11, 10), 2, DamageState::Healthy { variant: 0 });
    let mut terrain = damaged_data_resolved_terrain(42);
    terrain.test_set_high_bridge_rim_tiles(crate::map::bridge_rim_tiles::HighBridgeRimTiles::from_ini(
        40, b"[General]\nBridgeBottomRight1=3\nBridgeBottomRight2=3\nBridgeTopLeft1=1\nBridgeTopLeft2=2\nBridgeMiddle1=7\nBridgeMiddle2=12\n"));

    let _ = bs.body_cell_advance_state(10, 10, true, &mut terrain);

    assert!(
        terrain.pavement_damaged_at(11, 10),
        "perpendicular target must acquire damaged_variant after DamageA write"
    );
    assert!(
        terrain.pavement_damaged_at(10, 10),
        "same-tile_id seed neighbor must acquire damaged_variant via flood-fill propagation"
    );
}

#[test]
fn g4_collapse_path_keeps_damaged_variant_set() {
    let mut bs = BridgeRuntimeState::default();
    // Pre-damaged anchor + perpendicular target, both already flagged
    // damaged_variant=true. The collapse step must NOT clear the bit.
    seed_isolated_anchor(&mut bs, (10, 10), 1, DamageState::Damaged);
    seed_isolated_anchor(&mut bs, (11, 10), 2, DamageState::Healthy { variant: 0 });
    let mut terrain = damaged_data_resolved_terrain(42);
    terrain.test_set_high_bridge_rim_tiles(crate::map::bridge_rim_tiles::HighBridgeRimTiles::from_ini(
        40, b"[General]\nBridgeBottomRight1=3\nBridgeBottomRight2=3\nBridgeTopLeft1=1\nBridgeTopLeft2=2\nBridgeMiddle1=7\nBridgeMiddle2=12\n"));

    for (rx, ry) in [(10, 10), (11, 10)] {
        terrain.cell_mut(rx, ry).unwrap().bridge_facts.raw_flags |= 0x2000;
    }
    let _ = bs.body_cell_advance_state(10, 10, true, &mut terrain);

    assert!(
        terrain.pavement_damaged_at(10, 10),
        "collapse must preserve damaged_variant on seed cell (state=true from collapse callers)"
    );
    assert!(
        terrain.pavement_damaged_at(11, 10),
        "collapse must preserve damaged_variant on perpendicular target"
    );
}

#[test]
fn ordinary_engineer_overlay_repair_preserves_pavement_damage() {
    let (mut sim, rules, _) = build_sim();
    // Admit damaged-data tiles so an accidental pavement clear would
    // affect this fixture; native ordinary overlay repair must preserve it.
    sim.resolved_terrain = Some(damaged_data_resolved_terrain(42));
    let cabhut = spawn_cabhut(&mut sim, 9, 10);
    let engineer = spawn_engineer(&mut sim, 9, 10);
    sim.substrate
        .entities
        .get_mut(engineer)
        .unwrap()
        .capture_target = Some(cabhut);
    seed_destroyed_bridge(&mut sim);
    // Pre-flag every bridge cell as damaged-variant.
    {
        for &(rx, ry) in BRIDGE_CELLS {
            sim.resolved_terrain
                .as_mut()
                .unwrap()
                .cell_mut(rx, ry)
                .unwrap()
                .bridge_facts
                .raw_flags |= 0x2000;
        }
    }

    step(&mut sim, &rules);

    let terrain = sim.resolved_terrain.as_ref().unwrap();
    for &(rx, ry) in BRIDGE_CELLS {
        assert!(
            terrain.pavement_damaged_at(rx, ry),
            "cell ({rx},{ry}) pavement damage must survive native ordinary overlay repair"
        );
    }
}

#[test]
fn ordinary_engineer_overlay_repair_does_not_clear_neighbor_pavement() {
    let (mut sim, rules, _) = build_sim();
    sim.resolved_terrain = Some(damaged_data_resolved_terrain(42));
    let cabhut = spawn_cabhut(&mut sim, 9, 10);
    let engineer = spawn_engineer(&mut sim, 9, 10);
    sim.substrate
        .entities
        .get_mut(engineer)
        .unwrap()
        .capture_target = Some(cabhut);
    seed_destroyed_bridge(&mut sim);

    // Add an off-span bridge cell at (10, 14): same tile_id as BRIDGE_CELLS,
    // adjacent to (10, 13). NOT a member of anchor_span 1, so it is NOT
    // visited by the ordinary overlay repair walk. An erroneous connected
    // pavement clear would reach it from the adjacent span cell.
    {
        let bs = sim.bridge_state.as_mut().unwrap();
        bs.test_seed_cell(
            10,
            14,
            BridgeRuntimeCell {
                deck_present: true,
                destroyable: true,
                deck_level: 0,
                bridge_group_id: Some(1),
                damage_state: DamageState::Destroyed,
                axis: Some(Axis::NS),
                role: BridgeCellRole::Body,
                anchor_span_id: None,
                overlay_byte: 0,
                bridgehead_anchor_class: crate::sim::bridge_state::BridgeheadAnchorClass::Variant0,
            },
        );
        for &(rx, ry) in BRIDGE_CELLS {
            sim.resolved_terrain
                .as_mut()
                .unwrap()
                .cell_mut(rx, ry)
                .unwrap()
                .bridge_facts
                .raw_flags |= 0x2000;
        }
    }

    sim.resolved_terrain
        .as_mut()
        .unwrap()
        .cell_mut(10, 14)
        .unwrap()
        .bridge_facts
        .raw_flags |= 0x2000;
    step(&mut sim, &rules);

    let terrain = sim.resolved_terrain.as_ref().unwrap();
    assert!(
        terrain.pavement_damaged_at(10, 14),
        "ordinary overlay repair must not flood-clear off-span pavement"
    );
}

/// Build a small NS-axis bridge with a bridgehead at (2, 4) (h=8) and an
/// anchor at (2, 2) (h=4). Used by the bridgehead-direct-damage integration
/// test. Resolved-terrain dims: 5x5.
fn build_ns_bridge_with_bridgehead_for_dispatch() -> (
    crate::map::resolved_terrain::ResolvedTerrainGrid,
    BridgeRuntimeState,
) {
    use crate::map::resolved_terrain::ResolvedTerrainCell;
    use crate::sim::bridge_state::BridgeheadAnchorClass;
    let mut cells = Vec::with_capacity(25);
    for ry in 0..5u16 {
        for rx in 0..5u16 {
            let template_height: u8 = if rx == 2 {
                match ry {
                    4 => 8,
                    3 => 6,
                    2 => 4,
                    _ => 0,
                }
            } else {
                0
            };
            cells.push(ResolvedTerrainCell {
                // level must be >= 4 so the HighStateMachine path matches.
                // Z-gate accepts impact_z within [level-1, level+1].
                level: 4,
                template_height,
                has_bridge_deck: true,
                bridge_walkable: true,
                bridge_deck_level: 4,
                ..crate::map::resolved_terrain::test_flat_cell(rx, ry)
            });
        }
    }
    let mut resolved = crate::map::resolved_terrain::ResolvedTerrainGrid::from_cells(5, 5, cells);
    // The area-damage gate requires the input's Middle tile class; a synthetic
    // Bridgehead role/overlay18 alone does not establish native admission.
    resolved.cell_mut(2, 4).unwrap().final_tile_index = 1019;
    resolved.test_set_high_bridge_rim_tiles(
        crate::map::bridge_rim_tiles::HighBridgeRimTiles::from_ini(
            1000,
            b"[General]\nBridgeMiddle1=20\nBridgeMiddle2=40\n",
        ),
    );

    // Build bridge state: bridgehead at (2, 4), anchor at (2, 2), and two
    // perpendicular Anchor neighbors at (1, 2) / (3, 2). Overlay 0x18 keeps
    // these cells out of the raw-body HighDirect range and routes the
    // dispatcher to the HighStateMachine path.
    //
    // Initial construction via `from_resolved_terrain` sets the global
    // `bridge_destroyable_flag = true` (required by the orchestrator's
    // outer gate); then `test_seed_cell` overrides per-cell state.
    let mut bs = BridgeRuntimeState::from_resolved_terrain(&resolved, true, 1500);
    bs.test_seed_cell(
        2,
        4,
        BridgeRuntimeCell {
            deck_present: true,
            destroyable: true,
            deck_level: 4,
            bridge_group_id: Some(1),
            damage_state: DamageState::Healthy { variant: 0 },
            axis: Some(Axis::NS),
            role: BridgeCellRole::Bridgehead,
            anchor_span_id: None,
            overlay_byte: 0x18,
            bridgehead_anchor_class: BridgeheadAnchorClass::Variant0,
        },
    );
    bs.test_seed_cell(
        2,
        2,
        BridgeRuntimeCell {
            deck_present: true,
            destroyable: true,
            deck_level: 4,
            bridge_group_id: Some(1),
            damage_state: DamageState::Healthy { variant: 0 },
            axis: Some(Axis::NS),
            role: BridgeCellRole::Anchor,
            anchor_span_id: Some(1),
            overlay_byte: 0x20,
            bridgehead_anchor_class: BridgeheadAnchorClass::Variant0,
        },
    );
    bs.test_seed_cell(
        3,
        2,
        BridgeRuntimeCell {
            deck_present: true,
            destroyable: true,
            deck_level: 4,
            bridge_group_id: Some(1),
            damage_state: DamageState::Healthy { variant: 0 },
            axis: Some(Axis::NS),
            role: BridgeCellRole::Anchor,
            anchor_span_id: Some(1),
            overlay_byte: 0x21,
            bridgehead_anchor_class: BridgeheadAnchorClass::Variant0,
        },
    );
    bs.test_seed_cell(
        1,
        2,
        BridgeRuntimeCell {
            deck_present: true,
            destroyable: true,
            deck_level: 4,
            bridge_group_id: Some(1),
            damage_state: DamageState::Healthy { variant: 0 },
            axis: Some(Axis::NS),
            role: BridgeCellRole::Anchor,
            anchor_span_id: Some(1),
            overlay_byte: 0x22,
            bridgehead_anchor_class: BridgeheadAnchorClass::Variant0,
        },
    );
    (resolved, bs)
}

/// Integration test: IonCannon damage at a high bridgehead retries the
/// state-machine path while the first call returns false, so the same event
/// reaches slot `+3` collapse on the second attempt.
#[test]
fn ramp_fire_collapses_high_bridgehead_on_ion_retry() {
    use crate::sim::bridge_state::{BridgeDamageEvent, BridgeheadAnchorClass};
    let mut sim = Simulation::new();
    let (resolved, bs) = build_ns_bridge_with_bridgehead_for_dispatch();
    sim.resolved_terrain = Some(resolved);
    sim.bridge_state = Some(bs);

    let rules = bridge_repair_test_rules();
    sim.resolve_type_handles(&rules);

    let pre_bridgehead = *sim.bridge_state.as_ref().unwrap().cell(2, 4).unwrap();

    for visit in 0..10 {
        let state_changed = crate::sim::world::bridge_orchestrator::apply_bridge_damage_events(
            &mut sim,
            &rules,
            &[BridgeDamageEvent {
                rx: 2,
                ry: 4,
                damage: 999,
                warhead_ref: crate::sim::intern::InternedId::default(),
                is_ion_cannon: true,
                impact_z_leptons: 416,
            }],
        );
        // Slot +3 collapse signals a path-grid refresh.
        assert!(
            state_changed,
            "high bridgehead direct damage must signal state_changed after slot +3 collapse",
        );
        let generation = sim.radar_terrain_dirty_generation;
        assert_eq!(
            generation,
            visit + 1,
            "each bridge transition after a completed radar update re-arms the same cells",
        );
        assert!(!sim.radar_terrain_dirty_cells.is_empty());
        assert!(sim.acknowledge_radar_terrain_dirty(generation));
    }
    assert!(sim.radar_terrain_dirty_cells.is_empty());

    let bs = sim.bridge_state.as_ref().unwrap();
    // Bridgehead's own damage_state untouched.
    let post_bridgehead = *bs.cell(2, 4).unwrap();
    assert_eq!(
        post_bridgehead.damage_state, pre_bridgehead.damage_state,
        "bridgehead damage_state must not change on direct fire",
    );
    // Anchor's bridgehead_anchor_class stays at the most-damaged variant.
    assert_eq!(
        bs.cell(2, 2).unwrap().bridgehead_anchor_class,
        BridgeheadAnchorClass::AboutToFall,
        "anchor tile-class remains the most-damaged bridgehead slot",
    );
    assert!(
        matches!(bs.cell(2, 2).unwrap().damage_state, DamageState::Destroyed),
        "anchor row receives bridgehead slot +3 BlowUpBridge collapse",
    );
    assert!(
        matches!(
            bs.cell(2, 4).unwrap().damage_state,
            DamageState::Healthy { .. }
        ),
        "hit bridgehead cell itself is not the collapsed row",
    );
}

// Live519C07 ->573540 ->57F440 ->5800D0 publication owns stream routing.
// Transition/callback arithmetic is checked by bridge_ordinary_repair.json;
// SimRng's mapgen_range.json regression checks the retained-state range helper.
// These integration tests enter the already-admitted production receiver with
// resident TMP/overlay inputs and all navigation owners installed.
const LIVE_REPAIR_STRIP: &[(u16, u16)] = &[(17, 14), (17, 15), (17, 16)];

fn live_repair_fixture() -> (
    Simulation,
    RuleSet,
    crate::map::overlay_types::OverlayTypeRegistry,
    u64,
) {
    let (mut sim, rules, registry) = crate::sim::world::entry_test_fixture::fixture();
    let engineer = sim
        .spawn_object("ENGINEER", "Americans", 16, 15, 0, &rules)
        .unwrap();
    let owner = sim.substrate.entities.get(engineer).unwrap().owner();
    for (index, &(x, y)) in LIVE_REPAIR_STRIP.iter().enumerate() {
        sim.resolved_terrain
            .as_mut()
            .unwrap()
            .cell_mut(x, y)
            .unwrap()
            .bridge_facts
            .overlay_id = Some(231);
        let overlays = sim.overlay_grid.as_mut().unwrap();
        overlays.place_overlay(x, y, 231, 0xA0 + index as u8);
        overlays.cell_mut(x, y).wall_owner = Some(owner);
    }
    (sim, rules, registry, engineer)
}

fn assert_live_repair_strip(sim: &Simulation, engineer: u64, expected_overlay: u8) {
    let owner = sim.substrate.entities.get(engineer).unwrap().owner();
    for (index, &(x, y)) in LIVE_REPAIR_STRIP.iter().enumerate() {
        let terrain = sim.resolved_terrain.as_ref().unwrap().cell(x, y).unwrap();
        let overlay = sim.overlay_grid.as_ref().unwrap().cell(x, y);
        assert_eq!(terrain.bridge_facts.overlay_id, Some(expected_overlay));
        assert_eq!(overlay.overlay_id, Some(expected_overlay));
        assert_eq!(overlay.overlay_data, 0xA0 + index as u8);
        assert_eq!(overlay.wall_owner, Some(owner));
    }
}

#[test]
fn ordinary_engineer_repair_consumes_mapgen_only_and_preserves_overlay_metadata() {
    let (mut sim, rules, registry, engineer) = live_repair_fixture();
    let scenario_before = sim.scenario_rng.logical_state();
    let main_before = sim.main_rng.logical_state();
    let mut expected_mapgen = crate::sim::rng::SimRng::new(0);
    assert_eq!(expected_mapgen.next_high_two_bits(), 1);

    assert!(
        crate::sim::world::bridge_orchestrator::repair_from_engineer(
            &mut sim,
            &rules,
            Some(&registry),
            engineer,
        )
        .expect("live repair must complete its Recalc and navigation callbacks")
    );

    assert_eq!(sim.scenario_rng.logical_state(), scenario_before);
    assert_eq!(sim.main_rng.logical_state(), main_before);
    assert_eq!(
        sim.mapgen_rng.logical_state(),
        expected_mapgen.logical_state()
    );
    assert_live_repair_strip(&sim, engineer, 0xCE);
}

/// An installed post-generation cursor feeds the live owner's next repair draw.
#[test]
fn generated_map_bridge_repair_continues_post_rmg_mapgen_stream() {
    let mut generated = crate::sim::rng::SimRng::new(0xBEEF);
    for _ in 0..353 {
        let _ = generated.next_u32();
    }
    let generated = generated.logical_state();
    let continuation = || {
        MapGenRngContinuation::from_native_parts(
            generated.words,
            usize::try_from(generated.index_a).expect("test MapGen cursor A is non-negative"),
            usize::try_from(generated.index_b).expect("test MapGen cursor B is non-negative"),
        )
    };
    let mut expected = crate::sim::rng::SimRng::from_mapgen_continuation(continuation());
    let expected_variant = expected.next_high_two_bits();
    let (mut sim, rules, registry, engineer) = live_repair_fixture();
    sim.mapgen_rng = crate::sim::rng::SimRng::from_mapgen_continuation(continuation());
    let scenario_before = sim.scenario_rng.logical_state();
    let main_before = sim.main_rng.logical_state();

    assert!(
        crate::sim::world::bridge_orchestrator::repair_from_engineer(
            &mut sim,
            &rules,
            Some(&registry),
            engineer,
        )
        .expect("live repair must retain the installed MapGen continuation")
    );

    assert_eq!(sim.scenario_rng.logical_state(), scenario_before);
    assert_eq!(sim.main_rng.logical_state(), main_before);
    assert_eq!(sim.mapgen_rng.logical_state(), expected.logical_state());
    assert_live_repair_strip(&sim, engineer, 0xCD + expected_variant);
}

#[test]
fn two_identical_sims_repair_with_identical_hash_and_streams() {
    let (mut sim_a, rules_a, registry_a, engineer_a) = live_repair_fixture();
    let (mut sim_b, rules_b, registry_b, engineer_b) = live_repair_fixture();
    for (sim, rules, registry, engineer) in [
        (&mut sim_a, &rules_a, &registry_a, engineer_a),
        (&mut sim_b, &rules_b, &registry_b, engineer_b),
    ] {
        assert!(
            crate::sim::world::bridge_orchestrator::repair_from_engineer(
                sim,
                rules,
                Some(registry),
                engineer,
            )
            .expect("live repair must complete")
        );
        assert_live_repair_strip(sim, engineer, 0xCE);
    }
    assert_eq!(sim_a.state_hash(), sim_b.state_hash());
    assert_eq!(
        sim_a.scenario_rng.logical_state(),
        sim_b.scenario_rng.logical_state()
    );
    assert_eq!(
        sim_a.main_rng.logical_state(),
        sim_b.main_rng.logical_state()
    );
    assert_eq!(
        sim_a.mapgen_rng.logical_state(),
        sim_b.mapgen_rng.logical_state()
    );
}
