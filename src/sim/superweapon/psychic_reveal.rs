//! The Psychic Reveal's launch: `SuperClass::Launch @ 0x006CC390` case 11
//! (`0x006CD70C..0x006CD7E7`).
//!
//! A charged Super (`+0x6F`) takes the target cell's GetCoords (vt+0x48,
//! `0x00486840`: its centre at its ground height, no bridge deck) and calls
//! `MapClass::RevealArea @ 0x005678E0` around it twice, with `[CombatDamage]
//! PsychicRevealRadius=` (`Rules+0xFEC`), the Super's house and no outline,
//! unreveal or line of sight: final 0 (`0x006CD773`), then final 1
//! (`0x006CD79C`) — [`vision::psychic_reveal`]. It then plays
//! `[AudioVisual] PsychicRevealActivateSound=` at the cell's coordinate
//! (`0x006CD7BF`). No EVA line, anim or radar event.
//!
//! RevealArea lifts the centre toward the map's north by the coordinate's
//! height, reveals nothing when the lifted centre is off the map `Size=`
//! diamond, skips cells off it, clamps the radius to 10, and writes the
//! Super's house's map and, under `[AudioVisual] AllyReveal=`, the map of
//! each house that house counts as allied.
//!
//! The local player's tail (`0x006CD7C4..0x006CD7E2`, then the shared
//! `0x006CD51E`) clears the selected Super and drops the queued
//! `EVA_PsychicRevealReady` line; the app does both from the launch event.
//!
//! Evidence: `tools/superweapon_oracle.py` section `psychic_launch` (Unicorn
//! on gamemd.exe: case 11, RevealArea a recorded stub), replayed in
//! `psychic_reveal_tests.rs`; RevealArea's walk and house gate against
//! `tools/spatial_oracle/reveal_area.py` in `vision/reveal_area_tests.rs`.
//!
//! RESIDUALS:
//! - A negative `PsychicRevealRadius=` indexes before RevealArea's count
//!   table natively; VERA reveals nothing. No retail value is negative.
//! - The activate sound plays at the cell's ground-level centre (the launch
//!   event carries the cell); native's coordinate has the cell's height.
//!   Presentation only.
//! - The leaf's CellChangeNotify (`0x004A9D94` -> `0x005865F0`) calls
//!   DiscoveredBy (vt+0x198) on the objects of each cell it changes; VERA
//!   drops it for every reveal. Skirmish objects are discovered when placed
//!   (`discover_cell_put_object`); a campaign's objects that a reveal
//!   uncovers are not discovered by it.
//!
//! Ledger: no Scenario draws, timer writes or detach calls.
//!
//! ## Dependency rules
//! - Part of sim/ — depends on rules/, sim/vision, sim/world.
//! - sim/ NEVER depends on render/, ui/, audio/, net/.

#[cfg(test)]
#[path = "psychic_reveal_tests.rs"]
mod tests;

use crate::rules::ruleset::RuleSet;
use crate::sim::intern::InternedId;
use crate::sim::vision;
use crate::sim::world::{SimSoundEvent, Simulation};

/// What [`launch`] called, in the oracle's terms (observation only).
#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq)]
enum Observed {
    /// RevealArea's pair of calls: the coordinate and the radius.
    RevealArea([i32; 3], u16),
}

#[cfg(test)]
thread_local! {
    /// Observation only: what [`launch`] called on this thread while a test
    /// holds `Some`.
    static OBSERVED: std::cell::RefCell<Option<Vec<Observed>>> =
        const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
fn observe(call: Observed) {
    OBSERVED.with(|log| {
        if let Some(log) = log.borrow_mut().as_mut() {
            log.push(call);
        }
    });
}

/// Launch case 11 for `owner`'s Super of type `sw_type` at (target_rx,
/// target_ry): see the module doc. Returns whether the Super was charged.
pub fn launch(
    sim: &mut Simulation,
    rules: &RuleSet,
    owner: InternedId,
    target_rx: u16,
    target_ry: u16,
    sw_type: InternedId,
) -> bool {
    if !super::is_charged(sim, owner, sw_type) {
        return false;
    }
    let coord = super::fire::cell_coords(sim, (target_rx, target_ry));
    let radius = rules
        .general
        .psychic_reveal_radius
        .clamp(0, i32::from(u16::MAX)) as u16;
    #[cfg(test)]
    observe(Observed::RevealArea(coord, radius));
    sim.fog.alliances = sim.house_alliances.clone();
    let config = sim.sight_reveal_config(Some(rules));
    vision::psychic_reveal(
        &mut sim.fog,
        owner,
        (target_rx, target_ry),
        coord[2],
        radius,
        &config,
        &sim.interner,
    );
    sim.sound_events.push(SimSoundEvent::SuperWeaponLaunched {
        owner,
        sw_type,
        rx: target_rx,
        ry: target_ry,
    });
    true
}
