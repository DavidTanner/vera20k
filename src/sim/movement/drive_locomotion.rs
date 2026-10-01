//! DriveLocomotion runtime helpers.
//!
//! This module owns Drive-specific state updates that should not leak into the
//! generic `MovementTarget` path. Detailed DriveTrack consumption remains in
//! `track_host`; this file handles the Drive-local speed fraction scaffold.

use crate::sim::components::FootSpeedState;
#[cfg(test)]
use crate::sim::components::{DriveCoord, DriveLocomotionRuntime, ShipLocomotionRuntime};
use crate::sim::game_entity::GameEntity;
use crate::util::fixed_math::SimFixed;

// Drive qwords 0x7E6240/0x7E6248/0x7E6250 (Ship 0x7F1308/0x7F1310/0x7F1318):
// the 0.3 brake floor, the 0.1 sinking floor and the 0.0015 sinking brake,
// promoted binary32; crush cap 0x7E3548 (binary64 0.2).
const DRIVE_DESTINATION_BRAKE_FLOOR: SimFixed = SimFixed::lit("0.3");
const SINKING_BRAKE_FLOOR: SimFixed = SimFixed::lit("0.1");
const CRUSH_SLOWDOWN_CAP: SimFixed = SimFixed::lit("0.2");

/// `DriveLocomotionClass::Do_Turn @ 0x004B0EF0` (ILocomotion +0x4C): one
/// `FacingClass::Set @ 0x004C9220` on the owner's PrimaryFacing (+0x388).
/// Re-issuing the destination the facing already holds keeps its running
/// timer (tools/mcv_deploy_oracle.json turns). The hull then animates from the
/// frame-anchored `body_facing`.
pub(crate) fn drive_do_turn(entity: &mut GameEntity, desired: u16, frame: u32) {
    entity.body_facing.set(desired, frame);
}

/// The live inputs of one Drive/Ship speed prefix (Drive 0x004B0F20..0x004B1295,
/// Ship 0x006A05F0..0x006A095D), read by `track_speed::advance`.
pub(super) struct TrackSpeedPrefix {
    /// Accelerates= (type +0xDBD).
    pub accelerates: bool,
    /// A Unit whose type is Passive= (type +0xE0C).
    pub unit_passive: bool,
    /// The class selector (+0x58).
    pub selector: i32,
    /// The type's stored speed (type +0x678), leptons per frame.
    pub raw_type_speed: i32,
    pub accel: SimFixed,
    pub decel: SimFixed,
    /// SlowdownDistance= (type +0x2F8).
    pub slowdown_distance: i32,
    /// `track_speed::braking_distance`.
    pub distance: i32,
    /// Techno+0x3CD (`SinkingState`).
    pub sinking: bool,
    /// Foot+0x6B5.
    pub crush_slowdown: bool,
}

