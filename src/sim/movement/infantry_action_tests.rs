use super::*;
use crate::rules::ini_parser::IniFile;
use crate::util::fixed_math::SimFixed;
use serde_json::Value;

#[test]
fn do_action_refuses_unchanged_and_uninterruptible_actions() {
    // Oracle astar_null rows: Doing 0 with request 0 leaves frame/timer alone.
    assert!(!do_action_admits(0, 0, false));
    assert!(do_action_admits(-1, 0, false));
    // Walk (3) is interruptible in the 0x7EAF7C table; Die1 (11) is not.
    assert!(do_action_admits(3, 0, false));
    assert!(!do_action_admits(11, 0, false));
    assert!(do_action_admits(11, 0, true));
    assert!(!do_action_admits(11, 11, true));
}

/// The retail `[RocketeerSequence]` (artmd.ini) with the Hover record's
/// value as given; `WALKJET` flies the Jumpjet locomotor without `JumpJet=`.
fn rocketeer_rules(hover: &str) -> RuleSet {
    let mut rules = RuleSet::from_ini(&IniFile::from_str(
        "[InfantryTypes]\n0=JUMPJET\n1=WALKJET\n\
         [JUMPJET]\nStrength=125\nJumpJet=yes\nBalloonHover=yes\nCrashable=yes\n\
         Locomotor={92612C46-F71F-11d1-AC9F-006008055BB5}\nSpeedType=Hover\n\
         MovementZone=Fly\nJumpjetSpeed=30\nJumpjetClimb=20\nJumpjetCrash=25\n\
         JumpjetHeight=500\nJumpjetNoWobbles=yes\n\
         [WALKJET]\nStrength=125\nImage=JUMPJET\nBalloonHover=yes\n\
         Locomotor={92612C46-F71F-11d1-AC9F-006008055BB5}\nSpeedType=Hover\n\
         MovementZone=Fly\nJumpjetSpeed=30\nJumpjetHeight=500\n",
    ))
    .unwrap();
    let art = IniFile::from_str(&format!(
        "[JUMPJET]\nSequence=RocketeerSequence\n\
         [RocketeerSequence]\nReady=0,1,1\nGuard=0,1,1\nProne=86,1,6\nWalk=8,6,6\n\
         FireUp=164,6,6\nDown=260,2,2\nCrawl=86,6,6\nUp=276,2,2\nFireProne=212,6,6\n\
         Idle1=56,15,0,S\nIdle2=71,15,0,E\nDie1=134,15,0\nDie2=149,15,0\nDie3=0,0,0\n\
         Die4=0,0,0\nDie5=0,0,0\nFly=292,6,6\nHover={hover}\nFireFly=370,6,6\n\
         Tumble=340,15,0\nAirDeathStart=340,8,0\nAirDeathFalling=348,1,0\n\
         AirDeathFinish=349,6,0\nParadrop=418,1,0\nCheer=419,8,0,E\nPanic=8,6,6\n"
    ));
    rules.replace_art_registry_for_test(crate::rules::art_data::ArtRegistry::from_ini(&art));
    let registry = crate::rules::infantry_sequence::parse_infantry_sequence_registry(&art);
    rules.replace_animation_sequences_for_test(
        crate::rules::animation_sequence::build_animation_sequence_catalog(&rules, Some(&registry)),
    );
    rules
}

