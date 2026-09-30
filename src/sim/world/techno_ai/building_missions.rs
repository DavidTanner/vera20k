//! A building's mission work in `BuildingClass::Update` (`0x0043FB20`),
//! around and inside `TechnoClass::AI_Update`:
//!
//! - the ready check before the Techno AI (`0x0043FE27..0x0043FE54`) and the
//!   one after it (`0x0043FF91..0x0043FFB4`): with `+0x6DD` set, a successful
//!   Commence of the queued mission clears the byte
//!   ([`Simulation::mission_building_ready_commence`]). The first also needs
//!   BState (`+0x534`) out of 0, the construction body; a build-up's frames
//!   run in the late region (`sim::building_construction`) and never reach
//!   either check, and a sale past its stage 1 refuses every queue, so VERA
//!   has no state in which the BState test decides.
//! - `MissionClass::AI` (`0x005B3060`, called at `0x006FA655`): when the
//!   dispatch timer (`+0xC8`) is due and Health is above zero, the current
//!   mission's handler runs and its return is the next delay
//!   (`0x005B32F2..0x005B3302`). Guard and Sticky run Mission_Guard
//!   (`0x004496B0`, dispatch table `0x005B34E8`), Area Guard too (`0x00449A40`
//!   jumps to it), Attack runs Mission_Attack (`0x0044ACF0`), Selling runs
//!   Sell ([`Simulation::visit_building_down`]), and a mission the building
//!   has no handler for, or none, a MissionClass stub (450 frames).
//! - the Gattling block after the Techno AI (`0x0043FE5B..0x0043FF8B`): the
//!   idle decay of a Gattling type and its turret-animation steps
//!   ([`gattling_idle`]).
//! - `ProcessDelayedFire` (`0x004503F0`, called at `0x004400F4` while Health
//!   is above zero): the countdown of a delayed fire Mission_Attack armed,
//!   and what it does when the countdown ends ([`process_delayed_fire`]).
//! - the range drop at the end of the Update (`0x00440378..0x004403C6`).
//!
//! A Gattling type (`IsGattling=`, the Gattling Cannon) charges and decays its
//! stages inside both handlers (`combat::gattling`), each call taking the
//! mission's `+0xC4` count, the frames since it was last zeroed, and zeroing
//! it: Mission_Guard decays first thing on every dispatch (`0x004496C1..
//! 0x004496DF`), Mission_Attack charges after its FireAt and on FACING and
//! REARM, and decays on its drop tail, BUSY and CLOAKED. In Attack the count
//! is the previous handler's return, so a cannon charges RateUp per frame it
//! spends attacking; outside Attack, Update's idle decay takes RateDown per
//! frame once `GuardAreaTargetingDelay + 5` frames have passed since its last
//! shot.
//!
//! The Update's two FireAts, Mission_Attack's (`0x0044B6D0`) and
//! ProcessDelayedFire's (`0x00450492`), are VERA's combat emission: the visit
//! asks the combat phase for the shot
//! ([`crate::sim::combat::FireRequests::buildings`]), which emits it without
//! asking GetFireError again. A building fires no shot without that request.
//!
//! A Prism tower (`[General] PrismType=`) never takes Mission_Attack's FireAt
//! arm: it recruits one charged tower of its House per visit to beam at it,
//! then arms its own delayed shot, whose damage grows with the count
//! ([`prism_arm`], [`Simulation::take_support_bonus`]).
//!
//! Evidence: `tools/spatial_oracle/building_guard_attack.json`, replayed in
//! the tests below: Mission_Guard's returns and Scenario draws per arm,
//! Mission_Attack's return and actions per fire-error code,
//! BuildingClass::SetTarget's decisions, Unlimbo's mission (`0x0044D6A0`) and
//! the dispatch cadence of Update's mission pieces run natively frame by
//! frame (the shot every ROF + 1 frames for an even ROF, every ROF frames
//! for an odd one). `tools/spatial_oracle/building_prism.json`: the Prism
//! arm's recruitment and master arm, ProcessDelayedFire's two modes, the
//! support bonus and its damage, the multi-tower cadence and the `[General]`
//! Prism reader, run natively. `tools/spatial_oracle/building_gattling.json`
//! (`building_gattling_tests`): every Gattling call of both handlers and of
//! the idle decay, and whole engagements frame by frame.
//!
//! RESIDUALS, each with its later owner:
//! - The frame position of a building's shot. Native fires both FireAts
//!   inside the building's own Logic visit; VERA emits them in the combat
//!   phase after the Logic pass, like every class's FireAt (the draw-order
//!   residual `docs/plans/2026-09-21-combat-parity.md` records for all of
//!   them). Trigger: every building shot. Effect: the shot's Scenario draws
//!   (GetROF's `RandomRanged(0, 2)` at `0x006FD09E`, the bullet's own) and
//!   its rearm and ammo writes follow the Logic visits of the objects after
//!   the building, not precede them; the bullet still takes its first AI at
//!   the tail of the same pass ([`Simulation::visit_combat_tail`]).
//!   Frequency: every frame a building fires while a later object draws.
//!   Downstream: the Scenario stream's order in that frame. The pass's one
//!   reader of another building's rearm, the Prism walk, counts a requested
//!   shot as rearming ([`prism_supporter`]); the support count the shot
//!   clears (`0x004504CD`) is read only in its own tower's visits. The
//!   request carries the visit's target and weapon ([`BuildingShot`]), so a
//!   Gattling charge after the FireAt or a later retarget leaves the shot as
//!   the visit made it. FireAt (`0x006FDD50`) draws `g_MainRng` only through
//!   callees off the Gattling Cannon's path (SpawnRadEruption `0x006FD800`,
//!   EBolt::Init `0x004C2A60`, audio; an instruction scan), so the charge's
//!   loop draw keeps its native place in that stream. Later owner: FireAt
//!   moving into each object's Logic visit.
//! - The Prism beams are not drawn. A support beam (`0x0044ABD0`: a
//!   LaserDrawClass from the supporter's weapon-0 FLH to the stored point in
//!   the House's `LaserColor`, width 3, `PrismSupportDuration=` frames) and
//!   FireAt's main beam of an `IsLaser=` weapon (`0x006FF4CC..0x006FF544`,
//!   width 5 on a supported shot) have no presentation owner: VERA has no
//!   LaserDrawClass. Trigger: every Prism tower and Prism tank shot. Effect:
//!   the beams are invisible; damage, timing and anims are unaffected (no
//!   RNG, no sim state). Later owner: laser drawing, its own presentation
//!   chain.
//! - The docking radio of an unarmed building's Guard (the depot/airfield
//!   docking chain). Status 1 of a `UnitRepair=` (`+0x16A9`), `UnitReload=`
//!   (`+0x16AA`) or `Bunker=` (`+0x16AB`) type walks its radio contacts
//!   (`0x00449817..0x00449918`): one on Enter under 64 leptons away that answers
//!   ROGER to message `0x13` gets the building a queued Repair and the
//!   handler returns 1 without its `RandomRanged(0, 2)`
//!   (`0x00449942..0x0044995C`). A `UnitReload=` type also sends its contact
//!   `0x1D`, then `0x13`, and queues Repair on ROGER before its usual draw
//!   (`0x00449970..0x004499B5`). VERA's handler stays on Guard and draws.
//!   Trigger: a Service Depot's repair, an airfield's reload, a Tank Bunker
//!   entry (GADEPT, NADEPT, YADEPT, GAAIRC, AMRADR, NATBNK). Effect: the
//!   building's queued mission and the Scenario draw count differ while the
//!   contact docks, so every later Scenario draw differs from native.
//!   Frequency: common in ordinary play. The contacts' answers to `0x13` and
//!   `0x1D`, and whether that Repair commences (the unarmed arm sets no
//!   `+0x6DD`), belong to that chain with BuildingClass::Mission_Repair
//!   (`0x0044B780`); the dock owners run the repair and reload meanwhile.
//!   The WeaponsFactory's ClearBibArea (`0x00449540`) after the walk is
//!   dormant: no retail WeaponsFactory type clears HasStupidGuardMode.
//! - A voxel building's HVA animation is not drawn: the building's voxel draw
//!   (`0x0043DA80`, Building vt+0x4E4) takes its main and turret HVA frames
//!   from `+0x148` modulo their frame counts, which a Gattling type advances
//!   while its value is above 0 (the Gattling Cannon's barrels spin) and any
//!   other type on its OK and REARM arms; VERA's building voxel presentation
//!   (`emit_building_turret_vxl`) draws frame 0. Presentation only; its own
//!   presentation chain (#757).
//! - Status 0's `Begin_Mode(1)` (`0x0044995D`): the idle body, presentation.
//! - Dormant with retail data: the SAM arm (`0x0044AD07`, `SAM=` unset), the
//!   upgrade arm (`0x0044B2BC`, no `PowersUpBuilding=`), Mission_Guard's
//!   SuperWeapon gate (`0x00449716..0x00449753`, no armed type sets
//!   `SuperWeapon=`), the waypoint-planning hook (`0x0044AFB1`, VERA has no
//!   planning mode) and BuildingClass::SetTarget's TickTank/Artillary
//!   undeploy (`0x00443C07..0x00443C54`, neither key set).
//! - `+0x6DD` has two more homes, `BuildingUp::done` and `BuildingDown::done`
//!   (`sim::components`), each written and read only by its own build-up or
//!   sale; a finished build-up queues and commences Guard itself
//!   (`Simulation::tick_building_up`) where the ready check after the Techno
//!   AI would. No player-visible effect while each byte has one reader.
//!   Later owner: Mission_Construction's frames moving into this dispatch.
//!
//! ## Dependency rules
//! - Part of sim/; sim/ never depends on render/, ui/, sidebar/, audio/, net/.

