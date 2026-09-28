//! Minimal AI opponent — produces deterministic commands via the same Command API as players.
//!
//! A stand-in for the computer's attacks, which its teams' attack script
//! actions will make (`sim::team_script_vm::actions`, not ported):
//! periodically, on an eighth frame, it sends idle units at the nearest enemy
//! base, sparing the members of teams whose script runs.
//!
//! RESIDUAL: its AttackMove is a player order, which takes each member of a
//! team stalled on an unported action off that team (`0x004C735D`). Once
//! empty, the formed team is destroyed and its AI trigger records a failure
//! (`Simulation::destroy_team`), and new teams may recruit the wave's units
//! while their mission is recruitable. Trigger: every computer attack team,
//! each wave (225 frames). Effect: attack teams dissolve as they set out
//! instead of fighting as teams, and their triggers' weights fall as when
//! such a team is wiped out, only sooner. The attack actions' port deletes
//! this stand-in. A
//! computer house's units come from its teams' needs (`sim::ai_team_creation`,
//! `sim::ai_unit_choice`), its buildings from its Construction Yard
//! (`sim::ai_base_building`), both made at its factory buildings
//! (`production::factory_ai`), and its Construction Yard maker deploys
//! through its own missions (`sim::mcv_deploy`), not through this loop.
//!
//! All decisions are deterministic (uses SimRng). AI commands are injected
//! into the same command stream as player commands, so replays stay valid.
//!
//! ## Dependency rules
//! - Part of sim/ — depends on rules/, map/
//! - sim/ NEVER depends on render/, ui/, sidebar/, audio/, net/

use crate::map::entities::EntityCategory;
use crate::rules::ruleset::RuleSet;
use crate::sim::command::{Command, CommandEnvelope};
use crate::sim::intern::InternedId;
use crate::sim::production;
use crate::sim::world::Simulation;

/// How often (in native frames) the AI considers an attack wave.
const AI_THINK_INTERVAL_FRAMES: u32 = 8;

/// How often (in native frames) the AI sends an attack wave.
const AI_ATTACK_INTERVAL_FRAMES: u32 = 225;

/// Minimum native frame before the AI sends its first attack.
const AI_FIRST_ATTACK_FRAME: u32 = 150;

/// Maximum units to send per attack wave.
const AI_ATTACK_WAVE_SIZE: usize = 8;

/// Per-AI-owner persistent state.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AiPlayerState {
    /// House/owner name this AI controls.
    pub owner: InternedId,
    /// Native frame when the last attack wave was sent.
    pub last_attack_frame: u32,
}

impl AiPlayerState {
    pub fn new(owner: InternedId) -> Self {
        Self {
            owner,
            last_attack_frame: 0,
        }
    }
}

/// Run one AI decision cycle for all AI players. Returns commands to inject.
pub fn tick_ai(
    sim: &Simulation,
    ai_players: &mut [AiPlayerState],
    rules: &RuleSet,
) -> Vec<CommandEnvelope> {
    let mut commands: Vec<CommandEnvelope> = Vec::new();
    let execute_tick = sim.session.tick.saturating_add(1);
    let current_frame = sim.session.binary_frame;

    for ai in ai_players.iter_mut() {
        // A house defeated this tick issues no commands at all. gamemd
        // evaluates each house's defeat before its AI manage/produce step;
        // Phase 8's house rung runs every defeat gate before this loop, so
        // the flag is current.
        // Without this gate the defeat reorder would be a no-op.
        if crate::sim::house_state::house_state_for_owner_id(&sim.houses, ai.owner)
            .is_some_and(|h| h.is_defeated)
        {
            continue;
        }

        let owner_str = sim.interner.resolve(ai.owner);
        // Only think every N native frames to avoid spamming.
        if !current_frame.is_multiple_of(AI_THINK_INTERVAL_FRAMES) {
            continue;
        }

        // A ConYard is any structure with UndeploysInto= set (data-driven from rules.ini).
        if !has_conyard_dynamic(sim, owner_str, rules) {
            continue; // No conyard — can't do anything.
        }

        if current_frame >= AI_FIRST_ATTACK_FRAME
            && current_frame.wrapping_sub(ai.last_attack_frame) >= AI_ATTACK_INTERVAL_FRAMES
        {
            let attack_cmds = send_attack_wave(sim, owner_str, rules, execute_tick);
            if !attack_cmds.is_empty() {
                ai.last_attack_frame = current_frame;
                commands.extend(attack_cmds);
            }
        }
    }

    commands
}

/// Check if the owner has any ConYard-class structure (one with UndeploysInto= set).
fn has_conyard_dynamic(sim: &Simulation, owner: &str, rules: &RuleSet) -> bool {
    has_owned_structure_matching(sim, owner, |type_id| {
        rules
            .object(type_id)
            .is_some_and(|object| object.undeploys_into.is_some())
    })
}

