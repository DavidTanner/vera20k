//! Rust regression tests for script actions that act on every member, and
//! for the building actions 46 and 47.

use super::*;
use crate::rules::ini_parser::IniFile;
use crate::rules::team_ai_ini::TeamAiDefinitionSource;
use crate::sim::house_state::HouseState;
use crate::sim::team_script_vm::{TeamScriptAction, TeamScriptDefinition};

const RULES: &str = "\
[InfantryTypes]
0=E1
[BuildingTypes]
0=GAPOWR
[E1]
Strength=125
Speed=4
Locomotor={4A582744-9839-11D1-B709-00A024DDAFD1}
[GAPOWR]
Strength=750
";

fn rules() -> RuleSet {
    RuleSet::from_ini_with_fixed_art_for_test(
        &IniFile::from_str(RULES),
        &IniFile::from_str("[GAPOWR]\nFoundation=2x2\n"),
    )
    .expect("team action rules")
}

/// The building tests' power plants: the Russians' own, the Americans' near
/// and far ones, and the Germans'.
const PLANTS: [(&str, u16, u16); 4] = [
    ("Russians", 13, 13),
    ("Americans", 20, 10),
    ("Americans", 30, 10),
    ("Germans", 16, 14),
];

/// A Russian team of three E1 at (10..=12, 10) running `actions`, with the
/// Americans as the Russians' enemy and power plants of the given owners at
/// the given cells. Answers the team, its members and the plants in order.
fn building_team(
    sim: &mut Simulation,
    rules: &RuleSet,
    actions: &[(i32, i32)],
    plants: &[(&str, u16, u16)],
) -> (u64, Vec<u64>, Vec<u64>) {
    let mut house = |name: &str, side| {
        let id = sim.interner.intern(name);
        sim.houses
            .insert(id, HouseState::new(id, side, None, false, 0, 10));
        sim.session.house_order.push(id);
        id
    };
    let russians = house("Russians", 1);
    let americans = house("Americans", 0);
    house("Germans", 0);
    sim.houses.get_mut(&russians).unwrap().enemy_house = Some(americans);
    let plants = plants
        .iter()
        .map(|&(owner, rx, ry)| {
            sim.spawn_object_at_height("GAPOWR", owner, rx, ry, 0, 0, rules)
                .expect("GAPOWR spawns")
        })
        .collect();
    let members: Vec<u64> = (10..=12)
        .map(|rx| {
            sim.spawn_object_at_height("E1", "Russians", rx, 10, 0, 0, rules)
                .expect("E1 spawns")
        })
        .collect();
    let script = sim.interner.intern("BUILDING SCRIPT");
    sim.team_script_vm.register_script(TeamScriptDefinition {
        id: script,
        source: TeamAiDefinitionSource::FixedAimd,
        actions: actions
            .iter()
            .map(|&(action_id, argument)| TeamScriptAction {
                action_id,
                argument,
            })
            .collect(),
    });
    let team = sim
        .team_script_vm
        .create_team(russians, script, members.clone(), 0);
    (team, members, plants)
}

/// Advance until `done` holds for the team, at most `frames` frames.
fn run_team(
    sim: &mut Simulation,
    rules: &RuleSet,
    grid: &crate::sim::pathfinding::PathGrid,
    team: u64,
    frames: usize,
    done: impl Fn(&crate::sim::team_script_vm::TeamScriptState) -> bool,
) {
    for _ in 0..frames {
        if sim.team_script_vm.team(team).is_none_or(&done) {
            return;
        }
        let commands = sim.take_due_commands();
        sim.advance_tick(&commands, Some(rules), Some(grid), None, 33);
    }
}

