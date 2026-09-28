//! Slice 8 — global lockstep parity harness.
//!
//! Records a deterministic multi-faction skirmish as a `ReplayLog` and re-runs it
//! through the same registry-aware `ReplayRunner` master-frame path the live
//! game uses, asserting (1)
//! every tick's replayed hash equals the recorded hash (intra-run determinism)
//! and (2) the final hash equals a committed baseline. This is the project-wide
//! desync tripwire for the whole mission/radio substrate migration.
//!
//! Coverage: two hostile houses; an Allied war factory + refinery + harvester
//! over a seeded ore patch (the harvester gets a `Miner` component at spawn,
//! reaches the patch and cuts — that state folds into the hash);
//! tanks + infantry under scripted Move/AttackMove/Stop, with the two sides
//! closing to combat range (exercises mission retask, movement, targeting/
//! retaliation, and the RNG streams). The harvester carries the real
//! `Harvester`/`Dock`/`Storage` flags and the refinery `Refinery=yes`.
//!
//! Scope note: this is a determinism + baseline guard, not a miner-dock test.
//! Driving a harvester physically to ore and through the full refinery dock
//! handshake needs movement world-setup (terrain costs / resolved terrain) that
//! the dedicated miner-dock suite (`miner_tests.rs`) provides and owns; this
//! harness only guards that the miner system stays wired and deterministic.

use super::*;
use crate::map::entities::{EntityCategory, MapEntity};
use crate::map::overlay_types::OverlayTypeRegistry;
use crate::rules::ini_parser::IniFile;
use crate::rules::ruleset::RuleSet;
use crate::sim::command::{Command, CommandEnvelope};
use crate::sim::mission::{MissionId, MissionType};
use crate::sim::overlay_grid::OverlayGrid;
use crate::sim::pathfinding::PathGrid;
use crate::sim::replay::{ReplayHeader, ReplayLog, ReplayRunner};
use std::collections::BTreeMap;
use std::io::Write;

/// Optional observations of the two bounded replay fixtures, never golden input.
/// The file records existing owners without changing or draining their state.
pub(super) fn replay_diagnostic_file(name: &str) -> Option<std::io::BufWriter<std::fs::File>> {
    let root = std::path::PathBuf::from(std::env::var_os("VERA20K_REPLAY_DIAGNOSTICS")?);
    std::fs::create_dir_all(&root).unwrap();
    Some(std::io::BufWriter::new(
        std::fs::File::create(root.join(format!("{name}.jsonl"))).unwrap(),
    ))
}

pub(super) fn record_replay_diagnostic(
    writer: &mut Option<std::io::BufWriter<std::fs::File>>,
    sim: &Simulation,
    result: Option<&TickResult>,
    commands: &[CommandEnvelope],
    draws: &[serde_json::Value],
) {
    let Some(writer) = writer else { return };
    let actors: Vec<_> = sim
        .substrate
        .entities
        .keys_sorted()
        .into_iter()
        .map(|id| sim.substrate.entities.get(id).unwrap())
        .collect();
    let fires: Vec<_> = sim
        .fire_events
        .iter()
        .map(|fire| {
            serde_json::json!({
                "attacker":fire.attacker_id, "target":fire.target,
                "weapon":sim.resolve(fire.weapon_id),
                "position":[fire.fire_coord.x,fire.fire_coord.y,fire.fire_coord.z],
            })
        })
        .collect();
    serde_json::to_writer(
        &mut *writer,
        &serde_json::json!({
            "next_frame":sim.session.binary_frame,
            "tick_result":result.map(|r| serde_json::json!({"tick":r.tick,
                "committed":r.frame_committed,"executed_commands":r.executed_commands,
                "state_hash":r.state_hash,"terminal_score_finalized":r.terminal_score_finalized})),
            "commands":commands,"entities":actors,"logic_order":sim.logic_order(),
            "fire_events_accumulated":fires,
            "lifecycle_outputs_accumulated":format!("{:?}",sim.lifecycle_outputs),
            "rng":{"scenario":sim.scenario_rng.native_state_hex(),
                "main":sim.main_rng.native_state_hex(),"mapgen":sim.mapgen_rng.native_state_hex()},
            "draws":draws,
        }),
    )
    .unwrap();
    writeln!(writer).unwrap();
    writer.flush().unwrap();
}

pub(super) fn print_replay_summary(name: &str, sim: &Simulation) {
    if std::env::var_os("VERA20K_REPLAY_DIAGNOSTICS").is_none() {
        return;
    }
    let actors: Vec<_> = sim
        .substrate
        .entities
        .keys_sorted()
        .into_iter()
        .map(|id| {
            let e = sim.substrate.entities.get(id).unwrap();
            serde_json::json!({"id":id,"type":sim.resolve(e.type_ref()),
            "health":e.health.current,"position":e.position,"mission":e.mission,
            "target":e.attack_target,"nav":e.navigation.nav_com,
            "passive_timer":e.passive_scan_timer,"last_scan":e.last_target_scan_frame,
            "rearm":e.rearm_timer})
        })
        .collect();
    println!(
        "[replay observation {name}] {}",
        serde_json::json!({
            "next_frame":sim.session.binary_frame,"hash":sim.state_hash(),
            "streams":[sim.scenario_rng.state(),sim.main_rng.state(),sim.mapgen_rng.state()],
            "actors":actors,"fire_count":sim.fire_events.len(),
        })
    );
}

