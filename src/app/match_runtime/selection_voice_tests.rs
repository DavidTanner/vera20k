//! Native VoiceSelect708EB0 / QueueVoice708D90 comparisons, using the
//! executable corpus and production Rules/audio owners without a device.

use super::{selection_voice_event, selection_voice_request};
use crate::app::audio_runtime::AppAudioRuntime;
use crate::audio::events::{GameSoundEvent, SoundEventQueue};
use crate::audio::sfx::SfxRng;
use crate::audio::voice_queue::VoiceQueue;
use crate::map::entities::EntityCategory;
use crate::rules::ruleset::RuleSet;
use crate::sim::components::Health;
use crate::sim::game_entity::GameEntity;
use crate::sim::house_state::HouseState;
use crate::sim::world::Simulation;

fn corpus() -> serde_json::Value {
    serde_json::from_str(crate::test_fixture::text(
        "tools/input_oracle/selection_navigation.json",
    ))
    .unwrap()
}

fn sound_name(id: i64) -> String {
    if id == -1 {
        String::new()
    } else {
        format!("Sound{id}")
    }
}

#[test]
fn reached_normal_voice_calls_match_native_queue_latches_and_complete_main_rng() {
    let native = corpus();
    let mut histories = 0;
    let mut calls = 0;
    let mut separate_receivers = 0;
    for history in native["voice_histories"].as_array().unwrap() {
        if history["slave"] == true || history["robot_offline"] == true {
            // These use different type vectors; their receiver selection is
            // an explicit separate lifecycle/list mechanism, not a normal
            // VoiceSelect parity claim.
            separate_receivers += 1;
            continue;
        }
        let label = history["id"].as_str().unwrap();
        let voices: Vec<_> = history["voice_list"]
            .as_array()
            .unwrap()
            .iter()
            .map(|id| sound_name(id.as_i64().unwrap()))
            .collect();
        let mut random = SfxRng::seeded(history["seed"].as_u64().unwrap());
        assert_eq!(
            random.native_state_hex(),
            history["rng_before_hex"]["main"].as_str().unwrap(),
            "{label}"
        );
        let mut queue = VoiceQueue::new();
        queue.queue(
            1,
            &sound_name(history["initial_queued_voice"].as_i64().unwrap()),
        );
        let mut events = SoundEventQueue::new();
        for step in history["steps"].as_array().unwrap() {
            let before = random.native_state_hex();
            if let Some(event) = selection_voice_request(
                1,
                &voices,
                history["voice_enabled"].as_bool().unwrap_or(true),
                history["owner"].as_str().unwrap_or("local") == "local",
                &mut random,
            ) {
                let request = &step["voice_requests"][0];
                assert_eq!(
                    event.sound_id(),
                    sound_name(request["sound_id"].as_i64().unwrap()),
                    "{label}"
                );
                events.push(event);
            }
            // Exercise the same request-to-latch transfer as the production
            // sound-event drain; inspection leaves the queue intact.
            for event in events.iter() {
                let GameSoundEvent::UnitSelected {
                    speaker_id,
                    sound_id,
                } = event
                else {
                    panic!("unexpected event {event:?}");
                };
                queue.queue(*speaker_id, sound_id);
            }
            let _ = events.drain();
            let expected = sound_name(step["queued_voices"]["1"].as_i64().unwrap());
            assert_eq!(
                queue.pending_for(1),
                (!expected.is_empty()).then_some(expected.as_str()),
                "{label}"
            );
            assert_eq!(
                before == random.native_state_hex(),
                step["rng_unchanged"]["main"].as_bool().unwrap(),
                "{label}"
            );
            assert_eq!(step["rng_unchanged"]["scenario"], true, "{label}");
            assert_eq!(step["rng_unchanged"]["mapgen"], true, "{label}");
            calls += 1;
        }
        assert_eq!(
            random.native_state_hex(),
            history["rng_after_hex"]["main"].as_str().unwrap(),
            "{label}"
        );
        for expected in history["main_next_four"].as_array().unwrap() {
            assert_eq!(
                u64::from(random.next_u32()),
                expected.as_u64().unwrap(),
                "{label}"
            );
        }
        histories += 1;
    }
    assert_eq!((histories, calls, separate_receivers), (24, 92, 4));
}

