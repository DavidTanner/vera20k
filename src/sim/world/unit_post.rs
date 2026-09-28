//! Per-object UnitClass post-Foot host (the write half of the Facing slot).
//!
//! Post-Foot UnitClass slot order (gamemd `UnitClass::AI` steps 3m–3r; see
//! `docs/plans/2026-06-10-s3-unit-postfoot-ordering-design.md`):
//!   1. Fire         — per-attacker in combat Phase 2, live order        [LANDED L2/S3]
//!   2. Facing       — destinations read per-object in the combat Phase-2
//!                     window (pre-death state; kill-tick aim hold),
//!                     applied here post-batch                           [LANDED S3]
//!   3. GuardTerrain — Guard + invalid terrain + sight → self-destroy    [SLOT — UNCHECKED, needs RE]
//!   4. HarvestBrain — idle Harvester/Weeder → Harvest decision          [SLOT — miner substrate owns]
//!   5. Anim/Ammo    — the per-unit anim/ammo wrapper                    [SLOT — target unresolved, needs RE]
//!   6. (no SpawnManager slot here — see below)
//!
//! Correction (verified 2026-08-03): the SpawnManager is NOT a post-Foot
//! `UnitClass::AI` slot. `decompile_function 0x006F9E50` shows
//! `TechnoClass::AI_Update` dispatching it directly — `if (this+0x2D0)
//! (*(vtable+0x5C))()` — after the self-heal/power block and before the cloak
//! block, for every techno rather than for units only. VERA runs it as its own
//! pass right after the combat phase (`sim::spawn_manager::tick_spawn_managers`,
//! called from `World::advance_tick` Phase 5), which preserves the native
//! "this object fired and set its spawn target, then its manager reads it"
//! ordering within the tick.
//! Pre-fire idle turret scan + AI auto-hunt / stuck-harvester rescue are
//! S4 / AI-deferred respectively.
//!
//! AUTHORITATIVE for Unit barrel facing: destinations are computed per-object
//! in `combat::tick_combat_with_fog` Phase 2 (immediately after each Unit
//! attacker's own fire resolution; a residual pass covers target-less and
//! in-transport Units over the same `keys_sorted()` coverage the legacy sweep
//! had) and applied here, after the damage/death batch, at the unchanged
//! write point. Reading pre-death state is the S3 fidelity fix: a unit whose
//! target dies this tick keeps aiming at it this tick; idle-return begins the
//! next tick. `FacingClass::set` is pure in `(state, binary_frame)`, so the
//! apply point within Phase 5 does not change the resulting facing state.
//!
//! Depends on `sim/movement/turret` (facing math). Never depends on
//! render/ui/sidebar/audio/net (sim invariant #1). Dispatch is a
//! `category == Unit` filter — no trait object / dyn (invariant #2).

use crate::rules::ruleset::RuleSet;
use crate::sim::entity_store::EntityStore;
use crate::sim::game_entity::GameEntity;
use crate::sim::intern::StringInterner;

/// When true, Unit barrel facing is owned by the per-object path (combat
/// Phase-2 read window + `apply_unit_facing`) and `tick_turret_rotation`
/// skips Units.
pub(crate) const L2_UNIT_POST_AUTHORITATIVE: bool = true;

