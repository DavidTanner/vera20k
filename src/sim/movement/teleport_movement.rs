//! Teleport (chrono) locomotor — instant relocation with chrono delay.
//!
//! Implements the Teleport state machine for chrono-style movement:
//! Relocate (instant, one frame) → ChronoDelay (being_warped countdown) → Idle.
//!
//! Self-teleport relocates the unit in a single frame (Phase 0), then the unit
//! sits at the destination 50% translucent for `chrono_delay` frames until fully
//! materialized.
//!
//! Units with `Locomotor=Teleport` use this. A `Teleporter=` unit (the Chrono
//! Miner, whose primary locomotor is this one) drives instead wherever the
//! Unit setter's Teleporter arm (`0x007423CD`, `movement/track_path.rs`)
//! installs a Drive piggyback: every destination except a cell MOVE_HERE
//! while radio slot 0 is a `DockUnload=` building.
//!
//! No pathfinding — the unit is relocated instantly. The object turn runs the
//! rest of the warp around the relocation in native order (detach sweep,
//! departure WarpOut, parasite eject, Mark(UP), then after it Mark(DOWN),
//! `Per_Cell_Process(2)`, the NULL assign and the arrival WarpOut).
//!
//! ## Dependency rules
//! - Part of sim/ — depends on sim/game_entity, sim/entity_store, sim/locomotor.
//! - sim/ NEVER depends on render/, ui/, sidebar/, audio/, net/.

use crate::rules::locomotor_type::LocomotorKind;
use crate::rules::ruleset::GeneralRules;
use crate::sim::components::AnimClassSpawnDescriptor;
use crate::sim::debug_event_log::DebugEventKind;
use crate::sim::entity_store::EntityStore;
use crate::sim::intern::InternedId;
use crate::util::fixed_math::isqrt_i64;
use crate::util::lepton::CELL_CENTER_LEPTON;

const TELEPORT_WARP_DRAW_FLAGS: u32 = 0x600;
const TELEPORT_WARP_DELAY: u16 = 0;
const TELEPORT_WARP_LOOP_COUNT: i32 = 1;
const TELEPORT_WARP_Z_ADJUST: i32 = 0;
const TELEPORT_WARP_REVERSE: bool = false;

/// The `[General] WarpOut=` `AnimClass` constructor row Teleport Process
/// builds at the owner's Location: the departure (`0x00719442`) and the
/// arrival (`0x00719791`). The row constants are read from the native
/// constructor sites. The coordinate is not: native passes the owner's exact
/// `+0x9C` coordinate, VERA the cell centre and height level of its cell.
pub(crate) fn warp_out_anim(
    warp_out_type: InternedId,
    rx: u16,
    ry: u16,
    z: u8,
) -> AnimClassSpawnDescriptor {
    let mut anim_spawn = AnimClassSpawnDescriptor::new(
        warp_out_type,
        rx,
        ry,
        CELL_CENTER_LEPTON,
        CELL_CENTER_LEPTON,
        z,
    );
    anim_spawn.delay = TELEPORT_WARP_DELAY;
    anim_spawn.loop_count = TELEPORT_WARP_LOOP_COUNT;
    anim_spawn.draw_flags = TELEPORT_WARP_DRAW_FLAGS;
    anim_spawn.z_adjust = TELEPORT_WARP_Z_ADJUST;
    anim_spawn.reverse = TELEPORT_WARP_REVERSE;
    anim_spawn
}

/// Phase within the teleport state machine.
///
/// Phase 0 relocates instantly in one frame, then the chrono delay timer
/// counts down while the unit is semi-transparent at the destination.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum TeleportPhase {
    /// Instant relocation between the owner's Mark(UP) and Mark(DOWN). Executes in
    /// one frame, then transitions to ChronoDelay.
    Relocate,
    /// Post-warp chrono delay: unit sits at destination 50% translucent,
    /// `being_warped_ticks` counts down each frame. When it reaches 0 the
    /// teleport is complete and the base locomotor is restored.
    ChronoDelay,
}

