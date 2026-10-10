//! EBolt4C2830 reverse tactical traversal and4C29E0 scene cleanup. These
//! copied effects have no Abstract ID, save record or ordinary source link.
//! Unlike lasers they age on each tactical composite, even without Logic.

use crate::render::electric_bolt::{ElectricBoltDraw, ElectricBoltPalette};
use crate::render::surface_line::{SurfaceLine, SurfaceLineViewport};
use crate::sim::combat::electric_bolt::ElectricBoltBirth;
use crate::sim::rng::MainRngDraws;

struct Bolt {
    birth: ElectricBoltBirth,
    phase: i32,
    decay: i32,
}

#[derive(Debug, serde::Serialize)]
pub(crate) struct ElectricBoltObservation {
    birth_frame: i32,
    from: [i32; 3],
    to: [i32; 3],
    z_adjust: i32,
    alternate_color: bool,
    phase: i32,
    decay: i32,
}

#[derive(Default)]
pub(crate) struct ElectricBolts {
    live: Vec<Bolt>,
    lines: Vec<SurfaceLine>,
}

impl ElectricBolts {
    pub(crate) fn create(&mut self, birth: ElectricBoltBirth) {
        self.live.push(Bolt {
            phase: birth.phase,
            birth,
            decay: 0x10000,
        });
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.live.is_empty()
    }

    /// Manager4C2830 never uses elapsed time. A rejected clip still increments
    /// phase and shifts lifetime; a zero entry skips projection and increment.
    /// Geometry alone borrows Main. Upload/chunk replay only reads its output.
    pub(crate) fn composite(
        &mut self,
        viewport: SurfaceLineViewport,
        palette: Option<ElectricBoltPalette>,
        main: &mut MainRngDraws,
    ) -> &[SurfaceLine] {
        self.lines.clear();
        for bolt in self.live.iter_mut().rev() {
            if bolt.decay != 0 {
                let draw = ElectricBoltDraw {
                    from: bolt.birth.from,
                    to: bolt.birth.to,
                    z_adjust: bolt.birth.z_adjust,
                    phase: bolt.phase,
                    alternate_color: bolt.birth.alternate_color,
                };
                draw.lines(
                    viewport,
                    palette.expect("loaded tactical PALETTE.PAL"),
                    main,
                    |line| self.lines.push(line),
                );
                bolt.phase = bolt.phase.wrapping_add(1);
            }
            bolt.decay >>= 1;
        }
        self.live.retain(|bolt| bolt.decay != 0);
        &self.lines
    }

    pub(crate) fn observations(&self) -> impl Iterator<Item = ElectricBoltObservation> + '_ {
        self.live.iter().map(|bolt| ElectricBoltObservation {
            birth_frame: bolt.birth.frame,
            from: [bolt.birth.from.x, bolt.birth.from.y, bolt.birth.from.z],
            to: [bolt.birth.to.x, bolt.birth.to.y, bolt.birth.to.z],
            z_adjust: bolt.birth.z_adjust,
            alternate_color: bolt.birth.alternate_color,
            phase: bolt.phase,
            decay: bolt.decay,
        })
    }

    /// Scenario53493F clears the entire nonserialized vector. Source death
    /// alone does not detach an ordinary6FD460 bolt (its owner link is null).
    pub(crate) fn clear_on_load(&mut self) {
        self.live.clear();
        self.lines.clear();
    }
}

#[cfg(test)]
#[path = "electric_bolt_lifetime_tests.rs"]
mod tests;
