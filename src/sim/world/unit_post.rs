//! The write half of UnitClass's live post-Foot Facing slot.
//!
//! `combat::world_receiver::foot_fire_at_target` reads and writes this slot
//! immediately after the same Unit's Fire_At_Target (`7365E1`, `7365E8`),
//! before the second Ready/Commence checkpoint and the next Logic object.
//! The current target and selected weapon are read after FireAt's mutations;
//! a Bullet it created has not yet reached its later live Logic slot.
//!
//! Facing math stays in `sim/movement/turret`; this owner commits the hull,
//! turret and rotation-latch writes in native order. SpawnManager is a
//! separate TechnoClass AI_Update consumer, not a Unit post-Foot slot.

use crate::rules::ruleset::RuleSet;
use crate::sim::entity_store::EntityStore;
use crate::sim::game_entity::GameEntity;
use crate::sim::intern::StringInterner;

/// Unit barrel facing is owned by its live object slot, so the legacy
/// `tick_turret_rotation` sweep skips Units.
pub(crate) const L2_UNIT_POST_AUTHORITATIVE: bool = true;

/// Commit one or more Unit Facing slots at their caller's object boundary.
/// `FacingClass::set` is a no-op when the destination already matches.
/// Signed ROT is refreshed from rules for each apply.
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
///
/// It then mirrors the animated hull back into `entity.facing`, which is VERA's
/// authoritative 8-bit heading. gamemd has no such byte — `+0x388` IS the
/// heading — so the mirror is what keeps rendering, movement and the fire gate
/// on the same value. Units the movement tick owns (`movement_target` set) are
/// skipped: that path already mirrors, and clearing its interpolator here would
/// fight it.
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
        after.apply(update, rot, entity.movement_target.is_some(), binary_frame);
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
    /// VERA's 8-bit heading, which mirrors the animated hull.
    facing: u8,
    /// The hull, `+0x388`.
    hull: Option<crate::sim::movement::FacingClass>,
    /// The turret, `+0x3A0`.
    barrel: Option<crate::sim::movement::FacingClass>,
    /// The `+0x6AF` rotation latch.
    latch: bool,
}

impl UnitFacing {
    fn of(entity: &GameEntity) -> Self {
        Self {
            facing: entity.facing,
            hull: entity.body_facing,
            barrel: entity.barrel_facing,
            latch: entity.turret_rotation_latch,
        }
    }

    fn store(self, entity: &mut GameEntity) {
        entity.facing = self.facing;
        entity.body_facing = self.hull;
        entity.barrel_facing = self.barrel;
        entity.turret_rotation_latch = self.latch;
    }

    /// Steps 1–4 in binary order, then the heading mirror, which the movement
    /// tick owns while a path is live (`path_live`).
    fn apply(
        &mut self,
        update: &crate::sim::combat::UnitFacingUpdate,
        rot: i32,
        path_live: bool,
        binary_frame: u32,
    ) {
        // 1. `Fire_At_Target` case 2's hull turn, which native completes before
        //    `Facing_Update` is entered at all (`0x007365E1`/`0x007365E8`).
        if let Some(desired) = update.hull_destination {
            let facing = self.facing;
            let hull = self.hull.get_or_insert_with(|| {
                crate::sim::movement::FacingClass::new(u16::from(facing) << 8, rot)
            });
            hull.set_rot(rot);
            hull.set(desired, binary_frame);
            let raw_destination = hull.destination();
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
        // Mirror the animated hull into the 8-bit heading.
        if !path_live && let Some(ref hull) = self.hull {
            self.facing = (hull.current(binary_frame) >> 8) as u8;
        }
    }
}
