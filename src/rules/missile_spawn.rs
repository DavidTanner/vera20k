//! Missile-spawn globals — the three hardcoded rocket families the spawn
//! manager treats as "missile-style" children.
//!
//! The per-slot `IsMissileSpawn` flag does **not** come from the child's
//! `MissileSpawn=` key. `SpawnManagerClass` compares the resolved `Spawns=`
//! TechnoType pointer against three fixed `RulesClass` slots —
//! `[General] V3RocketType=`, `DMislType=`, `CMislType=` — and sets the flag
//! only on a match. That is what this module models.
//!
//! The child's own `MissileSpawn=` key is a *separate* test the manager also
//! makes, on a different decision: in the Launching arm it reads
//! `childType+0xD68` to pick retreat (fire-and-forget) versus return-to-dock,
//! and `FUN_0054e3b0` reads it again to pick the retreat-list path versus an
//! outright kill. So a modded child with `MissileSpawn=yes` that is not one of
//! the three named types takes the fire-and-forget branch but never gets the
//! slot flag, and so never gets the pause+tilt kamikaze window. That asymmetry
//! is a real YR quirk. The two sets coincide in stock YR.
//!
//! The same three families carry the launch-pause / tilt frame counts and the
//! detonation damage + warhead used when the missile impacts.
//!
//! Sources: `docs/research/SPAWN_MANAGER_CLASS_GHIDRA_REPORT.md` §6 (Rules
//! slot offsets, hardcoded pointer test), `docs/research/
//! ROCKET_LOCOMOTION_CLASS_GHIDRA_REPORT.md` §3–§4 (RocketStruct fields,
//! detonation warhead selection), plus `decompile_function 0x006B7230`
//! (`SpawnManagerClass::AI`) read this session for the state-1 timer sources.
//!
//! ## Dependency rules
//! - Part of rules/ — no dependencies on sim/, render/, ui/, etc.

use crate::rules::ini_parser::IniSection;

/// `RulesClass::Constructor @ 0x00665650` seeds, `0x006678D3`-`0x006679E5`
/// (`EDI` = 10 from `0x006656D9`). The three type slots (`0x00667935`,
/// `0x00667998`, `0x00667A04`) and six warhead slots (`0x00666B5E`-
/// `0x00666B7C`) start null, held here as an empty name that matches no type.
mod ctor {
    pub const V3_PAUSE_FRAMES: i32 = 0;
    pub const V3_TILT_FRAMES: i32 = 60;
    pub const V3_DAMAGE: i32 = 1000;
    pub const DMISL_PAUSE_FRAMES: i32 = 10;
    pub const DMISL_TILT_FRAMES: i32 = 60;
    pub const DMISL_DAMAGE: i32 = 1000;
    pub const CMISL_DAMAGE: i32 = 500;
}

/// Which of the three hardcoded rocket families a spawn child belongs to.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub enum MissileFamily {
    /// `[General] V3RocketType=` — V3 Launcher.
    V3Rocket,
    /// `[General] DMislType=` — Dreadnought.
    DMisl,
    /// `[General] CMislType=` — Boomer.
    CMisl,
}

/// Per-family launch timing, detonation damage and warhead.
#[derive(Debug, Clone)]
pub struct MissileSpawnParams {
    /// Resolved child TechnoType section name (`V3ROCKET` / `DMISL` / `CMISL`).
    pub type_name: String,
    /// `*PauseFrames` — frames the missile rests on the launcher before tilting.
    pub pause_frames: i32,
    /// `*TiltFrames` — frames the tilt-to-firing-position takes.
    pub tilt_frames: i32,
    /// `[General] *Damage` — damage applied at impact (rookie/veteran).
    pub damage: i32,
    /// `[General] *EliteDamage` — damage applied when the launcher is elite.
    pub elite_damage: i32,
    /// `[CombatDamage] *Warhead` — impact warhead (rookie/veteran).
    pub warhead: String,
    /// `[CombatDamage] *EliteWarhead` — impact warhead when the launcher is elite.
    pub elite_warhead: String,
}

impl MissileSpawnParams {
    /// Damage for a launcher at the supplied veterancy (0 rookie / 100 veteran
    /// / 200 elite). Only the elite band swaps to the elite value; gamemd's
    /// `RocketLocomotion::Detonate` selects on the elite flag alone.
    pub fn damage_for(&self, veterancy: u16) -> i32 {
        if veterancy >= ELITE_VETERANCY {
            self.elite_damage
        } else {
            self.damage
        }
    }