/// Action 21 (`0x006E9650`): every member scatters from the null coordinate,
/// forced, so each takes a neighbouring cell to walk to, and the action is
/// finished at once.
#[test]
fn scatter_sends_every_member_to_a_neighbouring_cell() {
    let rules = rules();
    let mut sim = Simulation::with_seed(0x6E9650);
    let owner = sim.interner.intern("Russians");
    sim.houses
        .insert(owner, HouseState::new(owner, 1, None, false, 0, 10));
    sim.session.house_order.push(owner);
    let grid = crate::sim::arena_fixture::flat_arena(&mut sim, &rules);
    let starts = [(10, 10), (11, 10), (12, 10)];
    let members: Vec<u64> = starts
        .iter()
        .map(|&(rx, ry)| {
            sim.spawn_object_at_height("E1", "Russians", rx, ry, 0, 0, &rules)
                .expect("E1 spawns")
        })
        .collect();
    let script = sim.interner.intern("SCATTER SCRIPT");
    sim.team_script_vm.register_script(TeamScriptDefinition {
        id: script,
        source: TeamAiDefinitionSource::FixedAimd,
        actions: vec![
            TeamScriptAction {
                action_id: 21,
                argument: 0,
            },
            TeamScriptAction {
                action_id: 49,
                argument: 0,
            },
        ],
    });
    let team = sim
        .team_script_vm
        .create_team(owner, script, members.clone(), 0);

    for _ in 0..30 {
        if sim
            .team_script_vm
            .team(team)
            .is_some_and(|team| team.cursor() >= 1)
        {
            break;
        }
        let commands = sim.take_due_commands();
        sim.advance_tick(&commands, Some(&rules), Some(&grid), None, 33);
    }

    let state = sim.team_script_vm.team(team).expect("the team lives");
    assert!(state.cursor() >= 1, "action 21 finished");
    assert!(state.refusal().is_none());
    for (member, start) in members.iter().zip(starts) {
        match sim
            .substrate
            .entities
            .get(*member)
            .unwrap()
            .navigation
            .nav_com
        {
            Some(NavTargetRef::Cell { rx, ry }) => assert!(
                rx.abs_diff(start.0) <= 1 && ry.abs_diff(start.1) <= 1 && (rx, ry) != start,
                "member {member} heads for ({rx}, {ry}) from {start:?}"
            ),
            other => panic!("member {member} did not scatter: {other:?}"),
        }
    }
}

/// Through the production readers, the one retail script naming action 21
/// (`0CA2192C-G` "Soviet Flak Defense": 58, 21, 11) runs only ported actions,
/// so the one TeamType running it no longer idles on 21.
#[test]
fn the_retail_scatter_script_runs_ported_actions() {
    use crate::rules::retail_ini_fixture::{retail_ini, retail_rules_and_art};
    use crate::rules::team_ai_ini::TeamAiIniRegistry;
    let Some((rules_ini, art_ini)) = retail_rules_and_art() else {
        return;
    };
    let rules = RuleSet::from_ini_with_fixed_art_for_test(&rules_ini, &art_ini).unwrap();
    let aimd = retail_ini("aimd.ini").expect("retail AIMD");
    let registry = TeamAiIniRegistry::from_sources(&aimd, &IniFile::from_str(""), true);
    let mut sim = Simulation::new();
    sim.intern_rule_type_ids(&rules);
    sim.resolve_type_handles(&rules);
    sim.install_team_ai_registry(&registry, &rules)
        .expect("retail AIMD installs");
    let vm = &sim.team_script_vm;
    let scripts: Vec<_> = vm
        .scripts
        .values()
        .filter(|script| script.actions.iter().any(|action| action.action_id == 21))
        .collect();
    assert_eq!(scripts.len(), 1);
    let actions: Vec<i32> = scripts[0]
        .actions
        .iter()
        .map(|action| action.action_id)
        .collect();
    assert_eq!(actions, [58, 21, 11]);
    assert!(actions.iter().all(|&action| action_is_ported(action)));
    let running = vm
        .team_types
        .values()
        .filter(|team_type| team_type.script_id == scripts[0].id)
        .count();
    assert_eq!(running, 1);
}