/// Per-frame result returned by the special locomotor Process adapters.
///
/// The native Process vtable slot owns the completion return; keeping that
/// result explicit prevents callers from inferring completion from an absent
/// movement target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpecialMovementOutcome {
    Continue,
    Complete,
    Abort,
}

/// State for an in-progress teleport.
///
/// Set by `teleport_move_to()` and cleared when the chrono delay
/// expires. The render system reads `being_warped_ticks` to apply 50%
/// translucency while the unit materializes.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TeleportState {
    /// Current phase in the teleport sequence.
    pub phase: TeleportPhase,
    /// Destination cell coordinates.
    pub target_rx: u16,
    pub target_ry: u16,
    /// Chrono delay countdown in native gameplay frames. While > 0 the unit is "being warped"
    /// and the renderer draws it at 50% alpha. Set from the distance-based formula
    /// in the original engine: `delay = distance_leptons / ChronoDistanceFactor`,
    /// clamped to `ChronoMinimumDelay`.
    pub being_warped_ticks: u32,
}

impl TeleportState {
    /// `TeleportLocomotionClass::Is_Moving` (`0x00718080`), which its
    /// `Is_Moving_Now` reaches through the base thunk (`0x004B6610`): the
    /// +0x30 request byte. Move_To sets it (`0x007181DB`); Stop_Moving
    /// (`0x00718254`) and Process (`0x00719BD2`) clear it.
    ///
    /// RESIDUAL: VERA keeps no request byte and answers with the Relocate
    /// phase. Those writes do not establish a Relocate-only lifetime, so this
    /// is a legacy adapter, not parity, until Teleport's request lifecycle is
    /// ported.
    pub(crate) fn is_moving(&self) -> bool {
        self.phase == TeleportPhase::Relocate
    }

    /// YR TeleportLocomotionClass::Process @ 0x007192f0 exposes separate
    /// warp-out and warp-in producer bytes. Relocation is the departure
    /// producer; the post-relocation delay is the arrival producer.
    pub fn warp_out_active(&self) -> bool {
        self.phase == TeleportPhase::Relocate
    }

    pub fn warp_in_active(&self) -> bool {
        self.phase == TeleportPhase::ChronoDelay && self.being_warped_ticks > 0
    }

    /// The relocation frame is removed from normal targeting before its cell
    /// and occupancy mutation; it becomes targetable again while materializing.
    #[cfg(test)]
    pub fn is_targetable(&self) -> bool {
        self.phase == TeleportPhase::ChronoDelay
    }
}

/// Compute the chrono warp delay in native gameplay frames from distance.
///
/// When `ChronoTrigger=yes`, delay scales linearly with distance in leptons,
/// divided by `ChronoDistanceFactor` (default 48), clamped to at least
/// `ChronoMinimumDelay` (default 16). Short distances below `ChronoRangeMinimum`
/// are forced to the minimum.
pub fn compute_chrono_delay(rules: &GeneralRules, distance_leptons: i32) -> u32 {
    if !rules.chrono_trigger {
        return rules.chrono_minimum_delay.max(0) as u32;
    }
    let mut delay = if rules.chrono_distance_factor > 0 {
        distance_leptons / rules.chrono_distance_factor
    } else {
        0
    };
    if delay < rules.chrono_minimum_delay {
        delay = rules.chrono_minimum_delay;
    }
    if distance_leptons < rules.chrono_range_minimum {
        delay = rules.chrono_minimum_delay;
    }
    delay.max(0) as u32
}

