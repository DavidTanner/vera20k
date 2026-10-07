//! The local client's selected Super (`0x008809A0`, VERA's SuperWeapon
//! targeting mode) as the player's Supers move it, and the ChronoPlacement
//! anims the client hides.
//!
//! The selection's writers are the sidebar (`SelectClass::Action`), building
//! placement (`0x004FB8A9`), `SuperClass::Launch` and the revoke/suspend
//! pass (the Ghidra xrefs to `0x008809A0`). None drops it because the
//! selected Super is not charged or not granted, so the Chrono Warp case 3
//! selects stays selected although retail grants it to no house.

use std::collections::BTreeSet;

use crate::app::types::TargetingMode;
use crate::audio::events::{GameSoundEvent, SoundEventQueue};
use crate::rules::ruleset::RuleSet;
use crate::rules::superweapon_type::SuperWeaponKind;
use crate::sim::anim_class::AnimId;
use crate::sim::world::{SimSoundEvent, Simulation};

/// The `[SuperWeaponTypes]` entry Launch case 3 selects for the player's next
/// click (`0x006CC46E`, the literal index 4: retail ChronoWarpSpecial).
fn chrono_warp_selection(rules: &RuleSet) -> Option<&str> {
    rules
        .super_weapon_order
        .get(crate::sim::superweapon::CHRONO_WARP_SELECTION_INDEX)
        .map(String::as_str)
}

/// Whether the local selection is the Chrono Warp (`0x008809A0 == 4`).
pub(crate) fn chrono_warp_selected(targeting: Option<&TargetingMode>, rules: &RuleSet) -> bool {
    let selected = targeting.and_then(TargetingMode::as_super_weapon);
    selected.is_some_and(|name| {
        chrono_warp_selection(rules).is_some_and(|warp| warp.eq_ignore_ascii_case(name))
    })
}

/// The selection writes of the local player's Supers: Launch case 3 selects
/// the Chrono Warp (`0x006CC46E`), case 4 clears the selection
/// (`0x006CCD1C`), and the revoke/suspend pass clears it when the selected
/// Super's hold changes or it is lost (`HouseClass @ 0x0050AF10`,
/// `0x0050B181..0x0050B190`). Other houses' Supers leave it alone.
pub(super) fn follow_selection_writes(
    targeting: &mut Option<TargetingMode>,
    events: &[SimSoundEvent],
    sim: &Simulation,
    rules: &RuleSet,
    local_owner_name: Option<&str>,
) {
    for event in events {
        let (SimSoundEvent::SuperWeaponLaunched { owner, sw_type, .. }
        | SimSoundEvent::SuperWeaponStatusChanged { owner, sw_type }) = event
        else {
            continue;
        };
        if !local_owner_name
            .is_some_and(|local| local.eq_ignore_ascii_case(sim.interner.resolve(*owner)))
        {
            continue;
        }
        let selected = targeting.as_ref().and_then(TargetingMode::as_super_weapon);
        if let SimSoundEvent::SuperWeaponStatusChanged { .. } = event {
            if selected
                .is_some_and(|name| name.eq_ignore_ascii_case(sim.interner.resolve(*sw_type)))
            {
                *targeting = None;
            }
            continue;
        }
        match rules
            .super_weapon(sim.interner.resolve(*sw_type))
            .map(|sw| sw.kind)
        {
            Some(SuperWeaponKind::ChronoSphere) => {
                *targeting = chrono_warp_selection(rules)
                    .map(|name| TargetingMode::SuperWeapon(name.to_string()));
            }
            Some(SuperWeaponKind::ChronoWarp) if selected.is_some() => {
                *targeting = None;
            }
            _ => {}
        }
    }
}

