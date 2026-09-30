//! Infantry fear, prone stance, idle actions, and Crawls speed helpers.
//!
//! This module owns sim-authoritative infantry stance state. Animation reflects
//! this state; combat and movement do not infer prone status from animation.
//!
//! ## Idle-action residuals
//! Mission Guard, Hunt and AreaGuard call the object receiver at their native
//! dispatch point. The receiver owns its admission, wait timer, Scenario draws
//! and facing change; fidgets synchronously call the existing Do_Action owner.
//! Remaining mechanisms are explicit:
//!
//! - Ground Infantry's stage still uses the relative animation cascade at
//!   frame end rather than Techno AI's absolute timer. Completion timing can
//!   therefore differ, affecting later idle admission and RNG. The sequence
//!   completion and its facing belong to `movement::infantry_action` and
//!   `animation`.
//! - **The one-in-three voice comment** on the second fidget. It draws from the
//!   process-global stream, not the scenario one — its gate is local-player-only
//!   and therefore client-dependent, which is exactly why it cannot sit on the
//!   lockstep stream. Not drawn here, so it cannot disturb the scenario cursor.
//!   Needs the audio seam. Frequency: roughly one idle turn in nine, audio only.
//! - NULL-source Scatter (`51D0D0`) after an AI Fraidycat's panic, or idle roll
//!   8 for COW/AI Fraidycat, remains outside the damage Scatter owner's
//!   represented coordinate-source path. It can consume additional Scenario
//!   draws and alter navigation before the mission's cadence draw. Frequency:
//!   civilians on city maps; downstream risk: position and RNG divergence.
//!
//! The fear/prone gate's `Is_Moving` substitution remains UNCHECKED. The idle
//! receiver uses the existing actual locomotor motion query instead.

use crate::rules::object_type::ObjectType;
use crate::sim::animation::SequenceKind;
use crate::sim::game_entity::GameEntity;
use crate::util::fixed_math::{SIM_ZERO, SimFixed};

const MAX_FEAR: u16 = 300;
const FIRST_HIT_FEAR: u16 = 100;
/// Highest fear a damaging hit still latches back up to `FIRST_HIT_FEAR`.
///
/// gamemd's fear setter takes the repeated-hit ladder only when the hit has no
/// damager *or* fear is already above this value; every other damaging hit
/// re-latches. So the latch is not a first-hit special case — it fires on every
/// hit taken while fear sits anywhere in `0..=99`, which is the whole three-second
/// window after an infantryman stands back up.
const FEAR_LATCH_CEILING: u16 = 99;
const REPEATED_RED_ADD: u16 = 50;
const REPEATED_YELLOW_ADD: u16 = 25;
#[cfg(test)]
const REPEATED_GREEN_ADD: u16 = 12;
const PRONE_THRESHOLD: u16 = 50;
const VETERAN_LEVEL: u16 = 100;
const ELITE_LEVEL: u16 = 200;

pub fn has_veteran_fearless_ability(obj: &ObjectType, entity: &GameEntity) -> bool {
    if entity.veterancy() >= ELITE_LEVEL {
        obj.veteran_fearless || obj.elite_fearless
    } else if entity.veterancy() >= VETERAN_LEVEL {
        obj.veteran_fearless
    } else {
        false
    }
}

pub fn is_fear_application_blocked(obj: &ObjectType, entity: &GameEntity) -> bool {
    obj.fearless || has_veteran_fearless_ability(obj, entity)
}

pub fn can_decay_fear(obj: &ObjectType) -> bool {
    !obj.fearless
}

pub fn apply_panic_force(obj: &ObjectType, entity: &mut GameEntity) {
    if is_fear_application_blocked(obj, entity) {
        return;
    }
    if let Some(infantry) = entity.infantry.as_mut() {
        infantry.fear_level = MAX_FEAR;
    }
}

pub fn apply_fear_from_damage(
    obj: &ObjectType,
    entity: &mut GameEntity,
    damage_landed: i32,
    damager_present: bool,
    condition_red_ratio: f64,
    condition_yellow_ratio: f64,
) {
    if damage_landed == 0 || entity.health.current == 0 || is_fear_application_blocked(obj, entity)
    {
        return;
    }
    let Some(infantry) = entity.infantry.as_mut() else {
        return;
    };
    // gamemd: a hit that names a damager and finds fear at or below the latch
    // ceiling *sets* fear outright — 300 for a Fraidycat, 100 otherwise — instead
    // of adding to it. Only a hit with no damager, or one taken while fear is
    // already above the ceiling, runs the health-band ladder below.
    if damager_present && infantry.fear_level <= FEAR_LATCH_CEILING {
        infantry.fear_level = if obj.fraidycat {
            MAX_FEAR
        } else {
            FIRST_HIT_FEAR
        };
        return;
    }

    let add = repeated_fear_add(
        entity.health.ratio(obj.strength),
        condition_red_ratio,
        condition_yellow_ratio,
    );
    infantry.fear_level = infantry.fear_level.saturating_add(add).min(MAX_FEAR);
}

fn repeated_fear_add(
    ratio: crate::util::native_x87::MaskedX87Value,
    condition_red_ratio: f64,
    condition_yellow_ratio: f64,
) -> u16 {
    use crate::util::native_x87::{MaskedX87Chop53 as X87, MaskedX87Ordering, NativeF64Bits};
    // Infantry518CEC..518D29: independent Greater predicates. Unordered
    // retains50 at the red compare and skips the yellow halving.
    let red = X87::load_f64(NativeF64Bits::from_bits(condition_red_ratio.to_bits()));
    let yellow = X87::load_f64(NativeF64Bits::from_bits(condition_yellow_ratio.to_bits()));
    let mut add = if X87::compare(ratio, red) == MaskedX87Ordering::Greater {
        REPEATED_YELLOW_ADD
    } else {
        REPEATED_RED_ADD
    };
    if X87::compare(ratio, yellow) == MaskedX87Ordering::Greater {
        add /= 2;
    }
    add
}

