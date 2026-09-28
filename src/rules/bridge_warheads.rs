//! `[CombatDamage]` fixed warhead references used by bridge/combat routing.
//!
//! Bridge damage gating + collapse fallout cite two specific warhead names by
//! [CombatDamage] key:
//! - IonCannonWarhead: bypasses BridgeStrength RNG gate; enables 3-retry loop in
//!   Apply_area_damage (gamemd Rules+0xFF0).
//! - C4Warhead: used as the killing warhead in BlowUpBridge ground-occupant
//!   force_kill (gamemd Rules+0xFA8).
//! - CrushWarhead: admits the source object to Apply_area_damage's ground
//!   receiver list even when its ObjectType has DamageSelf=no (Rules+0xFAC).
//!
//! Stored as raw INI strings here; resolved to interned `WarheadId`s at world
//! init time when the warhead registry is populated.
//!
//! ## Dependency rules
//! - Part of rules/ — no dependencies on sim/, render/, ui/, etc.

use crate::rules::ini_parser::IniSection;

/// Default warhead names for bridge-related combat from `[CombatDamage]`.
///
/// Each field is the section name of a `WarheadType` (resolved later against
/// `RuleSet::warhead_id_by_name`). Defaults match retail rulesmd.ini.
#[derive(Debug, Clone)]
pub struct BridgeWarheads {
    /// `[CombatDamage] IonCannonWarhead=` (default `"IonCannonWH"`).
    /// Bypasses BridgeStrength RNG gate; enables retry loop.
    pub ion_cannon_name: String,
    /// `[CombatDamage] C4Warhead=` (default `"Super"`).
    /// Used as killing warhead in bridge-collapse ground kill.
    pub c4_name: String,
    /// `[CombatDamage] CrushWarhead=` (default `"Crush"`). Distinct from C4.
    pub crush_name: String,
}

impl Default for BridgeWarheads {
    fn default() -> Self {
        Self {
            ion_cannon_name: "IonCannonWH".to_string(),
            c4_name: "Super".to_string(),
            crush_name: "Crush".to_string(),
        }
    }
}

impl BridgeWarheads {
    /// Parse from a `[CombatDamage]` `IniSection`. Missing keys use defaults.
    pub fn from_ini_section(section: &IniSection) -> Self {
        let default = Self::default();
        // ReadString 0x80 ahead of each warhead lookup (`0x0066CA96`,
        // `0x0066C327`, `0x0066C365`).
        let name = |key: &str, default: String| {
            section.read_name(key, 0x80).map_or(default, str::to_string)
        };
        Self {
            ion_cannon_name: name("IonCannonWarhead", default.ion_cannon_name),
            c4_name: name("C4Warhead", default.c4_name),
            crush_name: name("CrushWarhead", default.crush_name),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::ini_parser::IniFile;

    #[test]
    fn defaults_match_retail_rulesmd() {
        let bw = BridgeWarheads::default();
        assert_eq!(bw.ion_cannon_name, "IonCannonWH");
        assert_eq!(bw.c4_name, "Super");
        assert_eq!(bw.crush_name, "Crush");
    }

    #[test]
    fn parses_keys_from_combat_damage_section() {
        let ini = IniFile::from_str(
            "[CombatDamage]\nIonCannonWarhead=CustomIon\nC4Warhead=CustomC4\nCrushWarhead=CustomCrush\n",
        );
        let section = ini.section("CombatDamage").unwrap();
        let bw = BridgeWarheads::from_ini_section(section);
        assert_eq!(bw.ion_cannon_name, "CustomIon");
        assert_eq!(bw.c4_name, "CustomC4");
        assert_eq!(bw.crush_name, "CustomCrush");
    }

    #[test]
    fn missing_keys_fall_back_to_defaults() {
        let ini = IniFile::from_str("[CombatDamage]\nFixtureOnly=1\n");
        let section = ini.section("CombatDamage").unwrap();
        let bw = BridgeWarheads::from_ini_section(section);
        assert_eq!(bw.ion_cannon_name, "IonCannonWH");
        assert_eq!(bw.c4_name, "Super");
        assert_eq!(bw.crush_name, "Crush");
    }
}
