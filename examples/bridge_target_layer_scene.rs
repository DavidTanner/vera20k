//! Production Hills bridge target-layer composition witness.
//! Run: bridge_target_layer_scene RETAIL_DIR NEW_SAVE_PREFIX [MAP_FILE]
//! The fixture supplies two hostile human houses and a ten-frame rearm entry;
//! normal Move, passive AI, firing, collapse and Engineer hut repair execute.
//! This is production composition, not native whole-scene equivalence.
use std::io::Write;
use std::path::Path;
use vera20k::headless_scenario::{HeadlessScenario, SIM_TICK_MS};
use vera20k::sim::command::{Command, CommandEnvelope};
use vera20k::sim::intern::InternedId;
use vera20k::sim::mission::MissionTimer;
use vera20k::sim::timer::CdTimer;

fn frame(scene: &mut HeadlessScenario, commands: &[CommandEnvelope]) {
    scene
        .runtime
        .advance_frame_for_tooling(commands, SIM_TICK_MS)
        .expect("ordinary production frame");
}
fn envelope(scene: &HeadlessScenario, owner: InternedId, command: Command) -> CommandEnvelope {
    CommandEnvelope::new(owner, scene.sim().session.tick + 1, command)
}
fn spawn(
    scene: &mut HeadlessScenario,
    owner: InternedId,
    name: &str,
    xy: (u16, u16),
) -> Option<u64> {
    let runtime = &mut scene.runtime;
    let owner_name = runtime.simulation.resolve(owner).to_owned();
    runtime
        .simulation
        .spawn_object(name, &owner_name, xy.0, xy.1, 0, &runtime.resources.rules)
}
fn move_fv(
    scene: &mut HeadlessScenario,
    owner: InternedId,
    start: (u16, u16),
    destination: (u16, u16),
    stop_at: (u16, u16),
    deck: bool,
) -> u64 {
    let id = spawn(scene, owner, "FV", start).expect("ordinary FV bank/road placement");
    move_existing_fv(scene, owner, id, destination, stop_at, deck);
    id
}
fn move_existing_fv(
    scene: &mut HeadlessScenario,
    owner: InternedId,
    id: u64,
    destination: (u16, u16),
    stop_at: (u16, u16),
    deck: bool,
) {
    let position = &scene.sim().entities().get(id).unwrap().position;
    let start = (position.rx, position.ry);
    let now = scene.sim().session.binary_frame;
    scene
        .runtime
        .simulation
        .entities_mut()
        .get_mut(id)
        .unwrap()
        .passive_scan_timer = MissionTimer::armed(now, 1_000_000);
    let command = envelope(
        scene,
        owner,
        Command::Move {
            entity_id: id,
            target_rx: destination.0,
            target_ry: destination.1,
            queue: false,
        },
    );
    let mut stop_sent = false;
    for tick in 0..1200 {
        // Bridge-cell Move goals select the deck. An exterior road goal plus
        // ordinary Stop during traversal admits the ground layer instead.
        let stop = (!stop_sent
            && destination != stop_at
            && scene
                .sim()
                .entities()
                .get(id)
                .is_some_and(|e| (e.position.rx, e.position.ry) == stop_at))
        .then(|| {
            stop_sent = true;
            envelope(scene, owner, Command::Stop { entity_id: id })
        });
        frame(
            scene,
            if tick == 0 {
                std::slice::from_ref(&command)
            } else if let Some(stop) = stop.as_ref() {
                std::slice::from_ref(stop)
            } else {
                &[]
            },
        );
        let e = scene.sim().entities().get(id).unwrap();
        if (e.position.rx, e.position.ry) == stop_at && e.movement_target.is_none() {
            assert_eq!(e.on_bridge, deck);
            assert_eq!(
                e.position.exact_z_leptons,
                Some(if deck { 1040 } else { 624 })
            );
            println!(
                "FV{id} Move {start:?}->{destination:?}, stop{stop_at:?}: deck{deck} XYZ{:?}, {}frames",
                e.position.exact_z_leptons,
                tick + 1
            );
            return;
        }
    }
    panic!("FV{id} failed normal Move {start:?}->{destination:?}, stop{stop_at:?}");
}
fn save(scene: &HeadlessScenario, prefix: &str, phase: &str) {
    let path = format!("{prefix}-{phase}.bin");
    let rules = scene.runtime.resources.rules.simulation_config_hash();
    let bytes = vera20k::sim::snapshot::GameSnapshot::save_validated(
        scene.sim(),
        scene.map.ini.content_hash(),
        rules,
        &format!("Hills bridge target layers: {phase}"),
        0,
    );
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .expect("new save path")
        .write_all(&bytes)
        .unwrap();
    println!(
        "Saved {path}: map{:016x} rules{rules:016x} frame{} state{:016x}",
        scene.map.ini.content_hash(),
        scene.sim().session.binary_frame,
        scene.sim().state_hash()
    );
}
fn structural(scene: &HeadlessScenario) -> bool {
    scene
        .sim()
        .resolved_terrain
        .as_ref()
        .unwrap()
        .cell(64, 69)
        .unwrap()
        .bridge_facts
        .has_structural_bridge()
}
fn main() {
    let mut args = std::env::args().skip(1);
    let retail = args.next().expect("retail directory");
    let prefix = args.next().expect("new save prefix");
    let map = args.next().unwrap_or_else(|| "Hills.mmx".into());
    let mut scene = vera20k::headless_scenario::load(Path::new(&retail), &map, 0x0B21_D6E5)
        .expect("retail production loader");
    let owner = scene.sim().session.current_house.unwrap();
    for xy in [(64, 69), (66, 69)] {
        let c = scene
            .sim()
            .resolved_terrain
            .as_ref()
            .unwrap()
            .cell(xy.0, xy.1)
            .unwrap();
        assert_eq!((c.level, c.slope_type, c.base_yr_cell_land_type), (6, 0, 7));
        assert_eq!(c.base_speed_costs.track, Some(100));
        assert!(!c.base_ground_walk_blocked);
        assert!(c.bridge_facts.has_structural_bridge());
    }
    let source = move_fv(&mut scene, owner, (64, 72), (64, 69), (64, 69), true);
    let target = move_fv(&mut scene, owner, (62, 69), (68, 69), (66, 69), false);
    let stop = [
        envelope(&scene, owner, Command::Stop { entity_id: source }),
        envelope(&scene, owner, Command::Stop { entity_id: target }),
    ];
    frame(&mut scene, &stop);
    let enemy = scene.runtime.simulation.intern("Russians");
    scene
        .runtime
        .simulation
        .houses
        .entry(enemy)
        .or_insert_with(|| vera20k::sim::house_state::HouseState::new(enemy, 0, None, true, 0, 10));
    scene
        .runtime
        .simulation
        .entities_mut()
        .change_owner(target, enemy);
    let now = scene.sim().session.binary_frame;
    let e = scene
        .runtime
        .simulation
        .entities_mut()
        .get_mut(source)
        .unwrap();
    e.passive_scan_timer.clear();
    e.rearm_timer = CdTimer::started(now as i32, 10);
    frame(&mut scene, &[]);
    let e = scene.sim().entities().get(source).unwrap();
    assert!(e.attack_target.is_none());
    assert_eq!(e.last_target_scan_frame, now);
    assert!(!e.passively_acquired_target);
    println!(
        "Busy opposite-layer scan: source{source} target{target} scan_frame{now} next_frame{} timer{:?}; no acquisition",
        scene.sim().session.binary_frame,
        e.passive_scan_timer
    );
    save(&scene, &prefix, "intact");
    for _ in 0..40 {
        frame(&mut scene, &[]);
        assert!(
            scene
                .sim()
                .entities()
                .get(source)
                .unwrap()
                .attack_target
                .is_none()
        );
    }
    let attacker = spawn(&mut scene, owner, "FV", (64, 72)).expect("ordinary bank IFV");
    let mut seen = std::collections::BTreeSet::new();
    let mut collapsed = false;
    for tick in 0..18000 {
        let commands = if scene
            .sim()
            .entities()
            .get(attacker)
            .unwrap()
            .attack_target
            .is_none()
        {
            vec![envelope(
                &scene,
                owner,
                Command::ForceAttackCell {
                    attacker_id: attacker,
                    target_rx: 64,
                    target_ry: 69,
                },
            )]
        } else {
            Vec::new()
        };
        frame(&mut scene, &commands);
        for (&id, _) in scene
            .sim()
            .projectiles
            .iter()
            .filter(|(_, p)| p.source_id == attacker)
        {
            seen.insert(id);
        }
        if !structural(&scene) {
            collapsed = true;
            println!(
                "Bridge collapsed after{}frames and{}ordinary IFV projectiles; layer actors alive{:?}",
                tick + 1,
                seen.len(),
                [source, target].map(|id| scene.sim().entities().get(id).map(|e| e.health.current))
            );
            save(&scene, &prefix, "collapsed");
            break;
        }
    }
    assert!(collapsed, "ordinary IFV did not collapse stock bridge");

    // Finish the real attack before issuing repair. In-flight shots retain
    // their normal lifetime; the fixture does not delete projectiles.
    let stop = envelope(
        &scene,
        owner,
        Command::Stop {
            entity_id: attacker,
        },
    );
    frame(&mut scene, &[stop]);
    for _ in 0..2000 {
        if scene
            .sim()
            .projectiles
            .iter()
            .all(|(_, projectile)| projectile.source_id != attacker)
        {
            break;
        }
        frame(&mut scene, &[]);
    }
    assert!(
        scene
            .sim()
            .projectiles
            .iter()
            .all(|(_, projectile)| projectile.source_id != attacker),
        "the original attack must finish before repair"
    );
    let hut = scene
        .sim()
        .entities()
        .values()
        .find(|entity| {
            scene.sim().resolve(entity.type_ref()) == "CABHUT"
                && (entity.position.rx, entity.position.ry) == (68, 74)
        })
        .expect("authored Hills CABHUT")
        .stable_id();
    let engineer =
        spawn(&mut scene, owner, "ENGINEER", (66, 76)).expect("legal ordinary Engineer approach");
    assert_eq!(
        scene
            .runtime
            .resources
            .rules
            .object("ENGINEER")
            .unwrap()
            .speed_type,
        vera20k::rules::locomotor_type::SpeedType::Foot
    );
    let enter = envelope(
        &scene,
        owner,
        Command::CaptureBuilding {
            engineer_id: engineer,
            target_building_id: hut,
        },
    );
    let mut saved_approach = false;
    let mut repaired = false;
    for tick in 0..1200 {
        frame(
            &mut scene,
            if tick == 0 {
                std::slice::from_ref(&enter)
            } else {
                &[]
            },
        );
        if let Some(actor) = scene.sim().entities().get(engineer) {
            if !saved_approach && (actor.position.rx, actor.position.ry) == (67, 75) {
                save(&scene, &prefix, "engineer-approach");
                saved_approach = true;
            }
        } else {
            assert!(
                structural(&scene),
                "consumed Engineer must restore the deck"
            );
            assert!(
                scene
                    .sim()
                    .path_grid()
                    .unwrap()
                    .cell(64, 69)
                    .unwrap()
                    .bridge_walkable
            );
            println!(
                "ENGINEER{engineer} entered CABHUT{hut} and repaired the bridge in{}frames",
                tick + 1
            );
            save(&scene, &prefix, "repaired");
            repaired = true;
            break;
        }
    }
    assert!(
        saved_approach && repaired,
        "ordinary Engineer must approach, enter and repair"
    );
    // The stopped attacker still occupies the bank. Reuse that live object;
    // spawning a second FV onto its occupied cell must correctly be refused.
    move_existing_fv(&mut scene, owner, attacker, (64, 69), (64, 69), true);
    println!("FV{attacker} entered the repaired side deck through normal navigation");
    save(&scene, &prefix, "repaired-deck-occupied");
}
