//! Building docking system — repair depot (`UnitRepair=yes`) admission,
//! waiting, service and release.
//!
//! Native shape (gamemd.exe, all read this task):
//!
//! - **Admission is contact-slot capacity, no stored queue.** The depot's
//!   `RadioClass` contact array is sized once at construction:
//!   `BuildingClass::Constructor` 0x0043BCBD reads `Type+0x1780`
//!   (`NumberOfDocks=`), clamps `< 1 ⇒ 1` and calls `Set_Contact_Count`
//!   (0x0043BCD0). Nothing in the binary keeps a waiting list.
//! - **The order admits one unit; the rest pend.** The player's Enter order
//!   runs `UnitClass::Set_Destination` 0x00741970 with the depot
//!   ([`order_onto_depot`]): a free depot is sent DOCKING and HELLOs the unit
//!   into its slot, and the unit drives for the pad; a depot holding a contact
//!   becomes a damaged unit's pending entry (`Unit+0x500`, 0x00741D9F) with no
//!   destination. A pending unit's Enter dispatch parks it beside the depot
//!   (`0x0070D8F0`), and its `FootClass::AI` asks HELLO then CAN_LOAD every
//!   frame (`0x0070D7E0`, [`try_pending_entry`]): whichever pending unit's AI
//!   runs first after the slot frees takes it and drives for the pad. No
//!   unit without the slot is sent to the pad.
//! - **A contact re-probes on its Enter cadence.** `FootClass::Mission_Enter`
//!   0x004D9290 sends `0x0E` to `Contacts[0]` (or to the archive target
//!   `+0x218`, 0x004D929F) on every dispatch, then re-arms
//!   `ftol([Enter] Rate * 900) + RandomRanged(0, 2)` (0x004D946C..0x004D9497,
//!   Scenario stream).
//! - **`BuildingClass::Receive_Radio(0x0E)` 0x0043C2D0 for a UnitRepair
//!   building** (0x0043C7E9..): `+0x660` power flag off ⇒ 10; already linked
//!   AND `Transmit(0x22)` == 10 (0x0043C824..C842) ⇒ 10; the Hospital/Armory
//!   branch (`Type+0x16C1/+0x16C2`, 0x0043CB0C) is NOT taken, so a depot never
//!   evicts a repaired occupant on a probe; not linked and
//!   `Has_Free_Or_Own_Contact_Slot` 0x0065ADF0 ⇒ the building HELLOs the sender
//!   (0x0043C8B0..C8C3); then `0x13` and return 1 (0x0043C9F5..CA37). There
//!   is no `0x12` move assignment for depots — the unit's own NavCom (the
//!   depot, whose `+0x4C` is the pad) drives it.
//! - **`ObjectClass::Receive_Radio(0x22)` 0x005F5320**: health ratio ≥
//!   `Rules+0x16F8` ⇒ 10 else 1. `Rules+0x16F8` is not an INI key:
//!   `RulesClass::ReadAudioVisual` 0x0066B323/0x0066B32D stores the double 1.0
//!   unconditionally. Completion tests ordered ratio >= 1, including signed
//!   Strength and masked division by zero; unordered does not complete.
//! - **Reply 10 to the waiter**: `Mission_Enter` 0x004D92D0 sends BREAK and
//!   calls `Enter_Idle_Mode` (+0x484 = 0x00738970 → Guard for a plain unit).
//! - **Release of a repaired occupant** is the building's repair mission
//!   (`BuildingClass::MissionRepairAndProduce` 0x0044B780, 0x0044C2AE..C4B0 and
//!   0x0044BD5E..): on `0x1C` reply 0x21 the occupant gets `Queue_Mission(Move)`,
//!   `Set_Destination(exit cell)` and BREAK, and its ArchiveTarget and pending
//!   entry cleared. The exit cell is the depot's own rally point (its
//!   ArchiveTarget, 0x0044C40D..C454) when set, else
//!   `BuildingClass::GetDockCellForObject` 0x0044EFB0: the foundation exit list
//!   (`Type+0xED4`, initializer 0x0045C300) walked in order until the unit can
//!   enter the cell. Scatter (`FootClass::Receive_Radio 0x17`) is only reached
//!   from the Hospital/Armory loop and is not part of the depot flow.
//!
//! The repair step itself (Techno `0x1C`, cost via the type vtable) is an
//! adjacent lane: `repair_tick` keeps the pre-existing VERA-internal cost math
//! (gamemd equivalent UNCHECKED). Native never ejects for insufficient funds
//! (0x20 ⇒ state 1 and retry); the `NO_FUNDS_GRACE_TICKS` eject below is
//! VERA-internal drift retained from the previous FSM.
//!
//! ## Dependency rules
//! - Part of sim/ — depends on rules/, sim/radio, sim/movement, sim/mission.
//! - sim/ NEVER depends on render/, ui/, sidebar/, audio/, net/.

use crate::rules::ruleset::RuleSet;
use crate::sim::combat::TargetKind;
use crate::sim::components::Health;
use crate::sim::intern::InternedId;
use crate::sim::mission::authority::EntityReadyInputProvider;
use crate::sim::mission::timer::MissionTimer;
use crate::sim::mission::{MissionId, MissionType};
use crate::sim::movement::locomotor::MovementLayer;
use crate::sim::radio::{self, RadioMessage, RadioPayload, RadioResponse};
use crate::sim::world::Simulation;
use crate::util::fixed_math::ra2_speed_to_leptons_per_second;

/// Dock state machine phase for a unit interacting with a repair depot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum DockPhase {
    /// The depot is the unit's pending entry (`Unit+0x500`): ordered while
    /// the depot held a contact, the unit holds no slot and no destination
    /// toward the pad. Its Enter dispatch parks it near the depot
    /// (`0x0070D8F0`) and its `FootClass::AI` asks for the slot every frame
    /// ([`try_pending_entry`], `0x0070D7E0`).
    WaitForDock,
    /// Holds a contact slot and drives onto the pad (NavCom = the depot).
    /// Ordered onto an offline depot, it holds only the depot as its
    /// ArchiveTarget until its next Enter dispatch.
    EnterDock,
    /// On the dock pad, receiving repair (HP restored, credits deducted).
    Servicing,
    /// Repaired or out of funds — being released off the pad.
    ExitDock,
}

/// Per-entity docking state, stored as `Option<DockState>` on `GameEntity`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DockState {
    /// StableEntityId of the target repair depot building.
    pub dock_building_id: u64,
    /// Current phase of the dock state machine.
    pub phase: DockPhase,
    /// Ticks remaining until the next repair step fires.
    pub service_timer: u32,
    /// Consecutive ticks with insufficient credits (triggers exit after grace).
    pub no_funds_ticks: u32,
    /// Mirror of the unit's `Mission_Enter` handler return
    /// (`ftol([Enter] Rate*900) + RandomRanged(0,2)`, 0x004D946C): the last
    /// probe's frame and delay. The GATE is the object's own mission dispatch
    /// timer (`MissionCom`), written by `mission_enter_dispatch` from the
    /// object-AI slot; this copy only keeps the dock FSM's per-unit cadence
    /// readable.
    #[serde(default)]
    pub enter_retry: MissionTimer,
}

impl DockState {
    /// A depot entry in `phase`, no timers.
    pub fn new(dock_building_id: u64, phase: DockPhase) -> Self {
        Self {
            dock_building_id,
            phase,
            service_timer: 0,
            no_funds_ticks: 0,
            enter_retry: MissionTimer::default(),
        }
    }
}

/// Grace period in ticks before a docked unit exits due to insufficient funds.
/// ~2 seconds at 15 Hz. VERA-internal (native retries without ejecting).
const NO_FUNDS_GRACE_TICKS: u32 = 30;

/// Outcome of one repair-depot service step — the depot's `REPAIR_TICK`
/// trichotomy. Carries the per-step payload the caller applies, so the money/
/// heal math lives in one pure place (`repair_tick`) instead of inline in the
/// dock FSM. Maps to the dock-bus [`RadioResponse`] code via [`Self::radio_response`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepairResponse {
    /// A repair step fired: heal `heal` HP and deduct `cost` credits.
    Roger { heal: i32, cost: i32 },
    /// Not enough credits for this step. `grace` is the incremented no-funds
    /// counter; the caller exits the dock once it reaches [`NO_FUNDS_GRACE_TICKS`].
    InsufficientFunds { grace: u32 },
    /// Fully repaired — exit the dock.
    RepairComplete,
}

impl RepairResponse {
    /// The `RadioClass` response code this maps to on the dock bus.
    #[cfg(test)]
    pub fn radio_response(self) -> RadioResponse {
        match self {
            RepairResponse::Roger { .. } => RadioResponse::Roger,
            RepairResponse::InsufficientFunds { .. } => RadioResponse::InsufficientFunds,
            RepairResponse::RepairComplete => RadioResponse::RepairComplete,
        }
    }
}

/// Decide one repair-depot service step (the `REPAIR_TICK` trichotomy). Pure
/// Cost arithmetic retains the existing VERA adapter:
/// `total = cost * repair_percent / 100`, `cost_per_step = max(1, total *
/// repair_step / max_hp)`, funded ⇒ `Roger`, unfunded ⇒ `InsufficientFunds`
/// (grace incremented), native ratio already-full ⇒ `RepairComplete`. No clock/RNG.
/// VERA-internal cost math. Natively the depot's repair reads the type's
/// vt+0xB0/+0xB4 (`0x006F4CFE`): the repair step cost and `RepairStep=` the
/// building repair uses (`production::repair_step_cost`); porting it is the
/// depot repair's chain.
pub fn repair_tick(
    hp: i32,
    strength: i32,
    unit_cost: i32,
    repair_percent: u16,
    repair_step: u16,
    credits: i32,
    no_funds_ticks: u32,
) -> RepairResponse {
    if repair_is_complete(hp, strength) {
        return RepairResponse::RepairComplete;
    }
    let total_repair_cost = (unit_cost as i64 * repair_percent as i64 / 100) as i32;
    let cost_per_step = if strength > 0 {
        (total_repair_cost as i64 * repair_step as i64 / i64::from(strength)).max(1) as i32
    } else {
        1
    };
    if credits >= cost_per_step {
        RepairResponse::Roger {
            heal: i32::from(repair_step),
            cost: cost_per_step,
        }
    } else {
        RepairResponse::InsufficientFunds {
            grace: no_funds_ticks + 1,
        }
    }
}

/// Object5F5339..537A and Techno6F4DE5..6F4E21 test x87 C0 clear.
/// Unlike a signed HP >= Strength shortcut, this retains negative/zero divisors.
fn repair_is_complete(hp: i32, strength: i32) -> bool {
    matches!(
        Health { current: hp }.compare_ratio(strength, 1.0),
        crate::util::native_x87::MaskedX87Ordering::Equal
            | crate::util::native_x87::MaskedX87Ordering::Greater
    )
}

