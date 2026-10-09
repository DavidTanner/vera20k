//! Replay original CompareItems/AddCameo/Recalculate outputs. The fixture
//! supplies type/House inputs; it does not establish full CanBuild timing.

use std::collections::{HashMap, HashSet};
use std::fmt::Write;

use serde::Deserialize;
use serde_json::Value;

use super::SidebarTab;
use super::cameo_order::{Cameo, CameoId, CameoKey, CameoStrips};
use crate::rules::ini_parser::IniFile;
use crate::rules::native_processing::{RulesLayerKind, RulesLayerStack};
use crate::rules::ruleset::{HouseCostFactors, RuleSet};
use crate::sim::intern::StringInterner;
use crate::util::native_x87::NativeF32Bits;

fn corpus() -> Value {
    let corpus: Value = serde_json::from_str(crate::test_fixture::text(
        "tools/sidebar_oracle/cameo_order.json",
    ))
    .unwrap();
    assert_eq!(corpus["schema"], 1);
    corpus
}

#[derive(Deserialize)]
struct NativeType {
    id: String,
    kind: i32,
    side: i32,
    tech_level: i32,
    cost: i32,
    ui_name: String,
    considered_aircraft: bool,
    naval: bool,
    build_cat: i32,
    recharge_frames: i32,
    free_unit: Option<String>,
}

struct Fixture {
    types: Vec<NativeType>,
    rules: RuleSet,
    interner: StringInterner,
    side: i32,
    factors: HouseCostFactors,
}

fn factor_bits(values: &Value) -> [NativeF32Bits; 5] {
    let values = values.as_array().unwrap();
    assert_eq!(values.len(), 5);
    std::array::from_fn(|slot| {
        NativeF32Bits::from_bits((values[slot].as_f64().unwrap() as f32).to_bits())
    })
}

impl Fixture {
    fn new(context: &Value) -> Self {
        let types: Vec<NativeType> = serde_json::from_value(context["types"].clone()).unwrap();
        // Feed raw Cost/FreeUnit/BuildCat inputs to the production reader and
        // Cost_Of owner, including the BuildingType override. No cost formula
        // or ordering implementation is reproduced by this adapter.
        let mut ini = String::from("[General]\nSeparateAircraft=yes\n");
        for (kind, section) in [
            (7, "BuildingTypes"),
            (16, "InfantryTypes"),
            (40, "VehicleTypes"),
            (3, "AircraftTypes"),
        ] {
            writeln!(ini, "[{section}]").unwrap();
            for (index, item) in types.iter().filter(|item| item.kind == kind).enumerate() {
                writeln!(ini, "{index}={}", item.id).unwrap();
            }
        }
        for item in types
            .iter()
            .filter(|item| !matches!(item.kind, 31 | 32 | 57))
        {
            writeln!(ini, "[{}]\nCost={}", item.id, item.cost).unwrap();
            if item.build_cat == 5 {
                writeln!(ini, "BuildCat=Combat").unwrap();
            }
            if let Some(free) = &item.free_unit {
                writeln!(ini, "FreeUnit={free}").unwrap();
            }
        }
        let mut interner = StringInterner::new();
        for item in &types {
            interner.intern(&item.id);
        }
        Self {
            types,
            rules: RuleSet::from_ini(&IniFile::from_str(&ini)).unwrap(),
            interner,
            side: context["current_side"].as_i64().unwrap() as i32,
            factors: HouseCostFactors {
                country: context
                    .get("country")
                    .map_or([NativeF32Bits::ONE; 5], factor_bits),
                factory_plant: context
                    .get("plant")
                    .map_or([NativeF32Bits::ONE; 5], factor_bits),
            },
        }
    }

