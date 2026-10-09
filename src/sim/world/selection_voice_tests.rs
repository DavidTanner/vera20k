//! Original VoiceSelect708EB0 and QueueVoice708D90 compared through the
//! existing Simulation Main owner, including terrain-prefix continuation.

use super::Simulation;
use crate::sim::rng::trace_draws;

fn sound_name(id: i64) -> String {
    if id == -1 {
        String::new()
    } else {
        format!("Sound{id}")
    }
}

#[test]
fn normal_voice_requests_match_native_rejections_draws_and_complete_main_state() {
    let native: serde_json::Value = serde_json::from_str(crate::test_fixture::text(
        "tools/input_oracle/selection_navigation.json",
    ))
    .unwrap();
    let mut histories = 0;
    let mut calls = 0;
    let mut separate_receivers = 0;
    for history in native["voice_histories"].as_array().unwrap() {
        if history["slave"] == true || history["robot_offline"] == true {
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
        let mut sim = Simulation::with_seed(history["seed"].as_u64().unwrap());
        if let Some(prefix) = history["main_prefix"]["draws"].as_array() {
            assert_eq!(
                sim.main_rng.native_state_hex(),
                history["main_prefix"]["before_state_hex"].as_str().unwrap(),
                "{label}"
            );
            let (_, mut main) = sim.terrain_load_draws();
            for expected in prefix {
                assert_eq!(u64::from(main.next_u32()), expected.as_u64().unwrap());
            }
        }
        assert_eq!(
            sim.main_rng.native_state_hex(),
            history["rng_before_hex"]["main"].as_str().unwrap(),
            "{label}"
        );
        let scenario_before = sim.scenario_rng.logical_state();
        let mapgen_before = sim.mapgen_rng.logical_state();
        let hash_before = sim.state_hash();
        let mut queued = sound_name(history["initial_queued_voice"].as_i64().unwrap());
        for step in history["steps"].as_array().unwrap() {
            let main_before = sim.main_rng.logical_state();
            let (request, draws) = trace_draws(|| {
                sim.normal_selection_voice(
                    &voices,
                    history["voice_enabled"].as_bool().unwrap_or(true),
                    history["owner"].as_str().unwrap_or("local") == "local",
                )
            });
            if let Some(sound_id) = request {
                assert_eq!(
                    sound_id,
                    sound_name(step["voice_requests"][0]["sound_id"].as_i64().unwrap()),
                    "{label}"
                );
                queued = sound_id.to_string();
            }
            assert_eq!(
                queued,
                sound_name(step["queued_voices"]["1"].as_i64().unwrap()),
                "{label}"
            );
            assert_eq!(
                draws
                    .iter()
                    .map(|draw| draw["value"].clone())
                    .collect::<Vec<_>>(),
                *step["main_draws"].as_array().unwrap(),
                "{label}"
            );
            assert_eq!(
                main_before == sim.main_rng.logical_state(),
                step["rng_unchanged"]["main"].as_bool().unwrap(),
                "{label}"
            );
            assert_eq!(sim.scenario_rng.logical_state(), scenario_before, "{label}");
            assert_eq!(sim.mapgen_rng.logical_state(), mapgen_before, "{label}");
            assert_eq!(sim.state_hash(), hash_before, "{label}");
            assert_eq!(step["rng_unchanged"]["scenario"], true, "{label}");
            assert_eq!(step["rng_unchanged"]["mapgen"], true, "{label}");
            calls += 1;
        }
        assert_eq!(
            sim.main_rng.native_state_hex(),
            history["rng_after_hex"]["main"].as_str().unwrap(),
            "{label}"
        );
        for expected in history["main_next_four"].as_array().unwrap() {
            assert_eq!(
                u64::from(sim.main_rng.next_u32()),
                expected.as_u64().unwrap(),
                "{label}"
            );
        }
        histories += 1;
    }
    assert_eq!((histories, calls, separate_receivers), (25, 96, 4));
}