/// The depot's dock cell: the cell of its `GetCoords`
/// ([`ground_pose::object_center_xy`], `BuildingClass::GetCoords` 0x00447AC0,
/// the foundation centre). `GetDockCoord` for `UnitRepair` with
/// `NumberOfDocks == 1` adds `DockingOffset0`, which is not represented:
/// GADEPT 3x3 has none, and NADEPT 4x3's `128,0,0` stays inside the same cell
/// (origin + (2, 1)).
///
/// [`ground_pose::object_center_xy`]: crate::sim::movement::ground_pose::object_center_xy
pub fn depot_dock_cell(depot: &crate::sim::game_entity::GameEntity) -> (u16, u16) {
    let [x, y] = crate::sim::movement::ground_pose::object_center_xy(depot);
    (x.div_euclid(256) as u16, y.div_euclid(256) as u16)
}

/// The foundation exit list `Type+0xED4` (`0x0089D368 + foundation_id * 0x78`),
/// as written by the static initializer at 0x0045C300 (decoded by simulating
/// its straight-line stores). Relative to the building's NW cell, terminated
/// by `(0x7FFF, 0x7FFF)` in the binary.
///
/// - `1x1` (id 0, row 0x0089D368): S, SW, SE, W, E, N, NW, NE.
/// - `3x3` (id 6, row 0x0089D638): south row, S corners, W/E columns, north
///   row, N corners.
/// - `4x3` (id 12, row 0x0089D908): byte-identical to the `3x3` row — the
///   binary reuses the 3x3 list, so `(3, y)` entries fall inside a 4x3
///   footprint and are skipped by the enter check.
///
/// Other foundations: generated in the same ring order from their own
/// width/height — VERA-internal, gamemd rows UNCHECKED (no stock `UnitRepair`
/// building uses them).
pub fn foundation_exit_list(foundation: &str) -> Vec<(i32, i32)> {
    let def = crate::rules::foundation::foundation_def(foundation);
    match def.id {
        0 => vec![
            (0, 1),
            (-1, 1),
            (1, 1),
            (-1, 0),
            (1, 0),
            (0, -1),
            (-1, -1),
            (1, -1),
        ],
        6 | 12 => ring_exit_list(3, 3),
        _ => ring_exit_list(i32::from(def.width), i32::from(def.height)),
    }
}

fn ring_exit_list(w: i32, h: i32) -> Vec<(i32, i32)> {
    let mut out = Vec::with_capacity(((w + 2) * (h + 2) - w * h) as usize);
    for x in 0..w {
        out.push((x, h));
    }
    out.push((-1, h));
    out.push((w, h));
    for y in (0..h).rev() {
        out.push((-1, y));
        out.push((w, y));
    }
    for x in 0..w {
        out.push((x, -1));
    }
    out.push((-1, -1));
    out.push((w, -1));
    out
}

/// `BuildingClass::GetDockCellForObject` 0x0044EFB0 for a depot: the first
/// exit-list cell the unit can enter (in bounds, walkable, no vehicle/building
/// occupant on the ground layer). Native asks the unit's `vtable+0x1AC`
/// (cell, -1, -1, 0, 0); VERA stands the grid + occupancy test in for it
/// (infantry-only occupants are not rejected — UNCHECKED against native).
pub fn depot_exit_cell(
    sim: &Simulation,
    building_rx: u16,
    building_ry: u16,
    foundation: &str,
) -> Option<(u16, u16)> {
    for (dx, dy) in foundation_exit_list(foundation) {
        let x = i32::from(building_rx) + dx;
        let y = i32::from(building_ry) + dy;
        if x < 0 || y < 0 || x > i32::from(u16::MAX) || y > i32::from(u16::MAX) {
            continue;
        }
        let (x, y) = (x as u16, y as u16);
        if let Some(grid) = sim.path_grid()
            && (x >= grid.width() || y >= grid.height() || !grid.is_walkable(x, y))
        {
            continue;
        }
        let blocked = sim
            .substrate
            .occupancy
            .get(x, y)
            .is_some_and(|cell| cell.has_blockers_on(MovementLayer::Ground));
        if blocked {
            continue;
        }
        return Some((x, y));
    }
    None
}

/// Manhattan distance between two cell coordinates.
fn cell_distance(ax: u16, ay: u16, bx: u16, by: u16) -> u32 {
    let dx = (ax as i32 - bx as i32).unsigned_abs();
    let dy = (ay as i32 - by as i32).unsigned_abs();
    dx.max(dy)
}

/// Arm the `Mission_Enter` epilogue: `ftol([Enter] Rate*900)` first (no RNG),
/// then one `RandomRanged(0, 2)` on the Scenario stream (0x004D9483..0x004D9497),
/// summed. Same draw site and order as the miner's `schedule_enter_retry`.
fn arm_enter_retry(sim: &mut Simulation, rules: &RuleSet, timer: &mut MissionTimer) {
    let delay = sim.mission_rate_epilogue(rules, MissionType::Enter);
    timer.arm(sim.session.binary_frame, delay.max(0) as u32);
}

/// The occupant's Unit class setter `vt+0x480(cell, 1)` (`0x00741970`,
/// [`Simulation::set_unit_destination`]), as `MissionRepairAndProduce`
/// releases a repaired unit (0x0044C473..C496: `Queue_Mission(Move)`,
/// `Map[cell]`, then `vt+0x480`) and as the Foot tail of the depot order
/// drives a contact onto the pad. Its Teleporter arm installs a Drive over a
/// Chrono Miner's Teleport (`0x007425E6..0x0074277E`; a depot is no
/// `DockUnload=` contact). The route to the pad or exit cell is the first
/// Process's Find_Path, whose Unit search admits the depot footprint through
/// `Can_Enter_Cell` (`0x0073F0A0`, UnitRepair arm 0x0073F761 and
/// radio-contact arm 0x0073F57C..5A2).
fn issue_pad_move(sim: &mut Simulation, rules: &RuleSet, id: u64, target: (u16, u16)) {
    let hover = sim.substrate.entities.get(id).is_some_and(|unit| {
        unit.locomotor.as_ref().is_some_and(|loco| {
            loco.active_kind() == crate::rules::locomotor_type::LocomotorKind::Hover
        })
    });
    if !hover {
        sim.set_unit_destination(
            id,
            crate::sim::components::NavTargetRef::cell(target.0, target.1),
            rules,
            true,
        );
        return;
    }
    // RESIDUAL (#689, no native Hover host): the Unit setter's Hover arm is
    // the pass lane's ground move, whose goal resolver relocates the blocked
    // pad cell, so a Hover occupant would park beside the depot and never
    // dock. Until Hover moves through Unit Find_Path, it keeps the pre-setter
    // grid-ignoring direct move onto the pad. Trigger: a Hover unit (ROBO,
    // SAPC, LCRF, YHVR) sent to a depot. Effect: no native NavCom or route
    // costs for that last move. Risk: route shape only.
    let speed = sim
        .resolve_move_info(id, Some(rules))
        .map(|info| info.speed)
        .unwrap_or_else(|| ra2_speed_to_leptons_per_second(4));
    let timing =
        crate::sim::movement::DestinationTiming::from_rules(sim.session.binary_frame, rules.into());
    if crate::sim::movement::issue_direct_move(
        &mut sim.substrate.entities,
        id,
        target,
        speed,
        timing,
    ) && let Some(target) = sim
        .substrate
        .entities
        .get_mut(id)
        .and_then(|entity| entity.movement_target.as_mut())
    {
        target.bypass_grid = true;
    }
}

/// The cell of the depot's ArchiveTarget, its rally point (read at
/// `0x0044C40D`: the target's vt+0x48 coordinate, `>> 8` toward zero). A
/// rally click archives a cell ([`GameEntity::rally_cell`]).
///
/// [`GameEntity::rally_cell`]: crate::sim::game_entity::GameEntity::rally_cell
fn depot_rally_cell(
    sim: &Simulation,
    depot: &crate::sim::game_entity::GameEntity,
) -> Option<(u16, u16)> {
    match depot.archive_target()? {
        TargetKind::Cell(x, y) => Some((x, y)),
        TargetKind::Entity(id) => {
            let [x, y] = crate::sim::movement::ground_pose::object_center_xy(
                sim.substrate.entities.get(id)?,
            );
            Some((u16::try_from(x / 256).ok()?, u16::try_from(y / 256).ok()?))
        }
    }
}

/// BREAK the unit↔depot link over the bus (both slots cleared). No-op when the
/// unit holds no contact with the depot.
fn break_depot_contact(sim: &mut Simulation, unit_id: u64, depot_id: u64) {
    let linked = sim
        .substrate
        .entities
        .get(unit_id)
        .is_some_and(|unit| unit.radio_contacts.contains(depot_id));
    if linked {
        let _ = radio::transmit(
            sim,
            unit_id,
            depot_id,
            RadioMessage::Break,
            RadioPayload::default(),
            None,
        );
    }
}

/// `Queue_Mission` (vt+0x1E8) with its commence-now argument.
fn queue_mission(sim: &mut Simulation, id: u64, mission: MissionId, commence_now: i32) {
    let now = sim.session.binary_frame;
    let _ = sim.mission_queue_exact(id, mission, commence_now, now, &EntityReadyInputProvider);
}

/// HELLO from the unit to the depot, sized first to the depot's contact
/// capacity: the ctor's `max(NumberOfDocks, 1)` (0x0043BCBD..BCD0), grown
/// here rather than at spawn. A free slot links both ends.
fn hello_depot(sim: &mut Simulation, rules: &RuleSet, id: u64, depot: u64) -> RadioResponse {
    let capacity = sim
        .substrate
        .entities
        .get(depot)
        .and_then(|depot| sim.object_type(depot.type_ref(), rules))
        .map(|obj| obj.dock_contact_capacity() as usize);
    if let (Some(capacity), Some(depot)) = (capacity, sim.substrate.entities.get_mut(depot)) {
        depot.radio_contacts.set_capacity(capacity);
    }
    radio::transmit(
        sim,
        id,
        depot,
        RadioMessage::Hello,
        RadioPayload::default(),
        Some(rules),
    )
}