    fn candidates(&self) -> Vec<Cameo> {
        self.types
            .iter()
            .map(|item| {
                let id = self.interner.get(&item.id).unwrap();
                if matches!(item.kind, 31 | 32 | 57) {
                    Cameo {
                        id: CameoId::SuperWeapon(id),
                        tab: SidebarTab::Defense,
                        available: true,
                        key: CameoKey::SuperWeapon {
                            recharge_frames: item.recharge_frames,
                            name: item.ui_name.clone(),
                        },
                    }
                } else {
                    let vehicle = matches!(item.kind, 3 | 40);
                    Cameo {
                        id: CameoId::Object(id),
                        tab: match item.kind {
                            7 if item.build_cat == 5 => SidebarTab::Defense,
                            7 => SidebarTab::Building,
                            16 => SidebarTab::Infantry,
                            3 | 40 => SidebarTab::Vehicle,
                            kind => panic!("uncovered native kind {kind}"),
                        },
                        available: true,
                        key: CameoKey::Techno {
                            own_side: item.side == self.side,
                            considered_aircraft: vehicle && item.considered_aircraft,
                            naval: vehicle && item.naval,
                            tech_level: item.tech_level,
                            cost: self
                                .rules
                                .cost_of(self.rules.object(&item.id).unwrap(), Some(&self.factors)),
                            name: item.ui_name.clone(),
                        },
                    }
                }
            })
            .collect()
    }

    fn name(&self, id: CameoId) -> &str {
        match id {
            CameoId::Object(id) | CameoId::SuperWeapon(id) => self.interner.resolve(id),
        }
    }

    fn assert_order(&self, strips: &CameoStrips, expected: &Value, label: &str) {
        for tab in SidebarTab::all() {
            let actual: Vec<_> = strips
                .items(tab)
                .iter()
                .map(|item| self.name(item.id))
                .collect();
            assert_eq!(
                serde_json::json!(actual),
                expected[tab.tab_index()],
                "{label}, {tab:?}"
            );
        }
    }

    fn assert_costs(&self, expected: &Value) {
        for (name, value) in expected.as_object().unwrap() {
            assert_eq!(
                self.rules
                    .cost_of(self.rules.object(name).unwrap(), Some(&self.factors)),
                value.as_i64().unwrap() as i32,
                "original Cost_Of for {name} with {:?}",
                self.factors,
            );
        }
    }

    fn assert_comparisons(&self, cases: &Value) {
        let candidates = self.candidates();
        let by_name: HashMap<_, _> = candidates
            .iter()
            .map(|item| (self.name(item.id), &item.key))
            .collect();
        for case in cases.as_array().unwrap() {
            let left = case["left"].as_str().unwrap();
            let right = case["right"].as_str().unwrap();
            assert_eq!(
                by_name[left].inserts_before(by_name[right]),
                case["before_or_equal"].as_bool().unwrap(),
                "original CompareItems: {left} versus {right}",
            );
        }
    }
}

#[test]
fn original_pair_matrix_covers_utf16_ties_flag_overlap_and_key_hierarchy() {
    let corpus = corpus();
    let context = &corpus["comparisons"];
    assert_eq!(context["types"].as_array().unwrap().len(), 33);
    assert_eq!(context["cases"].as_array().unwrap().len(), 1089);
    Fixture::new(context).assert_comparisons(&context["cases"]);
}

#[test]
fn original_cost_contexts_use_country_plant_and_free_unit_owners() {
    let corpus = corpus();
    let contexts = corpus["cost_contexts"].as_array().unwrap();
    assert_eq!(contexts.len(), 4);
    for context in contexts {
        let fixture = Fixture::new(context);
        fixture.assert_costs(&context["native_costs"]);
        fixture.assert_comparisons(&context["cases"]);
        assert_eq!(context["cases"].as_array().unwrap().len(), 36);
    }
}