/// A Rocketeer in the state one corpus row declares: flown by the Jumpjet
/// locomotor in `owner.phase` with its moving byte, at the row's height, with
/// its Doing, speed fraction, firing latch and Health.
fn rocketeer(input: &Value) -> (Simulation, RuleSet, u64) {
    let hover_start = input["hover_start"].as_i64().unwrap_or(292);
    let rules = rocketeer_rules(&format!("{hover_start},6,6"));
    let mut sim = Simulation::with_seed(0);
    // Original Actions/States supplies A8ED84=1000.
    sim.session.binary_frame = 1000;
    let house = sim.interner.intern("Americans");
    sim.houses.insert(
        house,
        crate::sim::house_state::HouseState::new(house, 0, None, true, 0, 10),
    );
    sim.session.game_options.game_speed = input["game_speed"].as_i64().unwrap_or(1) as i32;
    let type_id = if input["jumpjet"].as_bool().unwrap_or(true) {
        "JUMPJET"
    } else {
        "WALKJET"
    };
    let id = sim
        .construct_object_limbo_at_height(type_id, "Americans", 10, 10, 64, 0, &rules)
        .expect("rocketeer");
    let height = input["height"].as_i64().unwrap_or(500) as i32;
    let entity = sim.substrate.entities.get_mut(id).unwrap();
    entity.lifecycle.in_limbo = false;
    // jumpjet_infantry_actions.Actions supplies OWNER+74=1; Do_Action's
    // Object5F6B90 height predicate requires this independently of XYZ.
    entity.lifecycle.cell_marked = true;
    entity.health.current = input["health"].as_i64().unwrap_or(125) as _;
    entity.on_bridge = input["on_bridge"].as_bool().unwrap_or(false);
    entity.position.exact_z_leptons = Some(height);
    entity
        .mission_leaf
        .set_infantry_doing_verified(input["doing"].as_i64().unwrap_or(-1) as i32)
        .unwrap();
    entity
        .foot_speed
        .set_speed_fraction_native_bits(input["fraction"].as_f64().unwrap_or(0.0).to_bits());
    entity
        .mission_leaf
        .set_foot_firing_sequence(u8::from(input["firing"].as_bool().unwrap_or(false)));
    if entity.locomotor.is_none() {
        entity.locomotor = Some(
            crate::sim::movement::locomotor::LocomotorState::from_object_type(
                rules.object(type_id).unwrap(),
                0,
            ),
        );
    }
    // The supplied signed +0xF8 and native clock belong to the Stage owner.
    entity.install_native_stage_fixture(crate::sim::stage::StageClass::from_native_fixture(
        input["stage"].as_i64().unwrap_or(0) as i32,
        0,
        crate::sim::timer::CdTimer::from_raw(17, 91),
        92,
        1,
    ));
    let locomotor = entity.locomotor.as_mut().expect("Jumpjet locomotor");
    locomotor.altitude = SimFixed::from_num(height);
    let runtime = locomotor.jumpjet_runtime_mut().expect("Jumpjet runtime");
    runtime.phase = input["owner"]["phase"].as_i64().unwrap_or(2) as i32;
    runtime.moving = input["owner"]["moving"].as_bool().unwrap_or(true);
    (sim, rules, id)
}

/// Compare the retained native dwords, including a refused action's clock.
fn assert_stage(sim: &Simulation, id: u64, output: &Value, name: &str) {
    let entity = sim.substrate.entities.get(id).unwrap();
    let stage = entity.native_stage();
    let expected: Vec<i32> = output["timer"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| value.as_i64().unwrap() as i32)
        .collect();
    assert_eq!(
        stage.value(),
        output["stage"].as_i64().unwrap() as i32,
        "{name}: stage"
    );
    assert_eq!(
        [
            stage.timer().start_frame(),
            stage.timer().duration(),
            stage.rate()
        ],
        expected.as_slice(),
        "{name}: retained stage timer and rate"
    );
}

fn doing(sim: &Simulation, id: u64) -> i32 {
    sim.substrate
        .entities
        .get(id)
        .unwrap()
        .mission_leaf
        .as_infantry()
        .unwrap()
        .doing()
}