/// `UnitClass::Set_Destination @ 0x00741970` with the depot as destination
/// and Enter queued, as the player's Enter order reaches it (EventClass
/// MEGAMISSION `0x004C747C`), for a damaged Unit holding no contact:
/// - Enter arm (0x00741C4F..0x00741E8E): a depot holding any contact
///   (`In_Radio_Contact`, 0x00741D22) archives the depot (0x00741D41), and a
///   damaged unit (health ratio below `Rules+0x16F8` = 1.0, 0x00741D85..D9B)
///   takes it as its pending entry (`Unit+0x500`, 0x00741D9F) and a NULL
///   destination. The UnitRepair arm then archives that NULL (0x00742C48..C52,
///   slot 0 is not this unit), so the unit ends with no archive, no NavCom
///   and the depot pending.
/// - A free depot is sent DOCKING (0x00741DD6). Online, its UnitRepair
///   receiver HELLOs the unit into its slot (0x0043C8A4..C8C3) and answers
///   ROGER (0x0043CCF2); the UnitRepair arm's HELLO and DOCKING to that
///   contact answer ROGER (0x00742C59..C8E) and the Foot tail takes the
///   depot as NavCom, whose `+0x4C` is `GetDockCoord`, the pad.
/// - Offline (`+0x660`), DOCKING answers NEGATORY (0x0043C7FB): the unit
///   archives the depot (0x00741E09); the UnitRepair arm's HELLO links it and
///   its DOCKING to the contact answers NEGATORY, so it BREAKs and takes a
///   NULL destination (0x00742C90..CA6).
///
/// RESIDUAL: the NavCom is the pad cell (`Cell` destination), not the depot
/// object. Its `+0x4C` coordinate is the same cell for `NumberOfDocks=1`
/// (`GetDockCoord` slot 0, `building_coordinate::dock_coordinate`); what
/// differs is the object-NavCom readers (PerCellProcess 0x0073A3B1 compares
/// the contact with NavCom). Trigger: every depot order. Effect: none on the
/// route; VERA's dock pass detects the pad arrival instead. Frequency: every
/// repair. Risk: none known.
pub(crate) fn order_onto_depot(sim: &mut Simulation, rules: &RuleSet, id: u64, depot: u64) {
    let Some((busy, online, pad)) = sim.substrate.entities.get(depot).map(|depot| {
        (
            !depot.radio_contacts.is_empty(),
            depot.building_online(),
            depot_dock_cell(depot),
        )
    }) else {
        return;
    };
    if busy {
        if let Some(unit) = sim.substrate.entities.get_mut(id) {
            unit.set_archive_target(None);
            unit.dock_state = Some(DockState::new(depot, DockPhase::WaitForDock));
        }
        sim.assign_null_destination(id, Some(rules));
        return;
    }
    if !online {
        if let Some(unit) = sim.substrate.entities.get_mut(id) {
            unit.set_archive_target(Some(TargetKind::Entity(depot)));
            unit.dock_state = Some(DockState::new(depot, DockPhase::EnterDock));
        }
        sim.assign_null_destination(id, Some(rules));
        return;
    }
    let _ = hello_depot(sim, rules, id, depot);
    if let Some(unit) = sim.substrate.entities.get_mut(id) {
        unit.dock_state = Some(DockState::new(depot, DockPhase::EnterDock));
    }
    issue_pad_move(sim, rules, id, pad);
}

/// `FootClass::TryEnterTransport @ 0x0070D7E0` (the name is a lead; it serves
/// every `Unit+0x500` pending entry) for a pending depot, called from
/// `FootClass::AI` (0x004DAEDC) whenever vt+0x1D8 (the warp-in) is clear:
/// - the depot no longer alive (`+0x90`, 0x0070D7F2) drops the entry;
/// - HELLO (0x0070D815): NEGATORY while the depot's slot is held, and the
///   entry waits for the next frame (the Aircraft/Helipad re-probe arm
///   0x0070D894..D8CD is not a depot's);
/// - CAN_LOAD (0x0F, 0x0070D825): ROGER ⇒ `Queue_Mission(Enter, 1)`,
///   `Set_Destination(depot, 1)` and the entry cleared (0x0070D836..D84F):
///   the linked unit drives onto the pad; any other answer ⇒
///   `Queue_Mission(NONE)`, `Set_Destination(NULL, 1)`, the entry cleared and
///   BREAK to the depot (0x0070D85E..D889).
///
/// So whichever pending unit's `FootClass::AI` runs first after the slot
/// frees takes it, in live-object order.
pub(crate) fn try_pending_entry(sim: &mut Simulation, rules: &RuleSet, id: u64) {
    let Some(depot) = sim.substrate.entities.get(id).and_then(|unit| {
        unit.dock_state
            .as_ref()
            .filter(|dock| dock.phase == DockPhase::WaitForDock)
            .map(|dock| dock.dock_building_id)
    }) else {
        return;
    };
    let Some(pad) = sim
        .substrate
        .entities
        .get(depot)
        .filter(|depot| depot.is_ai_alive())
        .map(depot_dock_cell)
    else {
        if let Some(unit) = sim.substrate.entities.get_mut(id) {
            unit.dock_state = None;
        }
        return;
    };
    if hello_depot(sim, rules, id, depot) != RadioResponse::Roger {
        return;
    }
    let reply = radio::transmit(
        sim,
        id,
        depot,
        RadioMessage::CanEnter,
        RadioPayload::default(),
        Some(rules),
    );
    if reply == RadioResponse::Roger {
        queue_mission(sim, id, MissionId::from_known(MissionType::Enter), 1);
        if let Some(unit) = sim.substrate.entities.get_mut(id) {
            unit.dock_state = Some(DockState::new(depot, DockPhase::EnterDock));
        }
        issue_pad_move(sim, rules, id, pad);
    } else {
        queue_mission(sim, id, MissionId::NONE, 0);
        sim.assign_null_destination(id, Some(rules));
        if let Some(unit) = sim.substrate.entities.get_mut(id) {
            unit.dock_state = None;
        }
        break_depot_contact(sim, id, depot);
    }
}

/// Whether the unit's Enter dispatch is the depot's: it holds a depot dock
/// state, and for a pending entry no other contact. `Mission_Enter` probes
/// `Contacts[0]` first (0x004D9294), so a harvester that keeps a pending
/// depot while it docks at its refinery enters the refinery.
pub(crate) fn depot_owns_enter(unit: &crate::sim::game_entity::GameEntity) -> bool {
    unit.dock_state.as_ref().is_some_and(|dock| {
        dock.phase != DockPhase::WaitForDock || unit.radio_contacts.slot(0).is_none()
    })
}

/// `FootClass::Mission_Enter @ 0x004D9290` for a repair-depot unit, run from
/// the unit's OWN mission-dispatch slot (the Unit `Enter` arm of the object-AI
/// shell) so the epilogue draw lands where native draws it: inside this
/// unit's `TechnoClass::AI` visit, interleaved in object order with every
/// other object's dispatch draws — not as a batch after production.
///
/// Native body, every dispatch:
/// - `Transmit(0x0E)` to `Contacts[0]`, else to the ArchiveTarget
///   (0x004D929F); reply `1` (or `+0x418`) keeps the unit in Enter, the NavCom
///   drives it; any other reply ⇒ BREAK (0x004D92D0) +
///   `Enter_Idle_Mode(0,1)` (0x004D92E2), which is the "repaired ⇒ 10" exit;
/// - neither ⇒ the pending entry (`0x0070D8F0`, 0x004D9427): a pending depot
///   still alive sends the unit to its NearbyLocation (`0x00703590` seeded at
///   the depot's coordinate) with `Queue_Mission(Move)` (0x0070D918..D95F),
///   so a waiter parks beside the depot rather than on its pad. With no such
///   cell, `Enter_Idle_Mode(0,1)` (0x004D945C), which keeps the entry; with
///   the depot dead the entry is dropped;
/// - then, on EVERY path, `ftol([Enter] Rate*900)` followed by ONE
///   `RandomRanged(0, 2)` (0x004D946C..0x004D9497, Scenario stream) — the
///   handler return. So a dispatch that leaves Enter still draws.
///
/// Returns that handler delay; `DockState::enter_retry` mirrors it so the
/// dock FSM's readers keep a per-unit cadence view. The pad-side servicing
/// and release stay in [`tick_building_docks`] (native: the building's
/// `MissionRepairAndProduce` 0x0044B780 repair tick), untouched here.
pub(crate) fn mission_enter_dispatch(sim: &mut Simulation, rules: &RuleSet, id: u64) -> i32 {
    let now = sim.session.binary_frame;
    let Some((dock_building_id, phase, hp, strength, owner, archived)) =
        sim.substrate.entities.get(id).and_then(|unit| {
            let ds = unit.dock_state.as_ref()?;
            Some((
                ds.dock_building_id,
                ds.phase,
                unit.health.current,
                sim.object_type(unit.type_ref(), rules)?.strength,
                unit.owner(),
                unit.archive_target() == Some(TargetKind::Entity(ds.dock_building_id)),
            ))
        })
    else {
        return 1;
    };
    if matches!(phase, DockPhase::Servicing | DockPhase::ExitDock) {
        // On the pad the unit natively sits on the queued Sleep the building
        // assigned it; VERA keeps Enter as the representation and dispatches
        // it as a no-draw one-frame return (VERA-internal, the base
        // `Mission_Sleep` return value UNCHECKED). No probe, no draw.
        return 1;
    }

    // Depot still a valid probe target (alive, own house, UnitRepair)?
    let depot_online = sim
        .substrate
        .entities
        .get(dock_building_id)
        .filter(|depot| depot.health.current > 0 && !depot.dying && depot.owner() == owner)
        .filter(|depot| {
            sim.object_type(depot.type_ref(), rules)
                .is_some_and(|obj| obj.unit_repair)
        })
        .map(|depot| depot.building_online());

    let linked = sim
        .substrate
        .entities
        .get(id)
        .is_some_and(|unit| unit.radio_contacts.contains(dock_building_id));
    let mut leave = false;
    match depot_online {
        // 0x004D92B2: DOCKING to the contact or the archived depot.
        Some(online) if linked || archived => {
            // 0x0043C7FB: an offline depot (its `+0x660` latch, which a
            // Temporal warp clears) answers 10 before any other test.
            // 0x0043C824..C842: a linked sender whose 0x22 answers 10 (ratio
            // >= 1.0) gets 10 back. Mission_Enter then BREAKs and calls
            // Enter_Idle_Mode(0, 1) at 0x004D92E2 (Guard for a plain unit,
            // 0x00738970; a Harvester=yes unit takes the harvester arm:
            // Harvest, or Guard for a human owner standing off ore — with the
            // depot contact already broken, the arm's radio gate only sees a
            // refinery link). The unit leaves Enter; the epilogue draw below
            // still happens.
            if !online || linked && repair_is_complete(hp, strength) {
                break_depot_contact(sim, id, dock_building_id);
                leave = true;
            } else if !linked {
                // 0x0043C8A4..C8C3: an archived unit that is no contact is
                // HELLOed into a free slot; the reply is ROGER either way.
                let _ = hello_depot(sim, rules, id, dock_building_id);
            }
        }
        // 0x0070D8F0 for the pending depot.
        Some(_) if phase == DockPhase::WaitForDock => {
            let parked = sim
                .techno_nearby_location(id, Some(dock_building_id), rules)
                .and_then(|(x, y)| Some((u16::try_from(x).ok()?, u16::try_from(y).ok()?)));
            match parked {
                Some(cell) => {
                    sim.set_unit_destination(
                        id,
                        crate::sim::components::NavTargetRef::cell(cell.0, cell.1),
                        rules,
                        true,
                    );
                    queue_mission(sim, id, MissionId::from_known(MissionType::Move), 0);
                }
                // 0x004D945C: Enter_Idle_Mode; the entry stays.
                None => {
                    enter_idle_mode(sim, rules, id);
                    return epilogue(sim, rules, id, now);
                }
            }
        }
        // No contact, no archive, no pending entry (a link the depot broke):
        // Enter_Idle_Mode (0x004D945C).
        Some(_) => leave = true,
        // Depot gone: `tick_building_docks` drops the dock state this frame
        // (PointerExpired clears the contact and the pending entry).
        None => {}
    }
    if leave {
        if let Some(unit) = sim.substrate.entities.get_mut(id) {
            unit.dock_state = None;
            unit.movement_target = None;
        }
        enter_idle_mode(sim, rules, id);
    }
    epilogue(sim, rules, id, now)
}