/// `TeleportLocomotionClass::Move_To @ 0x00718100`, the one Teleport move
/// entry: the Unit and Infantry setters' Foot tail reaches it. A timer-locked
/// owner (`vt+0x380`, the Foot+0x6A0 paralysis
/// timer), one warped out (`vt+0x1D4`, Techno+0x270: a Temporal warp) or one
/// warping in (`vt+0x1D8`, +0x271: a teleporter's post-warp delay) refuses
/// with a raw NavCom clear (`0x0071820F`). Otherwise the destination cell's
/// centre is armed (`0x007181DB`, the +0x30 request) and the same turn's
/// Process warps there. A request already armed is not warped out: the
/// ordinary warp never sets +0x270, so Mission_Enter's re-assign re-arms it.
/// Evidence: tools/spatial_oracle/cmin_dock.json `teleport_move_to` rows.
///
/// RESIDUALS: the EMP/death-frame guard (`vt+0x37C`, unrepresented, as for
/// the Drive Move_To) and the Chronosphere's +0x270 (not represented); the
/// destination resolution's (`0x00718B70`) Unit Can_Enter_Cell refusal
/// (`vt+0x1AC`, `0x0071911D`) and its Find_Nearby_Passable_Cell replacement
/// (`0x00719185`), reached for any cell the Teleporter arm admitted (no Unit
/// in its list) that Can_Enter_Cell still refuses: infantry on the pad, or an
/// occupy bit another vehicle holds (the only case rows `pad_cannot_enter*`
/// cover); and that resolution's reservation bit (Unit `vt+0xF0`/`+0xF4`),
/// which the next resolution clears at the previous destination whoever
/// stands there. The Infantry-only arm (`0x0071816F..0x0071819F`: with the
/// Techno+0x1F8 override up, the destination cell's occupants are scattered)
/// is not ported; TechnoClass::Unlimbo raises +0x1F8 only around its own
/// setter call (`0x006F6E1B`/`0x006F6E34`), so no order reaches it.
///
/// The resolution's Infantry arm (`0x00718C86..0x0071908D`) is not ported
/// either: the spot in the cell (`0x00481180` at `0x00718E25`), the
/// infantryman's Can_Enter_Cell(cell, -1, -1) (`0x00718E8B`), whose refusal
/// nulls the destination (`0x00718E95..0x00718EAD`) so Move_To takes the NULL
/// setter (`0x007181F9`), and, for an object NavCom, the
/// Find_Nearby_Passable_Cell replacement (`0x0071900D`). VERA arms the cell
/// centre. Trigger: a Chrono Legionnaire, Commando or Ivan ordered onto an
/// occupied or full cell: several rallying from one barracks, or a pursuit,
/// whose VERA stand-in names the target's own cell (native's approach
/// `0x004D5690` picks one in range). Effect: it warps to the centre of that
/// cell, onto its target or other infantry, where native refuses the cell or
/// spreads the spots. Frequency: every such rally, and every attack order
/// beyond range. Risk: shared cells and sub-cells until the next order.
pub(crate) fn teleport_move_to(
    entity: &mut crate::sim::game_entity::GameEntity,
    target: (u16, u16),
    rules: &GeneralRules,
    is_harvester: bool,
    binary_frame: u32,
) -> bool {
    if entity.is_paralyzed(binary_frame) || entity.temporal.is_warped() || entity.is_warping_in() {
        entity.navigation.nav_com = None;
        return false;
    }
    arm_teleport(entity, target, rules, is_harvester)
}

/// `TeleportLocomotionClass::Stop_Moving @ 0x00718230` on the owner's active
/// Teleport: it nulls the armed destination (+0x18..+0x20) and clears the
/// +0x30/+0x32 request bytes. The post-warp delay is the owner's (+0x271),
/// so a warping-in owner keeps it.
pub(crate) fn teleport_stop_moving(entity: &mut crate::sim::game_entity::GameEntity) {
    if teleport_process_active(entity)
        && entity
            .teleport_state
            .as_ref()
            .is_some_and(|state| state.phase == TeleportPhase::Relocate)
    {
        entity.teleport_state = None;
    }
}

/// Whether the owner's Teleport Process runs this frame: Teleport is its
/// active locomotor. A Teleport stashed under a Drive piggyback (the Chrono
/// Miner's) runs nothing, so a warp it armed waits for End_Piggyback.
pub(crate) fn teleport_process_active(entity: &crate::sim::game_entity::GameEntity) -> bool {
    entity
        .locomotor
        .as_ref()
        .is_some_and(|locomotor| locomotor.active_kind() == LocomotorKind::Teleport)
}

