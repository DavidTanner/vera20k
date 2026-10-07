//! Replays `tools/spatial_oracle/aircraft_idle_retreat.json`'s `enter_idle`
//! rows: the original `0x004176F0` up to its airborne arm.

use super::*;
use serde_json::{Value, json};

fn rows() -> Vec<Value> {
    let oracle: Value = serde_json::from_str(crate::test_fixture::text(
        "tools/spatial_oracle/aircraft_idle_retreat.json",
    ))
    .expect("aircraft_idle_retreat.json");
    oracle["enter_idle"].as_array().unwrap().clone()
}

fn flag(input: &Value, key: &str) -> bool {
    input[key].as_bool().unwrap()
}

/// Logs each call as the oracle's stubs and hooks do, answers from the row,
/// and keeps Mission+0xBC and `+0x6D2` (the rows preset 7 and 1).
struct Recorder<'a> {
    input: &'a Value,
    calls: Vec<Value>,
    state: i64,
    latch: i64,
}

impl IdleHost for Recorder<'_> {
    fn restore(&mut self) -> MissionId {
        self.calls.push(json!(["restore"]));
        // The oracle's Restore stub moves the suspended selector to current.
        MissionId::from_raw(self.input["suspended"].as_i64().unwrap() as i32)
    }

    fn restart_patrol(&mut self) {
        self.state = 0;
        self.latch = 0;
    }

    fn commence(&mut self) {
        self.calls.push(json!(["commence"]));
    }

    fn foot_enter_idle(&mut self) -> bool {
        self.calls.push(json!(["foot_enter_idle", 0, 1]));
        flag(self.input, "foot")
    }

    fn clear_target(&mut self) {
        self.calls.push(json!(["assign_target", null]));
    }

    fn clear_destination(&mut self) {
        self.calls.push(json!(["assign_destination", null, 1]));
    }

    fn in_radio_contact(&mut self) -> bool {
        flag(self.input, "radio")
    }

    fn queue(&mut self, mission: MissionType) {
        self.calls.push(json!(["queue_mission", mission.id(), 0]));
    }

    fn ready(&mut self) -> bool {
        self.calls.push(json!(["ready"]));
        flag(self.input, "ready")
    }
}

/// Every row: the call log, the answer and the head's Patrol writes where the
/// port runs; where VERA's tree stands in, that the original reached the same
/// point: the airborne arm before any call, the dock hunt after the landed
/// arm's clears.
#[test]
fn enter_idle_mode_matches_the_original() {
    let rows = rows();
    assert_eq!(rows.len(), 353);
    for row in &rows {
        let input = &row["input"];
        let name = input["name"].as_str().unwrap();
        let int = |key: &str| input[key].as_i64().unwrap();
        let mission = |key: &str| MissionId::from_raw(int(key) as i32);
        // `0x004177BA..0x004177FC`.
        let landed =
            int("layer") == 2 || int("height") <= int("landing") || flag(input, "missile_spawn");
        let facts = IdleFacts {
            suspended: int("suspended") != -1,
            current: mission("current"),
            queued: mission("queued"),
            airstrike: flag(input, "airstrike"),
            mission_only: flag(input, "mission_only"),
            passengers: flag(input, "passengers"),
            team: flag(input, "team"),
            human: flag(input, "human"),
            armed: flag(input, "armed"),
            ammo: int("ammo") as i32,
            landed,
        };
        let mut host = Recorder {
            input,
            calls: Vec::new(),
            state: int("state"),
            latch: int("latch"),
        };
        let answer = enter_idle_mode(&facts, &mut host);
        let native = row["calls"].as_array().unwrap();
        let reached = |call: &str| native.iter().position(|logged| logged[0] == call);
        // The original's own test agrees: the airborne arm only off the
        // ground and above the landing altitude, after the Foot base.
        assert_eq!(
            reached("airborne_arm").is_some(),
            !landed && reached("foot_enter_idle").is_some(),
            "{name}: the airborne arm"
        );
        match answer {
            None if !landed => assert!(host.calls.is_empty(), "{name}: the tree stands in"),
            None => {
                let hunt = reached("dock_search").expect("the original hunts a dock");
                assert_eq!(
                    host.calls,
                    native[..hunt],
                    "{name}: calls before the dock hunt"
                );
            }
            Some(answer) => {
                assert_eq!(Value::Array(host.calls), row["calls"], "{name}: calls");
                assert_eq!(u64::from(answer), row["ret"].as_u64().unwrap(), "{name}");
                assert_eq!(
                    (host.state, host.latch),
                    (
                        row["state"].as_i64().unwrap(),
                        row["latch"].as_i64().unwrap()
                    ),
                    "{name}: Mission+0xBC and +0x6D2"
                );
            }
        }
    }
}