use super::target_scan::{
    can_fire_at, fire_error_with_overlay, select_weapon, weapon_at_index_for,
};
use super::{ObjectAiCtx, mission_handlers_run};
use crate::map::entities::EntityCategory;
use crate::rules::ruleset::RuleSet;
use crate::sim::building_art::requested_damage_state;
use crate::sim::combat::TargetKind;
use crate::sim::combat::combat_weapon::{self, WeaponSlot};
use crate::sim::combat::fire_error::FireError;
use crate::sim::combat::gattling::{StageCall, is_elite};
use crate::sim::combat::{BuildingShot, fire_coord};
use crate::sim::game_entity::{DelayedFire, PendingBuildingFire};
use crate::sim::mission::authority::LiveReadyInputProvider;
use crate::sim::mission::{MissionId, MissionType};
use crate::sim::projectile::ProjectilePayload;
use crate::sim::world::Simulation;

/// The MissionClass stubs a building's table holds for every mission it has
/// no handler of its own for (`0x005B2E10..0x005B2FC0`; its Capture,
/// Sabotage and Harvest slots jump to them, `0x0044B760`, `0x0044B770`):
/// 450 frames.
const DEFAULT_MISSION_DELAY: i32 = 450;
/// An unarmed HasStupidGuardMode building's Guard return (`0x004497F4`).
const STUPID_GUARD_DELAY: i32 = 100;
/// The building anim slots of `ActiveAnim=` and `SpecialAnim=` (Building
/// `+0x55C` + 4 x slot; their names at BuildingType `+0x1018` and `+0x11F4`).
const ACTIVE_ANIM_SLOT: u8 = 3;
const SPECIAL_ANIM_SLOT: u8 = 10;

/// One of Update's two ready checks (module doc).
pub(super) fn ready_commence(sim: &mut Simulation, id: u64) {
    if sim
        .substrate
        .entities
        .get(id)
        .is_none_or(|entity| entity.building_up.is_some())
    {
        return;
    }
    let now = sim.session.binary_frame;
    let _ = sim.mission_building_ready_commence(id, now);
}