/// Action 46 (`0x006EE310`): entered with `first` (after action 24), the team
/// takes the power plant its first member's search picks, skipping its own
/// house's: by mode 2 the nearest, the Germans' (a house it is not allied
/// with); by mode 3 the farthest, the Americans' far one. Its members are
/// sent at it. Mode 4 picks none and the action finishes at once.
#[test]
fn the_team_attacks_the_building_its_first_member_finds() {
    let rules = rules();
    for (mode, pick) in [(2, Some(3)), (3, Some(2)), (4, None)] {
        let mut sim = Simulation::with_seed(0x6EE310);
        let grid = crate::sim::arena_fixture::flat_arena(&mut sim, &rules);
        let (team, members, plants) = building_team(
            &mut sim,
            &rules,
            &[(24, 0), (46, mode << 16), (5, 100)],
            &PLANTS,
        );
        run_team(&mut sim, &rules, &grid, team, 4, |team| {
            team.cursor() >= 2 || team.mission_target.is_some()
        });
        let state = sim.team_script_vm.team(team).expect("the team lives");
        assert!(state.refusal().is_none(), "mode {mode}");
        let Some(pick) = pick else {
            assert_eq!(state.cursor(), 2, "mode {mode}: the action finished");
            assert_eq!(state.mission_target, None, "mode {mode}");
            continue;
        };
        assert_eq!(state.cursor(), 1, "mode {mode}");
        assert_eq!(
            state.mission_target,
            Some(TeamTarget::Object(plants[pick])),
            "mode {mode}"
        );
        let first = sim.substrate.entities.get(members[0]).unwrap();
        assert_eq!(
            first.attack_target.target(),
            Some(crate::sim::combat::TargetKind::Entity(plants[pick])),
            "mode {mode}: the first member's target"
        );
    }
}

/// Action 47 (`0x006EE3F0`): the team's mission target becomes a cell by the
/// nearest power plant not its own (mode 2: the Germans' at (16, 14)), and
/// its members move. Mode 4 finds none, and the action finishes at once.
#[test]
fn the_team_moves_to_the_building_its_first_member_finds() {
    let rules = rules();
    for mode in [2, 4] {
        let mut sim = Simulation::with_seed(0x6EE3F0);
        let grid = crate::sim::arena_fixture::flat_arena(&mut sim, &rules);
        let (team, members, _) = building_team(
            &mut sim,
            &rules,
            &[(24, 0), (47, mode << 16), (5, 100)],
            &PLANTS,
        );
        run_team(&mut sim, &rules, &grid, team, 4, |team| team.cursor() >= 2);
        let state = sim.team_script_vm.team(team).expect("the team lives");
        assert!(state.refusal().is_none(), "mode {mode}");
        if mode == 4 {
            assert_eq!(state.cursor(), 2, "the action finished");
            continue;
        }
        let Some(TeamTarget::Cell { x, y }) = state.mission_target else {
            panic!("mode {mode}: {:?}", state.mission_target);
        };
        assert!(
            x.abs_diff(16) <= 2 && y.abs_diff(14) <= 2,
            "the cell ({x}, {y}) lies by the Germans' plant"
        );
        assert_eq!(state.cursor(), 1, "the team is on its way");
        assert!(
            members.iter().any(|&member| sim
                .substrate
                .entities
                .get(member)
                .unwrap()
                .navigation
                .nav_com
                .is_some()),
            "the members move"
        );
    }
}

/// Run action 46 with `argument` (after action 24) until the team takes a
/// mission target or finishes the action; answers the team's state.
fn attack_building_target(
    sim: &mut Simulation,
    rules: &RuleSet,
    grid: &crate::sim::pathfinding::PathGrid,
    team: u64,
) -> (i32, Option<TeamTarget>) {
    run_team(sim, rules, grid, team, 4, |team| {
        team.cursor() >= 2 || team.mission_target.is_some()
    });
    let state = sim.team_script_vm.team(team).expect("the team lives");
    assert!(state.refusal().is_none());
    (state.cursor(), state.mission_target)
}