fn simulation(
    type_id: &str,
    category: EntityCategory,
    owner_name: &str,
    passive: bool,
) -> Simulation {
    let mut sim = Simulation::with_seed(1234);
    let local = sim.interner.intern("Local");
    let owner = sim.interner.intern(owner_name);
    let type_ref = sim.interner.intern(type_id);
    let mut house = HouseState::new(owner, 0, None, owner == local, 0, 10);
    house.multiplay_passive = passive;
    sim.houses.insert(owner, house);
    sim.session.game_mode_nonzero = true;
    sim.session.current_house = Some(local);
    sim.entities_mut()
        .insert(GameEntity::new_at_frame_zero_for_test(
            1,
            10,
            10,
            0,
            0,
            owner,
            Health { current: 100 },
            type_ref,
            category,
            0,
            5,
            false,
        ));
    sim
}

#[test]
fn select_wrapper_gates_precede_voice_draw_and_passive_queue_rejection_follows_it() {
    let rules = RuleSet::from_ini(&crate::rules::ini_parser::IniFile::from_str(
        "[VehicleTypes]\n0=TANK\n[TANK]\nVoiceSelect=Sound101,Sound202,Sound303\n",
    ))
    .unwrap();
    for (owner, passive, enabled, drew, queued) in [
        ("Local", false, true, true, true),
        ("Local", false, false, false, false),
        ("Other", false, true, false, false),
        ("Other", true, true, true, false),
        ("Other", true, false, false, false),
    ] {
        let sim = simulation("TANK", EntityCategory::Unit, owner, passive);
        let before_sim = sim.scenario_rng.native_state_hex();
        let mut random = SfxRng::seeded(1);
        let before = random.native_state_hex();
        let event = selection_voice_event(&sim, &rules, 1, enabled, &mut random);
        assert_eq!(event.is_some(), queued, "{owner}/{passive}/{enabled}");
        assert_eq!(
            before != random.native_state_hex(),
            drew,
            "{owner}/{passive}/{enabled}"
        );
        assert_eq!(sim.scenario_rng.native_state_hex(), before_sim);
    }
}

#[test]
fn retail_normal_selection_reaches_fixed_voice_list_and_device_independent_stream() {
    let Some(fixture) = crate::rules::retail_ini_fixture::retail_battle_rules() else {
        return;
    };
    let native = corpus();
    for (type_id, kind, category) in [
        ("E1", "infantry", EntityCategory::Infantry),
        ("MTNK", "unit", EntityCategory::Unit),
    ] {
        let object = fixture.rules.object(type_id).unwrap();
        assert!(
            !object.voice_select.is_empty(),
            "retail {type_id} has a selection voice"
        );
        // Bind native's supplied numeric Voc identities to the actual fixed
        // SOUNDMD names at the same slots. The original execution supplies
        // the chosen slot and full Main RNG continuation; no modulo oracle
        // is calculated in this test.
        let history = native["voice_histories"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| {
                row["kind"] == kind
                    && row["seed"] == 1
                    && row["voice_enabled"].is_null()
                    && row["owner"].is_null()
                    && row["slave"].is_null()
                    && row["robot_offline"].is_null()
                    && row["voice_list"].as_array().unwrap().len() == object.voice_select.len()
            })
            .expect("executed native list cardinality for retail common path");
        let slots = history["voice_list"].as_array().unwrap();
        let sim = simulation(type_id, category, "Local", false);
        let before_sim = sim.scenario_rng.native_state_hex();
        let mut runtime = AppAudioRuntime::new(None, None, SfxRng::seeded(1), false);
        assert!(runtime.sfx_player.is_none());
        for step in history["steps"].as_array().unwrap() {
            let request = &step["voice_requests"][0];
            let slot = slots
                .iter()
                .position(|id| *id == request["sound_id"])
                .unwrap();
            let event = selection_voice_event(&sim, &fixture.rules, 1, true, runtime.random_mut())
                .expect("normal retail selection queues its resolved voice without output");
            assert_eq!(event.sound_id(), object.voice_select[slot]);
        }
        assert_eq!(
            runtime.random_mut().native_state_hex(),
            history["rng_after_hex"]["main"].as_str().unwrap()
        );
        assert_eq!(sim.scenario_rng.native_state_hex(), before_sim);
    }
}