impl crate::sim::world::Simulation {
    /// Foot Limbo's first Limbo calls ILocomotion `+0x9C(0)` (`0x004DB324`).
    /// Teleport's (`0x0071A090`) calls the owner's vtable `+0xF4` at
    /// Head_To_Coord, which is the owner's Location (`0x0055ACA0`). For an
    /// infantryman that is what clears his sub-cell: Infantry Mark clears none
    /// (`0x0047EAFE`).
    pub(crate) fn release_teleport_occupation_before_foot_limbo(&mut self, id: u64) {
        let Some(coord) = self.substrate.entities.get(id).and_then(|entity| {
            (!entity.lifecycle.in_limbo && teleport_process_active(entity))
                .then(|| super::ground_pose::position_world_coord(&entity.position))
        }) else {
            return;
        };
        self.object_raw_receiver_at(id, coord, false);
    }
}

/// Process `0x00719375..0x007193C1`: an owner whose exact coordinate is
/// already the armed destination takes `vt+0x480(NULL, 1)` and Stop_Moving
/// instead of the warp (`0x007197AF`): no animation, sound or PerCell.
pub(crate) fn warp_destination_reached(
    entity: &crate::sim::game_entity::GameEntity,
    terrain: Option<&crate::map::resolved_terrain::ResolvedTerrainGrid>,
) -> bool {
    teleport_process_active(entity)
        && entity
            .teleport_state
            .as_ref()
            .filter(|state| state.phase == TeleportPhase::Relocate)
            .is_some_and(|state| {
                super::ground_pose::position_world_coord(&entity.position)
                    == super::navcom::target_cell_coord(state.target_rx, state.target_ry, terrain)
            })
}

/// Arm the warp: the destination request (`0x007181DB`) and its chrono delay,
/// from the Euclidean lepton distance (`compute_chrono_delay`). A harvester
/// (the Chrono Miner) takes no delay, so its Relocate finishes in one frame.
fn arm_teleport(
    entity: &mut crate::sim::game_entity::GameEntity,
    target: (u16, u16),
    rules: &GeneralRules,
    is_harvester: bool,
) -> bool {
    // Compute distance in leptons (1 cell = 256 leptons) for chrono delay.
    let dx = (entity.position.rx as i32 - target.0 as i32) * 256;
    let dy = (entity.position.ry as i32 - target.1 as i32) * 256;
    let dist_sq = (dx as i64) * (dx as i64) + (dy as i64) * (dy as i64);
    let distance_leptons = isqrt_i64(dist_sq) as i32;
    let chrono_ticks = if is_harvester {
        0
    } else {
        compute_chrono_delay(rules, distance_leptons)
    };

    // Remove any existing ground movement.
    entity.movement_target = None;

    // Attach the teleport state machine — starts in Relocate (instant).
    let teleport_state = TeleportState {
        phase: TeleportPhase::Relocate,
        target_rx: target.0,
        target_ry: target.1,
        being_warped_ticks: chrono_ticks,
    };
    entity.teleport_state = Some(teleport_state);
    entity.push_debug_event(
        0,
        DebugEventKind::SpecialMovementStart {
            kind: "Teleport".into(),
        },
    );

    true
}

/// Drop attack locks that name `teleporting_id`, the warp's first step: the
/// analogue of its Techno detach sweep (`0x0070D4A0`, called at
/// `0x007193C7`). Radio and presentation links remain root-owned integration
/// work.
///
/// RESIDUAL: the bullet sweep after it (`0x007193CC..0x007193F4`) retargets
/// every live bullet aimed at the owner (+0x10C) to the cell the owner leaves
/// (`0x00468430`; NULL for an airborne owner). VERA's projectiles keep homing
/// on the owner. Trigger: a shot in flight at a Chrono Miner, Legionnaire,
/// Commando or Ivan the frame it warps. Effect: the shot follows it to the
/// destination instead of landing on the old cell.
pub(crate) fn release_incoming_target_locks(entities: &mut EntityStore, teleporting_id: u64) {
    for id in entities.keys_sorted() {
        if id == teleporting_id {
            continue;
        }
        let Some(entity) = entities.get_mut(id) else {
            continue;
        };
        if entity.attack_target.as_ref().is_some_and(|target| {
            matches!(target.target, crate::sim::combat::TargetKind::Entity(id) if id == teleporting_id)
        }) {
            entity.attack_target = None;
        }
    }
}