/// The threat modes read the threat the Russians hold at each plant's
/// GetCoords cell, one cell south-east of a 2x2 plant's Location
/// (`0x006EEC83..0x006EECDB`), in 4x4 blocks: plant A at (11, 20) reads
/// block (3, 5), plant B at (23, 20) block (6, 5). Without threat the first
/// plant wins either mode, a tie keeping the first. Threat 1000 at block
/// (1, 5) and 400 at (7, 5) spill 500 onto A's Location block and 200 onto
/// B's GetCoords block, so mode 1 (most threat) takes B and mode 0 (least)
/// takes A, where Location reads would pick the other.
#[test]
fn the_threat_modes_read_each_building_s_getcoords_cell() {
    let rules = rules();
    for (threat, mode, pick) in [(false, 0, 0), (false, 1, 0), (true, 0, 0), (true, 1, 1)] {
        let mut sim = Simulation::with_seed(0x6EEBD0);
        let grid = crate::sim::arena_fixture::flat_arena(&mut sim, &rules);
        let plants = [("Americans", 11, 20), ("Americans", 23, 20)];
        let (team, _, plants) = building_team(
            &mut sim,
            &rules,
            &[(24, 0), (46, mode << 16), (5, 100)],
            &plants,
        );
        let russians = sim.interner.intern("Russians");
        let house = sim.houses.get_mut(&russians).unwrap();
        house.clear_spatial_threat();
        if threat {
            house.adjust_spatial_threat((5, 21), 1000).unwrap();
            house.adjust_spatial_threat((29, 21), 400).unwrap();
        }
        let (_, target) = attack_building_target(&mut sim, &rules, &grid, team);
        assert_eq!(
            target,
            Some(TeamTarget::Object(plants[pick])),
            "threat {threat}, mode {mode}"
        );
    }
}

/// The search's admission inputs (`0x006EEC27..0x006EEC6A`, `0x006EE32B`):
/// a house the Russians are allied with is skipped unless it is
/// `MultiplayPassive=`, and with `OnlyTargetHouseEnemy=` only the current
/// enemy's count, none without an enemy. Mode 2 takes the nearest admitted:
/// the Germans' plant at (16, 14), else the Americans' at (20, 10).
#[test]
fn the_building_search_admits_by_alliance_passivity_and_enemy() {
    use crate::sim::team_script_vm::{TeamTaskForceDefinition, TeamTypeDefinition};
    let rules = rules();
    // (allied with the Germans, Germans passive, only the enemy, an enemy)
    let cases = [
        ((false, false, false, true), Some(3)),
        ((true, false, false, true), Some(1)),
        ((true, true, false, true), Some(3)),
        ((false, false, true, true), Some(1)),
        ((false, false, true, false), None),
    ];
    for ((allied, passive, only_enemy, enemy), pick) in cases {
        let case = (allied, passive, only_enemy, enemy);
        let mut sim = Simulation::with_seed(0x6EE310);
        let grid = crate::sim::arena_fixture::flat_arena(&mut sim, &rules);
        let (team, _, plants) = building_team(
            &mut sim,
            &rules,
            &[(24, 0), (46, 2 << 16), (5, 100)],
            &PLANTS,
        );
        let russians = sim.interner.intern("Russians");
        let germans = sim.interner.intern("Germans");
        if allied {
            sim.house_alliances
                .entry("RUSSIANS".into())
                .or_default()
                .insert("GERMANS".into());
        }
        sim.houses.get_mut(&germans).unwrap().multiplay_passive = passive;
        if !enemy {
            sim.houses.get_mut(&russians).unwrap().enemy_house = None;
        }
        if only_enemy {
            let team_type = sim.interner.intern("BUILDING TEAMTYPE");
            let task_force = sim.interner.intern("BUILDING TASKFORCE");
            let vm = &mut sim.team_script_vm;
            let script_id = vm.teams[&team].script_id;
            vm.register_task_force(TeamTaskForceDefinition {
                id: task_force,
                source: TeamAiDefinitionSource::FixedAimd,
                group: -1,
                entries: Vec::new(),
            });
            vm.register_team_type(TeamTypeDefinition {
                id: team_type,
                script_id,
                task_force_id: task_force,
                priority: 0,
                is_base_defense: false,
                suicide: false,
                aggressive: false,
                combined_movement_zone: crate::rules::locomotor_type::MovementZone::Normal,
                base_zone_relation_enforced: true,
                transport_crossing_required: false,
            });
            vm.team_type_ini
                .get_mut(&team_type)
                .unwrap()
                .only_target_house_enemy = true;
            vm.teams.get_mut(&team).unwrap().team_type_id = Some(team_type);
        }
        let (cursor, target) = attack_building_target(&mut sim, &rules, &grid, team);
        match pick {
            Some(pick) => assert_eq!(target, Some(TeamTarget::Object(plants[pick])), "{case:?}"),
            None => assert_eq!((cursor, target), (2, None), "{case:?}"),
        }
    }
}