/// `MissionClass::AI` (`0x005B3060`) for a building (module doc).
pub(super) fn dispatch(
    sim: &mut Simulation,
    id: u64,
    rules: Option<&RuleSet>,
    ctx: ObjectAiCtx<'_>,
) {
    let Some(entity) = sim.substrate.entities.get(id) else {
        return;
    };
    // A build-up's Construction mission belongs to its late-region owner.
    if entity.building_up.is_some() {
        return;
    }
    let current = entity.mission.current().known();
    if current == Some(MissionType::Selling) {
        // Sell returns 1 on every visit, so its dispatch is due on every frame
        // after the one that commenced it; the visit owns that frame test.
        sim.visit_building_down(id, rules, ctx.overlay_registry);
        return;
    }
    let Some(rules) = rules else {
        return;
    };
    let now = sim.session.binary_frame;
    if !entity.mission.dispatch_timer().due(now) || !mission_handlers_run(sim, id) {
        return;
    }
    let delay = match current {
        Some(MissionType::Guard | MissionType::Sticky | MissionType::AreaGuard) => {
            mission_guard(sim, id, rules)
        }
        Some(MissionType::Attack) => mission_attack(sim, id, rules, ctx),
        // BuildingClass's own Unload (`0x0044D880`), Construction
        // (`0x00449A50`), Repair (`0x0044B780`), Missile (`0x0044C980`) and
        // Open (`0x0044E440`) keep their existing owners.
        Some(
            MissionType::Unload
            | MissionType::Construction
            | MissionType::Repair
            | MissionType::Missile
            | MissionType::Open,
        ) => return,
        // Every other slot of the building's table, and no mission (above
        // `0x1F`, `0x005B30BB`), is a MissionClass stub.
        _ => DEFAULT_MISSION_DELAY,
    };
    if let Some(entity) = sim.substrate.entities.get_mut(id) {
        entity.mission.write_dispatch_epilogue(now as i32, delay);
    }
}

/// `Queue_Mission(mission, false)` then Commence, as both handlers switch
/// missions (`0x00449792`/`0x0044979C`, `0x0044B13E`/`0x0044B148`).
fn queue_and_commence(sim: &mut Simulation, id: u64, mission: MissionType, rules: &RuleSet) {
    let now = sim.session.binary_frame;
    let readiness = LiveReadyInputProvider { rules };
    let _ = sim.mission_queue_exact(id, MissionId::from_known(mission), 0, now, &readiness);
    let _ = sim.mission_commence_exact(id, now);
}

/// `ftol(rate x 900) + RandomRanged(0, 2)` on the Scenario stream, the rate
/// being the MissionControl entry of the building's current mission
/// (`0x005B3A00`).
fn rate_delay(sim: &mut Simulation, frames: u32, multiplier: i32) -> i32 {
    let base = frames.min(i32::MAX as u32) as i32;
    let jitter = sim.scenario_rng.next_range_u32_inclusive(0, 2) as i32;
    base.wrapping_mul(multiplier).wrapping_add(jitter)
}

/// `BuildingClass::Mission_Guard` (`0x004496B0`).
fn mission_guard(sim: &mut Simulation, id: u64, rules: &RuleSet) -> i32 {
    // The head (`0x004496C1..0x004496DF`), armed or not, before anything else
    // the handler reads or draws.
    gattling_step(sim, id, rules, StageCall::Update);
    let Some(entity) = sim.substrate.entities.get(id) else {
        return 0;
    };
    let Some(obj) = rules.object(sim.interner.resolve(entity.type_ref())) else {
        return 0;
    };
    let current = entity
        .mission
        .current()
        .known()
        .unwrap_or(MissionType::Guard);
    // vt+0x2AC, `BuildingClass::Is_Armed 0x00458DB0`.
    if combat_weapon::is_armed(entity, obj) {
        let empty_garrison = obj.can_be_occupied
            && entity
                .passenger_role
                .cargo()
                .is_none_or(|cargo| cargo.is_empty());
        let has_target = entity.attack_target.is_some();
        if let Some(entity) = sim.substrate.entities.get_mut(id) {
            // `0x00449701`.
            entity.mission_leaf.set_building_ready_latch(1);
        }
        if !obj.emp_pulse_cannon && !empty_garrison && has_target {
            queue_and_commence(sim, id, MissionType::Attack, rules);
            return 1;
        }
        // `0x004497AF..0x004497DA`: the AARate delay.
        let frames = rules.mission_control.aa_rate_frames(current);
        return rate_delay(sim, frames, 1);
    }
    if obj.has_stupid_guard_mode {
        return STUPID_GUARD_DELAY;
    }
    let status = entity.mission.handler_state();
    if status == 0
        && let Some(entity) = sim.substrate.entities.get_mut(id)
    {
        // `0x0044995D..0x00449966`: the idle body, then status 1.
        entity.mission.set_handler_state(1);
    }
    // `0x004499BB..0x00449A36`: Rate for a depot, three times it otherwise.
    let frames = rules.mission_control.rate_frames(current);
    rate_delay(sim, frames, if obj.unit_repair { 1 } else { 3 })
}

/// `BuildingClass::Mission_Attack` (`0x0044ACF0`).
fn mission_attack(sim: &mut Simulation, id: u64, rules: &RuleSet, ctx: ObjectAiCtx<'_>) -> i32 {
    let Some((target, weapon)) = attack_prelude(sim, id, rules) else {
        return 1;
    };
    let mut code = fire_error_with_overlay(sim, rules, id, target, weapon, ctx.overlay_registry);
    if code == FireError::Facing && voxel_turret_snaps(sim, id, rules, target) {
        code = fire_error_with_overlay(sim, rules, id, target, weapon, ctx.overlay_registry);
    }
    attack_arm(sim, id, rules, target, weapon, code)
}

