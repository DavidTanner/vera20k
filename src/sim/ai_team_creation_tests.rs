//! Replay of `tools/ai_team_oracle.json` against the computer's team
//! creation (`sim::ai_team_creation`) and its unit choices
//! (`sim::ai_unit_choice`).
//!
//! Each row carries what the native run's stubs answered (CanBuild, Cost_Of
//! and the money, IsRecruitable, the selector's eligibility test, the
//! draws); the Rust owner must ask in the same order and reach the same
//! result. The teams a chooser tallies are those the original asked for
//! their needs (`Get_Needed_Types`); which teams qualify
//! (`TeamScriptState::wants_members`) rests on instruction reading, as do
//! the eligibility gates these rows leave open.

use std::hash::{DefaultHasher, Hasher};

use super::*;
use crate::rules::ini_parser::IniFile;
use crate::sim::ai_unit_choice::{self, HarvesterFacts};
use crate::sim::house_state::{HouseDifficulty, HouseState};
use crate::sim::production::CanBuild;
use crate::sim::rng::SimRng;
use crate::sim::team_script_vm::{TeamScriptDefinition, TeamTaskForceDefinition};
use crate::sim::timer::CdTimer;
use crate::util::native_x87::NativeF64Bits;
use serde_json::Value;

fn rows(section: &str) -> Vec<Value> {
    let oracle: Value =
        serde_json::from_str(include_str!("../../tools/ai_team_oracle.json")).unwrap();
    oracle[section].as_array().unwrap().clone()
}

/// A dword as the original held it; the stubs record arguments unsigned.
fn dword(value: &Value) -> i32 {
    value.as_i64().unwrap() as u32 as i32
}

fn dwords(value: &Value) -> Vec<i32> {
    value.as_array().unwrap().iter().map(dword).collect()
}

fn flag(value: &Value) -> bool {
    value.as_bool().unwrap()
}

fn index(value: &Value) -> usize {
    usize::try_from(value.as_u64().unwrap()).unwrap()
}

fn events<'a>(row: &'a Value, kind: &str) -> Vec<&'a Value> {
    row["events"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|event| event[0] == kind)
        .collect()
}

/// The original's draws, `(low, high, answer)`, all on the Scenario RNG
/// (`Scenario+0x218`).
fn native_draws(row: &Value) -> Vec<(i32, i32, i32)> {
    events(row, "draw")
        .into_iter()
        .map(|draw| {
            assert_eq!(draw[1], 0x218, "a draw off the Scenario RNG");
            (dword(&draw[2]), dword(&draw[3]), dword(&draw[4]))
        })
        .collect()
}

/// `RandomRanged` answering a row's `answers` in order.
struct Draws {
    answers: std::vec::IntoIter<i32>,
    asked: Vec<(i32, i32, i32)>,
}

impl Draws {
    fn new(row: &Value) -> Self {
        Self {
            answers: dwords(&row["answers"]).into_iter(),
            asked: Vec::new(),
        }
    }

    fn draw(&mut self, low: i32, high: i32) -> i32 {
        let value = self.answers.next().expect("an answer for every draw");
        self.asked.push((low, high, value));
        value
    }
}

