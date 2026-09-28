//! The House base-defence response (`TechnoClass::RespondToBaseAttack @
//! 0x00708080`): its exact signed arithmetic, six-slot ranking pathology and
//! the transaction that recruits, queues and debits in native order.
//!
//! Its admission asks each candidate's GetFireError through the one owner
//! ([`super::fire_error_world::FireSubject`]), so the scan reads the whole
//! world while the transaction's mutations come before and after it.

use crate::map::entities::EntityCategory;
use crate::map::houses::is_allied_with;
use crate::rules::locomotor_type::MovementZone;
use crate::rules::ruleset::RuleSet;
use crate::sim::cell_rect::cell_is_in_playfield_height_aware;
use crate::sim::mission::authority::queue_entity_mission_deferred;
use crate::sim::mission::concrete_effects::{
    assign_target_commits, represented_assign_target_admitted,
};
use crate::sim::mission::{MissionId, MissionType};
use crate::sim::world::Simulation;
use crate::util::native_x87::{NativeF64Bits, X87Chop53, distance_3d_leptons};

use super::TargetKind;

const RESPONSE_LIST_CAPACITY: usize = 6;
const FRAMES_PER_MINUTE: i32 = 900;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ResponderClass {
    Infantry,
    Unit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ExistingTargetDisposition {
    NoneOrUnarmed,
    RequestedAttacker,
    OtherArmedTarget,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ThreatFacts {
    pub(crate) cost: i32,
    pub(crate) speed_leptons_per_frame: i32,
    pub(crate) current_coord: [i32; 3],
    pub(crate) attacker_coord: [i32; 3],
    pub(crate) primary_range_leptons: i32,
    pub(crate) existing_target: ExistingTargetDisposition,
    pub(crate) in_non_base_defense_team: bool,
    pub(crate) mission_is_harvest: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct RankedResponder {
    pub(crate) entity_id: u64,
    pub(crate) score: i32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ResponseSelection {
    remaining_budget: i32,
    minimum_score: i32,
    responders: Vec<RankedResponder>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ResponseMission {
    Rescue,
    AreaGuard,
}

/// Convert the Rules double through the native x87 `ftol` path. Invalid or
/// out-of-range values retain the x87 signed-indefinite low dword.
///
/// gamemd-derived: `TechnoClass__RespondToBaseAttack @ 0x0070878D` multiplies
/// `[General] BaseDefenseDelay` by the literal `900.0` before `Math__ftol`.
pub(crate) fn response_delay_frames(delay_minutes: f64) -> i32 {
    let loaded = X87Chop53::load_f64(NativeF64Bits::from_bits(delay_minutes.to_bits()));
    let Ok(delay) = loaded else {
        return i32::MIN;
    };
    X87Chop53::ftol_i32_low_masked(X87Chop53::mul(
        delay,
        X87Chop53::load_i32(FRAMES_PER_MINUTE),
    ))
}

/// Exact signed threat score used only by the base-defence responder.
///
/// gamemd-derived: `FootClass__Evaluate_Target_Threat @ 0x004D97A0` uses the
/// current 3-D lepton coordinates, `CoordStruct__Distance3D`'s
/// `Sqrt_Approx/ftol`, signed wrapping `cost << 10`, and integer travel time.
pub(crate) fn evaluate_target_threat(facts: ThreatFacts) -> i32 {
    match facts.existing_target {
        ExistingTargetDisposition::RequestedAttacker => return facts.cost.wrapping_neg(),
        ExistingTargetDisposition::OtherArmedTarget => return 0,
        ExistingTargetDisposition::NoneOrUnarmed => {}
    }
    if facts.in_non_base_defense_team || facts.mission_is_harvest || facts.cost == 0 {
        return 0;
    }

    let distance = distance_3d_leptons(facts.current_coord, facts.attacker_coord);
    let base = facts.cost.wrapping_shl(10);
    if distance <= facts.primary_range_leptons {
        return base;
    }

    let speed = facts.speed_leptons_per_frame.max(1);
    let travel_frames = distance
        .wrapping_sub(facts.primary_range_leptons)
        .wrapping_div(speed)
        .max(1);
    base.wrapping_div(travel_frames).max(1)
}

fn class_adjusted_score(score: i32, class: ResponderClass, victim_is_self_anchor: bool) -> i32 {
    if !victim_is_self_anchor || score == 0 {
        return score;
    }
    match class {
        // The Infantry loop applies its self-anchor multiplier before the
        // negative-score budget branch.
        ResponderClass::Infantry => score.wrapping_mul(100),
        // The Unit loop reaches its multiplier only on the positive arm.
        ResponderClass::Unit if score > 0 => score.wrapping_mul(10),
        ResponderClass::Unit => score,
    }
}

impl ResponseSelection {
    pub(crate) fn new(budget: i32) -> Self {
        Self {
            remaining_budget: budget,
            minimum_score: 0,
            responders: Vec::with_capacity(RESPONSE_LIST_CAPACITY),
        }
    }

    #[cfg(test)]
    pub(crate) fn remaining_budget(&self) -> i32 {
        self.remaining_budget
    }

    pub(crate) fn can_scan(&self) -> bool {
        self.remaining_budget > 0
    }

    /// Apply the Infantry/Unit self-anchor multiplier, debit already-engaged
    /// defenders, and reproduce the native six-slot minimum bug.
    ///
    /// gamemd-derived: `TechnoClass__RespondToBaseAttack @
    /// 0x00708340..0x0070868F`. While the list grows, `minimum_score` remains
    /// zero. The first later positive candidate therefore only exposes the real
    /// minimum (it replaces no slot); subsequent replacements overwrite every
    /// equal-minimum slot with the same candidate, preserving duplicates.
    pub(crate) fn consider(
        &mut self,
        entity_id: u64,
        raw_score: i32,
        class: ResponderClass,
        victim_is_self_anchor: bool,
    ) {
        let score = class_adjusted_score(raw_score, class, victim_is_self_anchor);
        if score < 0 {
            self.remaining_budget = self.remaining_budget.wrapping_add(score);
            return;
        }
        if score == 0 {
            return;
        }
        if self.responders.len() < RESPONSE_LIST_CAPACITY {
            self.responders.push(RankedResponder { entity_id, score });
            return;
        }
        if score <= self.minimum_score {
            return;
        }

        let old_minimum = self.minimum_score;
        let mut replaced = [false; RESPONSE_LIST_CAPACITY];
        for (index, responder) in self.responders.iter_mut().enumerate() {
            if responder.score == old_minimum {
                *responder = RankedResponder { entity_id, score };
                replaced[index] = true;
            }
        }

        self.minimum_score = score;
        for (index, responder) in self.responders.iter().enumerate() {
            if !replaced[index] && responder.score < self.minimum_score {
                self.minimum_score = responder.score;
            }
        }
    }

    /// Stable signed descending sort: equal scores never exchange positions.
    pub(crate) fn into_ranked(mut self) -> (i32, Vec<RankedResponder>) {
        for upper in (1..self.responders.len()).rev() {
            for index in 0..upper {
                if self.responders[index].score < self.responders[index + 1].score {
                    self.responders.swap(index, index + 1);
                }
            }
        }
        (self.remaining_budget, self.responders)
    }
}

/// Every selected occurrence consumes the draw. Team membership changes only
/// the interpretation, never whether the draw occurs.
///
/// gamemd-derived: `TechnoClass__RespondToBaseAttack @
/// 0x007086DB..0x00708718` queues Rescue for `0..=65`, else Area Guard; a
/// base-defence Team member is forced to Area Guard after the draw.
pub(crate) fn response_mission(draw_0_to_99: u32, in_base_defense_team: bool) -> ResponseMission {
    debug_assert!(draw_0_to_99 <= 99);
    if !in_base_defense_team && draw_0_to_99 <= 65 {
        ResponseMission::Rescue
    } else {
        ResponseMission::AreaGuard
    }
}

/// The assignment loop stops only after its signed wrapping cost sum strictly
/// exceeds the post-scan budget; equality deliberately continues.
pub(crate) fn add_assigned_cost(accumulated: i32, cost: i32, budget: i32) -> (i32, bool) {
    let accumulated = accumulated.wrapping_add(cost);
    (accumulated, accumulated > budget)
}

mod admission;

use super::combat_weapon::is_armed;
use admission::{
    candidate_admitted, current_target_disposition, destination_cell, entity_coord,
    primary_range_leptons, should_be_on_bridge_for_response,
};

/// Execute one complete native response transaction after the receiver owner
/// has selected the exact Building or protected-Techno call site.
///
/// gamemd-derived: `TechnoClass__RespondToBaseAttack @ 0x00708080`.
pub(crate) fn respond_to_base_attack(
    world: &mut Simulation,
    rules: &RuleSet,
    victim_id: u64,
    attacker_id: u64,
) {
    let entities = &world.substrate.entities;
    let interner = &world.interner;
    let Some(victim) = entities.get(victim_id) else {
        return;
    };
    let Some(attacker) = entities.get(attacker_id) else {
        return;
    };
    let Some(victim_object) = rules.object(interner.resolve(victim.type_ref())) else {
        return;
    };
    let Some(attacker_object) = rules.object(interner.resolve(attacker.type_ref())) else {
        return;
    };
    let victim_owner = victim.owner();
    let attacker_owner = attacker.owner();
    let budget = attacker_object
        .cost
        .wrapping_mul(rules.general.computer_base_defense_response);
    let current_frame = world.session.binary_frame as i32;

    if is_allied_with(
        &world.house_alliances,
        interner.resolve(victim_owner),
        interner.resolve(attacker_owner),
    ) || world
        .houses
        .get(&victim_owner)
        .is_some_and(|house| house.is_human)
        || attacker.lifecycle.in_limbo
        // `0x00708114`: `[g_GameMode 0x00A8B238] == 0 && this->Is_Armed
        // (vt+0x2AC)` — an armed victim in that game mode defends itself
        // instead of calling for help.
        || (!world.session.game_mode_nonzero && is_armed(victim, victim_object))
        || !matches!(
            attacker.category,
            EntityCategory::Unit | EntityCategory::Infantry
        )
        || victim_object.insignificant
        || !attacker
            .base_defense_response
            .cooldown
            .expired(current_frame)
    {
        return;
    }

    // Rust's map caches are derived rather than native globals. A live positive
    // scan cannot be exact without both authorities; fail atomically before the
    // native Team suspension point instead of partially mutating the response.
    if budget > 0 && (world.zone_grid.is_none() || world.resolved_terrain.is_none()) {
        return;
    }

    world.team_script_vm.suspend_teams_for_base_defense(
        victim_owner,
        rules.general.suspend_priority,
        current_frame,
        response_delay_frames(rules.general.suspend_delay_minutes),
    );
    let mut selection = ResponseSelection::new(budget);
    if !selection.can_scan() {
        return;
    }

    // The scan only reads; every mutation comes after it.
    let sim: &Simulation = world;
    let entities = &sim.substrate.entities;
    let attacker_coord = entity_coord(attacker, sim.resolved_terrain.as_ref());
    let victim_is_self_anchor = victim.archive_target() == Some(TargetKind::Entity(victim_id));
    let candidate_ids = entities.keys_sorted();
    for class in [ResponderClass::Infantry, ResponderClass::Unit] {
        for &candidate_id in &candidate_ids {
            if !selection.can_scan() {
                break;
            }
            let expected_category = match class {
                ResponderClass::Infantry => EntityCategory::Infantry,
                ResponderClass::Unit => EntityCategory::Unit,
            };
            let Some(candidate) = entities.get(candidate_id) else {
                continue;
            };
            if candidate.category != expected_category {
                continue;
            }
            let Some(candidate_object) = rules.object(sim.interner.resolve(candidate.type_ref()))
            else {
                continue;
            };
            if !candidate_admitted(sim, rules, candidate, candidate_object, victim, attacker_id) {
                continue;
            }

            let terrain = sim
                .resolved_terrain
                .as_ref()
                .expect("positive scan validated terrain");
            let destination = destination_cell(candidate, entities, Some(terrain));
            let victim_destination = destination_cell(victim, entities, Some(terrain));
            let Some(source_should_be_on_bridge) =
                should_be_on_bridge_for_response(candidate, entities, terrain)
            else {
                continue;
            };
            let source_in_playfield =
                cell_is_in_playfield_height_aware(destination, sim.playfield_bounds, Some(terrain));
            let movement_zone = (candidate_object.movement_zone != MovementZone::Invalid)
                .then_some(candidate_object.movement_zone);
            if !sim
                .zone_grid
                .as_ref()
                .expect("positive scan validated zone grid")
                .can_reach_base_defense_response(
                    movement_zone,
                    destination,
                    victim_destination,
                    source_should_be_on_bridge,
                    source_in_playfield,
                    i32::from(sim.session.map_width),
                    i32::from(sim.session.map_height),
                )
            {
                continue;
            }

            let raw_score = evaluate_target_threat(ThreatFacts {
                cost: candidate_object.cost,
                speed_leptons_per_frame: candidate_object.speed,
                current_coord: entity_coord(candidate, Some(terrain)),
                attacker_coord,
                primary_range_leptons: primary_range_leptons(
                    candidate,
                    candidate_object,
                    entities,
                    rules,
                    &sim.interner,
                ),
                existing_target: current_target_disposition(
                    candidate,
                    attacker_id,
                    entities,
                    rules,
                    &sim.interner,
                ),
                in_non_base_defense_team: sim
                    .team_script_vm
                    .team_for_member(candidate_id)
                    .is_some_and(|(_, is_base_defense)| !is_base_defense),
                mission_is_harvest: candidate.mission.current().known()
                    == Some(MissionType::Harvest),
            });
            selection.consider(candidate_id, raw_score, class, victim_is_self_anchor);
        }
    }

    let (budget, responders) = selection.into_ranked();
    if budget <= 0 {
        return;
    }

    let mut accumulated = 0;
    for responder in responders {
        let in_base_defense_team = world
            .team_script_vm
            .team_for_member(responder.entity_id)
            .is_some_and(|(_, is_base_defense)| is_base_defense);
        let draw = world.scenario_rng.next_range_u32_inclusive(0, 99);
        let mission = match response_mission(draw, in_base_defense_team) {
            ResponseMission::Rescue => MissionType::Rescue,
            ResponseMission::AreaGuard => MissionType::AreaGuard,
        };
        let attacker_commits = assign_target_commits(
            &world.substrate.entities,
            Some(TargetKind::Entity(attacker_id)),
        );
        let Some(responder_entity) = world.substrate.entities.get_mut(responder.entity_id) else {
            continue;
        };
        let Some(responder_object) =
            rules.object(world.interner.resolve(responder_entity.type_ref()))
        else {
            continue;
        };
        queue_entity_mission_deferred(responder_entity, MissionId::from_known(mission));
        responder_entity.set_archive_target(Some(TargetKind::Entity(victim_id)));
        represented_assign_target_admitted(
            responder_entity,
            Some(TargetKind::Entity(attacker_id)),
            attacker_commits,
        );
        let (next, overshot) = add_assigned_cost(accumulated, responder_object.cost, budget);
        accumulated = next;
        if overshot {
            if let Some(attacker) = world.substrate.entities.get_mut(attacker_id) {
                attacker.base_defense_response.cooldown.start(
                    current_frame,
                    response_delay_frames(rules.general.base_defense_delay_minutes),
                );
            }
            break;
        }
    }
}

#[cfg(test)]
#[path = "base_defense_response_tests.rs"]
mod tests;