/// Mission_Attack up to its GetFireError (`0x0044B00F`): with no target, the
/// null-target tail (`0x0044AF86..0x0044AFE0`, `+0x664 = 0` at `0x0044AF9F`),
/// which answers `None`;
/// otherwise SelectWeapon (`0x0044AFF2`) and `+0x6DD = 1` (`0x0044B008`),
/// answering the target and weapon.
fn attack_prelude(sim: &mut Simulation, id: u64, rules: &RuleSet) -> Option<(TargetKind, i32)> {
    let entity = sim.substrate.entities.get(id)?;
    let Some(target) = entity.attack_target.as_ref().map(|attack| attack.target) else {
        let _ = sim.assign_target_represented(id, None, Some(rules));
        clear_support_count(sim, id);
        if !waits(sim, id) {
            queue_and_commence(sim, id, MissionType::Guard, rules);
        }
        return None;
    };
    let weapon = select_weapon(sim, rules, id, Some(target));
    if let Some(entity) = sim.substrate.entities.get_mut(id) {
        entity.mission_leaf.set_building_ready_latch(1);
    }
    Some((target, weapon))
}

/// Mission_Attack's arm for GetFireError's `code` (the table at
/// `0x0044B728`): its actions, then its return.
fn attack_arm(
    sim: &mut Simulation,
    id: u64,
    rules: &RuleSet,
    target: TargetKind,
    weapon: i32,
    code: FireError,
) -> i32 {
    match code {
        FireError::Ok => {
            fire_arm(sim, id, rules, target, weapon);
            // The tail every OK arm reaches (`0x0044B6D6..0x0044B724`).
            if !gattling_step(sim, id, rules, StageCall::Increase) {
                advance_turret_anim(sim, id);
            }
            1
        }
        // `0x0044B0DE`: the drop tail (`+0x664 = 0` at `0x0044B0ED`).
        FireError::Ammo | FireError::Illegal | FireError::Cant | FireError::Range => {
            let _ = sim.assign_target_represented(id, None, Some(rules));
            clear_support_count(sim, id);
            if waits(sim, id) {
                return 1;
            }
            // `0x0044B113..0x0044B131`, before Commence zeroes the count.
            gattling_step(sim, id, rules, StageCall::Update);
            queue_and_commence(sim, id, MissionType::Guard, rules);
            clear_ai_counter(sim, id);
            1
        }
        // `0x0044B187` and `0x0044B1DE`: after Set_Desired, a Gattling type
        // charges (`0x0044B1C6`, `0x0044B21D`); any other keeps the count, and
        // on REARM advances `+0x148` (`0x0044B235..0x0044B23C`).
        FireError::Facing | FireError::Rearm => {
            aim_turret(sim, id, rules, target);
            if !gattling_step(sim, id, rules, StageCall::Increase) && code == FireError::Rearm {
                advance_turret_anim(sim, id);
            }
            2
        }
        // `0x0044B284`: uncloak, a Gattling type's decay (`0x0044B2AC`), then
        // the `0x0044B14E` tail.
        FireError::Cloaked => {
            crate::sim::combat::world_receiver::start_uncloaking_to_fire(sim, rules, id);
            gattling_step(sim, id, rules, StageCall::Update);
            aim_turret(sim, id, rules, target);
            clear_ai_counter(sim, id);
            1
        }
        // `0x0044B24F`: a Gattling type decays (`0x0044B26C`); any other
        // keeps the count.
        FireError::Busy => {
            gattling_step(sim, id, rules, StageCall::Update);
            1
        }
        // Codes 4, 7 and above 10 (`0x0044B14E`).
        FireError::Rotating | FireError::Moving | FireError::MustDeploy => {
            aim_turret(sim, id, rules, target);
            clear_ai_counter(sim, id);
            1
        }
    }
}

/// `Get_Mission() == Wait` (vt+0x184, `0x0044AFBA`/`0x0044B108`): the current
/// mission, else the queued one.
fn waits(sim: &Simulation, id: u64) -> bool {
    sim.substrate.entities.get(id).is_some_and(|entity| {
        entity.mission.effective() == MissionId::from_known(MissionType::Deliberate)
    })
}

/// `+0x664 = 0`, the Prism support count, after both tails' SetTarget(0).
fn clear_support_count(sim: &mut Simulation, id: u64) {
    if let Some(entity) = sim.substrate.entities.get_mut(id) {
        entity.prism_support_count = 0;
    }
}

/// `+0xC4 = 0`: Mission_Attack's zeroes (`0x0044B131`, `0x0044B174`,
/// `0x0044B1CB`, `0x0044B222`, `0x0044B271`, `0x0044B2B1`, `0x0044B6F4`) and
/// Mission_Guard's (`0x004496DF`).
fn clear_ai_counter(sim: &mut Simulation, id: u64) {
    if let Some(entity) = sim.substrate.entities.get_mut(id) {
        entity.mission.clear_ai_counter();
    }
}

/// Whether the building's type is a Gattling one (TechnoType `+0xCD5`).
fn is_gattling(sim: &Simulation, id: u64, rules: &RuleSet) -> bool {
    sim.substrate.entities.get(id).is_some_and(|entity| {
        sim.object_type(entity.type_ref(), rules)
            .is_some_and(|obj| obj.is_gattling)
    })
}

/// A Gattling type's stage call with the mission's `+0xC4` count, then
/// `+0xC4 = 0`, as both handlers make it. Answers whether the type is a
/// Gattling one; any other makes no call and keeps the count.
fn gattling_step(sim: &mut Simulation, id: u64, rules: &RuleSet, call: StageCall) -> bool {
    if !is_gattling(sim, id, rules) {
        return false;
    }
    let Some(ticks) = sim
        .substrate
        .entities
        .get(id)
        .map(|entity| entity.mission.ai_counter() as i32)
    else {
        return false;
    };
    match call {
        StageCall::Increase => sim.gattling_increase(id, rules, ticks),
        StageCall::Update => sim.gattling_update(id, rules, ticks),
    }
    clear_ai_counter(sim, id);
    true
}