/// Apply the precomputed Unit Facing slot (the write half of the post-Foot
/// Facing slot). `FacingClass::set` is pure in `(state, binary_frame)` and no
/// system writes Unit facings between the combat Phase-2 read window and this
/// site, so the apply point within Phase 5 does not affect the resulting
/// state. Idempotent — `set` is a no-op when the destination already matches.
/// The turret's signed ROT is refreshed from rules each apply, same as the
/// legacy sweep; the hull keeps the rate its constructor wrote.
///
/// Four writes land here, and their ORDER is native, not incidental:
/// 1. the hull destination from `UnitClass::Fire_At_Target @ 0x00736DF0` case 2
///    (`FacingClass::Set(+0x388)` at `0x00737004`, then the hull's raw
///    destination copied into the turret slot at `0x0073701C` — `FUN_004C9470`
///    returns the DESTINATION dword, not the animated value). Native runs the
///    whole of `Fire_At_Target` before `Facing_Update`
///    (`0x007365E1`/`0x007365E8`), so this precedes everything below;
/// 2. `Facing_Update` arm A's aim `Set` on the turret, `0x00736A89`
///    (`None` = native calls no `Set`, so the turret holds its aim);
/// 3. the `+0x6AF` rotation latch — `MOV [ESI+0x6AF],AL` at `0x00736B16`, the
///    value being `FacingClass::Is_Rotating(+0x3A0) @ 0x004C9480` called at
///    `0x00736B11`, with the unconditional clear at `0x00736AD5` covering the
///    turretless and not-rotating cases. Read next tick by
///    `UnitClass::GetFireError @ 0x00741233`;
/// 4. `Facing_Update` arm B's idle-return `Set` on the turret, `0x00736BDD`.
///
/// Steps 3 and 4 are in that order in the binary, and it matters:
/// `FacingClass::Set @ 0x004C9220` writes `duration = abs(delta)/rate` and
/// `start = g_CurrentFrameCounter`, so any `Set` of one or more `ROT=` steps
/// makes `Is_Rotating` true immediately. An aim `Set` therefore arms the latch
/// on the frame it is issued — which is what keeps the fire gate refusing for
/// the whole arc, including its first one or two frames — while an idle-return
/// `Set` leaves the latch clear, so a unit that re-acquires a target during an
/// idle swing-back is free to aim at it on the next frame.
pub(crate) fn apply_unit_facing(
    entities: &mut EntityStore,
    updates: &[crate::sim::combat::UnitFacingUpdate],
    rules: &RuleSet,
    interner: &StringInterner,
    binary_frame: u32,
) {
    for update in updates {
        let id = update.entity_id;
        let Some(entity) = entities.get(id) else {
            continue;
        };
        let rot = rules
            .object(interner.resolve(entity.type_ref()))
            .map(|obj| obj.turret_rot)
            .unwrap_or(5);
        let before = UnitFacing::of(entity);
        let mut after = before;
        after.apply(update, rot, binary_frame);
        // Most units hold their aim, so their `Set`s repeat the destination
        // and write nothing: hand a unit out only when a field changes.
        if after != before {
            after.store(
                entities
                    .get_mut(id)
                    .expect("a facing update's unit was just read"),
            );
        }
    }
}

/// The fields the Facing slot writes, stepped on a copy.
#[derive(Clone, Copy, PartialEq)]
struct UnitFacing {
    /// The hull, `+0x388`.
    hull: crate::sim::movement::FacingClass,
    /// The turret, `+0x3A0`.
    barrel: Option<crate::sim::movement::FacingClass>,
    /// The `+0x6AF` rotation latch.
    latch: bool,
}

impl UnitFacing {
    fn of(entity: &GameEntity) -> Self {
        Self {
            hull: entity.body_facing,
            barrel: entity.barrel_facing,
            latch: entity.turret_rotation_latch,
        }
    }

    fn store(self, entity: &mut GameEntity) {
        entity.body_facing = self.hull;
        entity.barrel_facing = self.barrel;
        entity.turret_rotation_latch = self.latch;
    }

    /// Steps 1–4 in binary order.
    fn apply(
        &mut self,
        update: &crate::sim::combat::UnitFacingUpdate,
        rot: i32,
        binary_frame: u32,
    ) {
        // 1. `Fire_At_Target` case 2's hull turn, which native completes before
        //    `Facing_Update` is entered at all (`0x007365E1`/`0x007365E8`).
        if let Some(desired) = update.hull_destination {
            self.hull.set(desired, binary_frame);
            let raw_destination = self.hull.destination();
            if let Some(ref mut barrel) = self.barrel {
                barrel.set_rot(rot);
                barrel.set(raw_destination, binary_frame);
            }
        }
        // 2. arm A's aim `Set` (`0x00736A89`) — before the latch store.
        if let Some(desired) = update.turret_destination
            && !update.turret_destination_is_idle_return
            && let Some(ref mut barrel) = self.barrel
        {
            barrel.set_rot(rot);
            barrel.set(desired, binary_frame);
        }
        // 3. the `+0x6AF` store (`0x00736B16`), evaluated on the barrel as it
        //    stands after the writes above. `is_some_and` reproduces the
        //    `0x00736AD5` clear for a turretless unit, which native leaves at 0
        //    because the `Type+0xCA1` test at `0x00736ADC` sends it past both
        //    `Is_Rotating` calls.
        self.latch = self
            .barrel
            .as_ref()
            .is_some_and(|barrel| barrel.is_rotating(binary_frame));
        // 4. arm B's idle-return `Set` (`0x00736BDD`) — after the latch store,
        //    so the swing-back arc does not arm it.
        if let Some(desired) = update.turret_destination
            && update.turret_destination_is_idle_return
            && let Some(ref mut barrel) = self.barrel
        {
            barrel.set_rot(rot);
            barrel.set(desired, binary_frame);
        }
    }
}
