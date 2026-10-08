//! Locomotor power: the flag, the edges that drive it, and its one effect.
//!
//! ## The native shape
//!
//! Power is a plain byte on the locomotor instance. `Power_On` writes 1,
//! `Power_Off` writes 0, `Is_Powered` reads it back. Both setters then
//! re-dispatch ILoco+60 to `IsPowered55A930`, returning the new flag.
//! Neither setter stops movement or
//! clears a destination — powering off is not a stop.
//!
//! The flag itself lives on [`LocomotorState`]; this module owns the
//! documentation of *when* it moves and what reads it, because that is the part
//! worth keeping in one place.
//!
//! ## The edges, and where they are wired
//!
//! Every one of these was read off a callsite whose receiver is the locomotor
//! interface on the host, not some other object's vtable:
//!
//! | edge | direction | wired in |
//! |---|---|---|
//! | a destination is accepted (command-time adapter locomotors) | on | the move-command entry |
//! | bunker sell/death release4593A0 | on | `docking::bunker_link::release_sell_destroy` |
//! | bunker normal release4595C0 | on | `docking::bunker_link::release_normal` |
//! | Unit destination tail742F48, on a repair/bunker building's ground cell | on | `track_path::unit_destination_power_on` |
//! | depot MissionRepair44B780 pending admission / servicing / state1 release | on / off / on | `docking::building_dock::mission_repair` |
//!
//! Both bunker calls dispatch ILoco+58 (Power_On55A8F0), before Force_Track
//! and the caller's separate speed write. The building+2E4 link gates them;
//! its only non-null producer is bunker installation. The old harvester label
//! on4595C0 does not make it a refinery release path.
//!
//! Ordinary Unit741970 orders on cells without a repair/bunker building keep
//! the byte (tools/spatial_oracle/track_order_path power rows). Its shared
//! tail powers on before Foot4D94B0 when Object5F6960's raw current cell has
//! a UnitRepair/Bunker building and lacks high-bridge flag0x100. Same-Nav,
//! deployment refusal and the accepted depot handshake return before that
//! tail. Executable controls: building_repair.depot_service.{json,md}.
//! Repair state0 admission failure restores power at44C808; state1's
//! NEED_MOVE failure restores an unpowered contact at44C61B. The same corpus
//! compares both fallbacks and their distance/moving/powered controls.
//! Infantry51AA40 has no PowerOn. The remaining command-time adapter locomotors still power on
//! when they accept a destination. A deploy-powered-off unit is powered on by
//! its undeploy, and a bunkered one by its release.
//!
//! ## The one observable effect
//!
//! **Hover.** An unpowered hover locomotor stops producing lift and sinks to the
//! ground; every other family ignores the flag entirely today. That asymmetry is
//! native, not a shortcut.
//!
//! ## Frequency
//!
//! How often a stock skirmish actually reaches the powered-off state is **not
//! established** for Hover. EMP-driven power is not modelled here. Deployment
//! and ordinary depot servicing produce the flag changes; depot departure
//! reaches the shared Unit setter's current-cell power-on gate. No pass has traced a stock sequence
//! that leaves a *hover* unit unpowered long enough to be seen sinking, so the
//! player-visible reach of this slice is unquantified. It is modelled because
//! the flag is real deterministic state that deploy flips, not because a
//! symptom demanded it.
//!
//! ## Explicitly out of scope
//!
//! EMP-drives-power; the Fly family's power-off RNG draws; anything reading ion
//! sensitivity, ion storms or the special-flags ion path; any lightning-storm to
//! locomotor-power coupling.
//!
//! ## Dependency rules
//! - Part of sim/ — depends on sibling movement state only.
//! - sim/ NEVER depends on render/, ui/, audio/, net/.

#[cfg(test)]
mod tests {
    use crate::rules::locomotor_type::LocomotorKind;
    use crate::sim::movement::locomotor::LocomotorState;

    #[test]
    fn a_fresh_locomotor_is_powered() {
        for kind in [
            LocomotorKind::Drive,
            LocomotorKind::Hover,
            LocomotorKind::Walk,
            LocomotorKind::Teleport,
        ] {
            assert!(
                LocomotorState::for_test_kind(kind).is_powered(),
                "{kind:?} must start powered — unpowered is a state something \
                 has to actively put a unit into"
            );
        }
    }

    #[test]
    fn power_off_then_on_round_trips() {
        let mut state = LocomotorState::for_test_kind(LocomotorKind::Hover);
        state.power_off();
        assert!(!state.is_powered());
        state.power_on();
        assert!(state.is_powered());
    }

    /// Powering off is not a stop: the native setters touch the flag and nothing
    /// else, so a destination already installed survives.
    #[test]
    fn powering_off_does_not_disturb_the_rest_of_the_locomotor() {
        let mut state = LocomotorState::for_test_kind(LocomotorKind::Drive);
        let before = state.clone();
        state.power_off();

        assert_eq!(state.kind, before.kind);
        assert_eq!(state.layer, before.layer);
        assert_eq!(state.altitude, before.altitude);
        assert_eq!(state.piggyback, before.piggyback);
    }
}