/// `+0x148 += 1`, the voxel turret's animation counter.
fn advance_turret_anim(sim: &mut Simulation, id: u64) {
    if let Some(entity) = sim.substrate.entities.get_mut(id) {
        entity.turret_anim_frame = entity.turret_anim_frame.wrapping_add(1);
    }
}

/// BuildingClass::Update's Gattling block once the Techno AI has returned
/// with the building alive (`0x0043FE5B`; a dead one returns from the Update),
/// for a Gattling type only:
/// - a value above 0 advances `+0x148` (`0x0043FE69..0x0043FE88`);
/// - unless the building's mission, else its queued one, is Attack
///   (`0x0043FEBE..0x0043FECB`): the idle decay
///   ([`crate::sim::combat::gattling::GattlingState::idle_decay`]) once more
///   than `GuardAreaTargetingDelay + 5` frames have passed since the last
///   shot (`0x0043FEE9..0x0043FF67`: `Frame - LastFireFrame`, signed, whatever
///   the target), then `+0x148` again while the value is above 0
///   (`0x0043FF6C..0x0043FF8B`).
pub(super) fn gattling_idle(sim: &mut Simulation, id: u64, rules: &RuleSet) {
    if !super::ai_alive(sim, id) {
        return;
    }
    let Some(obj) = sim
        .substrate
        .entities
        .get(id)
        .and_then(|entity| sim.object_type(entity.type_ref(), rules))
        .filter(|obj| obj.is_gattling)
    else {
        return;
    };
    let now = sim.session.binary_frame as i32;
    let Some(entity) = sim.substrate.entities.get_mut(id) else {
        return;
    };
    if entity.gattling.value() > 0 {
        entity.turret_anim_frame = entity.turret_anim_frame.wrapping_add(1);
    }
    if entity.mission.effective() == MissionId::from_known(MissionType::Attack) {
        return;
    }
    let since = now.wrapping_sub(entity.last_fire_frame as i32);
    if since > rules.general.guard_area_targeting_delay.wrapping_add(5) {
        let elite = is_elite(entity);
        entity.gattling.idle_decay(&obj.gattling_stages, elite);
    }
    if entity.gattling.value() > 0 {
        entity.turret_anim_frame = entity.turret_anim_frame.wrapping_add(1);
    }
}

/// The OK arm (`0x0044B2BC`): a Prism tower forwards ([`prism_arm`]); an
/// `IsAnimDelayedFire=` building arms its delayed shot (`0x0044B630..
/// 0x0044B666`: `+0x714` = DelayedFireDelay, `+0x708` = the weapon, `+0x704`
/// = 1) for [`process_delayed_fire`]; any other asks the combat phase for its
/// FireAt (`0x0044B6D0`) at the visit's target with SelectWeapon's weapon,
/// both of which the request carries: the tail's charge may step a Gattling
/// type's stage, and a later object may retarget the building, before the
/// combat phase fires.
fn fire_arm(sim: &mut Simulation, id: u64, rules: &RuleSet, target: TargetKind, weapon: i32) {
    let Some(obj) = sim
        .substrate
        .entities
        .get(id)
        .and_then(|entity| rules.object(sim.interner.resolve(entity.type_ref())))
    else {
        return;
    };
    if is_prism_type(rules, obj) {
        prism_arm(sim, id, rules, obj);
        return;
    }
    match rules
        .art()
        .resolve_metadata_entry(&obj.id, &obj.image)
        .filter(|art| art.is_anim_delayed_fire)
    {
        Some(art) => {
            let slot = if weapon == 1 {
                WeaponSlot::Secondary
            } else {
                WeaponSlot::Primary
            };
            arm_delayed_fire(
                sim,
                id,
                rules,
                obj,
                art.delayed_fire_delay,
                DelayedFire::Weapon(slot),
            );
        }
        None => {
            sim.fire_requests
                .buildings
                .insert(id, BuildingShot::Mission { weapon, target });
        }
    }
}

/// `Type == Rules+0x498` (`[General] PrismType=`, `0x0044B2F8`).
fn is_prism_type(rules: &RuleSet, obj: &crate::rules::object_type::ObjectType) -> bool {
    rules
        .general
        .prism_type
        .as_deref()
        .is_some_and(|prism_type| obj.id.eq_ignore_ascii_case(prism_type))
}

/// Arms a delayed fire (`+0x714` = `delay`, `+0x704` and `+0x708..+0x710`
/// from `fire`) and swaps the building's anims: the Active anim's slot 3 is
/// emptied (`0x00451E40`) and the SpecialAnim plays in slot 10, its Damaged
/// variant at or below ConditionYellow (`0x00451890`, the Tesla Coil's and
/// the Prism tower's charge, with their `Report=`). Nothing in play gives the
/// Active anim back when the SpecialAnim ends: BuildingClass's vt+0x28
/// (`0x0044E9AA`) replays it only for a `Grinding=` type, so it returns with a
/// power restore.
fn arm_delayed_fire(
    sim: &mut Simulation,
    id: u64,
    rules: &RuleSet,
    obj: &crate::rules::object_type::ObjectType,
    delay: i32,
    fire: DelayedFire,
) {
    let Some(entity) = sim.substrate.entities.get_mut(id) else {
        return;
    };
    entity.pending_building_fire = Some(PendingBuildingFire {
        remaining_ticks: delay,
        fire,
    });
    let damaged =
        requested_damage_state(entity.health, obj.strength, rules.general.condition_yellow);
    sim.clear_building_anim_slot(id, ACTIVE_ANIM_SLOT);
    let _ = sim.set_building_anim_slot(id, SPECIAL_ANIM_SLOT, damaged, false, 0, rules);
}

