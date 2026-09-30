//! DriveLocomotion runtime helpers.
//!
//! This module owns Drive-specific state updates that should not leak into the
//! generic `MovementTarget` path. Detailed DriveTrack consumption remains in
//! `track_host`; this file handles the Drive-local speed fraction scaffold.

#[cfg(test)]
use crate::sim::components::DriveCoord;
use crate::sim::components::{DriveLocomotionRuntime, FootSpeedState, ShipLocomotionRuntime};
use crate::sim::game_entity::GameEntity;
use crate::util::fixed_math::{SIM_ZERO, SimFixed};

const DRIVE_DESTINATION_BRAKE_FLOOR: SimFixed = SimFixed::lit("0.3");

/// `DriveLocomotionClass::Do_Turn @ 0x004B0EF0` (ILocomotion +0x4C): one
/// `FacingClass::Set @ 0x004C9220` on the owner's PrimaryFacing (+0x388).
/// Re-issuing the destination the facing already holds keeps its running
/// timer (tools/mcv_deploy_oracle.json turns). The hull then animates from the
/// frame-anchored `body_facing`.
pub(crate) fn drive_do_turn(entity: &mut GameEntity, desired: u16, frame: u32) {
    entity.body_facing.set(desired, frame);
}

/// Apply Drive's retained target to Foot before budget consumption.
///
/// Gamemd keeps the target fraction on DriveLocomotion and the applied/current
/// fraction on the owner through `SetSpeedFraction`4D3710. A controller swap
/// must leave that live owner value available to the retained invocation.
#[allow(clippy::too_many_arguments)]
pub(super) fn update_drive_speed_fraction(
    drive: &DriveLocomotionRuntime,
    owner_speed: &mut FootSpeedState,
    accelerates: bool,
    unit_passive: bool,
    raw_speed_per_frame: SimFixed,
    accel_factor: SimFixed,
    decel_factor: SimFixed,
    slowdown_distance: SimFixed,
    distance_to_goal: SimFixed,
) {
    update_vehicle_speed_fraction(
        drive.target_speed_fraction,
        drive.track.turn_index,
        owner_speed,
        accelerates,
        unit_passive,
        raw_speed_per_frame,
        accel_factor,
        decel_factor,
        slowdown_distance,
        distance_to_goal,
    );
}

/// Apply Ship's retained target to the owner-applied fraction.
///
/// The active Ship `Process_Drive_Track` body uses these same transitions
/// before calling the owner's `SetSpeedFraction` slot. The target belongs to
/// Ship; the applied value belongs to the live Foot owner.
#[allow(clippy::too_many_arguments)]
pub(super) fn update_ship_speed_fraction(
    ship: &ShipLocomotionRuntime,
    owner_speed: &mut FootSpeedState,
    accelerates: bool,
    unit_passive: bool,
    raw_speed_per_frame: SimFixed,
    accel_factor: SimFixed,
    decel_factor: SimFixed,
    slowdown_distance: SimFixed,
    distance_to_goal: SimFixed,
) {
    update_vehicle_speed_fraction(
        ship.target_speed_fraction,
        ship.track.turn_index,
        owner_speed,
        accelerates,
        unit_passive,
        raw_speed_per_frame,
        accel_factor,
        decel_factor,
        slowdown_distance,
        distance_to_goal,
    );
}