fn has_owned_structure_matching<F>(sim: &Simulation, owner: &str, mut matches: F) -> bool
where
    F: FnMut(&str) -> bool,
{
    sim.substrate.entities.values().any(|e| {
        !e.dying
            && !e.lifecycle.in_limbo
            && e.category == EntityCategory::Structure
            && sim.interner.resolve(e.owner()).eq_ignore_ascii_case(owner)
            && matches(sim.interner.resolve(e.type_ref()))
    })
}

/// Send an attack wave: gather idle military units and attack-move toward enemy.
fn send_attack_wave(
    sim: &Simulation,
    owner: &str,
    rules: &RuleSet,
    execute_tick: u64,
) -> Vec<CommandEnvelope> {
    let mut commands: Vec<CommandEnvelope> = Vec::new();

    // Find nearest enemy structure as attack target.
    let Some(target) = find_nearest_enemy_structure(sim, owner) else {
        return commands;
    };

    // Gather idle military units (no MovementTarget, not harvesters).
    let mut idle_units: Vec<(u64, u16, u16)> = Vec::new();
    for entity in sim.substrate.entities.values() {
        if !sim
            .interner
            .resolve(entity.owner())
            .eq_ignore_ascii_case(owner)
        {
            continue;
        }
        if entity.dying || entity.lifecycle.in_limbo {
            continue;
        }
        if !matches!(
            entity.category,
            EntityCategory::Unit | EntityCategory::Infantry
        ) {
            continue;
        }
        let type_id = sim.interner.resolve(entity.type_ref());
        if production::is_harvester_type(rules, type_id) {
            continue;
        }
        // A unit that deploys into a building (the MCV) is on its own
        // missions (`sim::mcv_deploy`), not an attacker.
        if rules
            .object(type_id)
            .is_some_and(|object| object.deploys_into.is_some())
        {
            continue;
        }
        // A team member follows its team, unless the team stays on a script
        // action VERA does not port.
        let teams = &sim.team_script_vm;
        if teams.team_for_member(entity.stable_id()).is_some()
            && !teams.member_team_stalled(entity.stable_id())
        {
            continue;
        }
        // Check if unit has no movement target (idle).
        if entity.movement_target.is_none() {
            idle_units.push((entity.stable_id(), entity.position.rx, entity.position.ry));
        }
    }
    idle_units.sort_by_key(|(sid, _, _)| *sid);

    // Send up to WAVE_SIZE units.
    let owner_id = match sim.interner.get(owner) {
        Some(id) => id,
        None => return commands,
    };
    for (entity_id, _, _) in idle_units.into_iter().take(AI_ATTACK_WAVE_SIZE) {
        commands.push(CommandEnvelope::new(
            owner_id,
            execute_tick,
            Command::AttackMove {
                entity_id,
                target_rx: target.0,
                target_ry: target.1,
                queue: false,
            },
        ));
    }

    if !commands.is_empty() {
        log::info!(
            "AI [{}] sending attack wave: {} units toward ({}, {})",
            owner,
            commands.len(),
            target.0,
            target.1
        );
    }

    commands
}

/// Find the center of the AI's base (average position of owned structures).
fn find_base_center(sim: &Simulation, owner: &str) -> Option<(u16, u16)> {
    let mut sum_x: i64 = 0;
    let mut sum_y: i64 = 0;
    let mut count: i64 = 0;
    for entity in sim.substrate.entities.values() {
        if !entity.dying
            && !entity.lifecycle.in_limbo
            && entity.category == EntityCategory::Structure
            && sim
                .interner
                .resolve(entity.owner())
                .eq_ignore_ascii_case(owner)
        {
            sum_x += i64::from(entity.position.rx);
            sum_y += i64::from(entity.position.ry);
            count += 1;
        }
    }
    if count == 0 {
        return None;
    }
    Some((
        u16::try_from(sum_x / count).unwrap_or(u16::MAX),
        u16::try_from(sum_y / count).unwrap_or(u16::MAX),
    ))
}