/// Parity with `tools/spatial_oracle/jumpjet_infantry_actions.json`: the
/// original Do_Action, locomotion action tail, firing arm and sequencer on a
/// Rocketeer flown by the real Jumpjet locomotor, the sequencer's AirDeath
/// arms included (AirDeathFinish's end UnInits it). Not compared here: the
/// locomotor stop of the Health-0 Stop_Driver re-entry, which needs a map
/// (`world::jumpjet_infantry_tests` compares it through the crash); and the
/// firing arm's walker FireUp path, covered by the ground firing corpus.
#[test]
fn jumpjet_infantry_actions_match_the_native_bodies() {
    let corpus: Value = serde_json::from_str(include_str!(
        "../../../tools/spatial_oracle/jumpjet_infantry_actions.json"
    ))
    .expect("corpus parses");
    let mut compared = 0;
    let mut truncated = 0;
    for (index, row) in corpus["rows"].as_array().expect("rows").iter().enumerate() {
        let input = &row["input"];
        let output = &row["output"];
        let name = format!("row {index} {input}");
        let kind = input["kind"].as_str().unwrap();
        let native_doing = output["doing"].as_i64().unwrap() as i32;
        let (mut sim, rules, id) = rocketeer(input);
        match kind {
            "do_action" => {
                let request = input["request"].as_i64().unwrap() as i32;
                let force = input["force"].as_bool().unwrap_or(false);
                let accepted = sim.infantry_do_action(id, request, force, &rules).unwrap();
                assert_eq!(accepted, output["accepted"].as_bool().unwrap(), "{name}");
            }
            "movement" => sim.infantry_movement_actions(id, &rules),
            "sequencer" => {
                // The object turn (`infantry_action_turn`) follows the
                // sequencer with the locomotion actions, which this row does
                // not run.
                let removed = sim.infantry_sequencer(id, &rules);
                let native_removed = output["recorded"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|event| event == "uninit");
                assert_eq!(removed, native_removed, "{name}: UnInit");
                if removed {
                    assert!(
                        sim.substrate
                            .entities
                            .get(id)
                            .is_none_or(|entity| !entity.lifecycle.object_alive),
                        "{name}: removed"
                    );
                    compared += 1;
                    continue;
                }
            }
            "firing" => {
                if !input["jumpjet"].as_bool().unwrap() {
                    continue;
                }
                sim.infantry_do_action(id, DO_FIRE_FLY, false, &rules)
                    .unwrap();
            }
            other => panic!("unknown row kind {other}"),
        }
        // A fraction within 2^-16 above 0.8 truncates to 0.8 itself
        // (`set_speed_fraction_native_bits`): the tail reads it as a hover
        // where native flies. The corpus probes that boundary on purpose.
        let fraction = input["fraction"].as_f64().unwrap_or(0.0);
        if kind == "movement" && fraction > 0.8 && fraction * 65536.0 < 52429.0 {
            if native_doing == DO_FLY {
                assert_eq!(doing(&sim, id), DO_HOVER, "{name}: truncated fraction");
                truncated += 1;
                continue;
            }
        }
        assert_eq!(doing(&sim, id), native_doing, "{name}");
        assert_stage(&sim, id, output, &name);
        compared += 1;
    }
    assert_eq!((compared, truncated), (497, 30));
}

/// The truncation of `FootClass::SetSpeedFraction @ 0x004D3710`'s stored
/// double to `SimFixed`: a Jumpjet at speed 30 braking by 3 reaches 3/30 and
/// 24/30, which the truncating native division leaves just below 0.1 and 0.8.
/// The clamp itself is checked against the original setter in
/// `components::tests::speed_fraction_setter_matches_the_original`.
#[test]
fn a_native_speed_fraction_is_truncated_below_its_thresholds() {
    let mut speed = crate::sim::components::FootSpeedState::default();
    let mut set = |bits: u64| {
        speed.set_speed_fraction_native_bits(bits);
        speed.applied_fraction().to_bits()
    };
    // 3/30 and 24/30 divided with truncation: one ulp below 0.1 and 0.8.
    let tenth = 0x3FB9_9999_9999_9999;
    let eight_tenths = 0x3FE9_9999_9999_9999;
    assert!(f64::from_bits(tenth) < 0.1 && f64::from_bits(eight_tenths) < 0.8);
    assert_eq!(set(tenth), 6553);
    assert_eq!(set(eight_tenths), 52428);
    // Neither reads as above its threshold; the next step above does.
    let mut entity =
        crate::sim::game_entity::GameEntity::test_default(1, "JUMPJET", "Americans", 0, 0);
    entity.foot_speed.set_speed_fraction_native_bits(tenth);
    assert!(!entity.foot_speed.above_tenth());
    entity
        .foot_speed
        .set_speed_fraction_native_bits(eight_tenths);
    assert!(!entity.foot_speed.above_eight_tenths());
    entity
        .foot_speed
        .set_speed_fraction_native_bits((4.0f64 / 30.0).to_bits());
    assert!(entity.foot_speed.above_tenth());
    entity
        .foot_speed
        .set_speed_fraction_native_bits((26.0f64 / 30.0).to_bits());
    assert!(entity.foot_speed.above_eight_tenths());
}

/// `takes_default_arm` against the native dispatch: the arm
/// `DoType_Sequencer` takes for each Doing, read from the tables at
/// `0x00520F1C` and `0x00520EFC` in the corpus.
#[test]
fn the_default_arm_follows_the_native_sequencer_tables() {
    let corpus: Value = serde_json::from_str(include_str!(
        "../../../tools/spatial_oracle/jumpjet_infantry_actions.json"
    ))
    .expect("corpus parses");
    let arms = corpus["sequencer_arms"].as_array().expect("arms");
    assert_eq!(arms.len(), 43);
    for arm in arms {
        let doing = arm[0].as_i64().unwrap() as i32;
        let native_default = arm[1].as_str().unwrap() == "00520CE6";
        assert_eq!(takes_default_arm(doing), native_default, "Doing {doing}");
    }
}