/// Mission_Attack's PrismType arm (`0x0044B310..0x0044B62B`), which never
/// reaches FireAt and does not read IsAnimDelayedFire or SelectWeapon's
/// weapon. While the tower's support count (`+0x664`) is below
/// `PrismSupportMax` (`0x0044B349`), the tower its House can recruit
/// ([`prism_supporter`]) is armed with a support beam aimed at this tower's
/// weapon-0 FLH and the count goes up (`0x0044B4CB..0x0044B590`). With none,
/// or at the cap, the tower arms its own delayed shot with weapon 0
/// (`0x0044B595..0x0044B62B`). Both arm through [`arm_delayed_fire`], with
/// the type's `DelayedFireDelay=` (`+0x16EC`; the supporter is a PrismType
/// tower too), and the arm returns 1 either way, so one tower is recruited per
/// visit.
fn prism_arm(
    sim: &mut Simulation,
    id: u64,
    rules: &RuleSet,
    obj: &crate::rules::object_type::ObjectType,
) {
    let delay = rules
        .art()
        .resolve_metadata_entry(&obj.id, &obj.image)
        .map_or(0, |art| art.delayed_fire_delay);
    let Some(master) = sim.substrate.entities.get(id) else {
        return;
    };
    let supporter = (master.prism_support_count < rules.general.prism_support.max)
        .then(|| prism_supporter(sim, id, rules, obj))
        .flatten();
    let Some(supporter) = supporter else {
        arm_delayed_fire(
            sim,
            id,
            rules,
            obj,
            delay,
            DelayedFire::Weapon(WeaponSlot::Primary),
        );
        return;
    };
    if let Some(master) = sim.substrate.entities.get_mut(id) {
        master.prism_support_count = master.prism_support_count.wrapping_add(1);
    }
    // `vt+0xB0(out, 0, {0, 0, 0})` (`0x0044B4F6`): the master's weapon-0 FLH.
    let Some(master) = sim.substrate.entities.get(id) else {
        return;
    };
    let to = fire_coord::fire_coordinate(
        sim,
        rules,
        &fire_coord::FireSource::of_entity(master),
        obj,
        0,
        (master.weapon_burst.index() & 1) as u8,
    )
    .coord;
    arm_delayed_fire(
        sim,
        supporter,
        rules,
        obj,
        delay,
        DelayedFire::SupportBeam { to },
    );
}

/// The Prism arm's walk (`0x0044B357..0x0044B4BD`) over the master's House's
/// buildings (House+0x68) in vector order. A tower is admitted when it is
/// alive (`+0x90`), of PrismType (the master's own type), its rearm timer
/// has run out, it counts no delayed fire down (`+0x714 == 0`; the mode is
/// not read), no Floating Disc drains it (`0x0070FEC0`), its mission
/// (current, else queued) is not Attack, it is not the master, and its
/// distance from the master (`Distance3D` of the two Locations: Sqrt_Approx,
/// ftol) is at most the master's weapon 1 range (`vt+0x168(1)`, the Secondary
/// `PrismSupport`'s 2048 in retail; inclusive, `0x0044B49A`). The nearest
/// wins; a tie keeps the lower vector index (`0x0044B49E..0x0044B4AA`).
/// Power, EMP, the tower's own target, a build-up or a sale are not asked.
///
/// FireAt starts the shooter's rearm inside its visit
/// (`0x006FF2B2..0x006FF2BB`), before the visits after it; VERA's combat
/// phase starts it after the Logic pass (module doc), so a tower whose shot
/// this pass has already asked for counts as rearming. That is FireAt's
/// answer for every ROF above zero; a zero ROF, which native leaves run out,
/// is not told apart.
fn prism_supporter(
    sim: &Simulation,
    id: u64,
    rules: &RuleSet,
    obj: &crate::rules::object_type::ObjectType,
) -> Option<u64> {
    let master = sim.substrate.entities.get(id)?;
    let house = sim.houses.get(&master.owner())?;
    let now = sim.session.binary_frame as i32;
    let location = |entity: &crate::sim::game_entity::GameEntity| {
        let coord = crate::sim::movement::ground_pose::position_world_coord(&entity.position);
        [coord.x, coord.y, coord.z]
    };
    let origin = location(master);
    let attack = MissionId::from_known(MissionType::Attack);
    let mut best: Option<(u64, i32)> = None;
    for &candidate_id in house.base_projection.buildings() {
        let Some(candidate) = sim.substrate.entities.get(candidate_id) else {
            continue;
        };
        let admitted = candidate.lifecycle.object_alive
            && candidate.type_ref() == master.type_ref()
            && candidate.rearm_timer.remaining(now) == 0
            && !sim.fire_requests.buildings.contains_key(&candidate_id)
            && candidate
                .pending_building_fire
                .map_or(0, |pending| pending.remaining_ticks)
                == 0
            && candidate.draining_me.is_none()
            && candidate.mission.effective() != attack
            && candidate_id != id;
        if !admitted {
            continue;
        }
        let distance = crate::util::native_x87::distance_3d_leptons(origin, location(candidate));
        let range = combat_weapon::weapon_range(
            master,
            obj,
            1,
            &sim.substrate.entities,
            rules,
            &sim.interner,
        );
        if distance > range {
            continue;
        }
        if best.is_none_or(|(_, nearest)| distance < nearest) {
            best = Some((candidate_id, distance));
        }
    }
    best.map(|(supporter, _)| supporter)
}