/// Find the nearest enemy structure to attack.
fn find_nearest_enemy_structure(sim: &Simulation, owner: &str) -> Option<(u16, u16)> {
    let base_center = find_base_center(sim, owner)?;
    let mut best: Option<(u32, u16, u16)> = None;

    for entity in sim.substrate.entities.values() {
        if entity.dying || entity.lifecycle.in_limbo {
            continue;
        }
        if entity.category != EntityCategory::Structure {
            continue;
        }
        let e_owner = sim.interner.resolve(entity.owner());
        if e_owner.eq_ignore_ascii_case(owner) {
            continue;
        }
        // Skip neutral/civilian houses.
        let up = e_owner.to_ascii_uppercase();
        if matches!(
            up.as_str(),
            "NEUTRAL" | "SPECIAL" | "CIVILIAN" | "GOODGUY" | "BADGUY"
        ) {
            continue;
        }

        let dx = entity.position.rx as i64 - base_center.0 as i64;
        let dy = entity.position.ry as i64 - base_center.1 as i64;
        let dist_sq = (dx * dx + dy * dy) as u32;
        match best {
            Some((d, _, _)) if dist_sq >= d => {}
            _ => best = Some((dist_sq, entity.position.rx, entity.position.ry)),
        }
    }
    best.map(|(_, rx, ry)| (rx, ry))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::ini_parser::IniFile;
    use crate::sim::components::Health;

    fn spawn_structure(
        sim: &mut Simulation,
        sid: u64,
        owner: &str,
        type_id: &str,
        rx: u16,
        ry: u16,
    ) {
        let owner_id = sim.interner.intern(owner);
        let type_id_interned = sim.interner.intern(type_id);
        let mut ge = crate::sim::game_entity::GameEntity::new_at_frame_zero_for_test(
            sid,
            rx,
            ry,
            0,
            0,
            owner_id,
            Health { current: 1000 },
            type_id_interned,
            EntityCategory::Structure,
            0,
            5,
            false,
        );
        ge.lifecycle.in_limbo = false;
        sim.substrate.entities.insert(ge);
        if sim.substrate.next_stable_object_id <= sid {
            sim.substrate.next_stable_object_id = sid + 1;
        }
    }

    #[test]
    fn test_ai_player_state_new() {
        let mut interner = crate::sim::intern::StringInterner::new();
        let owner_id = interner.intern("Russians");
        let state = AiPlayerState::new(owner_id);
        assert_eq!(state.owner, owner_id);
        assert_eq!(state.last_attack_frame, 0);
    }

    #[test]
    fn tick_ai_skips_defeated_house() {
        // A house flagged is_defeated must issue NO command (the Phase-8 defeat
        // gate). Baseline: a live house with a Construction Yard sends its idle
        // tank at the enemy base on an attack frame, so the empty result for the
        // defeated house proves the gate fired.
        let rules = RuleSet::from_ini(&IniFile::from_str(
            "[InfantryTypes]\n\
             [AircraftTypes]\n\
             [VehicleTypes]\n\
             0=TSTMCV\n\
             1=TSTTNK\n\
             [BuildingTypes]\n\
             0=TSTCYRD\n\
             [TSTMCV]\n\
             Name=Test MCV\n\
             DeploysInto=TSTCYRD\n\
             Speed=6\n\
             Strength=1000\n\
             TechLevel=1\n\
             Owner=Americans\n\
             [TSTTNK]\n\
             Speed=6\n\
             Strength=400\n\
             TechLevel=1\n\
             Owner=Americans\n\
             [TSTCYRD]\n\
             Name=Test ConYard\n\
             Foundation=2x2\n\
             UndeploysInto=TSTMCV\n\
             Strength=1000\n\
             TechLevel=1\n\
             Owner=Americans\n",
        ))
        .expect("rules parse");

        // Build a sim holding one idle tank and a Construction Yard for
        // "Americans", and a Soviet structure to attack.
        let build_sim = || {
            let mut sim = Simulation::new();
            sim.session.binary_frame = 232;
            spawn_structure(&mut sim, 2, "Americans", "TSTCYRD", 8, 8);
            spawn_structure(&mut sim, 3, "Soviets", "TSTCYRD", 30, 30);
            let owner_id = sim.interner.intern("Americans");
            let tank_type = sim.interner.intern("TSTTNK");
            let mut ge = crate::sim::game_entity::GameEntity::new_at_frame_zero_for_test(
                1,
                5,
                5,
                0,
                0,
                owner_id,
                Health { current: 400 },
                tank_type,
                EntityCategory::Unit,
                0,
                5,
                false,
            );
            ge.lifecycle.in_limbo = false;
            sim.substrate.entities.insert(ge);
            (sim, owner_id)
        };

        // No house registered -> not defeated -> the tank attacks.
        let (sim, owner_id) = build_sim();
        let mut ai = vec![AiPlayerState::new(owner_id)];
        let live = tick_ai(&sim, &mut ai, &rules);
        assert_eq!(live.len(), 1, "a live AI house sends its attack wave");
        assert!(matches!(live[0].payload, Command::AttackMove { .. }));

        // Defeated house -> the gate skips it -> no commands.
        let (mut sim, owner_id) = build_sim();
        let mut house =
            crate::sim::house_state::HouseState::new(owner_id, 0, None, false, 10_000, 10);
        house.is_defeated = true;
        sim.houses.insert(owner_id, house);
        let mut ai = vec![AiPlayerState::new(owner_id)];
        let defeated = tick_ai(&sim, &mut ai, &rules);
        assert!(
            defeated.is_empty(),
            "a defeated AI house must issue no command"
        );
    }
}