/// Fixed-point prefix scaffold. Drive4B0F74..126F and Ship6A0644..0937
/// first copy the target for Accelerates=false; otherwise Unit Passive and
/// signed selector>=64 bypass the entire ramp. Native executable coverage:
/// tools/spatial_oracle/track_speed_native.json.
///
/// Remaining production gaps include native double arithmetic, raw type speed,
/// full XYZ distance, sinking +3CD, crush +6B5, and linked-owner propagation.
/// The crush byte's actual producers include overlay-specific conditions; an
/// ordinary infantry crush alone does not establish that byte.
#[allow(clippy::too_many_arguments)]
fn update_vehicle_speed_fraction(
    target: SimFixed,
    selector: i32,
    owner_speed: &mut FootSpeedState,
    accelerates: bool,
    unit_passive: bool,
    raw_speed_per_frame: SimFixed,
    accel_factor: SimFixed,
    decel_factor: SimFixed,
    slowdown_distance: SimFixed,
    distance_to_goal: SimFixed,
) {
    // ProcessMovement retains an unclamped class target. Only the Foot
    // setter4D3710 clamps the applied fraction to [0,1].
    if !accelerates {
        owner_speed.set_speed_fraction(target);
        return;
    }
    if unit_passive || selector >= 64 {
        return;
    }
    let mut current = owner_speed.applied_fraction();
    if slowdown_distance > SIM_ZERO && distance_to_goal < slowdown_distance {
        current -= raw_speed_per_frame * decel_factor;
        if current < DRIVE_DESTINATION_BRAKE_FLOOR {
            current = DRIVE_DESTINATION_BRAKE_FLOOR;
        }
    } else if current < target {
        current += accel_factor;
        if current > target {
            current = target;
        }
    } else if target < current {
        current -= raw_speed_per_frame * decel_factor;
        if current < target {
            current = target;
        }
    }
    owner_speed.set_speed_fraction(current);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::movement::foot_speed::owner_current_speed_from_fraction;
    use crate::util::fixed_math::{SIM_HALF, SIM_ONE, SIM_ZERO};

    #[test]
    fn gsi_13_06_ship_speed_fraction_uses_locomotor_owned_state() {
        let mut owner_speed = FootSpeedState::default();
        let mut ship = ShipLocomotionRuntime::default();

        ship.target_speed_fraction = SIM_HALF;
        update_ship_speed_fraction(
            &ship,
            &mut owner_speed,
            false,
            false,
            SimFixed::from_num(10),
            SimFixed::lit("0.03"),
            SimFixed::lit("0.002"),
            SimFixed::from_num(500),
            SimFixed::from_num(1000),
        );
        assert_eq!(ship.target_speed_fraction, SIM_HALF);
        assert_eq!(owner_speed.applied_fraction(), SIM_HALF);

        owner_speed.set_speed_fraction(SIM_ZERO);
        ship.target_speed_fraction = SIM_ONE;
        update_ship_speed_fraction(
            &ship,
            &mut owner_speed,
            true,
            false,
            SimFixed::from_num(10),
            SimFixed::lit("0.03"),
            SimFixed::lit("0.002"),
            SimFixed::from_num(500),
            SimFixed::from_num(1000),
        );
        assert_eq!(ship.target_speed_fraction, SIM_ONE);
        assert_eq!(owner_speed.applied_fraction(), SimFixed::lit("0.03"));
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

        for _ in 0..10 {
            update_ship_speed_fraction(
                &ship,
                &mut owner_speed,
                true,
                false,
                speed / SimFixed::from_num(15),
                SimFixed::lit("0.03"),
                SimFixed::lit("0.002"),
                SIM_ZERO,
                SimFixed::from_num(256),
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
        update_drive_speed_fraction(
            &drive,
            &mut owner_speed,
            false,
            false,
            SimFixed::from_num(10),
            SimFixed::lit("0.03"),
            SimFixed::lit("0.002"),
            SimFixed::from_num(500),
            SimFixed::from_num(1000),
        );

        assert_eq!(drive.target_speed_fraction, SimFixed::lit("1.2"));
        assert_eq!(owner_speed.applied_fraction(), SIM_ONE);
    }

    #[test]
    fn accelerates_false_assigns_current_fraction_directly() {
        let mut owner_speed = FootSpeedState::default();
        owner_speed.set_speed_fraction(SIM_ZERO);
        let mut drive = DriveLocomotionRuntime::default();

        drive.target_speed_fraction = SIM_HALF;
        update_drive_speed_fraction(
            &drive,
            &mut owner_speed,
            false,
            false,
            SimFixed::from_num(10),
            SimFixed::lit("0.03"),
            SimFixed::lit("0.002"),
            SimFixed::from_num(500),
            SimFixed::from_num(1000),
        );

        assert_eq!(drive.target_speed_fraction, SIM_HALF);
        assert_eq!(owner_speed.applied_fraction(), SIM_HALF);
    }

    #[test]
    fn accelerates_true_ramps_current_fraction_upward() {
        let mut owner_speed = FootSpeedState::default();
        owner_speed.set_speed_fraction(SIM_ZERO);
        let mut drive = DriveLocomotionRuntime::default();

        drive.target_speed_fraction = SIM_ONE;
        update_drive_speed_fraction(
            &drive,
            &mut owner_speed,
            true,
            false,
            SimFixed::from_num(10),
            SimFixed::lit("0.03"),
            SimFixed::lit("0.002"),
            SimFixed::from_num(500),
            SimFixed::from_num(1000),
        );

        assert_eq!(drive.target_speed_fraction, SIM_ONE);
        assert_eq!(owner_speed.applied_fraction(), SimFixed::lit("0.03"));
    }

    #[test]
    fn accelerates_true_brakes_by_raw_speed_scaled_decel_with_floor() {
        let mut owner_speed = FootSpeedState::default();
        owner_speed.set_speed_fraction(SIM_HALF);
        let mut drive = DriveLocomotionRuntime::default();

        drive.target_speed_fraction = SIM_ONE;
        update_drive_speed_fraction(
            &drive,
            &mut owner_speed,
            true,
            false,
            SimFixed::from_num(10),
            SimFixed::lit("0.03"),
            SimFixed::lit("0.002"),
            SimFixed::from_num(500),
            SimFixed::from_num(499),
        );

        assert_eq!(
            owner_speed.applied_fraction(),
            SIM_HALF - SimFixed::from_num(10) * SimFixed::lit("0.002")
        );
    }

    #[test]
    fn accelerates_true_braking_uses_strict_slowdown_distance() {
        let mut owner_speed = FootSpeedState::default();
        owner_speed.set_speed_fraction(SIM_HALF);
        let mut drive = DriveLocomotionRuntime::default();

        drive.target_speed_fraction = SIM_ONE;
        update_drive_speed_fraction(
            &drive,
            &mut owner_speed,
            true,
            false,
            SimFixed::from_num(10),
            SimFixed::lit("0.03"),
            SimFixed::lit("0.002"),
            SimFixed::from_num(500),
            SimFixed::from_num(500),
        );

        assert_eq!(owner_speed.applied_fraction(), SimFixed::lit("0.53"));
    }
}