/// Through the production readers: the retail tech-capture scripts' building
/// arguments name the Tech Oil Derrick (`131143` = mode 2, index 71) and the
/// Tech Airport (`131142`, index 70), and the TeamTypes that run them reach
/// only ported actions.
#[test]
fn the_retail_tech_capture_scripts_run_ported_actions() {
    use crate::rules::retail_ini_fixture::{retail_ini, retail_rules_and_art};
    use crate::rules::team_ai_ini::TeamAiIniRegistry;
    let Some((rules_ini, art_ini)) = retail_rules_and_art() else {
        return;
    };
    let rules = RuleSet::from_ini_with_fixed_art_for_test(&rules_ini, &art_ini).unwrap();
    for (name, index) in [("CAOILD", 71), ("CAAIRP", 70)] {
        assert_eq!(rules.object(name).unwrap().base_plan_type_index, index);
    }
    let aimd = retail_ini("aimd.ini").expect("retail AIMD");
    let registry = TeamAiIniRegistry::from_sources(&aimd, &IniFile::from_str(""), true);
    let mut sim = Simulation::new();
    sim.intern_rule_type_ids(&rules);
    sim.resolve_type_handles(&rules);
    sim.install_team_ai_registry(&registry, &rules)
        .expect("retail AIMD installs");
    let vm = &sim.team_script_vm;
    let capture: Vec<_> = vm
        .scripts
        .values()
        .filter(|script| {
            script
                .actions
                .first()
                .is_some_and(|action| action.action_id == 46)
                && matches!(script.actions[0].argument, 131142 | 131143)
        })
        .collect();
    assert!(!capture.is_empty());
    for script in &capture {
        assert!(
            script
                .actions
                .iter()
                .all(|action| action_is_ported(action.action_id))
        );
    }
    let running = vm
        .team_types
        .values()
        .filter(|team_type| {
            capture
                .iter()
                .any(|script| script.id == team_type.script_id)
        })
        .count();
    assert_eq!(running, 6);
}

/// Action 9's types: an MCV that deploys into a 4x4 yard (as retail's), a simple
/// deployer, a `Deployer=` infantryman and a plain one, on buildable ground.
const DEPLOY_RULES: &str = "\
[InfantryTypes]
0=E1
1=GI
[VehicleTypes]
0=AMCV
1=SIEGE
[BuildingTypes]
0=YARD
[Clear]
Buildable=yes
[E1]
Strength=125
Speed=4
Locomotor={4A582744-9839-11D1-B709-00A024DDAFD1}
[GI]
Strength=125
Speed=4
Deployer=yes
Locomotor={4A582744-9839-11D1-B709-00A024DDAFD1}
[AMCV]
Strength=1000
Speed=5
ROT=5
DeploysInto=YARD
Locomotor={4A582741-9839-11D1-B709-00A024DDAFD1}
[SIEGE]
Strength=300
Speed=6
IsSimpleDeployer=yes
Locomotor={4A582741-9839-11D1-B709-00A024DDAFD1}
[YARD]
Strength=1000
ConstructionYard=yes
[Guard]
Rate=0.016
[Unload]
Rate=0.016
";

struct DeployTeam {
    sim: Simulation,
    rules: RuleSet,
    grid: crate::sim::pathfinding::PathGrid,
    team: u64,
    members: Vec<u64>,
}