/// `BuildingClass::ProcessDelayedFire` (`0x004503F0`), from Update
/// (`0x004400F4`) once Health is above zero, whatever the mission, power or
/// owner. An armed delayed fire (`+0x704` nonzero) counts `+0x714` down with a
/// signed pre-decrement and ends when it reaches zero or below, `+0x714 = 0`
/// (`0x00450401..0x00450419`); every end clears the mode (`0x00450452`,
/// `0x004504D7`).
/// - A shot (mode 1, `0x0045045E..0x00450492`) needs a target and
///   GetFireError(target, `+0x708`, range) answering OK; then its FireAt at
///   that target is asked of the combat phase ([`BuildingShot::Delayed`]),
///   whose bullet takes the support bonus ([`Simulation::take_support_bonus`]).
///   Otherwise the shot is dropped and `+0x664` kept.
/// - A support beam (mode 2, `0x0044ABD0`): the beam (not drawn, module
///   doc), `+0x664 = 0` (`0x0044ACCA`) and the downtime: the rearm timer
///   becomes {Frame, `PrismSupportDelay=`} (`0x0044ACD0..0x0044ACDC`). No
///   check, damage or Scenario draw.
pub(super) fn process_delayed_fire(
    sim: &mut Simulation,
    id: u64,
    rules: &RuleSet,
    ctx: ObjectAiCtx<'_>,
) {
    let now = sim.session.binary_frame as i32;
    let Some(entity) = sim.substrate.entities.get_mut(id) else {
        return;
    };
    // Health 0 returned before the call (`0x00440072`).
    if entity.dying || entity.health.current == 0 {
        return;
    }
    let Some(pending) = entity.pending_building_fire.as_mut() else {
        return;
    };
    pending.remaining_ticks = pending.remaining_ticks.wrapping_sub(1);
    if pending.remaining_ticks > 0 {
        return;
    }
    let fire = pending.fire;
    entity.pending_building_fire = None;
    match fire {
        DelayedFire::Weapon(slot) => {
            let Some(target) = entity.attack_target.as_ref().map(|attack| attack.target) else {
                return;
            };
            let weapon = match slot {
                WeaponSlot::Primary => 0,
                WeaponSlot::Secondary => 1,
            };
            if fire_error_with_overlay(sim, rules, id, target, weapon, ctx.overlay_registry)
                == FireError::Ok
            {
                sim.fire_requests
                    .buildings
                    .insert(id, BuildingShot::Delayed { slot, target });
            }
        }
        DelayedFire::SupportBeam { .. } => {
            entity.prism_support_count = 0;
            entity
                .rearm_timer
                .start(now, rules.general.prism_support.delay);
        }
    }
}

/// ProcessDelayedFire's support bonus (`0x004504A8..0x004504C7`):
/// `((PrismSupportModifier * count + 100) << 8) / 100`, the `imul` and add
/// wrapping in 32 bits and the division unsigned (`mul 0x51EB851F; shr edx,
/// 5`), in the bullet's 1/256 damage units.
fn support_multiplier(modifier: i32, count: i32) -> i32 {
    let scaled = (modifier.wrapping_mul(count).wrapping_add(100) as u32) << 8;
    (scaled / 100) as i32
}

/// Whether the building's weapon 0 (vt+0x3F8, `0x004526F0`) has a WeaponType
/// whose projectile is not `AA=` (BulletType `+0x2A4`). BuildingClass::SetTarget
/// admits every target when it has not (`0x00443BC0..0x00443BED`), and
/// ReceiveDamage's retaliation block stops (`0x004429B4..0x004429E5`).
pub(super) fn building_weapon0_aims(
    sim: &Simulation,
    rules: &RuleSet,
    id: u64,
    target: TargetKind,
) -> bool {
    weapon_at_index_for(sim, rules, id, Some(target), 0).is_some_and(|weapon| {
        !weapon
            .projectile
            .as_deref()
            .and_then(|projectile| rules.projectile(projectile))
            .is_some_and(|projectile| projectile.aa)
    })
}

/// The voxel-turret retry (`0x0044B017..0x0044B0CC`): a building with a turret
/// whose `TurretAnimIsVoxel=` is set, within one `ROT=` step of the target's
/// direction (vt+0x4E8 at `0x0044B056`, [`fire_coord::building_direction_to`];
/// `abs(low-byte ROT << 8)` as signed16, without FacingClass
/// SetROT's clamp; any miss at ROT 0), snaps its turret (`0x0044B0AC`) and
/// asks GetFireError again. Original decisions: building_fire_turn.json.
fn voxel_turret_snaps(sim: &mut Simulation, id: u64, rules: &RuleSet, target: TargetKind) -> bool {
    let now = sim.session.binary_frame;
    let Some(entity) = sim.substrate.entities.get(id) else {
        return false;
    };
    let Some(obj) = rules.object(sim.interner.resolve(entity.type_ref())) else {
        return false;
    };
    let (Some(barrel), true) = (
        entity.barrel_facing,
        obj.has_turret && obj.turret_anim_is_voxel,
    ) else {
        return false;
    };
    let Some(direction) = fire_coord::building_direction_to(sim, rules, entity, target) else {
        return false;
    };
    let delta = i32::from(barrel.current(now).wrapping_sub(direction) as i16);
    let rot_step = i32::from(((obj.turret_rot as u8 as u16) << 8) as i16).abs();
    if obj.turret_rot != 0 && delta.abs() > rot_step {
        return false;
    }
    if let Some(barrel) = sim
        .substrate
        .entities
        .get_mut(id)
        .and_then(|entity| entity.barrel_facing.as_mut())
    {
        barrel.snap(direction, now);
    }
    true
}

/// `+0x388.Set_Desired(vt+0x4E8(Target))` (`0x0044B16F`, `0x0044B1A8`,
/// `0x0044B1FF`; [`fire_coord::building_direction_to`]), at the type's `ROT=`:
/// the turret of a `Turret=yes` type, the body of any other.
fn aim_turret(sim: &mut Simulation, id: u64, rules: &RuleSet, target: TargetKind) {
    let now = sim.session.binary_frame;
    let Some(entity) = sim.substrate.entities.get(id) else {
        return;
    };
    let Some(rot) = rules
        .object(sim.interner.resolve(entity.type_ref()))
        .map(|obj| obj.turret_rot)
    else {
        return;
    };
    let Some(desired) = fire_coord::building_direction_to(sim, rules, entity, target) else {
        return;
    };
    if let Some(barrel) = sim
        .substrate
        .entities
        .get_mut(id)
        .and_then(|entity| entity.barrel_facing.as_mut())
    {
        barrel.set_rot(rot);
        barrel.set(desired, now);
    }
}