const HARNESS_SEED: u64 = 0xC0FFEE_1234;
const HARNESS_TICKS: u64 = 600;
const HARNESS_TICK_MS: u32 = 67;
/// AT-8: ticks at which the per-stream RNG cursors are compared record-vs-replay
/// (after the tick at this index executes).
const STREAM_CHECKPOINT_TICKS: &[u64] = &[149, 299, 449, 599];

/// Rust regression receipts from `bridge-fv-host-v15` (600 committed frames),
/// not native full-skirmish goldens. Record/replay checks every frame and all
/// three streams at the checkpoints above; these absolute pins also catch a
/// deterministic routing/cadence change in both passes.
///
/// The FV chain corrects IDLE/Stop (`0x004C74CB..0x004C76BB`: no ordinary
/// mission write) and the passive gate (`0x006FA697`: literal committed mission).
/// Stop now retains Attack and its dispatch timer; a cleared TarCom cannot scan
/// before the real Guard transition. The original executed Stop and collapse
/// cases are in `tools/spatial_oracle/fv_cell_attack/paid_conditional*.json`;
/// source/coverage is in that package's README. The live Unit FireAt host also
/// commits before later objects' AI (`0x007365E1`, `0x0055B613`).
///
/// The observed global fixture consumes 311 Scenario words after placement;
/// the historical Scenario fingerprint corresponds to 319 from the same input.
/// Main and MapGen are unchanged, and the 13-shot duel still leaves tank 6 at
/// 12 HP. The old trace does not retain individual words, so this is a bounded
/// causal account, not an attribution of every historical draw. Old hash-schema
/// projections cannot reconstruct the former mission/timer behavior; they are
/// retained in Git history rather than simulated by rewriting current state.
const FINAL_STREAM_STATES: (u64, u64, u64) = (
    0x5591_C022_8E71_3E8F,
    0x39F3_258B_A550_EB7C,
    0x1CE8_1848_7043_6163,
);
const GLOBAL_HARNESS_FINAL_HASH: u64 = 0xD29D_179C_FCC2_6586;

fn harness_ini() -> IniFile {
    // Multi-faction vehicles + infantry + buildings (war factory, refinery) plus a
    // real harvester (Harvester/Dock/Storage) and a real refinery (Refinery=yes)
    // so the miner dock path is reachable. Short weapon ranges keep combat to the
    // scripted engagements, keeping the scenario deterministic.
    //
    // KNOWN COVERAGE GAP — the fire-range gate is invisible to this tripwire.
    // `[M60] Range=5` and `[105mm] Range=6` are whole cells, neither weapon sets
    // `MinimumRange=` or `CellRangefinding=`, no projectile section exists (so
    // nothing is `Arcing=`), and no attacker is airborne. The 2026-09 change that
    // moved `TechnoClass::InRange` 0x006F7220 onto lepton ranges and onto the
    // source coordinate `TechnoClass::CanFireAt` 0x006F77B0 builds shifted the
    // reach of 60 of the 256 stock `Range=` weapons — Rhino and Apocalypse at
    // `Range=5.75` among them — and moved NO hash here, because this fixture
    // cannot express any of those inputs. Read an unchanged harness hash as
    // "this scenario is unaffected", never as "range behaviour did not move".
    // Anyone extending this fixture: a fractional `Range=` on `[105mm]` would
    // close the gap, at the cost of one re-baseline of GLOBAL_HARNESS_FINAL_HASH
    // plus the stream pins.
    //
    // The same gap covers the line-of-fire walk at `InRange`'s tail
    // (0x006F7642 -> 0x004CC310). It fires only for a projectile that sets
    // `SubjectToWalls=` or `SubjectToCliffs=`, and neither `[M60]` nor
    // `[105mm]` declares a `Projectile=` at all, so no projectile section
    // exists to carry either key; the only overlay here is `TIB01` ore, so
    // there is no wall on any line; and the terrain the scenario builds is
    // flat, so no four-Level cliff step exists either. Stock `[Cannon]` and
    // `[InvisibleLow]` — every cannon tank and every small-arms infantryman —
    // DO set both keys. Closing this half needs a projectile section plus a
    // wall overlay or a level step in the fixture terrain, and the same
    // re-baseline.
    IniFile::from_str(
        "[InfantryTypes]\n0=E1\n\n\
         [VehicleTypes]\n0=MTNK\n1=HARV\n\n\
         [AircraftTypes]\n\n\
         [BuildingTypes]\n0=GAWEAP\n1=GAREFN\n\n\
         [OverlayTypes]\n0=TIB01\n\n\
         [Tiberiums]\n0=Riparius\n\n\
         [Riparius]\nImage=1\nValue=25\n\n\
         [TIB01]\nTiberium=yes\n\n\
         [Tiberium]\nFoot=100%\nTrack=100%\nWheel=100%\n\n\
         [E1]\nLocomotor={4A582744-9839-11d1-B709-00A024DDAFD1}\nStrength=125\nArmor=flak\nSpeed=4\nPrimary=M60\n\n\
         [MTNK]\nLocomotor={4A582741-9839-11d1-B709-00A024DDAFD1}\nStrength=300\nArmor=heavy\nSpeed=6\nPrimary=105mm\n\n\
         [HARV]\nLocomotor={4A582741-9839-11d1-B709-00A024DDAFD1}\nStrength=600\nArmor=heavy\nSpeed=5\nHarvester=yes\nStorage=28\nDock=GAREFN\n\n\
         [GAWEAP]\nStrength=1000\nArmor=wood\nFoundation=4x3\n\n\
         [GAREFN]\nStrength=1000\nArmor=wood\nRefinery=yes\nDockUnload=yes\nFoundation=3x3\n\n\
         [M60]\nDamage=25\nROF=20\nRange=5\nWarhead=SA\n\n\
         [105mm]\nDamage=65\nROF=50\nRange=6\nWarhead=AP\n\n\
         [SA]\nVerses=100%,100%,100%,90%,70%,25%,100%,25%,25%,0%,0%\n\n\
         [AP]\nVerses=100%,100%,90%,75%,75%,75%,60%,30%,20%,0%,0%\n",
    )
}

