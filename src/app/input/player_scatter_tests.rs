//! Selected Scatter controls execute original536D80/730FE0/6FFE00/4C65E0.
//! The shared ground-input fixture supplies actor construction/flat terrain;
//! these checks cover input order, event bytes and cue admission, not movement.

use super::queue_scatter_orders;
use crate::audio::events::GameSoundEvent;
use crate::rules::{area_guard_tests as native, retail_ini_fixture};
use crate::sim::command::{
    Command, CommandRecord, DEPLOY_OPCODE, IDLE_OPCODE, PRIMARY_OPCODE, REPAIR_OPCODE,
    SCATTER_OPCODE, SELL_OPCODE, TargetEventRecord,
};
use serde_json::Value;

fn corpus() -> Value {
    serde_json::from_str(crate::test_fixture::text(
        "tools/input_oracle/player_scatter.json",
    ))
    .unwrap()
}

#[test]
#[ignore = "slow; run when changing what it covers"]
fn player_scatter_matches_native_selected_order_cue_and_no_voice_rng() {
    let controls = corpus();
    let shared = native::native();
    let mut checked = 0;
    for row in controls["rows"].as_array().unwrap() {
        let Some(retail) = retail_ini_fixture::retail_battle_rules() else {
            return;
        };
        let mut f = super::area_guard_tests::fixture(&shared, row, retail);
        let selected: Vec<_> = row["input"]["selection"]
            .as_array()
            .unwrap()
            .iter()
            .map(|name| {
                name.as_str()
                    .and_then(|name| f.ids.get(name))
                    .copied()
                    .unwrap_or(u64::MAX)
            })
            .collect();
        let boundary = &row["boundaries"][0];
        let last = boundary["last_sequence"].as_u64().unwrap();
        let calls: Vec<_> = row["ordered_calls"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|call| call["sequence"].as_u64().unwrap() <= last)
            .collect();
        let before_rng = f.sim.rng_state();
        let sounds = queue_scatter_orders(&mut f.sim, &f.rules, &selected);
        assert_eq!(
            boundary["before"]["rng"], boundary["after"]["rng"],
            "{}: native input has no RNG draws",
            row["name"]
        );
        assert_eq!(
            f.sim.rng_state(),
            before_rng,
            "{}: input never draws voice RNG",
            row["name"]
        );
        let expected_sounds: Vec<_> = calls
            .iter()
            .filter(|call| call["kind"] == "ScatterSound")
            .map(|call| call["sound_name"].as_str().unwrap())
            .collect();
        let actual_sounds: Vec<_> = sounds
            .iter()
            .map(|event| {
                let GameSoundEvent::UiSound { sound_id } = event else {
                    panic!("Scatter must only emit its centered command cue")
                };
                sound_id.as_str()
            })
            .collect();
        assert_eq!(
            actual_sounds, expected_sounds,
            "{}: centered cue",
            row["name"]
        );
        let expected_ids: Vec<_> = calls
            .iter()
            .filter(|call| call["kind"] == "TargetEvent_Construct")
            .map(|call| f.native_ids[&(call["args"][2].as_u64().unwrap())])
            .collect();
        let pending = f.sim.pending_command_snapshot(None);
        let actual_ids: Vec<_> = pending
            .iter()
            .map(|cmd| {
                assert_eq!(cmd.owner, f.sim.session.current_house.unwrap());
                let Command::Scatter { entity_id } = cmd.payload else {
                    panic!("not a Scatter event");
                };
                let record = f
                    .sim
                    .encode_target_event_record(
                        cmd.owner,
                        crate::sim::command::SCATTER_OPCODE,
                        entity_id,
                    )
                    .unwrap();
                assert_eq!(
                    TargetEventRecord::decode(&record).unwrap().entity_id as u64,
                    entity_id
                );
                entity_id
            })
            .collect();
        // Native full-OutList still constructs both events and cues; the
        // current offline semantic ingress is not its bounded queue owner.
        // Compare attempted records here, never claim overflow integration.
        assert_eq!(
            actual_ids, expected_ids,
            "{}: ordered attempted events",
            row["name"]
        );
        checked += 1;
    }
    assert_eq!(checked, 8);
}

#[test]
fn player_scatter_sound_uses_the_full_retail_layered_reader() {
    let empty = crate::rules::ruleset::RuleSet::from_ini(
        &crate::rules::ini_parser::IniFile::from_str("[General]\n"),
    )
    .unwrap();
    assert_eq!(
        empty.general.scatter_sound, None,
        "original66585F constructor sentinel -1"
    );
    let Some(retail) = retail_ini_fixture::retail_battle_rules() else {
        return;
    };
    let controls = corpus();
    let last = controls["command_sound_layers"]
        .as_array()
        .unwrap()
        .last()
        .unwrap();
    assert_eq!(
        retail.rules.general.scatter_sound.as_deref(),
        last["guard"]["name"].as_str()
    );
}

#[test]
fn player_scatter_constructor_preserves_native_padding_and_house_null_semantics() {
    for row in corpus()["constructor_rows"].as_array().unwrap() {
        let decode = |key: &str| {
            let text = row[key].as_str().unwrap();
            let bytes: Vec<_> = text
                .as_bytes()
                .as_chunks::<2>()
                .0
                .iter()
                .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
                .collect();
            CommandRecord::decode_exact(&bytes).unwrap()
        };
        let typed = |opcode| TargetEventRecord {
            opcode,
            house_id: row["args"][0].as_u64().unwrap() as i8,
            frame: row["frame"].as_u64().unwrap() as i32,
            entity_id: row["args"][2].as_u64().unwrap() as i32,
        };
        let mut record = decode("before");
        typed(SCATTER_OPCODE).write_into(&mut record);
        assert_eq!(record, decode("after"), "{}", row["name"]);
        // 0x004C661A stores the event-type argument as the opcode byte and
        // nothing else depends on it, so PRIMARY (Building ClickedAction
        // `0x00443494`), IDLE and DEPLOY (ClickedEvent `0x006FFE34`), and
        // REPAIR and SELL (`0x004AC187`) differ from these rows only there.
        for opcode in [
            PRIMARY_OPCODE,
            IDLE_OPCODE,
            DEPLOY_OPCODE,
            REPAIR_OPCODE,
            SELL_OPCODE,
        ] {
            let mut issued = decode("before");
            typed(opcode).write_into(&mut issued);
            let mut expected = decode("after");
            if expected.house_id() >= 0 {
                expected.set_issue_header(
                    opcode,
                    i32::from(expected.house_id()),
                    expected.frame_stamp(),
                );
            }
            assert_eq!(issued, expected, "{} as {opcode:#x}", row["name"]);
            let decoded = TargetEventRecord::decode(&issued);
            if issued.house_id() >= 0 {
                assert_eq!(decoded, Some(typed(opcode)), "{}", row["name"]);
            }
        }
    }
}