/// The end of `BuildingClass::Update` (`0x00440378..0x004403C6`), whatever the
/// mission: a target out of range of the weapon SelectWeapon picks for it
/// (`vt+0x3AC`, `0x006F7780`) is dropped; an aircraft only while it is low
/// (`AircraftClass 0x0041B980`, the V3 and Dreadnought rockets asking their
/// locomotor: [`crate::sim::movement::air_movement::is_low_flying`]).
pub(super) fn range_drop(sim: &mut Simulation, id: u64, rules: &RuleSet, ctx: ObjectAiCtx<'_>) {
    let Some(entity) = sim.substrate.entities.get(id) else {
        return;
    };
    // Health 0 returned before it (`0x00440072`).
    if entity.dying || entity.health.current == 0 {
        return;
    }
    let Some(target) = entity.attack_target.as_ref().map(|attack| attack.target) else {
        return;
    };
    let weapon = select_weapon(sim, rules, id, Some(target));
    if can_fire_at(sim, rules, id, target, weapon, ctx.overlay_registry) {
        return;
    }
    if let TargetKind::Entity(target_id) = target
        && sim.substrate.entities.get(target_id).is_some_and(|target| {
            target.category == EntityCategory::Aircraft
                && !crate::sim::movement::air_movement::is_low_flying(
                    target,
                    sim.resolved_terrain.as_ref(),
                    Some((rules, &sim.interner)),
                )
        })
    {
        return;
    }
    let _ = sim.assign_target_represented(id, None, Some(rules));
}

impl Simulation {
    /// The bonus ProcessDelayedFire writes on the bullet its FireAt returned
    /// (`0x00450496..0x004504CD`): a building with a support count (`+0x664`)
    /// gives the bullet the [`support_multiplier`] of it (`+0x150`) and
    /// restarts the count; with none the bullet keeps Construct's
    /// [`ProjectilePayload::UNSCALED`]. The combat phase asks it for a
    /// launched delayed shot only ([`BuildingShot::Delayed`]); FireAt's early
    /// exits and a refused launch return no bullet and keep the count.
    pub(crate) fn take_support_bonus(&mut self, id: u64, rules: &RuleSet) -> i32 {
        let Some(entity) = self.substrate.entities.get_mut(id) else {
            return ProjectilePayload::UNSCALED;
        };
        let count = entity.prism_support_count;
        if count == 0 {
            return ProjectilePayload::UNSCALED;
        }
        entity.prism_support_count = 0;
        support_multiplier(rules.general.prism_support.modifier, count)
    }

    /// `BuildingClass::SetTarget` (vt+0x3C8, `0x00443B90`)'s admission of a
    /// requested target for a building: a Selling building (`+0xAC`) or one
    /// not operational (vt+0x350, `0x004555D0`) takes none; any other keeps a
    /// target its slot-0 weapon cannot aim (none, or an `AA=` projectile,
    /// BulletType `+0x2A4`) or one in range of SelectWeapon's weapon
    /// (vt+0x3AC). Other objects admit every target here.
    ///
    /// RESIDUALS:
    /// - InRange's line of fire reads no OverlayTypeClass table here, because
    ///   `Simulation` holds none, so a wall between them is not seen. Trigger:
    ///   retaliation against, or an order onto, a target behind a wall.
    ///   Effect: the building takes a target native refuses; its next
    ///   Mission_Attack asks GetFireError with the table and drops it, and
    ///   the Guard visit that took it queued Attack without its
    ///   `RandomRanged(0, 2)`. Frequency: rare. Later owner: the overlay table
    ///   moving into `Simulation`.
    /// - The restore after a cell target expires
    ///   (`combat::combat_aoe::expire_cell_target_references`) holds only the
    ///   entity store and puts the suspended target back without this
    ///   admission. Trigger: a building whose retaliation suspended its
    ///   mission and whose later cell target expires. Effect: a Selling,
    ///   unpowered or out-of-range building keeps that target until its next
    ///   Mission_Attack or range drop. Frequency: rare. Later owner: that
    ///   restore moving onto `Simulation`.
    pub(crate) fn building_admits_target(
        &self,
        id: u64,
        requested: Option<TargetKind>,
        rules: &RuleSet,
    ) -> bool {
        let Some(entity) = self.substrate.entities.get(id) else {
            return true;
        };
        if entity.category != EntityCategory::Structure {
            return true;
        }
        if entity.mission.current().known() == Some(MissionType::Selling)
            || self.building_operational_state(id, rules) == Some(false)
        {
            return false;
        }
        let Some(target) = requested else {
            return true;
        };
        if !building_weapon0_aims(self, rules, id, target) {
            return true;
        }
        let weapon = select_weapon(self, rules, id, Some(target));
        can_fire_at(self, rules, id, target, weapon, None)
    }

    /// The phase-level combat fixture's stand-in for a building's object-pass
    /// visit (`combat::receiver_fixture`): Mission_Attack when the building
    /// holds a target, then ProcessDelayedFire, whose requests the receiver
    /// then serves, as BuildingClass::Update runs them before the frame's
    /// combat. The fixture honours no mission or dispatch timer.
    #[cfg(test)]
    pub(crate) fn fixture_building_visit(
        &mut self,
        id: u64,
        rules: &RuleSet,
        overlay_registry: Option<&crate::map::overlay_types::OverlayTypeRegistry>,
    ) {
        let ctx = ObjectAiCtx {
            overlay_registry,
            ..Default::default()
        };
        if self
            .substrate
            .entities
            .get(id)
            .is_some_and(|entity| entity.attack_target.is_some())
        {
            let _ = mission_attack(self, id, rules, ctx);
        }
        process_delayed_fire(self, id, rules, ctx);
    }
}

#[cfg(test)]
#[path = "building_missions_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "prism_support_tests.rs"]
mod prism_tests;

#[cfg(test)]
#[path = "building_gattling_tests.rs"]
mod gattling_tests;