/// Whether this infantryman is under way, in the sense the fear handler tests.
///
/// gamemd reads two separate things and takes either: the foot object's
/// destination field, and the locomotor's `Is_Moving` slot — which for the Walk
/// locomotor every infantryman carries is just its own moving byte, with none of
/// the extra conjuncts the *readiness* slot (`Is_Moving_Now`) layers on. Both
/// mean "this man has somewhere to be", so both map onto the two carriers VERA
/// keeps for that: the NavCom and the live movement order.
///
/// The correspondence is traced but UNCHECKED — no gamemd-derived executable
/// check compares the two.
fn fear_prone_is_under_way(entity: &GameEntity) -> bool {
    entity.navigation.nav_com.is_some() || entity.movement_target.is_some()
}

/// Decay fear one step and return the stance transition it forces, if any.
///
/// `player_controlled` is the owning house's player-control fact. gamemd refuses
/// to drop a *player-controlled* infantryman prone while he is on his way
/// somewhere — a squad walked through fire keeps walking instead of crawling —
/// while an AI-owned one goes down regardless.
pub fn tick_fear_decay_and_prone(
    obj: &ObjectType,
    entity: &mut GameEntity,
    player_controlled: bool,
) -> Option<SequenceKind> {
    if !can_decay_fear(obj) {
        return None;
    }
    // Sampled before the runtime borrow; gamemd reads both inside this handler.
    let under_way = player_controlled && fear_prone_is_under_way(entity);
    let dying = entity.dying;
    let deploying = entity.deploy_state.is_some();
    let Some(infantry) = entity.infantry.as_mut() else {
        return None;
    };
    if infantry.fear_level > 0 {
        infantry.fear_level -= 1;
    }
    if dying || deploying {
        return None;
    }

    if !infantry.is_prone && infantry.fear_level >= PRONE_THRESHOLD {
        // Player-control skip first, exactly as in gamemd: it precedes the
        // Fraidycat test and leaves fear decaying without any stance change.
        if under_way {
            return None;
        }
        if obj.crawls && !obj.fraidycat {
            infantry.is_prone = true;
            return Some(SequenceKind::Down);
        }
        return None;
    }
    if infantry.is_prone && infantry.fear_level < PRONE_THRESHOLD {
        infantry.is_prone = false;
        return Some(SequenceKind::Up);
    }
    None
}