/// Teleport Process (`0x007192F0`)'s own state for one owner, which the
/// object turn admits with Teleport as its active locomotor: Relocate moves
/// the owner in one frame (`0x00719631..0x007196B2`); ChronoDelay then counts
/// `being_warped_ticks` down each frame until the teleport completes. `None`
/// when the owner has no warp armed or warping in.
pub fn process_teleport(
    entities: &mut EntityStore,
    id: u64,
    sim_tick: u64,
    terrain: Option<&crate::map::resolved_terrain::ResolvedTerrainGrid>,
) -> Option<SpecialMovementOutcome> {
    let reached = entities
        .get(id)
        .is_some_and(|entity| warp_destination_reached(entity, terrain));
    let entity = entities.get_mut(id)?;
    let teleport = entity.teleport_state.as_mut()?;
    let mut finished = false;
    let outcome;

    // Track phase before processing to detect transitions.
    let phase_before = teleport.phase;

    match teleport.phase {
        // 0x007197AF: Stop_Moving only; the caller runs the NULL assign.
        TeleportPhase::Relocate if reached => {
            finished = true;
            outcome = SpecialMovementOutcome::Abort;
        }
        TeleportPhase::Relocate => {
            // Instant relocation in one frame.
            let destination =
                crate::sim::components::DriveCoord::cell(teleport.target_rx, teleport.target_ry, 0);
            super::ground_pose::set_position_world_xy(
                &mut entity.position,
                [destination.x, destination.y],
            );
            // Process719631..7196B2: SetCoords, resolve destination bridge,
            // then Object+1CC/5F5FA0 SetHeight(0). An old split altitude
            // must not reappear in the arrival XYZ.
            // RESIDUAL: native sets the Location through
            // FootClass::SetLocation (vt+0x1B4, 0x00719637 and again at
            // 0x00719684); VERA keeps no destination Z for it (SetHeight(0)
            // replaces the Z) and skips its OpenTopped rider tail. Trigger:
            // a loaded OpenTopped transport (retail: the Drive BFRT) warped
            // by a superweapon, whose SuperClass code gives any Foot a
            // Teleport locomotor (0x006CC989..0x006CC999); VERA ports no such
            // warp yet. Effect: the riders stay at the departure point.
            if let Some(terrain) = terrain {
                let cell = terrain
                    .native_cell_identity((teleport.target_rx as i16, teleport.target_ry as i16));
                entity.on_bridge = terrain.native_cell_flags(cell) & 0x100 != 0;
            }
            super::ground_pose::set_height(
                &mut entity.position,
                entity.on_bridge,
                0,
                terrain,
                None,
            );
            // The path layer follows the destination's OnBridge, as an
            // ordinary crossing commits it (`cell_arrival`). The cell
            // lists move through the caller's Mark pair around this
            // relocation (`0x007195D4` UP, `0x007196B8` DOWN).
            if let Some(locomotor) = entity.locomotor.as_mut() {
                locomotor.layer = if entity.on_bridge {
                    crate::sim::movement::locomotor::MovementLayer::Bridge
                } else {
                    crate::sim::movement::locomotor::MovementLayer::Ground
                };
            }
            // Harvester instant-warp: when chrono delay is 0, finish in one
            // frame (cleanup runs at end of this frame) — no post-warp lock.
            if teleport.being_warped_ticks == 0 {
                finished = true;
                outcome = SpecialMovementOutcome::Complete;
            } else {
                teleport.phase = TeleportPhase::ChronoDelay;
                outcome = SpecialMovementOutcome::Continue;
            }
        }
        TeleportPhase::ChronoDelay => {
            // Count down chrono delay frames. Unit remains 50% translucent until 0.
            if teleport.being_warped_ticks > 0 {
                teleport.being_warped_ticks -= 1;
            }
            if teleport.being_warped_ticks == 0 {
                finished = true;
                outcome = SpecialMovementOutcome::Complete;
            } else {
                outcome = SpecialMovementOutcome::Continue;
            }
        }
    }

    // Log phase transition if it changed.
    let phase_after = teleport.phase;
    if phase_after != phase_before {
        let phase_name = format!("{:?}", phase_after);
        // Drop the borrow on teleport before pushing debug event.
        let _ = teleport;
        entity.push_debug_event(
            sim_tick as u32,
            DebugEventKind::SpecialMovementPhase { phase: phase_name },
        );
    }
    if finished {
        entity.teleport_state = None;
        entity.push_debug_event(sim_tick as u32, DebugEventKind::SpecialMovementEnd);
    }
    Some(outcome)
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::rules::locomotor_type::LocomotorKind;
    use crate::sim::entity_store::EntityStore;
    use crate::sim::game_entity::GameEntity;
    use crate::sim::movement::locomotor::LocomotorState;

    /// An owner whose active locomotor is Teleport.
    fn teleport_owner(id: u64, name: &str, rx: u16, ry: u16) -> GameEntity {
        let mut entity = GameEntity::test_default(id, name, "Americans", rx, ry);
        entity.locomotor = Some(LocomotorState::for_test_kind(LocomotorKind::Teleport));
        entity
    }

    fn default_rules() -> GeneralRules {
        GeneralRules::default()
    }

    #[test]
    fn stop_moving_drops_an_armed_warp_but_not_a_warp_in() {
        let mut entities = EntityStore::new();
        entities.insert(teleport_owner(1, "CLEG", 5, 5));
        let rules = default_rules();
        let owner = entities.get_mut(1).unwrap();
        assert!(teleport_move_to(owner, (20, 20), &rules, false, 0));
        teleport_stop_moving(owner);
        assert!(owner.teleport_state.is_none());

        assert!(teleport_move_to(owner, (20, 20), &rules, false, 0));
        process_teleport(&mut entities, 1, 0, None);
        let owner = entities.get_mut(1).unwrap();
        assert!(owner.is_warping_in());
        teleport_stop_moving(owner);
        assert!(owner.is_warping_in());
    }

    #[test]
    fn test_teleport_issues_and_completes() {
        let mut entities = EntityStore::new();
        let mut e = teleport_owner(1, "CLEG", 5, 5);
        e.position.z = 0;
        entities.insert(e);
        let rules = default_rules();

        assert!(teleport_move_to(
            entities.get_mut(1).unwrap(),
            (20, 20),
            &rules,
            false,
            0
        ));
        let entity = entities.get(1).expect("should exist");
        let ts = entity
            .teleport_state
            .as_ref()
            .expect("should have TeleportState");
        assert_eq!(ts.phase, TeleportPhase::Relocate);
        assert!(
            ts.being_warped_ticks >= 16,
            "should have at least minimum delay"
        );

        // One admitted frame relocates instantly.
        process_teleport(&mut entities, 1, 0, None);

        let entity = entities.get(1).expect("should exist");
        assert_eq!(entity.position.rx, 20, "Should have relocated to target");
        assert_eq!(entity.position.ry, 20);
        let ts = entity.teleport_state.as_ref().expect("still warping");
        assert_eq!(
            ts.phase,
            TeleportPhase::ChronoDelay,
            "should be in chrono delay"
        );

        // Advance through the ChronoDelay countdown.
        let delay = ts.being_warped_ticks;
        for _ in 0..delay + 5 {
            process_teleport(&mut entities, 1, 0, None);
        }

        // TeleportState should be removed after completion.
        let entity = entities.get(1).expect("should exist");
        assert!(
            entity.teleport_state.is_none(),
            "TeleportState should be removed after completion"
        );
    }

    #[test]
    fn test_chrono_delay_formula() {
        let mut rules = default_rules();
        // Default: factor=48, minimum=16, trigger=true, range_minimum=0

        // Short distance: 256 leptons (1 cell) → 256/48 = 5, clamped to 16
        assert_eq!(compute_chrono_delay(&rules, 256), 16);

        // Medium distance: 5120 leptons (20 cells) → 5120/48 = 106
        assert_eq!(compute_chrono_delay(&rules, 5120), 106);

        // Very short distance below range minimum
        rules.chrono_range_minimum = 512;
        assert_eq!(compute_chrono_delay(&rules, 200), 16); // forced to minimum

        // ChronoTrigger=false → always minimum
        rules.chrono_trigger = false;
        assert_eq!(compute_chrono_delay(&rules, 5120), 16);
    }

    /// Harvester units skip the chrono lock entirely — when is_harvester=true
    /// the lock duration is 0 regardless of distance.
    #[test]
    fn test_harvester_skips_chrono_delay() {
        let mut entities = EntityStore::new();
        let e = GameEntity::test_default(1, "CMIN", "Americans", 5, 5);
        entities.insert(e);
        let rules = default_rules();

        // Long distance (~80 cells diagonal) — non-harvester computes ~604 frames delay.
        assert!(teleport_move_to(
            entities.get_mut(1).unwrap(),
            (90, 90),
            &rules,
            true,
            0
        ));
        let ts = entities
            .get(1)
            .and_then(|e| e.teleport_state.as_ref())
            .expect("should have TeleportState");
        assert_eq!(
            ts.being_warped_ticks, 0,
            "harvester instant-warp must zero the chrono lock"
        );
    }

    /// With is_harvester=true, the Relocate phase finishes the teleport in a single
    /// tick (skipping ChronoDelay).
    #[test]
    fn test_harvester_relocate_cleans_up_in_one_tick() {
        let mut entities = EntityStore::new();
        entities.insert(teleport_owner(1, "CMIN", 5, 5));
        let rules = default_rules();

        assert!(teleport_move_to(
            entities.get_mut(1).unwrap(),
            (20, 20),
            &rules,
            true,
            0
        ));

        // Single frame: position snaps, then cleanup runs because being_warped_ticks==0.
        process_teleport(&mut entities, 1, 0, None);

        let entity = entities.get(1).expect("should exist");
        assert_eq!(entity.position.rx, 20);
        assert_eq!(entity.position.ry, 20);
        assert!(
            entity.teleport_state.is_none(),
            "harvester teleport should clean up in one frame"
        );
    }

    /// Regression: non-harvester (Chrono Legionnaire path) still goes through the
    /// full Relocate → ChronoDelay countdown.
    #[test]
    fn test_non_harvester_uses_full_chrono_delay() {
        let mut entities = EntityStore::new();
        let e = teleport_owner(1, "CLEG", 5, 5);
        entities.insert(e);
        let rules = default_rules();

        assert!(teleport_move_to(
            entities.get_mut(1).unwrap(),
            (20, 20),
            &rules,
            false,
            0
        ));
        let initial_ticks = entities
            .get(1)
            .and_then(|e| e.teleport_state.as_ref())
            .map(|t| t.being_warped_ticks)
            .expect("teleport_state");
        assert!(
            initial_ticks > 0,
            "non-harvester must keep the distance-based chrono lock"
        );

        // Frame 1: Relocate snaps position and transitions to ChronoDelay (NOT cleanup).
        process_teleport(&mut entities, 1, 0, None);
        let ts = entities
            .get(1)
            .and_then(|e| e.teleport_state.as_ref())
            .expect("still warping after Relocate");
        assert_eq!(ts.phase, TeleportPhase::ChronoDelay);
        assert_eq!(ts.being_warped_ticks, initial_ticks);
    }

    #[test]
    fn teleport_exposes_distinct_warp_and_targetability_producers() {
        let relocate = TeleportState {
            phase: TeleportPhase::Relocate,
            target_rx: 1,
            target_ry: 1,
            being_warped_ticks: 10,
        };
        assert!(relocate.warp_out_active());
        assert!(!relocate.warp_in_active());
        assert!(!relocate.is_targetable());

        let arrival = TeleportState {
            phase: TeleportPhase::ChronoDelay,
            target_rx: 1,
            target_ry: 1,
            being_warped_ticks: 10,
        };
        assert!(!arrival.warp_out_active());
        assert!(arrival.warp_in_active());
        assert!(arrival.is_targetable());
    }
}