/// The client's hiding of a Super's ChronoPlacement anim: case 3 hides
/// another house's at once and stops its sound (`0x006CC485..0x006CC4A0`),
/// and every SuperClass::AI hides a held anim while the selection is not
/// the Chrono Warp and stops its sound (`0x006CBCD4..0x006CBCF9`). The byte
/// (AnimClass `+0x19D`) has no other writer for this anim, so a hidden one
/// stays hidden after its Super lets it go. `hidden` keeps the client's
/// bytes; anims that are gone leave it.
///
/// RESIDUAL: VERA drops the selection at the click, natively it stays until
/// case 4 runs; so the player's own anim hides from the click and its last
/// loop after the warp is not drawn.
pub(super) fn hide_placement_anims(
    hidden: &mut BTreeSet<AnimId>,
    sim: &Simulation,
    chrono_warp_selected: bool,
    local_owner_name: Option<&str>,
    output: &mut SoundEventQueue,
) {
    hidden.retain(|anim| sim.anim(*anim).is_some());
    for (owner, anim) in sim.super_placement_anims() {
        let local = local_owner_name
            .is_some_and(|local| local.eq_ignore_ascii_case(sim.interner.resolve(owner)));
        if (local && chrono_warp_selected) || !hidden.insert(anim) {
            continue;
        }
        output.push(GameSoundEvent::AnimationStopped {
            anim_id: anim,
            stop_sound_id: None,
            source: None,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::superweapon::chronosphere_tests::{
        SOURCE, charge_chronosphere, click, retail_world, step,
    };

    fn warp_selected() -> Option<TargetingMode> {
        Some(TargetingMode::SuperWeapon("ChronoWarpSpecial".to_string()))
    }

    /// Case 3 selects the Chrono Warp for its player (`0x006CC46E`), case 4
    /// clears the selection (`0x006CCD1C`); another house's launches leave
    /// the local selection alone.
    #[test]
    fn the_local_chronosphere_launches_move_the_selection() {
        let Some((rules, mut sim, local)) = retail_world() else {
            return;
        };
        let remote = sim.interner.intern("Russians");
        let sphere = sim.interner.intern("ChronoSphereSpecial");
        let warp = sim.interner.intern("ChronoWarpSpecial");
        let launched = |owner, sw_type| SimSoundEvent::SuperWeaponLaunched {
            owner,
            sw_type,
            rx: 21,
            ry: 21,
        };
        let mut targeting = None;
        follow_selection_writes(
            &mut targeting,
            &[launched(remote, sphere)],
            &sim,
            &rules,
            Some("Americans"),
        );
        assert_eq!(targeting, None);
        follow_selection_writes(
            &mut targeting,
            &[launched(local, sphere)],
            &sim,
            &rules,
            Some("Americans"),
        );
        assert_eq!(targeting, warp_selected());
        assert!(chrono_warp_selected(targeting.as_ref(), &rules));
        follow_selection_writes(
            &mut targeting,
            &[launched(remote, warp)],
            &sim,
            &rules,
            Some("Americans"),
        );
        assert_eq!(targeting, warp_selected());
        follow_selection_writes(
            &mut targeting,
            &[launched(local, warp)],
            &sim,
            &rules,
            Some("Americans"),
        );
        assert_eq!(targeting, None);
        // A building placement is not a Super selection: case 4 keeps it.
        let mut placing = Some(TargetingMode::BuildingPlacement("GAPOWR".to_string()));
        follow_selection_writes(
            &mut placing,
            &[launched(local, warp)],
            &sim,
            &rules,
            Some("Americans"),
        );
        assert!(placing.is_some());
    }

    /// The revoke/suspend pass drops the player's selection when the selected
    /// Super's hold changes or it is lost (`0x0050B181..0x0050B190`); another
    /// Super's or another house's report leaves it.
    #[test]
    fn the_selected_supers_hold_change_or_loss_drops_the_selection() {
        let Some((rules, mut sim, local)) = retail_world() else {
            return;
        };
        let remote = sim.interner.intern("Russians");
        let sphere = sim.interner.intern("ChronoSphereSpecial");
        let warp = sim.interner.intern("ChronoWarpSpecial");
        let changed = |owner, sw_type| SimSoundEvent::SuperWeaponStatusChanged { owner, sw_type };
        let mut targeting = warp_selected();
        follow_selection_writes(
            &mut targeting,
            &[changed(local, sphere), changed(remote, warp)],
            &sim,
            &rules,
            Some("Americans"),
        );
        assert_eq!(targeting, warp_selected());
        follow_selection_writes(
            &mut targeting,
            &[changed(local, warp)],
            &sim,
            &rules,
            Some("Americans"),
        );
        assert_eq!(targeting, None);
    }

    /// The local player's ChronoPlacement anim shows while the Chrono Warp is
    /// selected and hides, with one sound stop, once it is not
    /// (`0x006CBCD4..0x006CBCF9`); another house's hides at once
    /// (`0x006CC485..0x006CC4A0`); a gone anim leaves the set.
    #[test]
    fn placement_anims_hide_off_the_chrono_warp_selection() {
        let Some((rules, mut sim, americans)) = retail_world() else {
            return;
        };
        charge_chronosphere(&mut sim, americans);
        click(&mut sim, &rules, americans, "ChronoSphereSpecial", SOURCE);
        let anims = sim.super_placement_anims();
        assert_eq!(anims.len(), 1);
        let own = anims[0].1;

        let mut hidden = BTreeSet::new();
        let mut output = SoundEventQueue::new();
        hide_placement_anims(&mut hidden, &sim, true, Some("Americans"), &mut output);
        assert!(hidden.is_empty());
        assert!(output.drain().is_empty());

        hide_placement_anims(&mut hidden, &sim, false, Some("Americans"), &mut output);
        hide_placement_anims(&mut hidden, &sim, false, Some("Americans"), &mut output);
        assert_eq!(hidden, BTreeSet::from([own]));
        let stops = output.drain();
        assert!(
            matches!(
                stops.as_slice(),
                [GameSoundEvent::AnimationStopped { anim_id, .. }] if *anim_id == own
            ),
            "{stops:?}"
        );

        // Seen from the Russians' client the anim is another house's.
        let mut theirs = BTreeSet::new();
        hide_placement_anims(&mut theirs, &sim, true, Some("Russians"), &mut output);
        assert_eq!(theirs, BTreeSet::from([own]));
        assert_eq!(output.drain().len(), 1);

        // Released by the warp, the anim plays out and leaves.
        click(&mut sim, &rules, americans, "ChronoWarpSpecial", (40, 40));
        for _ in 0..200 {
            if sim.anim(own).is_none() {
                break;
            }
            step(&mut sim, &rules);
        }
        assert!(sim.anim(own).is_none());
        hide_placement_anims(&mut hidden, &sim, false, Some("Americans"), &mut output);
        assert!(hidden.is_empty());
    }
}