/// RESIDUAL (GSI-08.13) — death sequences are not selected. `SequenceKind`
/// carries `Die1`..`Die5` and the sprite atlas can play them, but nothing in
/// sim ever picks one: a killed infantryman spawns the warhead's `InfDeath=`
/// anim (100 stock entries) and nothing else, so burn, electrocute, tumble and
/// vaporise all collapse into a single generic death. `WetDie1/2`, `Tumble` and
/// the `AirDeath*` ids are unmapped as well.
/// - Trigger: every infantry death.
/// - Player effect: no flame death, no Tesla frazzle, no vaporise — the visual
///   payoff of picking the right weapon against infantry is missing.
/// - Frequency: continuous; infantry are the most-killed class in the game.
/// - Downstream risk: sequence choice reads the warhead's death type, so it
///   couples this row to the warhead-effect row above it; the animation itself
///   is presentation, but the selection is sim state and hashed.
pub fn tick_fear_for_entities(
    entities: &mut crate::sim::entity_store::EntityStore,
    houses: &std::collections::BTreeMap<
        crate::sim::intern::InternedId,
        crate::sim::house_state::HouseState,
    >,
    rules: &crate::rules::ruleset::RuleSet,
    interner: &crate::sim::intern::StringInterner,
) {
    let keys = entities.keys_sorted();
    for id in keys {
        // InfantryClass::AI returns before its fear work while warped. With no
        // fear to decay and no prone stance to leave the handler changes
        // nothing.
        let Some(entity) = entities.get_mut_if(id, |entity| {
            !entity.ai_frozen()
                && entity
                    .infantry
                    .as_ref()
                    .is_some_and(|infantry| infantry.fear_level > 0 || infantry.is_prone)
        }) else {
            continue;
        };
        let Some(obj) = rules.object(interner.resolve(entity.type_ref())) else {
            continue;
        };
        // `HouseState::is_human` is this model's collapsed player-control fact —
        // the same byte pair gamemd's `IsPlayerControl` reads.
        let player_controlled = houses
            .get(&entity.owner())
            .is_some_and(|house| house.is_human);
        if let Some(sequence) = tick_fear_decay_and_prone(obj, entity, player_controlled) {
            if let Some(anim) = entity.animation.as_mut() {
                anim.switch_to(sequence);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Idle actions
// ---------------------------------------------------------------------------

/// Lower end of the idle wait, in frames per unit of `IdleActionFrequency`.
const IDLE_WAIT_FLOOR_FRAMES: i32 = 450;
/// Upper end of the idle wait, in frames per unit of `IdleActionFrequency`.
const IDLE_WAIT_CEILING_FRAMES: i32 = 1800;
/// Inclusive top of the draw gamemd uses as the idle wait's random fraction,
/// scaled by the recorded native reciprocal at `0x007E3570`.
const IDLE_WAIT_FRACTION_MAX: u32 = 0x7fff_fffe;
/// Inclusive top of the idle action roll. Eleven outcomes, one of which (0) is
/// the do-nothing arm — the reason infantry do not fidget on every timer expiry.
const IDLE_ROLL_MAX: u32 = 10;
/// Inclusive top of the idle facing draw — the eight infantry facings.
const IDLE_FACING_MAX: u32 = 7;
/// Facing bytes between two adjacent eighth-turns (256 / 8).
const IDLE_FACING_STEP: u8 = crate::util::direction::FACING_UNITS_PER_DIRECTION;
/// Fear above which a Fraidycat type panics out of the idle turn instead.
const IDLE_PANIC_FEAR: u16 = 50;
/// The one type whose idle roll is biased, by name. gamemd tests the object's
/// type against this string and, on a second sub-roll, forces the wandering arm.
const IDLE_BIASED_TYPE: &str = "COW";
/// The biased type takes the wander arm when its sub-roll lands under this.
const IDLE_BIAS_THRESHOLD: u32 = 5;

/// What one idle turn decided to do.
///
/// gamemd's eleven-way roll has four outcomes: nothing, the two fidget
/// sequences, and a random facing change (which four of the eleven arms take).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IdleAction {
    /// Roll 0 — the turn is spent without doing anything.
    Nothing,
    /// Rolls 3, 4, 5 — the first fidget.
    Fidget1,
    /// Rolls 1, 2, 7 — the second fidget. gamemd also rolls a one-in-three
    /// voice comment here for a local-player-owned man; that draw comes off the
    /// process-global stream and is not modelled (see the module residual).
    Fidget2,
    /// Rolls 6, 8, 9, 10 — turn to a random facing.
    TurnInPlace,
}

/// The frames an infantryman waits before his next idle turn.
///
/// Infantry51CDD9..51CE26 stores the two products as doubles, reloads their
/// difference, scales the signed draw by native7E3570, then consumes ftol's low
/// DWORD. The timer controls subsequent Scenario draws: the old milliunit
/// ratio changed this outcome for reader-admitted frequencies. Reuse the
/// existing deterministic arithmetic owner instead of quantizing or clamping.
/// Native execution: `anytown_damage/foot_missions` idle controls.
fn idle_wait_frames(frequency: f64, fraction: u32) -> u32 {
    use crate::util::native_x87::{MaskedX87Chop53 as X87, NativeF64Bits};
    let frequency = X87::load_f64(NativeF64Bits::from_bits(frequency.to_bits()));
    let ceiling = X87::load_f64(X87::store_f64_masked_chop(X87::mul(
        frequency,
        X87::load_i32(IDLE_WAIT_CEILING_FRAMES),
    )));
    let floor = X87::load_f64(X87::store_f64_masked_chop(X87::mul(
        frequency,
        X87::load_i32(IDLE_WAIT_FLOOR_FRAMES),
    )));
    let fraction = X87::mul(
        X87::load_i32(fraction as i32),
        X87::load_f64(crate::sim::rng::RANDOM_RANGED_UNIT_SCALE),
    );
    X87::ftol_i32_low_masked(X87::add(
        X87::mul(X87::sub(ceiling, floor), fraction),
        floor,
    )) as u32
}

/// Turn one idle roll into the action it selects.
fn idle_action_for_roll(roll: u32) -> IdleAction {
    match roll {
        1 | 2 | 7 => IdleAction::Fidget2,
        3 | 4 | 5 => IdleAction::Fidget1,
        6 | 8..=IDLE_ROLL_MAX => IdleAction::TurnInPlace,
        _ => IdleAction::Nothing,
    }
}

/// Infantry5216D0 calls the timer predicate7099E0, then actual ILocomotion+10
/// before prone6DB, firing68D and Doing6C4. Caller mission/logic-vector
/// admissions stay with those callers: neither NavCom nor animation nor a
/// target is an idle receiver gate. Unsupported motion payloads are explicit
/// errors; no adapter or sequence is substituted for their missing query.
fn idle_action_ready(entity: &GameEntity, frame: u32) -> Result<bool, String> {
    use crate::sim::movement::infantry_action::{DO_GUARD, DO_READY, DO_TREAD};
    let Some(infantry) = entity.infantry.as_ref() else {
        return Ok(false);
    };
    if !infantry.idle_action_timer.due(frame) {
        return Ok(false);
    }
    let moving = crate::sim::movement::motion_query::is_moving(entity)
        .ok_or("Infantry idle action requires represented locomotor motion")?;
    let leaf = entity
        .mission_leaf
        .as_infantry()
        .ok_or("Infantry idle action requires native Doing")?;
    Ok(!moving
        && !infantry.is_prone
        && leaf.firing_sequence_latch() == 0
        && matches!(leaf.doing(), DO_READY | DO_GUARD | DO_TREAD))
}

/// Point an idle infantryman at one of the eight facings, with no turn animation.
///
/// gamemd converts the `0..=7` draw to a facing byte of `index * 32` and pushes
/// it through the body's snap setter (`+0x388` Set_Current, `0x0051CF34`/
/// `0x0051D014`/`0x0051D092`) — the same no-smoothing path spawn and deploy
/// use, which is why an idle man appears to have simply turned rather than
/// rotated.
fn set_idle_facing(entity: &mut GameEntity, facing_index: u8, frame: u32) {
    let facing_byte = facing_index.wrapping_mul(IDLE_FACING_STEP);
    entity.body_facing.snap(u16::from(facing_byte) << 8, frame);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IdleTurn {
    Refused,
    Spent,
    Fidget(i32),
    NullSourceScatter,
}

/// Single Infantry51CDB0 decision body. The world receiver applies its class
/// action immediately, before a mission caller can spend its cadence draw.
fn select_idle_action(
    entity: &mut GameEntity,
    object: &ObjectType,
    controlled_by_human: bool,
    frequency: f64,
    rng: &mut crate::sim::rng::SimRng,
    frame: u32,
) -> Result<IdleTurn, String> {
    use crate::sim::movement::infantry_action::{DO_IDLE1, DO_IDLE2};
    if !idle_action_ready(entity, frame)? {
        return Ok(IdleTurn::Refused);
    }
    let wait = idle_wait_frames(
        frequency,
        rng.next_range_u32_inclusive(0, IDLE_WAIT_FRACTION_MAX),
    );
    let infantry = entity.infantry.as_mut().expect("admitted Infantry");
    infantry.idle_action_timer.defer(frame, wait);
    //51CE35..3E precede both the panic gate and every action/facing draw.
    if object.fraidycat && !controlled_by_human && infantry.fear_level > IDLE_PANIC_FEAR {
        return Ok(IdleTurn::NullSourceScatter);
    }
    let biased_type = object.id.eq_ignore_ascii_case(IDLE_BIASED_TYPE);
    let mut roll = rng.next_range_u32_inclusive(0, IDLE_ROLL_MAX);
    if biased_type && rng.next_range_u32_inclusive(0, IDLE_ROLL_MAX) < IDLE_BIAS_THRESHOLD {
        roll = 8;
    }
    Ok(match idle_action_for_roll(roll) {
        IdleAction::Fidget1 => IdleTurn::Fidget(DO_IDLE1),
        IdleAction::Fidget2 => IdleTurn::Fidget(DO_IDLE2),
        IdleAction::TurnInPlace => {
            let index = rng.next_range_u32_inclusive(0, IDLE_FACING_MAX) as u8;
            set_idle_facing(entity, index, frame);
            if roll == 8 && (biased_type || (object.fraidycat && !controlled_by_human)) {
                IdleTurn::NullSourceScatter
            } else {
                IdleTurn::Spent
            }
        }
        IdleAction::Nothing => IdleTurn::Spent,
    })
}

impl crate::sim::world::Simulation {
    /// Infantry UpdateIdleAction51CDB0, called synchronously by the mission
    /// handler. Returns its native admitted result even when Do_Action refuses.
    /// Original E1 AreaGuard4D6F2B invokes this before cadence4D703A/65C7E0;
    /// `anytown_damage/foot_missions` records the ordered native transaction.
    pub(crate) fn infantry_idle_action(
        &mut self,
        id: u64,
        rules: &crate::rules::ruleset::RuleSet,
    ) -> Result<bool, String> {
        let Some(actor) = self.substrate.entities.get(id) else {
            return Ok(false);
        };
        let object = self
            .object_type(actor.type_ref(), rules)
            .ok_or("Infantry idle action requires its native type")?;
        let controlled_by_human = self
            .houses
            .get(&actor.owner())
            .is_some_and(|house| house.is_controlled_by_human(self.session.game_mode_nonzero));
        let turn = select_idle_action(
            self.substrate
                .entities
                .get_mut(id)
                .expect("retained Infantry"),
            object,
            controlled_by_human,
            rules.general.idle_action_frequency,
            &mut self.scenario_rng,
            self.session.binary_frame,
        )?;
        match turn {
            IdleTurn::Refused => return Ok(false),
            IdleTurn::Fidget(action) => {
                //51CEEA/51CF4C pass action9/10, force0, random-stage0 for
                // every Infantry class, including ordinary ground E1.
                let _accepted = self.infantry_do_action(id, action, false, rules)?;
            }
            IdleTurn::NullSourceScatter => {
                // The existing damage Scatter owner accepts a real source;
                // native idle passes NullCoordA8F200, forced1, no-Kick0. Its
                // additional navigation/RNG path remains a separate mechanism.
                log::debug!("infantry {id} idle NULL-source Scatter is not represented");
            }
            IdleTurn::Spent => {}
        }
        Ok(true)
    }
}

/// Test compatibility for old logic-vector fixtures. Production mission
/// callers use the synchronous object receiver above. This adapter keeps only
/// outer AI/logic-vector gates and delegates every idle decision to its owner;
/// returned fidgets require the real Do_Action receiver to enact them.
#[cfg(test)]
pub(crate) fn tick_idle_actions(
    entities: &mut crate::sim::entity_store::EntityStore,
    order: &[u64],
    houses: &std::collections::BTreeMap<
        crate::sim::intern::InternedId,
        crate::sim::house_state::HouseState,
    >,
    rules: &crate::rules::ruleset::RuleSet,
    interner: &crate::sim::intern::StringInterner,
    rng: &mut crate::sim::rng::SimRng,
    frame: u32,
) -> Vec<(u64, i32)> {
    let mut fidgets = Vec::new();
    for &id in order {
        let Some(entity) = entities.get_mut(id) else {
            continue;
        };
        if (entity.lifecycle.in_limbo && !entity.passenger_role.in_open_transport())
            || !entity.is_active()
            || entity.ai_frozen()
        {
            continue;
        }
        let type_name = interner.resolve(entity.type_ref());
        let Some(obj) = rules.object(type_name) else {
            continue;
        };
        let controlled_by_human = houses
            .get(&entity.owner())
            .is_some_and(|house| house.is_controlled_by_human(false));
        if let IdleTurn::Fidget(action) = select_idle_action(
            entity,
            obj,
            controlled_by_human,
            rules.general.idle_action_frequency,
            rng,
            frame,
        )
        .expect("native idle fixture requires represented Infantry state")
        {
            fidgets.push((id, action));
        }
    }
    fidgets
}

pub fn is_prone_for_damage(entity: &GameEntity) -> bool {
    entity.infantry.is_some_and(|infantry| infantry.is_prone)
}

pub fn apply_prone_speed(speed: SimFixed, crawls: bool) -> SimFixed {
    if speed <= SIM_ZERO {
        return speed;
    }
    let whole_speed = speed.to_num::<i32>().max(0);
    let adjusted = if crawls {
        (whole_speed.saturating_mul(2) + 2) / 3
    } else {
        whole_speed + whole_speed / 2
    };
    SimFixed::from_num(adjusted)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::entities::EntityCategory;
    use crate::rules::ini_parser::IniFile;
    use crate::rules::object_type::ObjectCategory;
    use crate::rules::ruleset::RuleSet;
    use crate::sim::components::Health;
    use crate::sim::game_entity::{GameEntity, InfantryRuntime};
    use crate::sim::intern::test_intern;

    fn rules_for(section: &str) -> RuleSet {
        RuleSet::from_ini(&IniFile::from_str(&format!(
            "[InfantryTypes]\n0=E1\n\n[VehicleTypes]\n\n[AircraftTypes]\n\n[BuildingTypes]\n\n[E1]\nStrength=100\nArmor=flak\nSpeed=4\n{section}\n"
        )))
        .expect("rules should parse")
    }

    fn infantry_obj(section: &str, crawls: bool) -> crate::rules::object_type::ObjectType {
        let rules = rules_for(section);
        let mut obj = rules.object("E1").expect("E1").clone();
        obj.crawls = crawls;
        obj
    }

    fn infantry(hp: i32) -> GameEntity {
        let mut e = GameEntity::new_at_frame_zero_for_test(
            1,
            0,
            0,
            0,
            0,
            test_intern("Test"),
            Health { current: hp },
            test_intern("E1"),
            EntityCategory::Infantry,
            0,
            5,
            false,
        );
        e.infantry = Some(InfantryRuntime::new());
        // `ObjectLifecycle` defaults to in-limbo; a man standing on the map has
        // been unlimboed, and the idle gate reads that.
        e.lifecycle.in_limbo = false;
        e
    }

    #[test]
    fn wide_damage_and_live_signed_strength_reach_native_fear_ladder() {
        let mut obj = infantry_obj("", false);
        let mut entity = infantry(100);
        obj.strength = 100;
        apply_fear_from_damage(&obj, &mut entity, 65536, true, 0.25, 0.5);
        assert_eq!(entity.infantry.as_ref().unwrap().fear_level, 100);
        obj.strength = -100;
        apply_fear_from_damage(&obj, &mut entity, 65536, false, 0.25, 0.5);
        assert_eq!(entity.infantry.as_ref().unwrap().fear_level, 150);
        obj.strength = 400;
        apply_fear_from_damage(&obj, &mut entity, 65536, false, 0.25, 0.5);
        assert_eq!(entity.infantry.as_ref().unwrap().fear_level, 200);
        // Both comparisons execute; reversed thresholds are not an else-if ladder.
        obj.strength = 100;
        apply_fear_from_damage(&obj, &mut entity, 65536, false, 2.0, 0.5);
        assert_eq!(entity.infantry.as_ref().unwrap().fear_level, 225);
    }

    #[test]
    fn first_hit_and_fraidycat_set_fear() {
        let rules = rules_for("");
        let obj = rules.object("E1").unwrap();
        let mut e = infantry(90);
        apply_fear_from_damage(obj, &mut e, 10, true, 0.25, 0.5);
        assert_eq!(e.infantry.unwrap().fear_level, FIRST_HIT_FEAR);

        let rules = rules_for("Fraidycat=yes\n");
        let obj = rules.object("E1").unwrap();
        let mut e = infantry(90);
        apply_fear_from_damage(obj, &mut e, 10, true, 0.25, 0.5);
        assert_eq!(e.infantry.unwrap().fear_level, MAX_FEAR);
    }

    #[test]
    fn hit_inside_the_latch_band_snaps_fear_back_up() {
        // The gap this pins: a hit taken while fear is already part-way decayed.
        // gamemd re-latches to 100 (300 Fraidycat) anywhere in 0..=99, which is
        // what keeps infantry pinned prone under sustained fire; the previous
        // code returned without touching fear for 1..=99, so they popped up.
        let rules = rules_for("");
        let obj = rules.object("E1").unwrap();
        for start in [1u16, 40, FEAR_LATCH_CEILING] {
            let mut e = infantry(90);
            e.infantry.as_mut().unwrap().fear_level = start;
            apply_fear_from_damage(obj, &mut e, 10, true, 0.25, 0.5);
            assert_eq!(
                e.infantry.unwrap().fear_level,
                FIRST_HIT_FEAR,
                "fear {start} should re-latch to {FIRST_HIT_FEAR}"
            );
        }

        let rules = rules_for("Fraidycat=yes\n");
        let obj = rules.object("E1").unwrap();
        let mut e = infantry(90);
        e.infantry.as_mut().unwrap().fear_level = 40;
        apply_fear_from_damage(obj, &mut e, 10, true, 0.25, 0.5);
        assert_eq!(e.infantry.unwrap().fear_level, MAX_FEAR);
    }

    #[test]
    fn above_the_latch_band_still_takes_the_health_ladder() {
        // The boundary the latch must not swallow: at 100 the ladder applies, so
        // a full-health hit adds 12 rather than resetting to 100.
        let rules = rules_for("");
        let obj = rules.object("E1").unwrap();
        let mut e = infantry(100);
        e.infantry.as_mut().unwrap().fear_level = FEAR_LATCH_CEILING + 1;
        apply_fear_from_damage(obj, &mut e, 10, true, 0.25, 0.5);
        assert_eq!(
            e.infantry.unwrap().fear_level,
            FEAR_LATCH_CEILING + 1 + REPEATED_GREEN_ADD
        );
    }

    #[test]
    fn player_controlled_infantry_under_way_does_not_go_prone() {
        use crate::sim::components::{MovementTarget, NavTargetRef};

        let obj = infantry_obj("", true);

        // A player-owned man with a destination keeps walking; fear still decays.
        let mut walking = infantry(100);
        walking.infantry.as_mut().unwrap().fear_level = 51;
        walking.navigation.nav_com = Some(NavTargetRef::cell(4, 4));
        assert_eq!(tick_fear_decay_and_prone(&obj, &mut walking, true), None);
        let runtime = walking.infantry.unwrap();
        assert_eq!(runtime.fear_level, 50);
        assert!(!runtime.is_prone);

        // Same for a live movement order rather than a NavCom.
        let mut walking = infantry(100);
        walking.infantry.as_mut().unwrap().fear_level = 51;
        walking.movement_target = Some(MovementTarget::default());
        assert_eq!(tick_fear_decay_and_prone(&obj, &mut walking, true), None);
        assert!(!walking.infantry.unwrap().is_prone);

        // The identical AI-owned man goes down — the gate is player-control only.
        let mut ai = infantry(100);
        ai.infantry.as_mut().unwrap().fear_level = 51;
        ai.navigation.nav_com = Some(NavTargetRef::cell(4, 4));
        assert_eq!(
            tick_fear_decay_and_prone(&obj, &mut ai, false),
            Some(SequenceKind::Down)
        );
        assert!(ai.infantry.unwrap().is_prone);

        // And a player-owned man standing still still goes down.
        let mut standing = infantry(100);
        standing.infantry.as_mut().unwrap().fear_level = 51;
        assert_eq!(
            tick_fear_decay_and_prone(&obj, &mut standing, true),
            Some(SequenceKind::Down)
        );
        assert!(standing.infantry.unwrap().is_prone);
    }

    #[test]
    fn under_way_gate_never_blocks_standing_back_up() {
        use crate::sim::components::NavTargetRef;

        // The skip guards the Down branch only: a prone player-owned man ordered
        // to move must still get his Up when fear falls below the threshold.
        let obj = infantry_obj("", true);
        let mut prone = infantry(100);
        prone.infantry.as_mut().unwrap().fear_level = PRONE_THRESHOLD;
        prone.infantry.as_mut().unwrap().is_prone = true;
        prone.navigation.nav_com = Some(NavTargetRef::cell(4, 4));

        assert_eq!(
            tick_fear_decay_and_prone(&obj, &mut prone, true),
            Some(SequenceKind::Up)
        );
        assert!(!prone.infantry.unwrap().is_prone);
    }

    #[test]
    fn repeated_hit_adds_by_health_and_clamps() {
        let rules = rules_for("");
        let obj = rules.object("E1").unwrap();
        for (hp, expected) in [(80, 112), (50, 125), (25, 150)] {
            let mut e = infantry(hp);
            e.infantry.as_mut().unwrap().fear_level = 100;
            apply_fear_from_damage(obj, &mut e, 1, true, 0.25, 0.5);
            assert_eq!(e.infantry.unwrap().fear_level, expected);
        }
        let mut e = infantry(25);
        e.infantry.as_mut().unwrap().fear_level = 290;
        apply_fear_from_damage(obj, &mut e, 1, true, 0.25, 0.5);
        assert_eq!(e.infantry.unwrap().fear_level, MAX_FEAR);
    }

    #[test]
    fn fearless_type_and_abilities_block_application() {
        let rules = rules_for("Fearless=yes\n");
        let obj = rules.object("E1").unwrap();
        let mut e = infantry(90);
        apply_fear_from_damage(obj, &mut e, 1, true, 0.25, 0.5);
        apply_panic_force(obj, &mut e);
        assert_eq!(e.infantry.unwrap().fear_level, 0);

        let rules = rules_for("VeteranAbilities=FEARLESS\n");
        let obj = rules.object("E1").unwrap();
        let mut e = infantry(90);
        e.set_veterancy_rank(100);
        apply_fear_from_damage(obj, &mut e, 1, true, 0.25, 0.5);
        assert_eq!(e.infantry.unwrap().fear_level, 0);

        let rules = rules_for("EliteAbilities=FEARLESS\n");
        let obj = rules.object("E1").unwrap();
        let mut e = infantry(90);
        e.set_veterancy_rank(200);
        apply_panic_force(obj, &mut e);
        assert_eq!(e.infantry.unwrap().fear_level, 0);
    }

    #[test]
    fn decay_thresholds_and_fearless_decay_gate() {
        let obj = infantry_obj("", true);
        let mut e = infantry(100);
        e.infantry.as_mut().unwrap().fear_level = 50;
        assert_eq!(tick_fear_decay_and_prone(&obj, &mut e, false), None);
        assert!(!e.infantry.unwrap().is_prone);

        let mut e = infantry(100);
        e.infantry.as_mut().unwrap().fear_level = 51;
        assert_eq!(
            tick_fear_decay_and_prone(&obj, &mut e, false),
            Some(SequenceKind::Down)
        );
        assert!(e.infantry.unwrap().is_prone);

        let mut e = infantry(100);
        e.infantry.as_mut().unwrap().fear_level = 50;
        e.infantry.as_mut().unwrap().is_prone = true;
        assert_eq!(
            tick_fear_decay_and_prone(&obj, &mut e, false),
            Some(SequenceKind::Up)
        );
        assert!(!e.infantry.unwrap().is_prone);

        let rules = rules_for("Fearless=yes\n");
        let obj = rules.object("E1").unwrap();
        let mut e = infantry(100);
        e.infantry.as_mut().unwrap().fear_level = 100;
        assert_eq!(tick_fear_decay_and_prone(obj, &mut e, false), None);
        assert_eq!(e.infantry.unwrap().fear_level, 100);

        let obj = infantry_obj("VeteranAbilities=FEARLESS\n", true);
        let mut e = infantry(100);
        e.set_veterancy_rank(100);
        e.infantry.as_mut().unwrap().fear_level = 100;
        assert_eq!(
            tick_fear_decay_and_prone(&obj, &mut e, false),
            Some(SequenceKind::Down)
        );
        assert_eq!(e.infantry.unwrap().fear_level, 99);
    }

    #[test]
    fn fraidycat_rejects_fear_driven_down() {
        for crawls in [true, false] {
            let obj = infantry_obj("Fraidycat=yes\n", crawls);
            let mut e = infantry(100);
            e.infantry.as_mut().unwrap().fear_level = MAX_FEAR;

            assert_eq!(tick_fear_decay_and_prone(&obj, &mut e, false), None);
            let infantry = e.infantry.unwrap();
            assert_eq!(infantry.fear_level, MAX_FEAR - 1);
            assert!(!infantry.is_prone);
        }
    }

    #[test]
    fn crawls_gate_only_blocks_down_not_recovery() {
        let obj = infantry_obj("", false);
        let mut standing = infantry(100);
        standing.infantry.as_mut().unwrap().fear_level = 51;

        assert_eq!(tick_fear_decay_and_prone(&obj, &mut standing, false), None);
        let runtime = standing.infantry.unwrap();
        assert_eq!(runtime.fear_level, 50);
        assert!(!runtime.is_prone);

        let obj = infantry_obj("", true);
        let mut standing = infantry(100);
        standing.infantry.as_mut().unwrap().fear_level = 51;

        assert_eq!(
            tick_fear_decay_and_prone(&obj, &mut standing, false),
            Some(SequenceKind::Down)
        );
        let runtime = standing.infantry.unwrap();
        assert_eq!(runtime.fear_level, 50);
        assert!(runtime.is_prone);

        let obj = infantry_obj("", false);
        let mut prone = infantry(100);
        prone.infantry.as_mut().unwrap().fear_level = 50;
        prone.infantry.as_mut().unwrap().is_prone = true;

        assert_eq!(
            tick_fear_decay_and_prone(&obj, &mut prone, false),
            Some(SequenceKind::Up)
        );
        let runtime = prone.infantry.unwrap();
        assert_eq!(runtime.fear_level, 49);
        assert!(!runtime.is_prone);
    }

    fn idle_corpus() -> serde_json::Value {
        let native: serde_json::Value = serde_json::from_str(include_str!(
            "../../tools/spatial_oracle/anytown_damage/foot_missions.json"
        ))
        .unwrap();
        assert_eq!(
            native["native_sha256"],
            "1cdd1180e49024fbda8ad568caac2e86e856063ff67ab38f62b7d2c7bb84298c"
        );
        native
    }

    fn native_rng_state(rng: &crate::sim::rng::SimRng) -> serde_json::Value {
        let state = rng.logical_view();
        serde_json::json!({
            "disabled": state.disabled,
            "index_a": state.index_a,
            "index_b": state.index_b,
            "state": state.words,
        })
    }

    /// Full original51CDB0 + actual75AB30 + original51D6F0 on the physical
    /// E1/GISequence. Native reader66B3EA processes stock AudioVisual .15.
    /// Supplied ready/action/nav/target/timer controls isolate receiver
    /// admission; Guard/AreaGuard/Hunt caller admissions are separate tests.
    /// Native stack auxiliary timer words and the absolute sequence timer are
    /// outside this represented receiver comparison.
    #[test]
    fn native_e1_idle_receiver_matches_timer_action_facing_and_full_rng() {
        use crate::rules::native_processing::RulesLayerStack;
        use crate::sim::animation::Animation;
        use crate::sim::combat::AttackTarget;
        use crate::sim::components::{DriveCoord, NavTargetRef};
        use crate::sim::mission::MissionTimer;
        use crate::sim::movement::locomotor::LocomotorState;
        use crate::sim::rng::{SimRng, trace_draws};
        use crate::sim::world::Simulation;
        use serde_json::{Value, json};

        let Some(ini) = crate::rules::retail_ini_fixture::retail_ini("rulesmd.ini") else {
            return;
        };
        let Some(art) = crate::rules::retail_ini_fixture::retail_ini("artmd.ini") else {
            return;
        };
        let layers = RulesLayerStack::new(ini);
        let mut rules =
            RuleSet::from_processed_rules(&layers.process_with_fixed_art(&art).unwrap()).unwrap();
        rules.install_art_data(crate::rules::art_data::ArtRegistry::from_ini(&art));
        rules.bind_animation_sequences(
            &crate::rules::infantry_sequence::parse_infantry_sequence_registry(&art),
        );
        let native = idle_corpus();
        let signed = |value: &Value| value.as_i64().unwrap() as i32;
        let mut compared = 0;
        for row in native["retail_idle_rows"].as_array().unwrap() {
            if row["input"]["idle_control"]["entry"] != "idle" {
                continue;
            }
            let input = &row["input"];
            let before = &row["before"];
            let after = &row["after"];
            let name = input["name"].as_str().unwrap();
            let mut sim = Simulation::new();
            sim.session.binary_frame = 1;
            sim.scenario_rng = SimRng::new(input["scenario_seed"].as_u64().unwrap());
            assert_eq!(
                native_rng_state(&sim.scenario_rng),
                row["rng_before"]["scenario"],
                "{name} supplied RNG"
            );
            let frequency_bits: String = rules
                .general
                .idle_action_frequency
                .to_le_bytes()
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect();
            assert_eq!(
                frequency_bits, input["idle_control"]["idle_action_frequency_bits"],
                "{name} production reader"
            );
            let owner = sim.interner.intern("Americans");
            let mut actor = GameEntity::new_at_frame_zero_for_test(
                1,
                (signed(&before["position"][0]) / 256) as u16,
                (signed(&before["position"][1]) / 256) as u16,
                4,
                0,
                owner,
                Health { current: 100 },
                sim.interner.intern("E1"),
                EntityCategory::Infantry,
                0,
                0,
                false,
            );
            actor.lifecycle.in_limbo = false;
            actor.position.sub_x = SimFixed::from_num(signed(&before["position"][0]) % 256);
            actor.position.sub_y = SimFixed::from_num(signed(&before["position"][1]) % 256);
            actor.position.exact_z_leptons = Some(signed(&before["position"][2]));
            actor.on_bridge = before["on_bridge"] != 0;
            let object = rules.object("E1").unwrap();
            let mut loco = LocomotorState::from_object_type(object, 1);
            if before["walk_moving"]["value"].as_u64().unwrap() != 0 {
                // Existing native Move_To producer owns this retained byte;
                // readiness queries it independently of NavCom and animation.
                loco.set_walk_destination(Some(DriveCoord {
                    x: 22400,
                    y: 12544,
                    z: 416,
                }));
            }
            actor.locomotor = Some(loco);
            actor.infantry = Some(InfantryRuntime::new());
            let infantry = actor.infantry.as_mut().unwrap();
            infantry.is_prone = before["prone_6db"] != 0;
            infantry.idle_action_timer = MissionTimer::armed(
                signed(&before["idle_timer"][0]) as u32,
                signed(&before["idle_timer"][2]) as u32,
            );
            let doing = signed(&before["doing"]);
            actor
                .mission_leaf
                .set_infantry_doing_verified(doing)
                .unwrap();
            actor
                .mission_leaf
                .set_foot_firing_sequence(signed(&before["firing"]) as u8);
            let mut animation = Animation::new(
                crate::rules::infantry_sequence::action_kind(doing).unwrap_or(SequenceKind::Stand),
            );
            animation.frame_index = signed(&before["frame_f8"]) as u16;
            actor.animation = Some(animation);
            actor
                .body_facing
                .snap(signed(&before["primary_facing_words"][0]) as u16, 1);
            if before["target"] != "0x0" {
                actor.attack_target = Some(AttackTarget::new(2));
            }
            if before["nav"] != "0x0" {
                actor.navigation.nav_com = Some(NavTargetRef::cell(12, 10));
            }
            let target_before = actor.attack_target.as_ref().map(|target| target.target);
            let nav_before = actor.navigation.nav_com;
            sim.substrate.entities.insert(actor);
            let (admitted, draws) = trace_draws(|| sim.infantry_idle_action(1, &rules).unwrap());
            assert_eq!(admitted, row["returned_al"] != 0, "{name} return AL");
            let actor = sim.substrate.entities.get(1).unwrap();
            let leaf = actor.mission_leaf.as_infantry().unwrap();
            assert_eq!(leaf.doing(), signed(&after["doing"]), "{name} Doing");
            assert_eq!(
                leaf.firing_sequence_latch(),
                signed(&after["firing"]) as u8,
                "{name} firing"
            );
            assert_eq!(
                actor.infantry.unwrap().is_prone,
                after["prone_6db"] != 0,
                "{name} prone"
            );
            let timer = actor.infantry.unwrap().idle_action_timer;
            assert_eq!(
                json!([timer.start_frame as i32, timer.duration as i32]),
                json!([after["idle_timer"][0], after["idle_timer"][2]]),
                "{name} idle timer"
            );
            assert_eq!(
                actor.animation.as_ref().unwrap().frame_index,
                signed(&after["frame_f8"]) as u16,
                "{name} frame reset/preservation"
            );
            assert_eq!(
                actor.body_facing_current(1),
                signed(&after["primary_facing_words"][0]) as u16,
                "{name} facing"
            );
            assert_eq!(
                actor.attack_target.as_ref().map(|target| target.target),
                target_before,
                "{name} target untouched"
            );
            assert_eq!(
                actor.navigation.nav_com, nav_before,
                "{name} NavCom untouched"
            );
            let native_words: Vec<_> = row["events"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|event| event["kind"] == "rng_raw")
                .map(|event| event["value"].clone())
                .collect();
            let rust_words: Vec<_> = draws.iter().map(|event| event["value"].clone()).collect();
            assert_eq!(
                rust_words, native_words,
                "{name} raw draw order including rejection"
            );
            assert_eq!(
                native_rng_state(&sim.scenario_rng),
                row["rng_after"]["scenario"],
                "{name} full Scenario state"
            );
            compared += 1;
        }
        assert_eq!(compared, 28, "all appended direct receiver controls");
    }

    /// The original82 E1 AreaGuard row consumed the constructor frequency,
    /// before ReadAudioVisual. Keep that native boundary distinct from true
    /// retail .15; both timer receipts exercise the same arithmetic owner.
    #[test]
    fn native_default_and_retail_idle_wait_use_their_recorded_doubles() {
        let native = idle_corpus();
        let bits = |hex: &str| {
            let bytes: Vec<_> = hex
                .as_bytes()
                .chunks_exact(2)
                .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
                .collect();
            f64::from_le_bytes(bytes.try_into().unwrap())
        };
        let original = native["rows"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["input"]["name"] == "E1_area_guard_post")
            .unwrap();
        let retail = native["retail_idle_rows"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["input"]["name"] == "E1_retail_idle_direct_seed31")
            .unwrap();
        let draw = |row: &serde_json::Value| {
            row["events"]
                .as_array()
                .unwrap()
                .iter()
                .find(|event| event["kind"] == "rng" && event["caller"] == "0x51ce04")
                .unwrap()["result_eax"]
                .as_u64()
                .unwrap() as u32
        };
        let constructor_frequency = native["rules_reader_receipts"]
            ["handler_rules_before_physical"]["idle_action_frequency_bits"]
            .as_str()
            .unwrap();
        assert_eq!(
            idle_wait_frames(bits(constructor_frequency), draw(original)),
            original["after"]["idle_timer"][2].as_u64().unwrap() as u32,
        );
        assert_eq!(
            idle_wait_frames(
                bits(
                    retail["input"]["idle_control"]["idle_action_frequency_bits"]
                        .as_str()
                        .unwrap()
                ),
                draw(retail)
            ),
            retail["after"]["idle_timer"][2].as_u64().unwrap() as u32,
        );
        assert_ne!(
            original["after"]["idle_timer"][2],
            retail["after"]["idle_timer"][2]
        );
    }

    #[test]
    fn prone_speed_rounding_is_exact() {
        assert_eq!(
            apply_prone_speed(SimFixed::from_num(10), true),
            SimFixed::from_num(7)
        );
        assert_eq!(
            apply_prone_speed(SimFixed::from_num(11), true),
            SimFixed::from_num(8)
        );
        assert_eq!(
            apply_prone_speed(SimFixed::from_num(10), false),
            SimFixed::from_num(15)
        );
        assert_eq!(
            apply_prone_speed(SimFixed::from_num(11), false),
            SimFixed::from_num(16)
        );
    }

    #[test]
    fn object_category_import_keeps_rules_fixture_infantry() {
        assert_eq!(
            rules_for("").object("E1").unwrap().category,
            ObjectCategory::Infantry
        );
    }
}