/// One speed prefix: the class target (+0x50, `target`) and the Foot's
/// applied fraction (+0x578, through `SetSpeedFraction` 0x004D3710). Executed
/// native rows: tools/spatial_oracle/track_speed_native.json.
///
/// - Accelerates=false copies the target to the applied fraction.
/// - A selector of 64 or more, or a Passive Unit, skips the ramp.
/// - Inside SlowdownDistance (strict) the applied fraction brakes by raw type
///   speed x Deceleration down to 0.3; a sinking Techno brakes by 0.0015 x
///   raw speed down to 0.1 instead.
/// - The crush slowdown caps the target at 0.2 and applies it, braking or not.
/// - Otherwise the applied fraction ramps toward the target: up by
///   Acceleration, down by raw speed x Deceleration, never past it. An
///   applied fraction equal to the target calls no setter.
///
/// PRECISION: native works in binary64 x87 (chop53); this port works in
/// `SimFixed` (I16F16). Deceleration= is held to 2^-16, so a brake step can
/// differ from native by about raw speed x 2^-17 (about 1e-4 for a Rhino),
/// below one lepton of speed until raw speed reaches 128.
///
/// RESIDUAL: a Unit's ramp also propagates the applied fraction to its linked
/// members (0x004B1225 / 0x006A08ED); VERA has no linked-member chain.
pub(super) fn apply_track_speed_prefix(
    input: &TrackSpeedPrefix,
    target: &mut SimFixed,
    owner_speed: &mut FootSpeedState,
) {
    // ProcessMovement retains an unclamped class target. Only the Foot
    // setter clamps the applied fraction to [0,1].
    if !input.accelerates {
        owner_speed.set_speed_fraction(*target);
        return;
    }
    if input.selector >= 64 || input.unit_passive {
        return;
    }
    let applied = owner_speed.applied_fraction();
    let raw = SimFixed::from_num(input.raw_type_speed);
    let type_brake = raw * input.decel;
    // raw x 0.0015 as raw x 3 / 2000, rounded once.
    let sinking_brake = SimFixed::from_num(input.raw_type_speed * 3) / SimFixed::from_num(2000);
    let braking = if input.distance < input.slowdown_distance {
        Some((applied - type_brake).max(DRIVE_DESTINATION_BRAKE_FLOOR))
    } else if input.sinking {
        Some((applied - sinking_brake).max(SINKING_BRAKE_FLOOR))
    } else {
        None
    };
    let candidate = if input.crush_slowdown {
        *target = (*target).min(CRUSH_SLOWDOWN_CAP);
        *target
    } else if let Some(braked) = braking {
        braked
    } else if applied < *target {
        (applied + input.accel).min(*target)
    } else if applied > *target {
        (applied - type_brake).max(*target)
    } else {
        return;
    };
    owner_speed.set_speed_fraction(candidate);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::movement::foot_speed::owner_current_speed_from_fraction;
    use crate::util::fixed_math::{SIM_HALF, SIM_ONE, SIM_ZERO};

    fn prefix(
        accelerates: bool,
        raw_type_speed: i32,
        slowdown_distance: i32,
        distance: i32,
    ) -> TrackSpeedPrefix {
        TrackSpeedPrefix {
            accelerates,
            unit_passive: false,
            selector: 0,
            raw_type_speed,
            accel: SimFixed::lit("0.03"),
            decel: SimFixed::lit("0.002"),
            slowdown_distance,
            distance,
            sinking: false,
            crush_slowdown: false,
        }
    }

    #[test]
    fn gsi_13_06_stock_shp_current_speed_preserves_native_truncation() {
        use crate::util::fixed_math::ra2_speed_to_leptons_per_second;

        let stock_ship_speed = ra2_speed_to_leptons_per_second(8);
        assert_eq!(
            owner_current_speed_from_fraction(stock_ship_speed, SimFixed::lit("0.03")),
            0,
            "DLPH/SQD first acceleration step truncates to zero"
        );
        assert_eq!(
            owner_current_speed_from_fraction(stock_ship_speed, SimFixed::lit("0.06")),
            1
        );
        let stock_dron_speed = ra2_speed_to_leptons_per_second(10);
        assert_eq!(
            owner_current_speed_from_fraction(stock_dron_speed, SimFixed::lit("0.03")),
            0,
            "DRON's raw stage 25 still truncates 0.75 to zero"
        );
        assert_eq!(
            owner_current_speed_from_fraction(stock_dron_speed, SIM_ONE),
            25,
            "Accelerates=false DRON uses its full converted type speed"
        );
    }

    #[test]
    fn gsi_13_06_nonterminal_ship_post_stop_process_preserves_clamped_target() {
        use crate::util::fixed_math::ra2_speed_to_leptons_per_second;

        let speed = ra2_speed_to_leptons_per_second(8);
        let mut owner_speed = FootSpeedState::default();
        owner_speed.set_speed_fraction(SIM_HALF);
        let ship = ShipLocomotionRuntime {
            destination: None,
            head_to: Some(DriveCoord::cell(4, 3, 0)),
            target_speed_fraction: SimFixed::lit("0.3"),
            ..Default::default()
        };

        let mut ship = ship;
        for _ in 0..10 {
            apply_track_speed_prefix(
                &prefix(
                    true,
                    crate::util::fixed_math::ra2_speed_to_leptons_per_frame(8),
                    0,
                    256,
                ),
                &mut ship.target_speed_fraction,
                &mut owner_speed,
            );
            assert_eq!(ship.target_speed_fraction, SimFixed::lit("0.3"));
            assert!(owner_current_speed_from_fraction(speed, owner_speed.applied_fraction()) > 0);
        }

        assert_eq!(ship.destination, None);
        assert_eq!(ship.head_to, Some(DriveCoord::cell(4, 3, 0)));
        assert_eq!(owner_speed.applied_fraction(), SimFixed::lit("0.3"));
        assert_eq!(
            owner_current_speed_from_fraction(speed, owner_speed.applied_fraction()),
            6
        );
    }

    fn drive_entity_at(rx: u16, ry: u16) -> GameEntity {
        let mut entity = GameEntity::test_default(1, "HARV", "Americans", rx, ry);
        entity.locomotor = Some(
            crate::sim::movement::locomotor::LocomotorState::for_test_kind(
                crate::rules::locomotor_type::LocomotorKind::Drive,
            ),
        );
        entity.drive_locomotion = Some(DriveLocomotionRuntime::default());
        entity
    }

    fn drive_is_moving(entity: &GameEntity) -> bool {
        crate::sim::movement::motion_query::is_moving(entity) == Some(true)
    }

    /// GSI-06.12 GAP 3: a Drive's Is_Moving (ILocomotion+0x10, `0x004AFB80`)
    /// looks at the Drive locomotor's own destination and head-to coords — not
    /// at the owner's order. A non-null destination alone means "moving".
    #[test]
    fn gsi_06_12_drive_is_moving_reads_its_own_destination() {
        let mut entity = drive_entity_at(3, 3);
        entity.movement_target = Some(crate::sim::components::MovementTarget::default());
        assert!(!drive_is_moving(&entity), "an order alone is not moving");

        entity.drive_locomotion.as_mut().expect("drive").destination =
            Some(DriveCoord::cell(9, 3, 0));
        assert!(drive_is_moving(&entity));
    }

    /// With no destination, a null head-to is not moving and a head-to that
    /// already equals the owner's exact lepton X/Y is not moving either. Z is
    /// deliberately not part of the comparison.
    #[test]
    fn gsi_06_12_drive_is_moving_compares_head_to_against_the_owner_position() {
        let mut entity = drive_entity_at(3, 3);
        entity.position.sub_x = SimFixed::from_num(128);
        entity.position.sub_y = SimFixed::from_num(128);

        let drive = entity.drive_locomotion.as_mut().expect("drive");
        drive.head_to = Some(DriveCoord::cell(3, 3, 0));
        assert!(
            !drive_is_moving(&entity),
            "head-to at the owner's own cell centre is not moving"
        );

        let drive = entity.drive_locomotion.as_mut().expect("drive");
        drive.head_to = Some(DriveCoord::cell(3, 3, 7));
        assert!(
            !drive_is_moving(&entity),
            "a Z-only difference does not make it moving"
        );

        let drive = entity.drive_locomotion.as_mut().expect("drive");
        drive.head_to = Some(DriveCoord::cell(4, 3, 0));
        assert!(drive_is_moving(&entity));
    }

    #[test]
    fn a_downhill_target_stays_above_one_but_the_owner_fraction_does_not() {
        // `Process_Movement` @ 0x004B2630 writes `drive+0x50` unclamped on its
        // ordinary `drive+0x58 < 0x40` arm (`0x004B3E00`), so a 1.2 downhill
        // product survives on the locomotor-owned slot; the only native clamp is
        // `FootClass::SetSpeedFraction` @ 0x004D3710, and every arm of
        // `Process_Drive_Track` that writes the owner's fraction goes through
        // it, so that fraction never exceeds 1.
        let mut owner_speed = FootSpeedState::default();
        owner_speed.set_speed_fraction(SIM_ZERO);
        let mut drive = DriveLocomotionRuntime::default();

        drive.target_speed_fraction = SimFixed::lit("1.2");
        apply_track_speed_prefix(
            &prefix(false, 10, 500, 1000),
            &mut drive.target_speed_fraction,
            &mut owner_speed,
        );

        assert_eq!(drive.target_speed_fraction, SimFixed::lit("1.2"));
        assert_eq!(owner_speed.applied_fraction(), SIM_ONE);
    }
}