    /// Warhead for a launcher at the supplied veterancy. Same elite-only split
    /// as `damage_for`.
    pub fn warhead_for(&self, veterancy: u16) -> &str {
        if veterancy >= ELITE_VETERANCY {
            &self.elite_warhead
        } else {
            &self.warhead
        }
    }
}

/// Veterancy value at which a unit counts as elite (0/100/200 scale).
const ELITE_VETERANCY: u16 = 200;

/// The three hardcoded missile-spawn families.
#[derive(Debug, Clone)]
pub struct MissileSpawnRules {
    pub v3: MissileSpawnParams,
    pub dmisl: MissileSpawnParams,
    pub cmisl: MissileSpawnParams,
}

impl Default for MissileSpawnRules {
    fn default() -> Self {
        let family = |pause_frames, tilt_frames, damage| MissileSpawnParams {
            type_name: String::new(),
            pause_frames,
            tilt_frames,
            damage,
            elite_damage: damage,
            warhead: String::new(),
            elite_warhead: String::new(),
        };
        Self {
            v3: family(ctor::V3_PAUSE_FRAMES, ctor::V3_TILT_FRAMES, ctor::V3_DAMAGE),
            dmisl: family(
                ctor::DMISL_PAUSE_FRAMES,
                ctor::DMISL_TILT_FRAMES,
                ctor::DMISL_DAMAGE,
            ),
            // The manager's state-1 timer reads the DMisl pause/tilt slots for
            // every non-V3 family, so CMisl's own `CMisl*Frames` keys are not
            // consulted there. They are left out rather than
            // parsed-and-ignored.
            cmisl: family(
                ctor::DMISL_PAUSE_FRAMES,
                ctor::DMISL_TILT_FRAMES,
                ctor::CMISL_DAMAGE,
            ),
        }
    }
}

impl MissileSpawnRules {
    /// The `[General]` rocket reads (`0x00671212`-`0x006716C0`) and the
    /// `[CombatDamage]` warhead reads (`0x0066C3A4`-`0x0066C4D5`), each over
    /// the current slot. The type and warhead names are ReadString into 0x80
    /// bytes ahead of the type lookup (`0x0067BD30`, `0x0066C3B1`), which an
    /// absent key skips.
    pub fn from_ini_sections(general: &IniSection, combat_damage: &IniSection) -> Self {
        let mut out = Self::default();
        let g = general;
        for (key, slot) in [
            ("V3RocketType", &mut out.v3.type_name),
            ("DMislType", &mut out.dmisl.type_name),
            ("CMislType", &mut out.cmisl.type_name),
        ] {
            if let Some(name) = g.read_name(key, 0x80) {
                *slot = name.to_string();
            }
        }
        out.v3.pause_frames = g.read_int("V3RocketPauseFrames", out.v3.pause_frames);
        out.v3.tilt_frames = g.read_int("V3RocketTiltFrames", out.v3.tilt_frames);
        out.v3.damage = g.read_int("V3RocketDamage", out.v3.damage);
        out.v3.elite_damage = g.read_int("V3RocketEliteDamage", out.v3.elite_damage);
        out.dmisl.pause_frames = g.read_int("DMislPauseFrames", out.dmisl.pause_frames);
        out.dmisl.tilt_frames = g.read_int("DMislTiltFrames", out.dmisl.tilt_frames);
        out.dmisl.damage = g.read_int("DMislDamage", out.dmisl.damage);
        out.dmisl.elite_damage = g.read_int("DMislEliteDamage", out.dmisl.elite_damage);
        out.cmisl.damage = g.read_int("CMislDamage", out.cmisl.damage);
        out.cmisl.elite_damage = g.read_int("CMislEliteDamage", out.cmisl.elite_damage);
        // Every non-V3 family reads the DMisl slots in the manager's state-1
        // timer; keep CMisl's copy in lockstep so the timer is sourced from
        // one place.
        out.cmisl.pause_frames = out.dmisl.pause_frames;
        out.cmisl.tilt_frames = out.dmisl.tilt_frames;

        for (key, slot) in [
            ("V3Warhead", &mut out.v3.warhead),
            ("DMislWarhead", &mut out.dmisl.warhead),
            ("V3EliteWarhead", &mut out.v3.elite_warhead),
            ("DMislEliteWarhead", &mut out.dmisl.elite_warhead),
            ("CMislWarhead", &mut out.cmisl.warhead),
            ("CMislEliteWarhead", &mut out.cmisl.elite_warhead),
        ] {
            if let Some(name) = combat_damage.read_name(key, 0x80) {
                *slot = name.to_string();
            }
        }
        out
    }