fn harness_rules() -> RuleSet {
    let ini = harness_ini();
    RuleSet::from_ini(&ini).expect("harness rules should parse")
}

fn harness_overlays() -> OverlayTypeRegistry {
    OverlayTypeRegistry::from_ini(&harness_ini(), None)
}

fn unit(owner: &str, type_id: &str, cx: u16, cy: u16, cat: EntityCategory) -> MapEntity {
    MapEntity {
        owner: owner.to_string(),
        type_id: type_id.to_string(),
        health: 256,
        cell_x: cx,
        cell_y: cy,
        facing: 64,
        category: cat,
        sub_cell: 0,
        veterancy: 0,
        high: false,
        mission: None,
        recruitable_a: true,
        recruitable_b: true,
        structure_upgrades: [None, None, None],
        structure_ai_sellable: false,
        structure_ai_repairable: false,
    }
}

/// Build the recorded scenario into `sim`. Spawn order fixes stable ids
/// 1..=7 (war factory, refinery, harvester, Allied tank, Allied infantry,
/// Soviet tank, Soviet infantry).
fn seed_scenario(
    sim: &mut Simulation,
    rules: &RuleSet,
    heights: &BTreeMap<(u16, u16), u8>,
    overlays: &OverlayTypeRegistry,
) {
    // Flat map cells and the playfield, as a map load installs them before
    // placing objects: Unlimbo's Unit `Can_Enter_Cell` reads both, and the
    // harvester's ore scan (`FootClass::Is_Cell_Harvestable @ 0x004DCE80`)
    // admits only playfield cells of LandType 5.
    sim.resolved_terrain = Some(crate::map::resolved_terrain::test_flat_ground_grid(64));
    sim.playfield_bounds = Some(crate::map::playfield::PlayfieldBounds {
        base: 0,
        off_fc: -64,
        off_100: -1,
        off_104: 128,
        off_108: 65,
    });
    sim.spawn_from_map(
        &[
            unit("Americans", "GAWEAP", 3, 3, EntityCategory::Structure), // 1
            unit("Americans", "GAREFN", 3, 10, EntityCategory::Structure), // 2
            unit("Americans", "HARV", 8, 12, EntityCategory::Unit),       // 3
            unit("Americans", "MTNK", 10, 8, EntityCategory::Unit),       // 4
            unit("Americans", "E1", 11, 9, EntityCategory::Infantry),     // 5
            unit("Soviet", "MTNK", 40, 8, EntityCategory::Unit),          // 6
            unit("Soviet", "E1", 41, 9, EntityCategory::Infantry),        // 7
        ],
        Some(rules),
        heights,
    );
    // Seed the native CellClass overlay authority near the harvester.
    let tib01 = overlays.id_for_name("TIB01").expect("harness TIB01");
    let mut overlay_grid = OverlayGrid::new(64, 64);
    let terrain = sim.resolved_terrain.as_mut().expect("installed above");
    for (rx, ry) in [(12, 13), (13, 13), (12, 14), (13, 14)] {
        overlay_grid.place_overlay(rx, ry, tib01, 11);
        // RecalcAttributes: LandType 5 and its [Tiberium] speed row.
        overlay_grid.recalculate_runtime_cell(
            terrain,
            overlays,
            (rx, ry),
            crate::sim::overlay_grid::NavigationPublication::FrameBoundary,
        );
    }
    overlay_grid.take_dirty_cells();
    sim.overlay_grid = Some(overlay_grid);
}

/// Scripted commands keyed by `execute_tick` (fires when tick+1 == execute_tick).
fn harness_script() -> Vec<(u64, Command)> {
    vec![
        (
            2,
            Command::Move {
                entity_id: 4,
                target_rx: 24,
                target_ry: 8,
                queue: false,
            },
        ),
        (
            40,
            Command::AttackMove {
                entity_id: 4,
                target_rx: 38,
                target_ry: 8,
                queue: false,
            },
        ),
        (
            120,
            Command::Move {
                entity_id: 6,
                target_rx: 28,
                target_ry: 10,
                queue: false,
            },
        ),
        (300, Command::Stop { entity_id: 4 }),
        (
            320,
            Command::Move {
                entity_id: 4,
                target_rx: 8,
                target_ry: 8,
                queue: false,
            },
        ),
    ]
}

