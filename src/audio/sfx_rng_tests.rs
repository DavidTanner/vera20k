//! Original Random65C780 / RandomRanged65C7E0 execution pins both owners
//! after the shared reduction extraction, including exact trace draw counts.

use super::{SampleRng, SfxRng};
use crate::sim::rng::{SimRng, trace_draws};

#[test]
fn shared_audio_handles_and_scenario_reduction_match_executed_native_histories() {
    let native: serde_json::Value = serde_json::from_str(crate::test_fixture::text(
        "tools/rmg_oracle/vectors/rng.json",
    ))
    .unwrap();
    let mut compared = 0;
    for history in native["ranged_cases"].as_array().unwrap() {
        let label = history["id"].as_str().unwrap();
        let seed = history["seed"].as_u64().unwrap();
        let mut random = SfxRng::seeded(seed);
        let mut other_audio_consumer = random.clone();
        let mut scenario = SimRng::new(seed);
        for _ in 0..history["advance_raw"].as_u64().unwrap() {
            let _ = random.next_u32();
            let _ = scenario.next_u32();
        }
        assert_eq!(
            random.native_state_hex(),
            history["initial_state_hex"].as_str().unwrap(),
            "{label}"
        );
        assert_eq!(
            scenario.native_state_hex(),
            history["initial_state_hex"].as_str().unwrap(),
            "{label}"
        );
        for (index, step) in history["steps"].as_array().unwrap().iter().enumerate() {
            let before = step["before_state_hex"].as_str().unwrap();
            assert_eq!(random.native_state_hex(), before, "{label}/{index}");
            assert_eq!(scenario.native_state_hex(), before, "{label}/{index}");
            // Alternate two real handles as input, Theme and playback do;
            // cloning a generator instead of its handle fails at step1.
            let audio = if index % 2 == 0 {
                &mut random
            } else {
                &mut other_audio_consumer
            };
            let (actual, (sim_actual, trace)) = match step["kind"].as_str().unwrap() {
                "raw" => (
                    i64::from(audio.next_u32()),
                    trace_draws(|| i64::from(scenario.next_u32())),
                ),
                "ranged" => {
                    let low = step["low"].as_i64().unwrap() as i32;
                    let high = step["high"].as_i64().unwrap() as i32;
                    (
                        i64::from(audio.ranged(low, high)),
                        trace_draws(|| i64::from(scenario.next_range_i32_inclusive(low, high))),
                    )
                }
                kind => panic!("unhandled native draw {kind}"),
            };
            let expected = step["result"].as_i64().unwrap();
            assert_eq!(actual, expected, "audio {label}/{index}");
            assert_eq!(sim_actual, expected, "scenario {label}/{index}");
            assert_eq!(
                trace.len() as u64,
                step["raw_draw_count"].as_u64().unwrap(),
                "trace {label}/{index}"
            );
            assert_eq!(
                trace
                    .iter()
                    .map(|draw| draw["value"].clone())
                    .collect::<Vec<_>>(),
                *step["raw_draws"].as_array().unwrap(),
                "raw trace {label}/{index}",
            );
            let after = step["after_state_hex"].as_str().unwrap();
            assert_eq!(random.native_state_hex(), after, "audio {label}/{index}");
            assert_eq!(
                other_audio_consumer.native_state_hex(),
                after,
                "shared {label}/{index}"
            );
            assert_eq!(
                scenario.native_state_hex(),
                after,
                "scenario {label}/{index}"
            );
            compared += 1;
        }
    }
    assert_eq!(compared, 98);
}