/// `Enter_Idle_Mode(0, 1)` (vt+0x484): Guard for a plain unit
/// (`0x00738970`), the harvester arm for a `Harvester=yes` unit.
fn enter_idle_mode(sim: &mut Simulation, rules: &RuleSet, id: u64) {
    let is_miner = sim
        .substrate
        .entities
        .get(id)
        .is_some_and(|unit| unit.miner.is_some());
    let selector = if is_miner {
        crate::sim::world::harvester_enter_idle_mode_selector(sim, id, rules, false)
    } else {
        Some(MissionType::Guard)
    };
    if let Some(selector) = selector {
        queue_mission(sim, id, MissionId::from_known(selector), 0);
    }
}

/// The Mission_Enter epilogue: `ftol(Rate*900) + RandomRanged(0,2)`, every
/// dispatch.
fn epilogue(sim: &mut Simulation, rules: &RuleSet, id: u64, now: u32) -> i32 {
    let mut timer = MissionTimer::default();
    arm_enter_retry(sim, rules, &mut timer);
    if let Some(ds) = sim
        .substrate
        .entities
        .get_mut(id)
        .and_then(|unit| unit.dock_state.as_mut())
    {
        ds.enter_retry = timer;
    }
    debug_assert_eq!(timer.start_frame, now);
    i32::try_from(timer.duration).unwrap_or(i32::MAX)
}