/// Owner of every scripted command (all are issued by the Allied player).
fn due_commands(sim: &Simulation, script: &[(u64, Command)], tick: u64) -> Vec<CommandEnvelope> {
    let owner = sim.interner.get("Americans").expect("Americans interned");
    script
        .iter()
        .filter(|(t, _)| *t == tick + 1)
        .map(|(t, c)| CommandEnvelope::new(owner, *t, c.clone()))
        .collect()
}

#[test]
fn global_skirmish_replay_is_deterministic_and_baseline_stable() {
    let rules = harness_rules();
    let overlays = harness_overlays();
    let heights: BTreeMap<(u16, u16), u8> = BTreeMap::new();
    let grid = PathGrid::new(64, 64);
    let script = harness_script();

    // ---- Record pass: build a ReplayLog through the live advance_tick path. ----
    let mut rec = Simulation::with_seed(HARNESS_SEED);
    seed_scenario(&mut rec, &rules, &heights, &overlays);
    let mut diagnostic = replay_diagnostic_file("global");
    record_replay_diagnostic(&mut diagnostic, &rec, None, &[], &[]);
    let mut log = ReplayLog::new(ReplayHeader {
        version: 1,
        pixel_conversion_bounds: rec.session.pixel_conversion_bounds,
        tick_hz: 15,
        seed: HARNESS_SEED,
        map_name: "global_parity_harness".to_string(),
        rules_hash: 0,
    });
    // Coverage tripwire: the harvester (id 3) must be picked up by the miner
    // system — it acquires an ore target via the SearchOre path, drives to its
    // field and cuts. (The full dock handshake is the dedicated miner-dock
    // suites' coverage.) This guards that miner-component creation + the
    // acquisition path stay wired and contribute to the hash.
    let mut miner_engaged = false;
    // AT-8 stream pins: per-stream cursor fingerprints captured at checkpoint
    // ticks during record, re-asserted in replay. Total-hash equality can mask
    // a draw routed to the wrong stream when a compensating error exists;
    // per-stream checkpoints catch misrouting directly.
    let mut recorded_streams: Vec<(u64, u64, u64, u64)> = Vec::new();
    let mut first_uncommitted_frame = None;
    for tick in 0..HARNESS_TICKS {
        let due = due_commands(&rec, &script, tick);
        let mut advance = || {
            rec.advance_tick(
                &due,
                Some(&rules),
                &heights,
                Some(&grid),
                Some(&overlays),
                HARNESS_TICK_MS,
            )
        };
        let (result, draws) = if diagnostic.is_some() {
            crate::sim::rng::trace_draws(advance)
        } else {
            (advance(), Vec::new())
        };
        record_replay_diagnostic(&mut diagnostic, &rec, Some(&result), &due, &draws);

        // Event IDLE clears TarCom after Logic frame 299; it does not queue
        // Stop or reset MissionCom. The overdue passive timer still cannot
        // scan while the committed selector is Attack (native 0x006FA697).
        if (299..319).contains(&tick) {
            let tank = rec.substrate.entities.get(4).expect("stopped tank lives");
            assert_eq!(
                tank.mission.current(),
                MissionId::from_known(MissionType::Attack)
            );
            assert_eq!(tank.mission.queued(), MissionId::NONE);
            assert_eq!(tank.mission.mission_start_frame(), 40);
            assert_eq!(tank.mission.ai_counter(), tick as u32 - 39);
            assert_eq!(
                (
                    tank.mission.dispatch_timer().start_frame(),
                    tank.mission.dispatch_timer().delay()
                ),
                (40, 450),
                "Stop retains the active dispatch timer"
            );
            assert!(tank.attack_target.is_none());
            assert_eq!(tank.last_target_scan_frame, 0);
            assert_eq!(
                (
                    tank.passive_scan_timer.start_frame,
                    tank.passive_scan_timer.duration
                ),
                (0, 45)
            );
        }
        // The surviving tank's target dies after its shot at frame 590. Its
        // next Attack dispatch queues Guard at 592, but the passive slot still
        // reads Attack. Unit's later Commence promotes Guard; scanning is 593.
        if (590..=593).contains(&tick) {
            let tank = rec.substrate.entities.get(6).expect("surviving tank");
            assert!(tank.attack_target.is_none());
            let expected_mission = if tick < 592 {
                MissionType::Attack
            } else {
                MissionType::Guard
            };
            assert_eq!(
                tank.mission.current(),
                MissionId::from_known(expected_mission)
            );
            let expected_scan = if tick < 593 { (266, 28) } else { (593, 27) };
            assert_eq!(tank.last_target_scan_frame, expected_scan.0);
            assert_eq!(
                (
                    tank.passive_scan_timer.start_frame,
                    tank.passive_scan_timer.duration
                ),
                expected_scan
            );
        }

        if !result.frame_committed {
            first_uncommitted_frame.get_or_insert(tick);
        }

        if rec.substrate.entities.get(3).is_some_and(|h| {
            h.miner.as_ref().is_some_and(|m| m.harvesting) || h.navigation.nav_com.is_some()
        }) {
            miner_engaged = true;
        }
        log.record_tick(tick, due, result.state_hash);
        if STREAM_CHECKPOINT_TICKS.contains(&tick) {
            recorded_streams.push((
                tick,
                rec.scenario_rng.state(),
                rec.main_rng.state(),
                rec.mapgen_rng.state(),
            ));
        }
    }
    // The movement pass keeps its owner block sets current from the entity
    // store's touch log. Anything that hands out every entity mutably each
    // frame (`values_mut`) would quietly turn that back into a whole-world
    // read per frame, with correct results and no other symptom.
    assert_eq!(
        first_uncommitted_frame, None,
        "all scripted frames must commit"
    );
    let world_reads = rec.movement_pass_cache.block_index_world_rebuilds();
    // The blocker plane follows the same log. It is rebuilt from the whole map
    // only when the terrain epoch or the wall plane moves; this fixture's grid is
    // a legacy one without a retained wall plane, so every overlay write
    // counts, which this script does a couple of dozen times; a rebuild per
    // moving object's turn would be thousands.
    let plane_reads = rec.movement_pass_cache.blocker_plane_world_rebuilds();
    assert!(
        plane_reads <= 60,
        "the blocker plane was rebuilt from the whole map {plane_reads} times"
    );
    assert!(
        world_reads <= 2,
        "the block index read every entity {world_reads} times; look for a new all-entity mutable walk"
    );
    assert!(
        miner_engaged,
        "the miner system must engage the harvester (head for or cut ore) — \
         else miner-component creation or Mission_Harvest state 0 regressed"
    );

    // ---- Replay pass: fresh sim, real ReplayRunner, assert tick-by-tick.
    // The registry-aware entry uses the SAME master-frame path as the legacy
    // convenience entry, chunked at the stream checkpoints so the per-stream
    // cursors can be pinned between chunks. ----
    let mut rep = Simulation::with_seed(HARNESS_SEED);
    seed_scenario(&mut rep, &rules, &heights, &overlays);
    let mut replayed: Vec<u64> = Vec::with_capacity(log.ticks.len());
    let mut replayed_streams: Vec<(u64, u64, u64, u64)> = Vec::new();
    let mut chunk_start = 0usize;
    for &checkpoint in STREAM_CHECKPOINT_TICKS {
        let chunk_end = (checkpoint as usize + 1).min(log.ticks.len());
        let chunk = ReplayLog {
            header: log.header.clone(),
            ticks: log.ticks[chunk_start..chunk_end].to_vec(),
        };
        replayed.extend(ReplayRunner::run_fixture_with_overlay_registry(
            &mut rep,
            &chunk,
            Some(&rules),
            &heights,
            Some(&grid),
            Some(&overlays),
            HARNESS_TICK_MS,
        ));
        replayed_streams.push((
            checkpoint,
            rep.scenario_rng.state(),
            rep.main_rng.state(),
            rep.mapgen_rng.state(),
        ));
        chunk_start = chunk_end;
    }
    if chunk_start < log.ticks.len() {
        let tail = ReplayLog {
            header: log.header.clone(),
            ticks: log.ticks[chunk_start..].to_vec(),
        };
        replayed.extend(ReplayRunner::run_fixture_with_overlay_registry(
            &mut rep,
            &tail,
            Some(&rules),
            &heights,
            Some(&grid),
            Some(&overlays),
            HARNESS_TICK_MS,
        ));
    }
    assert_eq!(
        recorded_streams, replayed_streams,
        "per-stream cursor consistency: a nondeterminism moved streams between record and replay"
    );
    assert_eq!(
        replayed.len(),
        log.ticks.len(),
        "replay tick count must match record"
    );
    for (i, h) in replayed.iter().enumerate() {
        assert_eq!(
            *h, log.ticks[i].state_hash,
            "intra-run determinism: replay tick {i} hash must equal the recorded hash"
        );
    }

    let (_, final_scen, final_main, final_mapgen) =
        *recorded_streams.last().expect("final checkpoint recorded");
    let final_hash = *replayed.last().expect("at least one tick recorded");
    print_replay_summary("global", &rep);
    assert_eq!(
        (final_scen, final_main, final_mapgen),
        FINAL_STREAM_STATES,
        "absolute per-stream regression: establish the changed producer/cadence before updating"
    );
    assert!(
        rep.substrate
            .entities
            .get(2)
            .expect("harness refinery")
            .radio_contacts
            .is_empty(),
        "the fixture ends without a held refinery contact"
    );
    assert_eq!(
        rep.substrate.anims.len(),
        0,
        "the fixture emits no Anim objects"
    );
    assert!(
        rep.substrate
            .entities
            .values()
            .all(|e| e.gap_generator == Default::default())
    );
    assert!(
        rep.power_states
            .values()
            .all(|s| !s.has_drained_power_source)
    );
    assert!(
        rep.substrate
            .entities
            .values()
            .all(|e| e.building_storage == Default::default()
                && e.aircraft_ammo.is_none()
                && e.aircraft_mission.is_none()),
        "the fixture contains no storage or aircraft state"
    );
    assert!(rep.production.airfield_docks.is_empty());

    // Tank 4 first fires at frame 281; tank 6 retaliates at 283. Stop clears
    // tank 4's target at 299 and Move is issued at 319. A later hit overrides
    // that Move with Attack. Six returned hits leave tank 6 at 12 HP; its
    // seventh hit kills tank 4 at frame 590. These are Rust regression values,
    // not a native execution of this synthetic whole-skirmish fixture.
    assert_eq!(rep.fire_events.len(), 13);
    assert_eq!(
        rep.substrate
            .entities
            .get(4)
            .map(|tank| tank.health.current),
        None,
        "the retasked tank dies in its duel"
    );
    assert_eq!(
        rep.substrate
            .entities
            .get(6)
            .map(|tank| tank.health.current),
        Some(12),
        "tank 6 takes six 105mm hits (65 * 75% heavy)"
    );
    assert_eq!(
        final_hash, GLOBAL_HARNESS_FINAL_HASH,
        "committed global-harness baseline drifted: establish the changed behavior, RNG producer, or hash composition before updating"
    );
}