#[test]
fn the_choosers_match_the_original() {
    for (number, row) in rows("choosers").iter().enumerate() {
        let context = format!("{} chooser row {number}", row["kind"]);
        let teams = row["teams"].as_array().unwrap();
        let types = row["types"].as_array().unwrap();
        let tallied: Vec<(i32, Vec<usize>)> = events(row, "needed")
            .into_iter()
            .map(|asked| {
                let team = &teams[index(&asked[1])];
                // A needed type of another class (`["other", n]`) is not
                // this chooser's.
                let needed = team["needed"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter_map(Value::as_u64)
                    .map(|class_index| class_index as usize)
                    .collect();
                (dword(&team["created"]), needed)
            })
            .collect();
        let free: Vec<usize> = row["objects"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|object| flag(&object[1]))
            .map(|object| index(&object[0]))
            .collect();
        let mut asked_can_build = Vec::new();
        let candidates = ai_unit_choice::tally(
            types.len(),
            tallied,
            free,
            dword(&row["money"]),
            |class_index| {
                asked_can_build.push(class_index);
                match dword(&types[class_index][0]) {
                    0 => CanBuild::No,
                    1 => CanBuild::Yes,
                    -1 => CanBuild::AtLimit,
                    other => panic!("CanBuild answers {other}"),
                }
            },
            |class_index| dword(&types[class_index][1]),
        );
        let native_can_build: Vec<usize> = events(row, "can_build")
            .into_iter()
            .map(|asked| index(&asked[1]))
            .collect();
        assert_eq!(asked_can_build, native_can_build, "{context}: CanBuild");
        let fill = dword(&row["fill"][index(&row["difficulty"])]);
        let mut draws = Draws::new(row);
        let choice = ai_unit_choice::pick(&candidates, fill, |low, high| draws.draw(low, high));
        assert_eq!(draws.asked, native_draws(row), "{context}: draws");
        assert_eq!(choice.unwrap_or(-1), dword(&row["choice"]), "{context}");
    }
}

#[test]
fn the_harvester_branch_matches_the_original() {
    let mut sim = Simulation::new();
    let name = sim.interner.intern("H0");
    for (number, row) in rows("harvester").iter().enumerate() {
        let mut house = HouseState::new(name, 0, None, flag(&row["human"]), 0, dword(&row["tech"]));
        house.player_control = flag(&row["control"]);
        // The fixture's harvester is vehicle 7; its refinery undeploys into
        // vehicle 9.
        let facts = HarvesterFacts {
            harvester: (flag(&row["harvester"]) && flag(&row["owned"]))
                .then(|| (7, dword(&row["harvester_tech"]))),
            refinery: flag(&row["refinery"]).then(|| flag(&row["undeploys"]).then_some(9)),
            gatherers: dword(&row["gatherers"]),
            destinations: dword(&row["destinations"]),
            harvesters_per_refinery: dword(&row["per_refinery"]),
            slave_miners: dword(&row["slave_miners"]),
            current_iq: dword(&row["iq"]),
            iq_harvester: dword(&row["iq_harvester"]),
            no_ore: flag(&row["no_ore"]),
            human: house.is_controlled_by_human(dword(&row["game_mode"]) != 0),
            tech_level: house.tech_level,
        };
        let decision = ai_unit_choice::harvester_decision(&facts);
        assert_eq!(
            decision.unwrap_or(-1),
            dword(&row["choice"]),
            "harvester row {number}"
        );
        // Past the branch, the team pass (no candidates here) draws once.
        assert_eq!(
            native_draws(row).len(),
            usize::from(decision.is_none()),
            "harvester row {number}: draws"
        );
    }
}

/// An AI trigger `id` of `weight` whose bounds pin that weight, so the
/// feedback of an evicted team (`TeamClass::~TeamClass`, which the oracle
/// does not run) leaves it.
fn ai_trigger(
    id: InternedId,
    primary: InternedId,
    secondary: Option<InternedId>,
    weight: NativeF64Bits,
) -> TeamAiTriggerDefinition {
    TeamAiTriggerDefinition {
        id,
        tokens: Default::default(),
        display_name: String::new(),
        enabled: true,
        primary_team_type: Some(primary),
        owner: Some(TeamAiTriggerOwner::All),
        threshold: 0,
        condition: -1,
        object_type: None,
        comparison_mask: [0; 32],
        weights: [weight; 3],
        multiplayer: true,
        side: 0,
        storage_flag_d1: false,
        secondary_team_type: secondary,
        difficulty_enabled: [true; 3],
        source: TeamAiDefinitionSource::FixedAimd,
    }
}

/// A selector row's world: the house `H0` and its enemy `H1`, a TeamType
/// per `team_types` entry (its `IsBaseDefense=`), the row's teams in order
/// (forming unless `formed`) and its AI triggers.
struct SelectorFixture {
    sim: Simulation,
    rules: RuleSet,
    owner: InternedId,
    enemy: InternedId,
    team_types: Vec<InternedId>,
    triggers: Vec<InternedId>,
    teams: Vec<u64>,
}

fn selector_fixture(row: &Value) -> SelectorFixture {
    let mut rules = RuleSet::from_ini(&IniFile::from_str("")).unwrap();
    rules.general.total_ai_team_cap = dwords(&row["cap"]);
    rules.general.maximum_ai_defensive_teams = dwords(&row["max_defense"]);
    let mut sim = Simulation::new();
    sim.session.game_mode_nonzero = true;
    let owner = sim.interner.intern("H0");
    let enemy = sim.interner.intern("H1");
    for name in [owner, enemy] {
        sim.houses
            .insert(name, HouseState::new(name, 0, None, false, 0, 10));
    }
    sim.session.house_order = vec![owner, enemy];
    let house = sim.houses.get_mut(&owner).unwrap();
    house.difficulty = HouseDifficulty::from_native(dword(&row["difficulty"])).unwrap();
    house.ratio_ai_trigger_team = dword(&row["ratio"]);
    house.ai_activation.ai_triggers_active = flag(&row["active"]);
    house.enemy_house = flag(&row["enemy"]).then_some(enemy);
    let script = sim.interner.intern("SCRIPT");
    let task_force = sim.interner.intern("TASKFORCE");
    sim.team_script_vm.register_script(TeamScriptDefinition {
        id: script,
        actions: Vec::new(),
        source: TeamAiDefinitionSource::FixedAimd,
    });
    sim.team_script_vm
        .register_task_force(TeamTaskForceDefinition {
            id: task_force,
            group: -1,
            entries: Vec::new(),
            source: TeamAiDefinitionSource::FixedAimd,
        });
    let mut team_types = Vec::new();
    for (slot, base_defense) in row["team_types"].as_array().unwrap().iter().enumerate() {
        let id = sim.interner.intern(&format!("TEAMTYPE{slot}"));
        sim.team_script_vm.register_team_type(TeamTypeDefinition {
            id,
            script_id: script,
            task_force_id: task_force,
            priority: 0,
            is_base_defense: flag(base_defense),
            suicide: false,
            aggressive: false,
            combined_movement_zone: MovementZone::Normal,
            base_zone_relation_enforced: false,
            transport_crossing_required: false,
        });
        team_types.push(id);
    }
    let mut teams = Vec::new();
    for team in row["teams"].as_array().unwrap() {
        let team_owner = if team.get("ours").is_none_or(flag) {
            owner
        } else {
            enemy
        };
        let team_type = team_types[index(&team["type"])];
        let created = dword(&team["created"]);
        let vm = &mut sim.team_script_vm;
        teams.push(if team.get("formed").is_some_and(flag) {
            vm.create_team_from_type(team_owner, team_type, &[], None, created)
        } else {
            vm.construct_team(team_type, team_owner, true, created)
                .unwrap()
        });
    }
    let mut triggers = Vec::new();
    for (slot, trigger) in row["triggers"].as_array().unwrap().iter().enumerate() {
        let id = sim.interner.intern(&format!("TRIGGER{slot}"));
        let weight = u64::from_str_radix(trigger[2].as_str().unwrap(), 16).unwrap();
        sim.team_script_vm.register_ai_trigger(ai_trigger(
            id,
            team_types[index(&trigger[0])],
            trigger[1]
                .as_u64()
                .map(|second| team_types[second as usize]),
            NativeF64Bits::from_bits(weight),
        ));
        triggers.push(id);
    }
    SelectorFixture {
        sim,
        rules,
        owner,
        enemy,
        team_types,
        triggers,
        teams,
    }
}

#[test]
fn the_selector_matches_the_original() {
    for (number, row) in rows("selector").iter().enumerate() {
        let context = format!("{} selector row {number}", row["name"]);
        let SelectorFixture {
            mut sim,
            rules,
            owner,
            enemy,
            team_types,
            triggers,
            teams,
        } = selector_fixture(row);
        let answers: Vec<bool> = row["eligible"]
            .as_array()
            .unwrap()
            .iter()
            .map(flag)
            .collect();
        let mut asked = Vec::new();
        let mut draws = Draws::new(row);
        let picked = select_team_types_with(
            &mut sim,
            &rules,
            owner,
            |_, trigger, asked_enemy, defense_full| {
                let slot = triggers.iter().position(|&id| id == trigger.id).unwrap();
                asked.push((slot, asked_enemy, defense_full));
                answers[slot]
            },
            |_, low, high| draws.draw(low, high),
        );
        let slot_of = |id: InternedId| team_types.iter().position(|&tt| tt == id).unwrap();
        let picked: Vec<usize> = picked.into_iter().map(slot_of).collect();
        let native_picked: Vec<usize> = row["picked"]
            .as_array()
            .unwrap()
            .iter()
            .map(index)
            .collect();
        assert_eq!(picked, native_picked, "{context}: picked");
        let autocreate: Vec<i32> = team_types
            .iter()
            .map(|&id| i32::from(sim.team_script_vm.team_type_ini(id).unwrap().autocreate))
            .collect();
        assert_eq!(
            autocreate,
            dwords(&row["autocreate"]),
            "{context}: Autocreate="
        );
        let native_enemy = flag(&row["enemy"]).then_some(enemy);
        let native_asked: Vec<(usize, Option<InternedId>, bool)> = events(row, "eligible")
            .into_iter()
            .map(|call| (index(&call[1]), native_enemy, dword(&call[2]) != 0))
            .collect();
        assert_eq!(asked, native_asked, "{context}: eligibility asked");
        let live: Vec<u64> = sim
            .team_script_vm
            .teams_in_order()
            .map(|team| team.id())
            .collect();
        let destroyed: Vec<usize> = events(row, "destroy")
            .into_iter()
            .map(|call| index(&call[1]))
            .collect();
        let native_live: Vec<u64> = teams
            .iter()
            .enumerate()
            .filter(|(slot, _)| !destroyed.contains(slot))
            .map(|(_, &id)| id)
            .collect();
        assert_eq!(live, native_live, "{context}: teams left");
        assert_eq!(draws.asked, native_draws(row), "{context}: draws");
    }
}

#[test]
fn the_eligibility_arithmetic_matches_the_original() {
    for (number, row) in rows("eligibility").iter().enumerate() {
        let context = format!("{} eligibility row {number}", row["name"]);
        let eligible = flag(&row["eligible"]);
        match row["name"].as_str().unwrap() {
            "defense" => {
                let base_defense =
                    flag(&row["base_defense"]) || row["second"].as_bool() == Some(true);
                // The house plays Normal (index 1).
                let admitted = defense_gate(
                    flag(&row["enemy"]),
                    flag(&row["use_min_rule"]),
                    dword(&row["defense_teams"]),
                    dword(&row["min_defense"][1]),
                    base_defense,
                    flag(&row["flag"]),
                );
                assert_eq!(admitted, eligible, "{context}");
            }
            "compare" => {
                let mut mask = [0u8; 32];
                mask[..4].copy_from_slice(&dword(&row["amount"]).to_le_bytes());
                mask[4..8].copy_from_slice(&dword(&row["comparator"]).to_le_bytes());
                assert_eq!(compare(&mask, dword(&row["money"])), eligible, "{context}");
            }
            "power" => {
                let holds = power_condition_holds(
                    dword(&row["condition"]),
                    dword(&row["output"]),
                    dword(&row["drain"]),
                );
                assert_eq!(holds, eligible, "{context}");
            }
            other => panic!("no eligibility section {other}"),
        }
    }
}

#[test]
fn the_super_weapon_readiness_matches_the_original() {
    for (number, row) in rows("charge").iter().enumerate() {
        let remaining = CdTimer::from_raw(dword(&row["start"]), dword(&row["duration"]))
            .remaining(dword(&row["frame"]));
        let percent = NativeF32Bits::from_bits(dword(&row["percent"]) as u32);
        assert_eq!(
            charge_nearly_full(remaining, dword(&row["recharge"]), percent),
            flag(&row["ready"]),
            "charge row {number}"
        );
    }
}

fn rng_digest(rng: &SimRng) -> u64 {
    let mut hasher = DefaultHasher::new();
    rng.hash_state(&mut hasher);
    hasher.finish()
}

#[test]
fn the_team_block_matches_the_original() {
    for (number, row) in rows("team_block").iter().enumerate() {
        let mut rules = RuleSet::from_ini(&IniFile::from_str("")).unwrap();
        rules.general.team_delays = dwords(&row["delays"]);
        let mut sim = Simulation::new();
        sim.session.game_mode_nonzero = dword(&row["game_mode"]) != 0;
        sim.session.binary_frame = dword(&row["frame"]) as u32;
        let owner = sim.interner.intern("H0");
        let mut house = HouseState::new(owner, 0, None, flag(&row["human"]), 0, 10);
        house.player_control = flag(&row["control"]);
        house.multiplay_passive = flag(&row["passive"]);
        house.difficulty = HouseDifficulty::from_native(dword(&row["difficulty"])).unwrap();
        house.team_timer = CdTimer::from_raw(dword(&row["start"]), dword(&row["duration"]));
        sim.houses.insert(owner, house);
        sim.session.house_order = vec![owner];
        let before = rng_digest(&sim.scenario_rng);
        update_team_creation(&mut sim, &rules, owner);
        let timer = sim.houses[&owner].team_timer;
        assert_eq!(
            vec![timer.start_frame(), timer.duration()],
            dwords(&row["timer"]),
            "team block row {number}: timer"
        );
        // The selector's first draw, `RandomRanged(1, 100)`, marks its call.
        assert_eq!(
            rng_digest(&sim.scenario_rng) != before,
            dword(&row["selected"]) == 1,
            "team block row {number}: selector"
        );
    }
}