/// Advance building dock state machines for all entities with `dock_state`.
///
/// Called once per tick from `advance_tick()`, after the factory step.
/// Uses the two-phase snapshot pattern to avoid borrow conflicts. The
/// waiter's `0x0E` probe and its epilogue draw are NOT here — they run in the
/// unit's own dispatch slot ([`mission_enter_dispatch`]); this pass only
/// consumes the resulting link state and owns the pad-side service/release.
pub fn tick_building_docks(sim: &mut Simulation, rules: &RuleSet) {
    struct DockSnapshot {
        id: u64,
        owner: InternedId,
        type_ref: InternedId,
        rx: u16,
        ry: u16,
        hp: i32,
        strength: i32,
        moving: bool,
        dock_building_id: u64,
        phase: DockPhase,
        service_timer: u32,
        no_funds_ticks: u32,
    }

    let snapshots: Vec<DockSnapshot> = sim
        .substrate
        .entities
        .values()
        .filter_map(|e| {
            let ds = e.dock_state.as_ref()?;
            Some(DockSnapshot {
                id: e.stable_id(),
                owner: e.owner(),
                type_ref: e.type_ref(),
                rx: e.position.rx,
                ry: e.position.ry,
                hp: e.health.current,
                strength: sim.object_type(e.type_ref(), rules)?.strength,
                moving: e.movement_target.is_some(),
                dock_building_id: ds.dock_building_id,
                phase: ds.phase,
                service_timer: ds.service_timer,
                no_funds_ticks: ds.no_funds_ticks,
            })
        })
        .collect();

    if snapshots.is_empty() {
        return;
    }

    struct DockMutation {
        id: u64,
        new_phase: Option<DockPhase>,
        new_timer: Option<u32>,
        new_no_funds: Option<u32>,
        heal_amount: i32,
        strength: i32,
        repair_complete: bool,
        deduct_credits: i32,
        /// A paid Techno `Receive_Radio 0x1C` step ran.
        repaired: bool,
        clear_dock: bool,
        clear_movement: bool,
    }

    let mut mutations: Vec<DockMutation> = Vec::new();

    for snap in &snapshots {
        let mut m = DockMutation {
            id: snap.id,
            new_phase: None,
            new_timer: None,
            new_no_funds: None,
            heal_amount: 0,
            strength: snap.strength,
            repair_complete: false,
            deduct_credits: 0,
            repaired: false,
            clear_dock: false,
            clear_movement: false,
        };

        // Verify depot still exists and is alive/friendly.
        //
        // `BuildingClass::Receive_Radio(0x0E)` @ 0x0043C2D0 answers 10 to a
        // probe while the building is offline (`+0x660 == 0`, test at
        // 0x0043C7FB): the toggle/warp latch, not house power. The probe runs
        // in `mission_enter_dispatch`; this adapter only reads its result.
        let depot_info = sim
            .substrate
            .entities
            .get(snap.dock_building_id)
            .and_then(|depot| {
                if depot.health.current == 0 || depot.dying {
                    return None;
                }
                if depot.owner() != snap.owner {
                    return None;
                }
                let obj = sim.object_type(depot.type_ref(), rules)?;
                if !obj.unit_repair {
                    return None;
                }
                Some((
                    depot.position.rx,
                    depot.position.ry,
                    obj.foundation.clone(),
                    depot_dock_cell(depot),
                    depot_rally_cell(sim, depot),
                ))
            });

        let Some((depot_rx, depot_ry, foundation, (dock_rx, dock_ry), rally)) = depot_info else {
            // Depot gone or invalid — abort docking.
            break_depot_contact(sim, snap.id, snap.dock_building_id);
            m.clear_dock = true;
            mutations.push(m);
            continue;
        };

        // A warped object runs none of its AI (`GameEntity::ai_frozen`): the
        // approach is the waiter's own mission, the service step and the
        // release the depot's (`MissionRepairAndProduce`). Its timer holds.
        let actor = match snap.phase {
            DockPhase::Servicing | DockPhase::ExitDock => snap.dock_building_id,
            DockPhase::WaitForDock | DockPhase::EnterDock => snap.id,
        };
        if sim
            .substrate
            .entities
            .get(actor)
            .is_some_and(crate::sim::game_entity::GameEntity::ai_frozen)
        {
            continue;
        }

        let dist = cell_distance(snap.rx, snap.ry, dock_rx, dock_ry);

        match snap.phase {
            // A pending entry waits in its own FootClass::AI
            // ([`try_pending_entry`]); nothing here.
            DockPhase::WaitForDock => {}
            DockPhase::EnterDock => {
                // The 0x0E probe (and its draw) already ran in this unit's
                // own dispatch slot; only the resulting link is read here. An
                // unlinked unit (an offline depot's archive, or a link the
                // depot broke) is left to that dispatch.
                let linked = sim
                    .substrate
                    .entities
                    .get(snap.id)
                    .is_some_and(|unit| unit.radio_contacts.contains(snap.dock_building_id));

                if linked {
                    if dist == 0 {
                        // Pad arrival (PerCellProcess 0x15 ⇒ building repair
                        // mission). Native also queues Sleep on the unit; the
                        // exact mission stays represented as Enter here.
                        m.clear_movement = true;
                        m.new_phase = Some(DockPhase::Servicing);
                        m.new_timer = Some(rules.general.unit_repair_rate_ticks);
                    } else if !snap.moving {
                        // Natively the depot NavCom keeps driving; VERA
                        // re-issues the pad move to a contact that stopped
                        // short of it (VERA-internal stand-in, UNCHECKED).
                        issue_pad_move(sim, rules, snap.id, (dock_rx, dock_ry));
                    }
                }
            }
            DockPhase::Servicing => {
                if repair_is_complete(snap.hp, snap.strength) {
                    m.new_phase = Some(DockPhase::ExitDock);
                } else {
                    let timer = snap.service_timer.saturating_sub(1);
                    if timer == 0 {
                        // A repair step is due — resolve the REPAIR_TICK trichotomy.
                        let unit_cost = sim
                            .object_type(snap.type_ref, rules)
                            .map(|obj| obj.cost)
                            .unwrap_or(0);
                        let credits = crate::sim::house_state::house_state_for_owner(
                            &sim.houses,
                            sim.interner.resolve(snap.owner),
                            &sim.interner,
                        )
                        .map(|h| h.economy.credits)
                        .unwrap_or(0);

                        // The adapter's whole percent and positive step, as
                        // it read them before the rules kept the native values.
                        match repair_tick(
                            snap.hp,
                            snap.strength,
                            unit_cost,
                            (rules.general.repair_percent * 100.0).round() as u16,
                            rules.general.repair_step.max(1) as u16,
                            credits,
                            snap.no_funds_ticks,
                        ) {
                            RepairResponse::Roger { heal, cost } => {
                                m.heal_amount = heal;
                                m.deduct_credits = cost;
                                m.repaired = true;
                                m.new_no_funds = Some(0);
                                // Radio6F4DE5..6F4E21 completes on this same
                                // repair response and resets both health values.
                                if repair_is_complete(snap.hp.wrapping_add(heal), snap.strength) {
                                    m.repair_complete = true;
                                    m.new_phase = Some(DockPhase::ExitDock);
                                }
                            }
                            RepairResponse::InsufficientFunds { grace } => {
                                if grace >= NO_FUNDS_GRACE_TICKS {
                                    m.new_phase = Some(DockPhase::ExitDock);
                                } else {
                                    m.new_no_funds = Some(grace);
                                }
                            }
                            RepairResponse::RepairComplete => {
                                m.new_phase = Some(DockPhase::ExitDock);
                            }
                        }
                        m.new_timer = Some(rules.general.unit_repair_rate_ticks);
                    } else {
                        m.new_timer = Some(timer);
                    }
                }
            }
            DockPhase::ExitDock => {
                // MissionRepairAndProduce 0x0044C3EE..0x0044C4B7: the exit
                // cell is GetDockCellForObject (vt+0x4D4), replaced by the
                // cell of the depot's own ArchiveTarget, its rally point
                // (0x0044C40D..C454). With a valid cell: Queue_Mission(Move),
                // Set_Destination(cell, 1), the occupant's ArchiveTarget and
                // pending entry cleared (0x0044C49F, 0x0044C4B1), BREAK. An
                // invalid cell (0x0044C458..C46D) leaves the unit linked on
                // the pad and the building retries.
                //
                // RESIDUAL: the arm before it (0x0044C397..C3E9), for an
                // occupant with an ArchiveTarget whose house is not
                // human-controlled (`HouseClass::IsControlledByHuman @
                // 0x0050B730`), sends it to that archive instead. Trigger: an
                // AI-owned unit released by a depot. Effect: none today —
                // only the player's order docks a unit. Frequency: dormant.
                // Risk: an AI depot visit would release to the exit cell.
                let exit = rally.or_else(|| depot_exit_cell(sim, depot_rx, depot_ry, &foundation));
                if let Some(exit) = exit {
                    queue_mission(sim, snap.id, MissionId::from_known(MissionType::Move), 0);
                    issue_pad_move(sim, rules, snap.id, exit);
                    if let Some(unit) = sim.substrate.entities.get_mut(snap.id) {
                        unit.set_archive_target(None);
                    }
                    break_depot_contact(sim, snap.id, snap.dock_building_id);
                    m.clear_dock = true;
                }
            }
        }

        mutations.push(m);
    }

    // Apply mutations.
    for m in &mutations {
        let Some(entity) = sim.substrate.entities.get_mut(m.id) else {
            continue;
        };

        if m.clear_movement {
            entity.movement_target = None;
        }

        if m.clear_dock {
            entity.dock_state = None;
            continue;
        }

        if let Some(ref mut ds) = entity.dock_state {
            if let Some(phase) = m.new_phase {
                ds.phase = phase;
            }
            if let Some(timer) = m.new_timer {
                ds.service_timer = timer;
            }
            if let Some(nf) = m.new_no_funds {
                ds.no_funds_ticks = nf;
            }
        }

        if m.heal_amount > 0 {
            entity.health.current = entity.health.current.wrapping_add(m.heal_amount);
            entity.estimated_health.add_repair(m.heal_amount);
        }

        if m.repair_complete {
            // Radio6F4E17..21: Strength is written to actual and estimated HP.
            // Rules+16F8 is the fixed 1.0 completion threshold (66B323/66B32D).
            entity.health.current = m.strength;
            entity.estimated_health.reset(m.strength);
        }

        if m.deduct_credits > 0 {
            if let Some(house) = crate::sim::house_state::house_state_for_owner_mut(
                &mut sim.houses,
                sim.interner.resolve(entity.owner()),
                &sim.interner,
            ) {
                house.economy.credits = (house.economy.credits - m.deduct_credits).max(0);
            }
        }

        // Radio 0x1C `0x006F4D61..0x006F4DA6`: after the paid step, a Foot
        // with a parasite forces it off (suppression 50, then ExitUnit), so
        // the depot deletes a Terror Drone on its first repair step.
        if m.repaired
            && let Some(eater) = entity.parasite_eating_me
        {
            sim.parasite_force_release(
                eater,
                crate::sim::combat::parasite::FORCED_RELEASE_SUPPRESSION_FRAMES,
                rules,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::entities::EntityCategory;
    use crate::rules::ini_parser::IniFile;
    use crate::sim::command::Command;
    use crate::sim::components::{Health, NavTargetRef};
    use crate::sim::game_entity::GameEntity;
    use crate::sim::occupancy::CellListInsertion;
    use crate::sim::pathfinding::PathGrid;

    fn structure_at(rx: u16, ry: u16, foundation: &str) -> GameEntity {
        let mut depot = GameEntity::new_at_frame_zero_for_test(
            DEPOT,
            rx,
            ry,
            0,
            0,
            Default::default(),
            Health { current: 1000 },
            Default::default(),
            EntityCategory::Structure,
            0,
            5,
            false,
        );
        depot.foundation = foundation.to_string();
        depot
    }

    #[test]
    fn dock_cell_is_the_cell_of_the_foundation_centre() {
        // NADEPT 4x3: DockingOffset0=128,0,0 stays in origin + (2, 1).
        for (foundation, cell) in [
            ("3x3", (11, 21)),
            ("4x3", (12, 21)),
            ("2x2", (11, 21)),
            ("1x1", (10, 20)),
        ] {
            assert_eq!(
                depot_dock_cell(&structure_at(10, 20, foundation)),
                cell,
                "{foundation}"
            );
        }
    }

    #[test]
    fn cell_distance_same() {
        assert_eq!(cell_distance(5, 5, 5, 5), 0);
    }

    #[test]
    fn cell_distance_diagonal() {
        assert_eq!(cell_distance(5, 5, 8, 9), 4);
    }

    /// Exit rows decoded from the 0x0045C300 initializer (rows 0x0089D368,
    /// 0x0089D638, 0x0089D908).
    #[test]
    fn foundation_exit_lists_match_native_rows() {
        assert_eq!(
            foundation_exit_list("1x1"),
            vec![
                (0, 1),
                (-1, 1),
                (1, 1),
                (-1, 0),
                (1, 0),
                (0, -1),
                (-1, -1),
                (1, -1)
            ]
        );
        let three = vec![
            (0, 3),
            (1, 3),
            (2, 3),
            (-1, 3),
            (3, 3),
            (-1, 2),
            (3, 2),
            (-1, 1),
            (3, 1),
            (-1, 0),
            (3, 0),
            (0, -1),
            (1, -1),
            (2, -1),
            (-1, -1),
            (3, -1),
        ];
        assert_eq!(foundation_exit_list("3x3"), three);
        assert_eq!(
            foundation_exit_list("4x3"),
            three,
            "the binary reuses the 3x3 row for 4x3"
        );
    }

    // --- 7c: repair-depot REPAIR_TICK trichotomy (`repair_tick`) ---
    // cost=1000, percent=15 -> total=150; step=8, max_hp=300 -> cost_per_step=4.

    #[test]
    fn depot_repair_full_hp_returns_complete() {
        assert_eq!(
            repair_tick(300, 300, 1000, 15, 8, 0, 0),
            RepairResponse::RepairComplete
        );
    }

    #[test]
    fn depot_repair_funded_returns_roger_with_step_and_cost() {
        assert_eq!(
            repair_tick(100, 300, 1000, 15, 8, 10, 0),
            RepairResponse::Roger { heal: 8, cost: 4 }
        );
        assert_eq!(
            repair_tick(100, 300, 1000, 15, 8, 4, 0),
            RepairResponse::Roger { heal: 8, cost: 4 }
        );
    }

    #[test]
    fn depot_repair_unfunded_returns_insufficient_with_incremented_grace() {
        assert_eq!(
            repair_tick(100, 300, 1000, 15, 8, 3, 0),
            RepairResponse::InsufficientFunds { grace: 1 }
        );
        assert_eq!(
            repair_tick(100, 300, 1000, 15, 8, 0, 5),
            RepairResponse::InsufficientFunds { grace: 6 }
        );
    }

    #[test]
    fn depot_repair_cost_per_step_clamps_to_one() {
        assert_eq!(
            repair_tick(100, 300, 10, 15, 8, 1, 0),
            RepairResponse::Roger { heal: 8, cost: 1 }
        );
        assert_eq!(
            repair_tick(100, 300, 10, 15, 8, 0, 0),
            RepairResponse::InsufficientFunds { grace: 1 }
        );
    }

    #[test]
    fn depot_repair_funded_step_ignores_prior_grace() {
        assert_eq!(
            repair_tick(100, 300, 1000, 15, 8, 10, NO_FUNDS_GRACE_TICKS - 1),
            RepairResponse::Roger { heal: 8, cost: 4 }
        );
    }

    #[test]
    fn depot_repair_grace_reaches_cap() {
        assert_eq!(
            repair_tick(100, 300, 1000, 15, 8, 0, NO_FUNDS_GRACE_TICKS - 1),
            RepairResponse::InsufficientFunds {
                grace: NO_FUNDS_GRACE_TICKS
            }
        );
    }

    #[test]
    fn depot_repair_response_maps_to_radio_codes() {
        assert_eq!(
            RepairResponse::Roger { heal: 8, cost: 4 }.radio_response(),
            RadioResponse::Roger
        );
        assert_eq!(
            RepairResponse::InsufficientFunds { grace: 1 }.radio_response(),
            RadioResponse::InsufficientFunds
        );
        assert_eq!(
            RepairResponse::RepairComplete.radio_response(),
            RadioResponse::RepairComplete
        );
        assert_eq!(RadioResponse::Roger as u8, 0x01);
        assert_eq!(RadioResponse::InsufficientFunds as u8, 0x20);
        assert_eq!(RadioResponse::RepairComplete as u8, 0x21);
    }

    // --- Admission / waiting / release through the production order path ---

    const DEPOT: u64 = 500;
    const DEPOT_RX: u16 = 10;
    const DEPOT_RY: u16 = 10;

    fn depot_rules() -> RuleSet {
        depot_rules_with_strength(300)
    }

    fn depot_rules_with_strength(strength: i32) -> RuleSet {
        let ini = IniFile::from_str(&format!(
            "[General]\n\
             RepairPercent=15%\n\
             RepairStep=8\n\
             URepairRate=.016\n\
             [Enter]\n\
             Rate=.016\n\
             [InfantryTypes]\n\
             [VehicleTypes]\n\
             0=MTNK\n\
             1=HARV\n\
             [AircraftTypes]\n\
             [BuildingTypes]\n\
             0=GADEPT\n\
             [MTNK]\n\
             Name=Grizzly\n\
             Cost=700\n\
             Strength={strength}\n\
             Speed=6\n\
             Locomotor={{4A582741-9839-11d1-B709-00A024DDAFD1}}\n\
             [HARV]\n\
             Name=War Miner\n\
             Cost=1400\n\
             Strength=600\n\
             Speed=4\n\
             Locomotor={{4A582741-9839-11d1-B709-00A024DDAFD1}}\n\
             Harvester=yes\n\
             [GADEPT]\n\
             Name=Depot\n\
             Foundation=3x3\n\
             UnitRepair=yes\n\
             NumberImpassableRows=1\n\
             Strength=1000\n",
        ));
        RuleSet::from_ini(&ini).expect("depot rules")
    }

    fn spawn_entity(
        sim: &mut Simulation,
        sid: u64,
        type_id: &str,
        category: EntityCategory,
        rx: u16,
        ry: u16,
        hp: i32,
    ) {
        let owner_id = sim.interner.intern("Americans");
        let type_name = type_id;
        let type_id = sim.interner.intern(type_id);
        let mut ge = GameEntity::new_at_frame_zero_for_test(
            sid,
            rx,
            ry,
            0,
            0,
            owner_id,
            Health { current: hp },
            type_id,
            category,
            0,
            5,
            category == EntityCategory::Unit,
        );
        ge.lifecycle.in_limbo = false;
        if category == EntityCategory::Unit {
            // A live Drive mover: the depot's class setter and the Process
            // corridor need its locomotor.
            ge.locomotor = Some(
                crate::sim::movement::locomotor::LocomotorState::from_object_type(
                    depot_rules().object(type_name).expect("fixture unit type"),
                    0,
                ),
            );
            sim.substrate.occupancy.add(
                rx,
                ry,
                sid,
                MovementLayer::Ground,
                None,
                CellListInsertion::PrependNonBuilding,
            );
        }
        sim.substrate.entities.insert(ge);
        if sim.substrate.next_stable_object_id <= sid {
            sim.substrate.next_stable_object_id = sid + 1;
        }
    }

    fn spawn_depot(sim: &mut Simulation) {
        spawn_entity(
            sim,
            DEPOT,
            "GADEPT",
            EntityCategory::Structure,
            DEPOT_RX,
            DEPOT_RY,
            1000,
        );
        sim.substrate.entities.get_mut(DEPOT).unwrap().foundation = "3x3".to_string();
        for y in DEPOT_RY..DEPOT_RY + 3 {
            for x in DEPOT_RX..DEPOT_RX + 3 {
                sim.substrate.occupancy.add(
                    x,
                    y,
                    DEPOT,
                    MovementLayer::Ground,
                    None,
                    CellListInsertion::AppendBuilding,
                );
            }
        }
    }

    fn spawn_tank(sim: &mut Simulation, sid: u64, rx: u16, ry: u16) {
        spawn_entity(sim, sid, "MTNK", EntityCategory::Unit, rx, ry, 100);
    }

    fn setup(tank_count: u64) -> (Simulation, RuleSet) {
        let rules = depot_rules();
        let mut sim = Simulation::new();
        {
            use crate::sim::house_state::HouseState;
            let owner_id = sim.interner.intern("Americans");
            let mut house = HouseState::new(owner_id, 0, None, false, 0, 10);
            house.economy.credits = 10_000;
            sim.houses.insert(owner_id, house);
        }
        spawn_depot(&mut sim);
        for i in 0..tank_count {
            spawn_tank(&mut sim, 1 + i, 14 + i as u16, 11);
        }
        sim.install_fixture_path_grid(Some(&PathGrid::new(64, 64)));
        (sim, rules)
    }

    #[test]
    fn depot_completion_preserves_signed_and_masked_ratio_domain() {
        for (hp, strength, complete) in [
            (70_000, 100_000, false),
            (100_000, 70_000, true),
            (-20, -10, true),
            (-5, -10, false),
            (1, 0, true),
            (-1, 0, false),
            (0, 0, false),
        ] {
            assert_eq!(
                repair_is_complete(hp, strength),
                complete,
                "{hp}/{strength}"
            );
            assert_eq!(
                matches!(
                    repair_tick(hp, strength, 100, 15, 8, 10_000, 0),
                    RepairResponse::RepairComplete
                ),
                complete
            );
        }
    }

    #[test]
    fn depot_service_wraps_both_independent_signed_health_values() {
        let (mut sim, _) = setup(1);
        let rules = depot_rules_with_strength(i32::MAX);
        let unit = sim.substrate.entities.get_mut(1).unwrap();
        unit.health.current = i32::MAX - 1;
        unit.estimated_health =
            crate::sim::estimated_health::EstimatedHealth::from_raw(i32::MAX - 2);
        let dock = DockState::new(DEPOT, DockPhase::Servicing);
        unit.dock_state = Some(dock);
        tick_building_docks(&mut sim, &rules);
        let unit = sim.substrate.entities.get(1).unwrap();
        assert_eq!(unit.health.current, i32::MIN + 6);
        assert_eq!(unit.estimated_health.get(), i32::MIN + 5);
        assert_eq!(
            unit.dock_state.as_ref().unwrap().phase,
            DockPhase::Servicing
        );
    }

    #[test]
    fn depot_service_adds_reservations_and_resets_them_on_completion() {
        let (mut sim, rules) = setup(1);
        let unit = sim.substrate.entities.get_mut(1).unwrap();
        let dock = DockState::new(DEPOT, DockPhase::Servicing);
        unit.dock_state = Some(dock);
        unit.estimated_health = crate::sim::estimated_health::EstimatedHealth::from_raw(-20);

        tick_building_docks(&mut sim, &rules);
        let unit = sim.substrate.entities.get(1).unwrap();
        assert_eq!(unit.health.current, 108);
        assert_eq!(unit.estimated_health.get(), -12);
        assert_eq!(
            unit.dock_state.as_ref().unwrap().phase,
            DockPhase::Servicing
        );

        let unit = sim.substrate.entities.get_mut(1).unwrap();
        unit.health.current = 299;
        unit.estimated_health = crate::sim::estimated_health::EstimatedHealth::from_raw(-20);
        unit.dock_state.as_mut().unwrap().service_timer = 0;
        tick_building_docks(&mut sim, &rules);
        let unit = sim.substrate.entities.get(1).unwrap();
        assert_eq!(unit.health.current, 300);
        assert_eq!(unit.estimated_health.get(), 300);
        assert_eq!(unit.dock_state.as_ref().unwrap().phase, DockPhase::ExitDock);

        // Already-full admission bypasses the repair receiver entirely.
        let unit = sim.substrate.entities.get_mut(1).unwrap();
        unit.estimated_health = crate::sim::estimated_health::EstimatedHealth::from_raw(-20);
        unit.dock_state.as_mut().unwrap().phase = DockPhase::Servicing;
        tick_building_docks(&mut sim, &rules);
        assert_eq!(
            sim.substrate
                .entities
                .get(1)
                .unwrap()
                .estimated_health
                .get(),
            -20
        );
    }

    fn order_repair(sim: &mut Simulation, rules: &RuleSet, tank: u64) -> bool {
        sim.apply_command(
            "Americans",
            &Command::RepairAtDepot {
                entity_id: tank,
                depot_id: DEPOT,
            },
            Some(rules),
        )
    }

    /// One frame in production order: every unit's own object-AI visit (the
    /// Enter dispatch with its probe and draw), then the dock pass, then
    /// movement.
    fn tick(sim: &mut Simulation, rules: &RuleSet) {
        sim.session.binary_frame = sim.session.binary_frame.wrapping_add(1);
        visit_units(sim, rules);
        tick_building_docks(sim, rules);
        process_units(sim, rules);
        pending_entries(sim, rules);
        sim.session.tick += 1;
    }

    /// Every pending unit's `FootClass::AI` entry retry (`0x004DAEDC`), in
    /// live-object order, after its Process.
    fn pending_entries(sim: &mut Simulation, rules: &RuleSet) {
        for id in sim.substrate.entities.keys_sorted() {
            try_pending_entry(sim, rules, id);
        }
    }

    /// Every unit's ground locomotor Process, in live-object order.
    fn process_units(sim: &mut Simulation, rules: &RuleSet) {
        for id in sim.substrate.entities.keys_sorted() {
            if sim
                .substrate
                .entities
                .get(id)
                .is_some_and(|e| e.category == EntityCategory::Unit)
            {
                sim.process_ground_locomotor_one(id, Some(rules), None)
                    .expect("unit Process");
            }
        }
    }

    /// The object-AI pass restricted to units, in live-object (stable id)
    /// order — the slot native `Mission_Enter` dispatches from.
    fn visit_units(sim: &mut Simulation, rules: &RuleSet) {
        let units: Vec<u64> = sim
            .substrate
            .entities
            .keys_sorted()
            .into_iter()
            .filter(|&id| {
                sim.substrate
                    .entities
                    .get(id)
                    .is_some_and(|e| e.category == EntityCategory::Unit)
            })
            .collect();
        for id in units {
            sim.object_ai_visit_one(id, Some(rules), crate::sim::world::ObjectAiCtx::default());
        }
    }

    fn linked(sim: &Simulation, tank: u64) -> bool {
        sim.substrate
            .entities
            .get(tank)
            .is_some_and(|e| e.radio_contacts.contains(DEPOT))
            && sim
                .substrate
                .entities
                .get(DEPOT)
                .is_some_and(|d| d.radio_contacts.contains(tank))
    }

    fn phase(sim: &Simulation, tank: u64) -> Option<DockPhase> {
        sim.substrate
            .entities
            .get(tank)
            .and_then(|e| e.dock_state.as_ref().map(|d| d.phase))
    }

    fn pos(sim: &Simulation, tank: u64) -> (u16, u16) {
        let e = sim.substrate.entities.get(tank).unwrap();
        (e.position.rx, e.position.ry)
    }

    /// Production path: the RepairAtDepot command queues Enter and runs the
    /// Unit setter with the depot. The first order finds the depot free: its
    /// DOCKING links it (`0x00741DD6`, `0x0043C8A4..C8C3`) and it drives for
    /// the pad. The later two find a contact held (`0x00741D22`): each takes
    /// the depot as its pending entry with no destination and no archive
    /// (`0x00741D9F`, `0x00742C52`). No stored queue.
    #[test]
    fn depot_order_links_a_free_depot_and_pends_the_rest() {
        let (mut sim, rules) = setup(3);
        for tank in 1..=3 {
            assert!(order_repair(&mut sim, &rules, tank));
            let e = sim.substrate.entities.get(tank).unwrap();
            assert_eq!(
                e.mission.queued().known(),
                Some(MissionType::Enter),
                "player depot order queues Enter (7) through the exact authority"
            );
            assert_eq!(e.derived_mission().0, MissionType::Enter);
        }
        let pad = depot_dock_cell(sim.substrate.entities.get(DEPOT).unwrap());
        assert!(linked(&sim, 1));
        assert_eq!(phase(&sim, 1), Some(DockPhase::EnterDock));
        assert_eq!(
            sim.substrate.entities.get(1).unwrap().navigation.nav_com,
            Some(NavTargetRef::cell(pad.0, pad.1))
        );
        for tank in 2..=3 {
            let e = sim.substrate.entities.get(tank).unwrap();
            assert!(!linked(&sim, tank));
            assert_eq!(phase(&sim, tank), Some(DockPhase::WaitForDock));
            assert_eq!(e.navigation.nav_com, None);
            assert_eq!(e.archive_target(), None);
        }
        let depot = sim.substrate.entities.get(DEPOT).unwrap();
        assert_eq!(
            depot.radio_contacts.capacity(),
            1,
            "NumberOfDocks default 1"
        );
        assert_eq!(depot.radio_contacts.len(), 1);
        tick(&mut sim, &rules);
        for tank in 2..=3 {
            let ds = sim
                .substrate
                .entities
                .get(tank)
                .unwrap()
                .dock_state
                .clone()
                .unwrap();
            assert_eq!(ds.phase, DockPhase::WaitForDock, "the slot stays held");
            assert!(ds.enter_retry.is_armed());
            assert!((14..=16).contains(&ds.enter_retry.duration));
        }
    }

    /// Each Enter dispatch draws exactly one `RandomRanged(0,2)` on the
    /// Scenario stream (the Mission_Enter epilogue), none between dispatches.
    #[test]
    fn waiter_probe_draws_one_scenario_random_per_dispatch() {
        let (mut sim, rules) = setup(2);
        for tank in 1..=2 {
            assert!(order_repair(&mut sim, &rules, tank));
        }
        // Frame 1: the contact probes and the pending unit dispatches (two
        // draws).
        let mut shadow = sim.clone_scenario_rng();
        tick(&mut sim, &rules);
        shadow.next_range_u32_inclusive(0, 2);
        shadow.next_range_u32_inclusive(0, 2);
        assert_eq!(
            sim.scenario_rng.next_range_u32_inclusive(0, 1000),
            shadow.next_range_u32_inclusive(0, 1000),
            "two dispatches ⇒ two RandomRanged(0,2) draws"
        );
    }

    /// Between two of the contact's probes nothing draws.
    #[test]
    fn contact_probe_draws_nothing_between_dispatches() {
        let (mut sim, rules) = setup(1);
        assert!(order_repair(&mut sim, &rules, 1));
        tick(&mut sim, &rules);
        let mut shadow = sim.clone_scenario_rng();
        let due = {
            let ds = sim
                .substrate
                .entities
                .get(1)
                .unwrap()
                .dock_state
                .clone()
                .unwrap();
            ds.enter_retry.start_frame + ds.enter_retry.duration
        };
        for _ in 0..64 {
            if sim.session.binary_frame + 1 >= due {
                break;
            }
            tick(&mut sim, &rules);
        }
        assert_eq!(
            sim.scenario_rng.next_range_u32_inclusive(0, 1000),
            shadow.next_range_u32_inclusive(0, 1000),
            "no draw between dispatches"
        );
    }

    /// The `Mission_Enter` epilogue draw sits in the unit's OWN object-AI
    /// slot, interleaved in live-object order with other objects' dispatch
    /// draws: visiting the contact 1 draws exactly one `RandomRanged(0,2)`,
    /// visiting the Guard tank 2 between them draws its own Guard cadence
    /// jitter, visiting the pending unit 3 draws one more `(0,2)`, and the
    /// post-production dock pass draws nothing at all.
    #[test]
    fn waiter_probe_draw_sits_in_the_units_own_ai_slot_in_object_order() {
        let (mut sim, rules) = setup(3);
        assert!(order_repair(&mut sim, &rules, 1));
        assert!(order_repair(&mut sim, &rules, 3));
        // Tank 2 idles on Guard with a due dispatch timer.
        let now = sim.session.binary_frame;
        sim.mission_assign_exact(2, MissionId::from_known(MissionType::Guard), now)
            .expect("assign Guard");
        sim.session.binary_frame += 1;
        let ctx = crate::sim::world::ObjectAiCtx::default;

        let mut shadow = sim.clone_scenario_rng();
        sim.object_ai_visit_one(1, Some(&rules), ctx());
        shadow.next_range_u32_inclusive(0, 2);
        assert_eq!(
            sim.scenario_rng.state(),
            shadow.state(),
            "contact 1: exactly one (0,2) draw inside its own AI slot"
        );

        let before_guard = sim.scenario_rng.state();
        sim.object_ai_visit_one(2, Some(&rules), ctx());
        assert_ne!(
            sim.scenario_rng.state(),
            before_guard,
            "the Guard tank's dispatch draws between the two"
        );

        let mut shadow = sim.clone_scenario_rng();
        sim.object_ai_visit_one(3, Some(&rules), ctx());
        shadow.next_range_u32_inclusive(0, 2);
        assert_eq!(
            sim.scenario_rng.state(),
            shadow.state(),
            "pending 3: one (0,2) draw after the Guard tank's"
        );

        let after_ai = sim.scenario_rng.state();
        tick_building_docks(&mut sim, &rules);
        assert_eq!(
            sim.scenario_rng.state(),
            after_ai,
            "the post-production dock pass draws nothing"
        );
        assert!(linked(&sim, 1), "the first order holds the slot");
        assert!(!linked(&sim, 3));
    }

    /// The freed slot goes to the first pending unit whose `FootClass::AI`
    /// runs after the release (`0x0070D7E0`, live-object order), not to the
    /// first orderer: unit 3 is ordered before unit 2, and unit 2 docks next.
    /// Its HELLO links it, CAN_LOAD answers ROGER (it does not stand on the
    /// depot, IsOccupied 0x23), and it commences Enter with the pad as NavCom.
    #[test]
    fn freed_slot_goes_to_the_first_pending_unit_in_object_order() {
        let (mut sim, rules) = setup(3);
        for tank in [1, 3, 2] {
            assert!(order_repair(&mut sim, &rules, tank));
        }
        let mut released = false;
        for _ in 0..2000 {
            tick(&mut sim, &rules);
            if phase(&sim, 1).is_none() {
                released = true;
                break;
            }
        }
        assert!(released, "unit 1 releases");
        assert!(!linked(&sim, 1));
        assert_eq!(sim.substrate.entities.get(1).unwrap().health.current, 300);
        assert!(linked(&sim, 2), "the first pending unit in object order");
        assert!(!linked(&sim, 3));
        assert_eq!(phase(&sim, 2), Some(DockPhase::EnterDock));
        assert_eq!(phase(&sim, 3), Some(DockPhase::WaitForDock));
        let pad = depot_dock_cell(sim.substrate.entities.get(DEPOT).unwrap());
        let e = sim.substrate.entities.get(2).unwrap();
        // Queue_Mission(Enter, commence) promotes at once: Enter is current.
        assert_eq!(e.derived_mission().0, MissionType::Enter);
        assert_eq!(e.navigation.nav_com, Some(NavTargetRef::cell(pad.0, pad.1)));
    }

    /// A linked unit that has stopped short of the pad is sent on by the dock
    /// pass through its class setter (Unit `0x00741970`): a native NavCom on
    /// the pad and a Find_Path route from its next Process, no adapter.
    /// (Native keeps driving on the player order's NavCom; the re-issue is
    /// the VERA stand-in the dock pass documents.)
    #[test]
    fn a_stopped_linked_unit_is_sent_onto_the_pad_by_its_class_setter() {
        let (mut sim, rules) = setup(1);
        assert!(order_repair(&mut sim, &rules, 1));
        tick(&mut sim, &rules);
        assert!(linked(&sim, 1));
        sim.assign_null_destination(1, Some(&rules));
        let e = sim.substrate.entities.get_mut(1).unwrap();
        e.movement_target = None;
        assert_eq!(e.navigation.nav_com, None, "the unit stopped");
        tick(&mut sim, &rules);
        let pad = depot_dock_cell(sim.substrate.entities.get(DEPOT).unwrap());
        let e = sim.substrate.entities.get(1).unwrap();
        assert_eq!(phase(&sim, 1), Some(DockPhase::EnterDock));
        assert_eq!(e.navigation.nav_com, Some(NavTargetRef::cell(pad.0, pad.1)));
        assert!(!e.movement_target.as_ref().is_some_and(|t| t.adapter_route));
        for _ in 0..400 {
            tick(&mut sim, &rules);
            if phase(&sim, 1) == Some(DockPhase::Servicing) {
                break;
            }
        }
        assert_eq!(phase(&sim, 1), Some(DockPhase::Servicing));
        assert_eq!(pos(&sim, 1), pad);
    }

    /// Release drives the repaired unit off the pad to the foundation exit
    /// list's first enterable cell — (0, 3) below the NW corner for a 3x3 —
    /// with BREAK on both ends and a queued Move mission.
    #[test]
    fn repaired_unit_is_released_to_the_native_exit_cell_and_pad_is_vacated() {
        let (mut sim, rules) = setup(1);
        assert!(order_repair(&mut sim, &rules, 1));
        let pad = depot_dock_cell(sim.substrate.entities.get(DEPOT).unwrap());
        let mut reached_pad = false;
        let mut released = false;
        for _ in 0..2000 {
            tick(&mut sim, &rules);
            if phase(&sim, 1) == Some(DockPhase::Servicing) {
                reached_pad = true;
                assert_eq!(pos(&sim, 1), pad);
            }
            if reached_pad && phase(&sim, 1).is_none() {
                released = true;
                break;
            }
        }
        assert!(reached_pad && released);
        let e = sim.substrate.entities.get(1).unwrap();
        assert_eq!(e.health.current, 300);
        assert!(!linked(&sim, 1));
        assert_eq!(e.mission.queued().known(), Some(MissionType::Move));
        // MissionRepairAndProduce 0x0044C496: the class setter's NavCom; the
        // route is the next Process's Find_Path.
        let exit = (DEPOT_RX, DEPOT_RY + 3);
        assert_eq!(
            e.navigation.nav_com,
            Some(NavTargetRef::cell(exit.0, exit.1))
        );
        assert!(!e.movement_target.as_ref().is_some_and(|t| t.adapter_route));
        for _ in 0..200 {
            tick(&mut sim, &rules);
        }
        assert_eq!(
            pos(&sim, 1),
            exit,
            "pad vacated, unit parked on the exit cell"
        );
        assert!(
            sim.substrate
                .entities
                .get(DEPOT)
                .unwrap()
                .radio_contacts
                .is_empty()
        );
    }

    /// A depot whose online latch a Temporal warp cleared answers the probe
    /// with 10 before any other test (`0x0043C7FB`): a damaged linked waiter
    /// BREAKs and goes idle on its next probe.
    #[test]
    fn a_warped_depot_turns_its_waiters_away() {
        let (mut sim, rules) = setup(1);
        assert!(order_repair(&mut sim, &rules, 1));
        tick(&mut sim, &rules);
        assert!(linked(&sim, 1));
        sim.substrate.entities.get_mut(DEPOT).unwrap().temporal =
            crate::sim::temporal::TemporalState::warped_by_for_test(999);
        let due = {
            let ds = sim
                .substrate
                .entities
                .get(1)
                .unwrap()
                .dock_state
                .clone()
                .unwrap();
            ds.enter_retry.start_frame + ds.enter_retry.duration
        };
        for _ in 0..64 {
            if sim.session.binary_frame >= due {
                break;
            }
            tick(&mut sim, &rules);
        }
        assert!(!linked(&sim, 1));
        assert!(phase(&sim, 1).is_none());
        assert_eq!(
            sim.substrate
                .entities
                .get(1)
                .unwrap()
                .mission
                .queued()
                .known(),
            Some(MissionType::Guard)
        );
    }

    /// A linked unit that is already at full health answers 0x22 with 10, so
    /// its own probe returns 10: BREAK + Enter_Idle_Mode (Guard), no scatter.
    #[test]
    fn linked_full_health_waiter_breaks_and_goes_idle_on_its_next_probe() {
        let (mut sim, rules) = setup(2);
        for tank in 1..=2 {
            assert!(order_repair(&mut sim, &rules, tank));
        }
        tick(&mut sim, &rules);
        assert!(linked(&sim, 1));
        sim.substrate.entities.get_mut(1).unwrap().health.current = 300;
        let due = {
            let ds = sim
                .substrate
                .entities
                .get(1)
                .unwrap()
                .dock_state
                .clone()
                .unwrap();
            ds.enter_retry.start_frame + ds.enter_retry.duration
        };
        for _ in 0..64 {
            if sim.session.binary_frame >= due {
                break;
            }
            tick(&mut sim, &rules);
        }
        assert!(!linked(&sim, 1));
        assert!(phase(&sim, 1).is_none());
        assert_eq!(
            sim.substrate
                .entities
                .get(1)
                .unwrap()
                .mission
                .queued()
                .known(),
            Some(MissionType::Guard)
        );
        // The slot is free for the next waiter's probe.
        let due2 = {
            let ds = sim
                .substrate
                .entities
                .get(2)
                .unwrap()
                .dock_state
                .clone()
                .unwrap();
            ds.enter_retry.start_frame + ds.enter_retry.duration
        };
        for _ in 0..64 {
            if sim.session.binary_frame >= due2 {
                break;
            }
            tick(&mut sim, &rules);
        }
        assert!(linked(&sim, 2));
    }

    // --- A harvester at the depot ---
    //
    // Native dispatches a miner on Enter(7) through `FootClass::Mission_Enter
    // @ 0x004D9290` like any Foot object (`UnitClass` vtable `0x007F5C70 +
    // 0x240` = `0x007F5EB0` holds `0x004D9290`; the Harvest handler is only
    // reached on selector 10). The Unit Enter arm therefore runs for a miner
    // with a depot `DockState`, and the Harvest handler declines it.

    fn miner_cfg() -> crate::sim::miner::MinerConfig {
        crate::sim::miner::MinerConfig::default()
    }

    fn spawn_damaged_miner(sim: &mut Simulation, sid: u64, rx: u16, ry: u16) {
        spawn_entity(sim, sid, "HARV", EntityCategory::Unit, rx, ry, 100);
        let e = sim.substrate.entities.get_mut(sid).unwrap();
        e.miner = Some(crate::sim::miner::Miner::new(
            crate::sim::miner::MinerKind::War,
            &miner_cfg(),
            0,
        ));
    }

    /// The object-AI pass with the Harvest handler LIVE (`miner_config`
    /// supplied, as production does), in live-object order.
    fn visit_units_with_harvest(sim: &mut Simulation, rules: &RuleSet) {
        let units: Vec<u64> = sim
            .substrate
            .entities
            .keys_sorted()
            .into_iter()
            .filter(|&id| {
                sim.substrate
                    .entities
                    .get(id)
                    .is_some_and(|e| e.category == EntityCategory::Unit)
            })
            .collect();
        let cfg = miner_cfg();
        for id in units {
            sim.object_ai_visit_one(
                id,
                Some(rules),
                crate::sim::world::ObjectAiCtx {
                    overlay_registry: None,
                    terrain_spawner_cells: None,
                    miner_config: Some(&cfg),
                },
            );
        }
    }

    fn dispatch_timer(sim: &Simulation, id: u64) -> (i32, i32) {
        let t = sim
            .substrate
            .entities
            .get(id)
            .unwrap()
            .mission
            .dispatch_timer();
        (t.start_frame(), t.delay())
    }

    fn harvest_cursor(sim: &Simulation, id: u64) -> u32 {
        sim.substrate
            .entities
            .get(id)
            .unwrap()
            .mission
            .handler_state()
    }

    /// A damaged war miner ordered to the depot HELLOs on its Enter cadence
    /// (one Scenario `(0,2)` draw per dispatch, none between), links, is
    /// serviced to full and released with the Harvest cursor untouched. The
    /// dispatch timer has one writer: it moves only on frames where it was
    /// due at entry, and every write is the Enter epilogue (14..=16), never
    /// the Harvest handler's per-frame `1`.
    #[test]
    fn damaged_miner_at_depot_probes_links_is_serviced_and_released() {
        let (mut sim, rules) = setup(0);
        const MINER: u64 = 7;
        spawn_damaged_miner(&mut sim, MINER, 14, 11);
        assert!(order_repair(&mut sim, &rules, MINER));
        let cursor_at_order = harvest_cursor(&sim, MINER);

        let mut reached_pad = false;
        let mut released = false;
        let mut writes = 0u32;
        let mut prev_timer = dispatch_timer(&sim, MINER);
        for _ in 0..2000 {
            sim.session.binary_frame = sim.session.binary_frame.wrapping_add(1);
            let now = sim.session.binary_frame;
            let due_at_entry = sim
                .substrate
                .entities
                .get(MINER)
                .unwrap()
                .mission
                .dispatch_timer()
                .due(now);
            let waiting = matches!(
                phase(&sim, MINER),
                Some(DockPhase::WaitForDock | DockPhase::EnterDock)
            );
            let mut shadow = sim.clone_scenario_rng();
            visit_units_with_harvest(&mut sim, &rules);
            let timer = dispatch_timer(&sim, MINER);
            if waiting {
                if due_at_entry {
                    // Exactly one Enter dispatch: one (0,2) draw, one write.
                    shadow.next_range_u32_inclusive(0, 2);
                    assert_eq!(sim.scenario_rng.state(), shadow.state());
                    assert_eq!(timer.0, now as i32, "epilogue anchors at now");
                    assert!((14..=16).contains(&timer.1), "Enter cadence, got {timer:?}");
                    writes += 1;
                } else {
                    assert_eq!(sim.scenario_rng.state(), shadow.state(), "no draw");
                    assert_eq!(timer, prev_timer, "no writer on a pending frame");
                }
                assert_eq!(harvest_cursor(&sim, MINER), cursor_at_order);
            }
            prev_timer = timer;
            tick_building_docks(&mut sim, &rules);
            process_units(&mut sim, &rules);
            sim.session.tick += 1;
            if phase(&sim, MINER) == Some(DockPhase::Servicing) {
                reached_pad = true;
                assert!(linked(&sim, MINER));
            }
            if reached_pad && phase(&sim, MINER).is_none() {
                released = true;
                break;
            }
        }
        assert!(reached_pad, "miner never reached the pad");
        assert!(released, "miner never released");
        assert!(writes >= 1, "at least the first HELLO dispatch");
        let e = sim.substrate.entities.get(MINER).unwrap();
        assert_eq!(e.health.current, 600);
        assert!(e.miner.is_some(), "miner component survives the depot stay");
        assert!(!linked(&sim, MINER));
        assert_eq!(e.mission.queued().known(), Some(MissionType::Move));
        assert_eq!(harvest_cursor(&sim, MINER), cursor_at_order);
    }

    /// Single writer, directly: on a due frame the Harvest handler declines a
    /// miner on Enter with a depot dock state (timer and cursor untouched),
    /// and the object-AI visit then writes the timer exactly once with the
    /// Enter epilogue.
    #[test]
    fn harvest_handler_declines_miner_on_enter_with_depot_dock_state() {
        let (mut sim, rules) = setup(0);
        const MINER: u64 = 7;
        spawn_damaged_miner(&mut sim, MINER, 14, 11);
        assert!(order_repair(&mut sim, &rules, MINER));
        sim.session.binary_frame += 1;
        visit_units_with_harvest(&mut sim, &rules);
        assert!(linked(&sim, MINER));
        assert_eq!(
            sim.substrate
                .entities
                .get(MINER)
                .unwrap()
                .mission
                .current()
                .known(),
            Some(MissionType::Enter)
        );
        let (start, delay) = dispatch_timer(&sim, MINER);
        assert!((14..=16).contains(&delay));
        // Jump to the due frame.
        sim.session.binary_frame = (start + delay) as u32;
        let now = sim.session.binary_frame;
        let before = dispatch_timer(&sim, MINER);
        let cursor = harvest_cursor(&sim, MINER);
        let rng_before = sim.scenario_rng.state();
        let cfg = miner_cfg();
        crate::sim::miner::dispatch_harvest_for_object(&mut sim, &rules, &cfg, None, MINER);
        assert_eq!(
            dispatch_timer(&sim, MINER),
            before,
            "Harvest handler declined"
        );
        assert_eq!(harvest_cursor(&sim, MINER), cursor);
        assert_eq!(sim.scenario_rng.state(), rng_before);

        let mut shadow = sim.clone_scenario_rng();
        visit_units_with_harvest(&mut sim, &rules);
        shadow.next_range_u32_inclusive(0, 2);
        assert_eq!(sim.scenario_rng.state(), shadow.state(), "one (0,2) draw");
        let after = dispatch_timer(&sim, MINER);
        assert_eq!(after.0, now as i32);
        assert!(
            (14..=16).contains(&after.1),
            "Enter epilogue, got {after:?}"
        );
    }

    /// Exit-cell selection skips foundation/vehicle-blocked cells in list order.
    #[test]
    fn exit_cell_skips_blocked_cells_in_list_order() {
        let (mut sim, _rules) = setup(0);
        assert_eq!(
            depot_exit_cell(&sim, DEPOT_RX, DEPOT_RY, "3x3"),
            Some((DEPOT_RX, DEPOT_RY + 3))
        );
        spawn_tank(&mut sim, 7, DEPOT_RX, DEPOT_RY + 3);
        sim.substrate.occupancy.add(
            DEPOT_RX,
            DEPOT_RY + 3,
            7,
            MovementLayer::Ground,
            None,
            CellListInsertion::AppendBuilding,
        );
        assert_eq!(
            depot_exit_cell(&sim, DEPOT_RX, DEPOT_RY, "3x3"),
            Some((DEPOT_RX + 1, DEPOT_RY + 3))
        );
    }

    /// A repaired (full-HP) linked waiter that is a Harvester=yes unit takes
    /// the harvester arm of `Enter_Idle_Mode @ 0x00738970` after the BREAK
    /// (`FootClass::Mission_Enter` 0x004D92E2, args (0, 1)): Harvest for a
    /// non-human house regardless of land, Guard for a human house standing
    /// off ore. Plain units keep the Guard exit.
    #[test]
    fn linked_full_health_miner_waiter_takes_the_harvester_idle_arm() {
        use crate::sim::miner::{Miner, MinerConfig, MinerKind};
        use crate::sim::mission::MissionId;

        fn build(human: bool) -> (Simulation, RuleSet) {
            let (mut sim, rules) = setup(0);
            if human {
                let owner_id = sim.interner.intern("Americans");
                sim.houses.get_mut(&owner_id).unwrap().is_human = true;
            }
            spawn_entity(&mut sim, 1, "MTNK", EntityCategory::Unit, 14, 11, 300);
            {
                let unit = sim.substrate.entities.get_mut(1).unwrap();
                unit.miner = Some(Miner::new(MinerKind::War, &MinerConfig::default(), 0));
                unit.dock_state = Some(DockState::new(DEPOT, DockPhase::EnterDock));
                unit.mark_live_contact_with(DEPOT);
            }
            sim.substrate
                .entities
                .get_mut(DEPOT)
                .unwrap()
                .mark_live_contact_with(1);
            sim.mission_assign_exact(1, MissionId::from_known(MissionType::Enter), 0)
                .expect("unit exists");
            assert!(linked(&sim, 1));
            (sim, rules)
        }

        let (mut sim, rules) = build(false);
        mission_enter_dispatch(&mut sim, &rules, 1);
        assert!(!linked(&sim, 1));
        assert!(phase(&sim, 1).is_none());
        assert_eq!(
            sim.substrate.entities.get(1).unwrap().mission.queued(),
            MissionId::from_known(MissionType::Harvest),
            "a non-human house's miner always re-queues Harvest"
        );

        let (mut sim, rules) = build(true);
        mission_enter_dispatch(&mut sim, &rules, 1);
        assert!(!linked(&sim, 1));
        assert_eq!(
            sim.substrate.entities.get(1).unwrap().mission.queued(),
            MissionId::from_known(MissionType::Guard),
            "a human house's miner standing off ore parks on Guard"
        );
    }
}