const DENSE_SEED: u64 = 0x00BA771E_5EED;
const DENSE_TICKS: u64 = 300;
const DENSE_ROWS: u16 = 10;

/// S2 churn — DENSE arrival case: two facing tank columns (10 Allied vs 10 Soviet) both
/// ordered to converge on the same centre column, so a whole column reaches its
/// destination on the same tick and flips Move→Sleep together. Each Move is issued under
/// ITS OWN owner — the thin generic harness silently rejected one side's move as
/// non-owned, leaving only one real mover. This measures the *simultaneous* per-tick
/// churn the S2 authority flip must survive (a single-mover scenario understates it).
///
/// Scope note: this fixture was built to exercise movement/arrival churn only, and
/// for most of its life the tanks converged without engaging. That is no longer
/// true. Each tank is ordered under its own owner, arrives, and is then
/// ordered-then-idle. The fixture now includes target acquisition and combat;
/// its original finished-mission workaround has since been removed. The
/// position fingerprint below therefore covers engagement churn as well as
/// arrival churn.
/// Shared construction for the dense converging-battle fixture (20 tanks, two
/// facing columns converging on x=25; per-owner Move script due on tick 2).
/// Used by the churn measurement and the S2 position fingerprint below.
#[allow(clippy::type_complexity)]
fn dense_converging_setup() -> (
    Simulation,
    RuleSet,
    BTreeMap<(u16, u16), u8>,
    PathGrid,
    Vec<(u64, crate::sim::intern::InternedId, Command)>,
) {
    let rules = harness_rules();
    let heights: BTreeMap<(u16, u16), u8> = BTreeMap::new();
    let grid = PathGrid::new(64, 64);

    let mut sim = Simulation::with_seed(DENSE_SEED);
    let mut roster: Vec<MapEntity> = Vec::new();
    for i in 0..DENSE_ROWS {
        roster.push(unit("Americans", "MTNK", 10, 5 + i, EntityCategory::Unit));
        // ids 1..=10
    }
    for i in 0..DENSE_ROWS {
        roster.push(unit("Soviet", "MTNK", 40, 5 + i, EntityCategory::Unit)); // ids 11..=20
    }
    sim.spawn_from_map(&roster, Some(&rules), &heights);

    // Both columns converge on x=25, same row — they close together and arrive/stall
    // in formation. Each Move is under its OWN owner (the thin generic harness rejected
    // one side's move as non-owned, leaving a single real mover). Measures the
    // synchronized-arrival churn (a whole column flipping Move→Sleep on one tick).
    let allied = sim.interner.get("Americans").expect("Americans interned");
    let soviet = sim.interner.get("Soviet").expect("Soviet interned");
    let mut script: Vec<(u64, crate::sim::intern::InternedId, Command)> = Vec::new();
    for i in 0..DENSE_ROWS as u64 {
        let y = 5 + i as u16;
        script.push((
            2,
            allied,
            Command::Move {
                entity_id: 1 + i,
                target_rx: 25,
                target_ry: y,
                queue: false,
            },
        ));
        script.push((
            2,
            soviet,
            Command::Move {
                entity_id: 11 + i,
                target_rx: 25,
                target_ry: y,
                queue: false,
            },
        ));
    }
    (sim, rules, heights, grid, script)
}