fn replay_history(context: &Value) {
    let mut fixture = Fixture::new(context);
    let mut candidates = fixture.candidates();
    let mut strips = CameoStrips::default();
    let mut unavailable = HashSet::new();
    if let Some(costs) = context.get("initial_native_costs") {
        fixture.assert_costs(costs);
    }
    if let Some(initial) = context["initial_adds"].as_array() {
        for name in initial {
            let item = candidates
                .iter()
                .find(|item| fixture.name(item.id) == name.as_str().unwrap())
                .unwrap();
            assert!(strips.insert(item));
        }
        fixture.assert_order(
            &strips,
            &context["initial_order"],
            "initial capacity admissions",
        );
    }
    for (index, step) in context["steps"].as_array().unwrap().iter().enumerate() {
        let action = step["action"].as_str().unwrap();
        let label = format!("{} step {index}: {action}", context["id"]);
        match action {
            "add" => {
                let name = step["id"].as_str().unwrap();
                let item = candidates
                    .iter()
                    .find(|item| fixture.name(item.id) == name)
                    .unwrap();
                assert_eq!(
                    strips.insert(item),
                    step["accepted"].as_bool().unwrap(),
                    "{label}, {name}"
                );
            }
            "plant_factors" => {
                fixture.factors.factory_plant = factor_bits(&step["values"]);
                fixture.assert_costs(&step["native_costs"]);
                candidates = fixture.candidates();
                strips.refresh_keys(&candidates);
            }
            "factory_available" | "can_build" | "super_granted" => {
                let name = step["id"].as_str().unwrap();
                let id = candidates
                    .iter()
                    .find(|item| fixture.name(item.id) == name)
                    .unwrap()
                    .id;
                let available = step["value"]
                    .as_bool()
                    .unwrap_or_else(|| step["value"].as_i64().unwrap() != 0);
                if available {
                    unavailable.remove(&id);
                } else {
                    unavailable.insert(id);
                }
            }
            "recalculate" => {
                let tab = step["tab"].as_u64().unwrap() as usize;
                strips.retain(|id| {
                    let item = candidates.iter().find(|item| item.id == id).unwrap();
                    item.tab.tab_index() != tab || !unavailable.contains(&id)
                });
            }
            "init_clear" => strips = CameoStrips::default(),
            other => panic!("uncovered native history action {other}"),
        }
        if let Some(order) = step.get("order") {
            fixture.assert_order(&strips, order, &label);
        }
    }
}

#[test]
fn original_histories_preserve_survivors_and_reinsert_using_current_costs() {
    let corpus = corpus();
    let histories = corpus["histories"].as_array().unwrap();
    assert_eq!(histories.len(), 3);
    for history in histories {
        replay_history(history);
    }
}

#[test]
fn original_capacity_gate_and_duplicate_rejection_do_not_move_entries() {
    replay_history(&corpus()["capacity_history"]);
}

#[test]
fn reconciliation_adds_before_pruning_and_retries_on_the_next_recheck() {
    let corpus = corpus();
    let context = &corpus["capacity_history"];
    let fixture = Fixture::new(context);
    let mut candidates = fixture.candidates();
    let initial: HashSet<_> = context["initial_adds"]
        .as_array()
        .unwrap()
        .iter()
        .map(|name| name.as_str().unwrap())
        .collect();
    for item in &mut candidates {
        item.available = initial.contains(fixture.name(item.id));
    }
    let mut strips = CameoStrips::default();
    assert!(strips.reconcile(&candidates));
    fixture.assert_order(&strips, &context["initial_order"], "initial reconciliation");

    let steps = context["steps"].as_array().unwrap();
    for item in &mut candidates {
        item.available = fixture.name(item.id) != steps[0]["id"].as_str().unwrap();
    }
    assert!(
        !strips.reconcile(&candidates),
        "full strip rejects the new item before removing the old one"
    );
    fixture.assert_order(&strips, &steps[2]["order"], "first reconciliation");
    assert!(strips.reconcile(&candidates));
    fixture.assert_order(&strips, &steps[3]["order"], "second reconciliation");
}

#[test]
fn original_super_recharge_input_keeps_reader_history_and_native_rounding() {
    let corpus = corpus();
    let reader = &corpus["recharge_reader"];
    let mut layers = RulesLayerStack::new(IniFile::from_str(
        "[SuperWeaponTypes]\n0=TESTSW\n[TESTSW]\nType=LightningStorm\n",
    ));
    let read_frames = |layers: &RulesLayerStack| {
        RuleSet::from_rules_layers(layers)
            .unwrap()
            .super_weapon("TESTSW")
            .unwrap()
            .recharge_time_frames
    };
    assert_eq!(
        i64::from(read_frames(&layers)),
        reader["constructor_frames"]
    );
    for case in reader["cases"].as_array().unwrap() {
        assert_eq!(i64::from(read_frames(&layers)), case["before"]);
        let text = case["raw"].as_str().map_or_else(
            || "[TESTSW]\n".to_string(),
            |raw| format!("[TESTSW]\nRechargeTime={raw}\n"),
        );
        layers.push(RulesLayerKind::Scenario, IniFile::from_str(&text));
        assert_eq!(
            i64::from(read_frames(&layers)),
            case["recharge_frames"],
            "original retained RechargeTime reader: {case}",
        );
    }
}