impl DeployTeam {
    /// The Russians, a computer house that does not build its own base, on
    /// a flat arena, with one object per `(type, cell)` as a team running
    /// action 9, then action 2 (stay).
    fn new(spawns: &[(&str, u16, u16)]) -> Self {
        let rules = RuleSet::from_ini_with_fixed_art_for_test(
            &IniFile::from_str(DEPLOY_RULES),
            &IniFile::from_str("[YARD]\nFoundation=4x4\n"),
        )
        .expect("deploy rules");
        let mut sim = Simulation::with_seed(0x6ED4D0);
        let owner = sim.interner.intern("Russians");
        sim.houses
            .insert(owner, HouseState::new(owner, 1, None, false, 0, 10));
        sim.session.house_order.push(owner);
        let grid = crate::sim::arena_fixture::flat_arena(&mut sim, &rules);
        let members: Vec<u64> = spawns
            .iter()
            .map(|&(name, rx, ry)| {
                sim.spawn_object_at_height(name, "Russians", rx, ry, 0, 0, &rules)
                    .expect("member spawns")
            })
            .collect();
        let script = sim.interner.intern("DEPLOY SCRIPT");
        sim.team_script_vm.register_script(TeamScriptDefinition {
            id: script,
            source: TeamAiDefinitionSource::FixedAimd,
            actions: [(9, 0), (2, 0)]
                .into_iter()
                .map(|(action_id, argument)| TeamScriptAction {
                    action_id,
                    argument,
                })
                .collect(),
        });
        let team = sim
            .team_script_vm
            .create_team(owner, script, members.clone(), 0);
        Self {
            sim,
            rules,
            grid,
            team,
            members,
        }
    }

    fn tick(&mut self) {
        let commands = self.sim.take_due_commands();
        self.sim
            .advance_tick(&commands, Some(&self.rules), Some(&self.grid), None, 33);
    }

    /// Runs action 9 for the team once, outside a frame.
    fn deploy_action(&mut self) {
        self.sim
            .team_action_deploy(self.team, &self.rules, None, FrameEffects::default());
    }

    /// The action is finished (`+0x80`).
    fn finished(&self) -> bool {
        self.sim
            .team_script_vm
            .team(self.team)
            .is_some_and(|team| team.advance_pending())
    }

    /// Whether `id` was ordered to Unload: queued, or already its mission.
    fn unloading(&self, id: u64) -> bool {
        let unload = MissionId::from_known(MissionType::Unload);
        self.sim.substrate.entities.get(id).is_some_and(|entity| {
            entity.mission.queued() == unload || entity.mission.current() == unload
        })
    }

    /// The cells of the yards standing.
    fn yards(&self) -> Vec<(u16, u16)> {
        self.sim
            .substrate
            .entities
            .values()
            .filter(|entity| {
                !entity.dying && self.sim.interner.resolve(entity.type_ref()) == "YARD"
            })
            .map(|entity| (entity.position.rx, entity.position.ry))
            .collect()
    }

    /// The team is gone or past action 9.
    fn past_deploy(&self) -> bool {
        self.sim
            .team_script_vm
            .team(self.team)
            .is_none_or(|team| team.cursor() >= 1)
    }
}

/// Action 9 (`0x006ED4D0`) on a team's MCV standing on clear ground: the
/// yard may stand from the MCV's cell, so the MCV drops its destination and
/// queues Unload, and its Mission_Unload deploys it (the yard one cell up
/// and left of it, as Deploy places). The action holds while the MCV is in
/// the team; the MCV leaves it as it becomes the yard, and the team moves on.
#[test]
fn script_action_9_deploys_the_team_s_mcv() {
    let mut team = DeployTeam::new(&[("AMCV", 20, 22)]);
    let mcv = team.members[0];
    team.tick();
    assert!(team.unloading(mcv), "the MCV was ordered to Unload");
    assert!(!team.past_deploy(), "the MCV holds the action");
    for _ in 0..120 {
        if !team.yards().is_empty() && team.past_deploy() {
            break;
        }
        team.tick();
    }
    assert_eq!(team.yards(), [(19, 21)]);
    assert!(team.past_deploy(), "the team moved on");
}