/// S2 movement-neutrality tripwire: per-tick position fingerprint of the dense
/// converging scenario, captured PRE-flip (T2). The S2 dispatch flip changes
/// only `mission.current`/`tick_counter` write points — if this fingerprint
/// shifts, the flip moved someone: that is a bug, never a re-baseline.
/// Re-baselined ONCE after the flip validation closed, for the tube-gate fix:
/// off-tube non-adjacent path steps (sharp-turn fallback bumps) are no longer
/// killed on their issue tick, so movers that previously froze now drive —
/// an intended movement-behavior change, not dispatch-order drift.
/// Re-baselined for the Phase-0 native Main_Tick order: EventClass commands
/// now dispatch at the tail, after the live object/movement walk, so a move
/// accepted on frame N first advances its object on frame N+1.
/// Re-baselined 2026-08-02 for the GSI-04.12 bridge-marker slice (c0b688a6),
/// which moves positions on purpose: `FootPathQueue::reference_cell` advances
/// the path-reference cell when Drive accepts a direction, before the curve
/// physically crosses into the destination cell, and ship locomotion split out
/// of Drive. Hash composition is not involved — this fingerprint folds entity
/// positions directly, and its value was byte-identical with the pre-branch
/// hash schema swapped in.
/// Re-baselined 2026-08-02 for passive/opportunity target acquisition, and this
/// is the fixture where it finally bites. All twenty tanks get a plain Move
/// under their OWN owner, so each one commits the Move selector, arrives, and
/// then has nothing left running — no destination, no navigation goal, no
/// standing order. Those are exactly the objects the finished-mission bridge
/// releases back to Guard, so they now acquire each other on arrival and open
/// fire instead of sitting nose to nose. Positions move because units die.
///
/// It is worth recording why the sibling global-harness constants did NOT move
/// with it, since that looked wrong until it was instrumented: nothing in that
/// fixture is ordered-then-idle. Its Allied MTNK sits on the AttackMove
/// selector, which the gate does not admit, for the whole run; its other three
/// combatants sit on the `NONE` selector and were already scanning before this
/// change. The per-tick observations, and the one cause left UNCHECKED, are
/// recorded in this file's Git history.
/// Re-baselined 2026-08-04 with FINAL_STREAM_STATES for the same
/// constructed-`Rate` change; see the provenance note there.
/// Re-baselined 2026-08-05 for the Drive cell-admission gate. A curve is now
/// refused when the cell it would step into is refused by *either* arm of
/// gamemd's cell-entry predicate — a body in the cell's object list, or another
/// vehicle's occupation mark — where before the runtime consulted neither at
/// selection time. This fixture is twenty tanks converging on one column, so
/// movers that previously drove through each other now wait, scatter and
/// repath; positions move on purpose.
///
/// WITHDRAWN: an earlier revision of this note cited `FINAL_STREAM_STATES` to
/// certify that "no RNG draw moved and no stream was misrouted" for THIS
/// fixture. That claim is wrong on two axes and is retracted. First, the pin
/// lives in a different test — `s2_dense_scenario_position_fingerprint_stable`
/// pins no stream at all, so this twenty-tank convergence has ZERO RNG
/// observation of its own. Second, `SimRng::next_u32` advances as a pure
/// function of its own state, so even where the pin does apply an unchanged
/// final state proves only that the DRAW COUNT on that stream is unchanged; it
/// says nothing about which tick or which consumer took them.
///
/// ATTRIBUTION, MEASURED. This fixture folds only entity ids and positions, so
/// it carries no hash-schema component at all — and that is confirmed rather
/// than assumed. With `occupation_handoff` still on the struct but every
/// behaviour writer neutralised (the experiment written out at
/// `GLOBAL_HARNESS_PRE_LIFECYCLE_V28_HASH`), this fixture returns to exactly its
/// previous committed value `0x0FC6_3769_AADD_1F8A`. So its shift is 100%
/// behaviour and 0% schema — the mirror image of the global harness above.
///
/// What is still NOT separated: the individual contribution of the object-list
/// arm, the mask arm and the handoff mark, which landed together and were
/// neutralised together. UNVERIFIED.
/// Re-baselined 2026-09-08 for residual cell normalization. A parent (588f4079)
/// comparison of all 6000 tick/entity rows found five isolated world-XY pulses
/// (80 rows, maximum 23 leptons), all equal again on the next tick. The old delayed
/// paid-crossing return skipped that frame's remaining budget/interpolation;
/// the already-normalized cell now takes the native same-cell loop/tail
/// (4B1F56 / 4B22D9). Paths, missions, speeds and membership stayed equal;
/// final entity states differ only in exact Z. This is an intentional movement
/// correction, not a hash-fold change or full locomotor parity claim. The
/// existing early return on an actual paid crossing remains a separate limit.
/// See RAMP_UNIT_HEIGHT_GHIDRA_REPORT.md, Rust replay provenance.
// 2026-09-13: unpaid fresh budget0 retains current XY instead of eagerly
// publishing point0. First native-adjudicated divergence is tick2 (128 vs139);
// see docs/research/TRACK_PROCESS_REPLAY_REGRESSION_NOTES.md.
// 2026-09-20: destination commands no longer turn/admit ahead of fresh Process.
// All6000 rows were compared with passing bd5928e6: east column unchanged;
// west column exactly one tick later. Native4B3408 calls Do_Turn then returns
// before admission evenROT0. Independent review accepted this Rust regression
// re-pin; it is not a native whole-scenario golden. Same-frame facing repair
// changed no XY. See TRACK_PROCESS_REPLAY_REGRESSION_NOTES.md for scope/evidence.
// 2026-09-23: same-call track-end continuation. First divergence from main is
// tick 30, where the column's first track ends continue into their next
// tracks in that Process; both RNG streams match main over the 300 ticks.
const POSITION_FINGERPRINT: u64 = 0x828D_9C15_C129_26CE;