    /// Which hardcoded family (if any) this child TechnoType belongs to.
    ///
    /// This is the Rust stand-in for gamemd's three pointer-equality tests
    /// against `Rules+0x4E0 / +0x514 / +0x548`.
    pub fn family_of(&self, type_name: &str) -> Option<MissileFamily> {
        if type_name.eq_ignore_ascii_case(&self.v3.type_name) {
            Some(MissileFamily::V3Rocket)
        } else if type_name.eq_ignore_ascii_case(&self.dmisl.type_name) {
            Some(MissileFamily::DMisl)
        } else if type_name.eq_ignore_ascii_case(&self.cmisl.type_name) {
            Some(MissileFamily::CMisl)
        } else {
            None
        }
    }

    /// Launch/impact parameters for a family.
    pub fn params(&self, family: MissileFamily) -> &MissileSpawnParams {
        match family {
            MissileFamily::V3Rocket => &self.v3,
            MissileFamily::DMisl => &self.dmisl,
            MissileFamily::CMisl => &self.cmisl,
        }
    }

    /// Frames a missile slot waits in the post-launch `KamikazeWait` state.
    ///
    /// `SpawnManagerClass::AI` reads the V3 pause+tilt slots only when the
    /// manager's spawn type is the V3 rocket type; every other family reads
    /// the DMisl slots. `MissileSpawnRules` keeps CMisl's copy of those two
    /// fields equal to DMisl's, so this is a straight lookup.
    /// A negative sum expires the native timer at once, as zero does.
    pub fn kamikaze_wait_frames(&self, family: MissileFamily) -> u32 {
        let p = self.params(family);
        p.pause_frames.saturating_add(p.tilt_frames).max(0) as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::ini_parser::IniFile;

    fn parse(text: &str) -> MissileSpawnRules {
        let ini = IniFile::from_str(text);
        MissileSpawnRules::from_ini_sections(
            ini.section_or_empty("General"),
            ini.section_or_empty("CombatDamage"),
        )
    }

    #[test]
    fn unset_type_slots_match_no_family() {
        let rules = MissileSpawnRules::default();
        assert_eq!(rules.family_of("V3ROCKET"), None);
        assert_eq!(rules.v3.damage, 1000);
        assert_eq!(rules.cmisl.elite_damage, 500);
    }

    #[test]
    fn parses_general_and_combat_damage() {
        let rules = parse(
            "[General]\n\
             V3RocketType=V3ROCKET\n\
             V3RocketPauseFrames=0\n\
             V3RocketTiltFrames=60\n\
             V3RocketDamage=200\n\
             V3RocketEliteDamage=400\n\
             DMislType=DMISL\n\
             DMislPauseFrames=20\n\
             DMislTiltFrames=60\n\
             DMislDamage=300\n\
             CMislType=CMISL\n\
             \n\
             [CombatDamage]\n\
             V3Warhead=V3WH\n\
             V3EliteWarhead=V3EWH\n\
             DMislWarhead=DMISLWH\n",
        );
        assert_eq!(rules.v3.damage, 200);
        assert_eq!(rules.v3.elite_damage, 400);
        assert_eq!(rules.v3.warhead, "V3WH");
        assert_eq!(rules.v3.warhead_for(200), "V3EWH");
        assert_eq!(rules.v3.damage_for(100), 200);
        assert_eq!(rules.v3.damage_for(200), 400);
        assert_eq!(rules.dmisl.damage, 300);
        assert_eq!(rules.dmisl.warhead, "DMISLWH");
    }

    #[test]
    fn kamikaze_wait_uses_dmisl_frames_for_cmisl() {
        // gamemd's state-1 timer reads the DMisl pause/tilt slots for every
        // family except V3 — including CMisl, whose own CMisl*Frames keys are
        // never consulted there.
        let rules = parse(
            "[General]\n\
             V3RocketPauseFrames=0\n\
             V3RocketTiltFrames=60\n\
             DMislPauseFrames=20\n\
             DMislTiltFrames=60\n\
             CMislPauseFrames=20\n\
             CMislTiltFrames=100\n",
        );
        assert_eq!(rules.kamikaze_wait_frames(MissileFamily::V3Rocket), 60);
        assert_eq!(rules.kamikaze_wait_frames(MissileFamily::DMisl), 80);
        assert_eq!(rules.kamikaze_wait_frames(MissileFamily::CMisl), 80);
    }

    #[test]
    fn custom_type_names_reroute_the_family_test() {
        let rules = parse("[General]\nV3RocketType=MYROCKET\n");
        assert_eq!(rules.family_of("MYROCKET"), Some(MissileFamily::V3Rocket));
        assert_eq!(rules.family_of("V3ROCKET"), None);
    }
}