/// A Russian E1 inside the yard's foundation from the MCV's cell refuses the
/// placement: Flush_For_Placement (`0x0045EE70`) scatters its cell, the MCV
/// gets no order, is put back in its cell, and the action holds. Asked
/// again with the site clear, it is ordered to Unload, and it deploys.
#[test]
fn script_action_9_clears_the_site_before_the_mcv_deploys() {
    let mut team = DeployTeam::new(&[("AMCV", 20, 22)]);
    let mcv = team.members[0];
    let e1 = team
        .sim
        .spawn_object_at_height("E1", "Russians", 23, 24, 0, 0, &team.rules)
        .expect("E1 spawns");
    team.deploy_action();
    assert!(!team.unloading(mcv), "a refused site orders nothing");
    assert!(!team.finished());
    assert!(team.sim.substrate.occupancy.contains_entity(20, 22, mcv));
    let e1_entity = team.sim.substrate.entities.get(e1).unwrap();
    assert!(
        e1_entity.navigation.nav_com.is_some()
            || crate::sim::movement::motion_query::is_moving(&e1_entity) == Some(true),
        "the E1 was scattered"
    );

    team.sim.despawn_entity(e1);
    team.deploy_action();
    assert!(team.unloading(mcv), "a clear site orders Unload");
    assert!(!team.finished());
    for _ in 0..120 {
        if !team.yards().is_empty() {
            break;
        }
        team.tick();
    }
    assert_eq!(team.yards(), [(19, 21)]);
}

/// The yard is asked for with its origin at the MCV's own cell: Russian E1s
/// just outside that 4x4 foundation on each side leave it placeable, so the
/// MCV is ordered to Unload at once. An origin one cell off in any direction,
/// such as Deploy's (-1, -1), would take one of them in and refuse.
#[test]
fn script_action_9_asks_for_the_yard_at_the_mcv_s_own_cell() {
    let mut team = DeployTeam::new(&[("AMCV", 20, 22)]);
    let mcv = team.members[0];
    for (x, y) in [(19, 22), (20, 21), (24, 22), (20, 26)] {
        team.sim
            .spawn_object_at_height("E1", "Russians", x, y, 0, 0, &team.rules)
            .expect("E1 spawns");
    }
    team.deploy_action();
    assert!(team.unloading(mcv), "the site at the MCV's cell is clear");
}

/// A Russian E1 on the yard's origin cell, which only Deploy's (-1, -1)
/// offset reaches, passes action 9's own-cell test, so the MCV is ordered to
/// Unload and Deploy refuses. For an owner no human controls, the refusal
/// clears the site (`Flush_For_Placement`, `0x0073950F..0x00739536`): the E1
/// is told to leave, and the MCV deploys on a later try. A human owner's
/// refusal leaves the E1 where it stands, and the MCV never deploys.
#[test]
fn deploy_refused_for_a_computer_owner_clears_the_site() {
    for human in [false, true] {
        let mut team = DeployTeam::new(&[("AMCV", 20, 22)]);
        let owner = team.sim.interner.intern("Russians");
        team.sim.houses.get_mut(&owner).unwrap().is_human = human;
        let e1 = team
            .sim
            .spawn_object_at_height("E1", "Russians", 19, 21, 0, 0, &team.rules)
            .expect("E1 spawns");
        team.deploy_action();
        assert!(team.unloading(team.members[0]), "human {human}");
        let mut told_to_leave = false;
        for _ in 0..300 {
            team.tick();
            told_to_leave |= team.sim.substrate.entities.get(e1).is_some_and(|entity| {
                entity.navigation.nav_com.is_some()
                    || crate::sim::movement::motion_query::is_moving(&entity) == Some(true)
            });
            if !team.yards().is_empty() {
                break;
            }
        }
        assert_eq!(told_to_leave, !human, "human {human}");
        let yards = if human { vec![] } else { vec![(19, 21)] };
        assert_eq!(team.yards(), yards, "human {human}");
    }
}

/// A simple deployer and a `Deployer=` infantryman are ordered to Unload, a
/// plain infantryman is not, and with no `DeploysInto=` unit the action is
/// finished at once.
#[test]
fn script_action_9_orders_the_deployers_and_finishes() {
    let mut team = DeployTeam::new(&[("SIEGE", 10, 10), ("GI", 12, 10), ("E1", 13, 10)]);
    team.deploy_action();
    let [siege, gi, e1] = team.members[..] else {
        panic!("three members");
    };
    assert!(team.unloading(siege) && team.unloading(gi));
    assert!(!team.unloading(e1));
    assert!(team.finished(), "the action finished");
    team.tick();
    assert!(team.past_deploy());
}