#[test]
fn fresh_drive_turn_publishes_on_request_frame_and_restores_before_admission() {
    let rows: Vec<serde_json::Value> = serde_json::from_str(include_str!(
        "../../../tools/spatial_oracle/drive_fresh_turn.json"
    ))
    .unwrap();
    for rot in [0, 5] {
        let rules = RuleSet::from_ini(&IniFile::from_str(&format!(
            "[VehicleTypes]\n0=MTNK\n[MTNK]\nStrength=300\nSpeed=6\nROT={rot}\n\
             Locomotor={{4A582741-9839-11d1-B709-00A024DDAFD1}}\n"
        )))
        .unwrap();
        let mut sim = Simulation::with_seed(DENSE_SEED);
        let heights = BTreeMap::new();
        let grid = PathGrid::new(64, 64);
        sim.spawn_from_map(
            &[unit("Americans", "MTNK", 40, 5, EntityCategory::Unit)],
            Some(&rules),
            &heights,
        );
        let owner = sim.interner.get("Americans").unwrap();
        let native = rows
            .iter()
            .find(|row| {
                row["input"]["initial"] == 0x4000
                    && row["input"]["direction"] == 6
                    && row["input"]["rate"] == rot * 256
            })
            .unwrap();
        for tick in 1..=3 {
            let due = if tick == 2 {
                vec![CommandEnvelope::new(
                    owner,
                    tick,
                    Command::Move {
                        entity_id: 1,
                        target_rx: 25,
                        target_ry: 5,
                        queue: false,
                    },
                )]
            } else {
                Vec::new()
            };
            sim.advance_tick(
                &due,
                Some(&rules),
                &heights,
                Some(&grid),
                None,
                HARNESS_TICK_MS,
            );
        }
        let entity = sim.substrate.entities.get(1).unwrap();
        let call = &native["calls"][0];
        assert_eq!(
            u64::from(entity.facing),
            call["sampled_after"].as_u64().unwrap() >> 8,
            "ROT={rot}: same-frame native sample"
        );
        let drive = entity.drive_locomotion.as_ref().unwrap();
        assert_eq!(
            drive.track.turn_index, -1,
            "turn must return before admission"
        );
        assert!(drive.head_to.is_none());
        if rot > 0 {
            assert_eq!(
                u64::from(
                    entity
                        .body_facing
                        .as_ref()
                        .unwrap()
                        .current(sim.session.binary_frame - 1)
                ),
                call["sampled_after"].as_u64().unwrap(),
                "full16-bit native sample, before the next binary frame"
            );
            assert_eq!(
                entity.body_facing.as_ref().unwrap().timer_start_frame(),
                Some(call["timer_start"].as_u64().unwrap() as u32)
            );
        }
        // Native load resets Scenario to Seed0 and retains process streams.
        // Equalize only that documented load effect before comparing the
        // movement-state round trip and its continued full-world hashes.
        sim.scenario_rng = crate::sim::rng::SimRng::new(0);
        let bytes = crate::sim::snapshot::GameSnapshot::save(&sim, 1, 0, "Fresh turn", 0);
        let mut restored = crate::sim::snapshot::GameSnapshot::load(&bytes)
            .unwrap()
            .sim;
        restored.retain_in_scenario_process_state_from(&sim);
        restored.restore_after_snapshot_load().unwrap();
        for tick in 4..=35 {
            for world in [&mut sim, &mut restored] {
                world.advance_tick(
                    &[],
                    Some(&rules),
                    &heights,
                    Some(&grid),
                    None,
                    HARNESS_TICK_MS,
                );
            }
            assert_eq!(
                sim.state_hash(),
                restored.state_hash(),
                "ROT={rot}, tick={tick}"
            );
            if tick == 4 {
                let entity = sim.substrate.entities.get(1).unwrap();
                assert_eq!(
                    entity.drive_locomotion.as_ref().unwrap().head_to.is_some(),
                    rot == 0
                );
            }
        }
    }
}

#[test]
fn s2_dense_scenario_position_fingerprint_stable() {
    use std::hash::{Hash, Hasher};
    let (mut sim, rules, heights, grid, script) = dense_converging_setup();
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for tick in 0..DENSE_TICKS {
        let due: Vec<CommandEnvelope> = script
            .iter()
            .filter(|(t, _, _)| *t == tick + 1)
            .map(|(t, owner, c)| CommandEnvelope::new(*owner, *t, c.clone()))
            .collect();
        let _ = sim.advance_tick(
            &due,
            Some(&rules),
            &heights,
            Some(&grid),
            None,
            HARNESS_TICK_MS,
        );
        for (id, e) in sim.substrate.entities.iter_sorted() {
            (
                id,
                e.position.rx,
                e.position.ry,
                e.position.sub_x,
                e.position.sub_y,
            )
                .hash(&mut h);
        }
    }
    assert_eq!(
        h.finish(),
        POSITION_FINGERPRINT,
        "committed per-tick position sequence changed; attribute the first divergence before updating"
    );
}
