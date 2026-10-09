//! Replays `tools/spatial_oracle/airfield_reload.json`: the original
//! `BuildingClass::Mission_Repair @ 0x0044B780` on a `UnitReload=` airfield,
//! one call per row, through [`reload_visit`].

use super::*;
use serde_json::{Value, json};

fn rows() -> Vec<Value> {
    let oracle: Value = serde_json::from_str(crate::test_fixture::text(
        "tools/spatial_oracle/airfield_reload.json",
    ))
    .expect("airfield_reload.json");
    oracle["mission_repair"].as_array().unwrap().clone()
}

/// The oracle's contacts; a contact's id is its position plus one.
const CONTACTS: [&str; 4] = ["a", "b", "c", "d"];

fn name(contact: u64) -> &'static str {
    CONTACTS[contact as usize - 1]
}

/// Answers each call from the row and logs it as the oracle logs the native
/// call or read it stands for: `capacity` is a read of `+0xE8`, `release`
/// the three calls it makes.
struct Recorder<'a> {
    input: &'a Value,
    count: usize,
    mission_reads: [usize; 4],
    calls: Vec<String>,
}

impl<'a> Recorder<'a> {
    fn contact_input(&self, contact: u64) -> &'a Value {
        &self.input["contacts"][name(contact)]
    }
}

impl ReloadHost for Recorder<'_> {
    fn capacity(&mut self) -> usize {
        self.calls.push("count".into());
        self.count
    }

    fn contact(&mut self, slot: usize) -> Option<u64> {
        self.calls.push(format!("contact {slot}"));
        let held = self.input["slots"][slot].as_str()?;
        let index = CONTACTS.iter().position(|contact| *contact == held);
        Some(index.unwrap() as u64 + 1)
    }

    fn roger(&mut self, contact: u64, message: RadioMessage) -> bool {
        let code = format!("0x{:02X}", message.code());
        let contact_name = name(contact);
        self.calls.push(format!("transmit {code} {contact_name}"));
        self.contact_input(contact)["answers"][&code].as_i64()
            == Some(i64::from(RadioResponse::Roger.code()))
    }

    fn full_strength(&mut self, contact: u64) -> bool {
        self.calls.push(format!("full_strength {}", name(contact)));
        self.contact_input(contact)["health"].as_i64() == self.input["strength"].as_i64()
    }

    fn mission(&mut self, contact: u64) -> MissionId {
        self.calls.push(format!("mission {}", name(contact)));
        let missions = self.contact_input(contact)["missions"].as_array().unwrap();
        let read = &mut self.mission_reads[contact as usize - 1];
        let answer = missions[(*read).min(missions.len() - 1)].as_i64().unwrap();
        *read += 1;
        MissionId::from_raw(answer as i32)
    }

    fn queue_sleep(&mut self, contact: u64) {
        let sleep = MissionId::from_known(MissionType::Sleep).raw();
        self.calls
            .push(format!("queue_mission {} {sleep} 0", name(contact)));
    }

    fn release(&mut self, contact: u64) {
        let guard = MissionId::from_known(MissionType::Guard).raw();
        let contact = name(contact);
        self.calls.push(format!("enter_idle_mode {contact} 0 1"));
        self.calls.push(format!("assign_mission {contact} {guard}"));
        self.calls.push(format!("vt334 {contact}"));
        let recount = &self.input["recount"];
        if recount["contact"] == contact {
            self.count = recount["count"].as_u64().unwrap() as usize;
        }
    }

    fn queue_guard(&mut self) {
        let guard = MissionId::from_known(MissionType::Guard).raw();
        self.calls.push(format!("queue_mission airfield {guard} 0"));
    }
}

/// Every row: the call log and the returned frames.
#[test]
fn reload_visit_matches_the_original() {
    let rows = rows();
    assert_eq!(rows.len(), 304);
    let mut disagreements = Vec::new();
    for row in &rows {
        let input = &row["input"];
        let bits = input["reload_rate"].as_str().unwrap();
        let reload_rate = f64::from_bits(u64::from_str_radix(&bits[2..], 16).unwrap());
        let mut host = Recorder {
            input,
            count: input["count"].as_u64().unwrap() as usize,
            mission_reads: [0; 4],
            calls: Vec::new(),
        };
        let frames = reload_visit(reload_rate, &mut host);
        let rust = json!({ "ret": frames, "calls": host.calls });
        let native = json!({ "ret": row["ret"], "calls": row["calls"] });
        if rust != native {
            disagreements.push(format!(
                "{}:\n  native {native}\n  rust   {rust}",
                input["name"]
            ));
        }
    }
    assert!(
        disagreements.is_empty(),
        "{} of {} rows disagree:\n{}",
        disagreements.len(),
        rows.len(),
        disagreements.join("\n")
    );
}
