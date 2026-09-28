//! Parsing for `[TIBTRE*]`-style terrain object types (rules.ini sections).
//!
//! Distinct from `terrain_rules` (which parses LAND types like Clear/Rough/Water).
//! These are per-object-type definitions for terrain decorations — currently
//! all terrain objects contribute their theater-selected occupation byte to
//! resolved terrain. Only TIBTRE (Tiberium Tree) fields are consumed by the
//! ore-spawner system.

use crate::rules::foundation;
use crate::rules::ini_parser::IniSection;

/// Type-class data for a terrain object (e.g. `[TIBTRE01]`).
///
/// Only the fields the sim needs; render-only fields (LightVisibility, tints,
/// IsFlammable) are intentionally not parsed.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TerrainObjectType {
    /// Section name, e.g. "TIBTRE01".
    pub name: String,
    /// `SpawnsTiberium=yes` — periodically spawns ore in adjacent cells.
    pub spawns_tiberium: bool,
    /// `IsAnimated=yes` — required gate for SpawnsTiberium logic.
    pub is_animated: bool,
    /// `AnimationRate=` (frames per anim step). Currently parsed but unused
    /// in sim — animation timing is collapsed to single-phase. Kept for
    /// future render-side use and to surface mod-tuning differences.
    pub animation_rate: u8,
    /// `AnimationProbability=` × 1_000_000, stored as integer micros.
    /// Used directly in the sim tick: `rng.next_range_u32(1_000_000) < this`.
    /// Avoids f32 in the hot path.
    pub animation_probability_micros: u32,
    /// Inherited `Armor=`. TerrainTypeClass constructor defaults to Wood.
    pub armor: String,
    /// Inherited `Strength=`. The constructor value and explicit -1 resolve
    /// through `[General] TreeStrength` after successful ObjectType ReadINI.
    pub strength: i32,
    /// Inherited `Immune=`. Stock TIBTRE sets this, blocking normal terrain damage.
    pub immune: bool,
    /// Inherited `LegalTarget=`. `IsVeinhole=yes` force-enables it in gamemd.
    pub legal_target: bool,
    /// Inherited `Insignificant=`. TerrainTypeClass constructor defaults true.
    pub insignificant: bool,
    /// Inherited `RadarInvisible=`. TerrainTypeClass constructor defaults true.
    pub radar_invisible: bool,
    /// `WaterBound=`.
    pub water_bound: bool,
    /// `IsVeinhole=`; when true, gamemd also forces `LegalTarget=true`.
    pub is_veinhole: bool,
    /// `TemperateOccupationBits=`.
    pub temperate_occupation_bits: u8,
    /// `SnowOccupationBits=`.
    pub snow_occupation_bits: u8,
    /// Art `Foundation=`. Defaults to 1x1 until art.ini merge patches it.
    pub foundation: String,
}

impl TerrainObjectType {
    pub fn from_ini_section(name: &str, section: &IniSection) -> Self {
        Self::from_ini_section_with_tree_strength(
            name,
            section,
            crate::rules::ruleset::GeneralRules::default().tree_strength,
        )
    }

    pub fn from_ini_section_with_tree_strength(
        name: &str,
        section: &IniSection,
        tree_strength: i32,
    ) -> Self {
        // ReadDouble -> `FSTP dword [ESI+0x2A4]` (`0x0071E073`) over the
        // constructor's zero (`0x0071DACD`).
        let probability_f = section.read_float("AnimationProbability", 0.0);
        let animation_probability_micros: u32 =
            (probability_f.clamp(0.0, 1.0) * 1_000_000.0).round() as u32;
        let is_veinhole = section.read_bool("IsVeinhole", false);
        // Terrain ctor71DBAC initializes Strength=-1. ObjectType5F94D3 reads
        // with that current default; Terrain71DEC8..71DEDC substitutes
        // Rules+1144 when the stored result is exactly -1, including an
        // authored -1. Original execution: tools/spatial_oracle/terrain_strength.
        let strength = section.read_int("Strength", -1);

        Self {
            name: name.to_string(),
            spawns_tiberium: section.read_bool("SpawnsTiberium", false),
            is_animated: section.read_bool("IsAnimated", false),
            animation_rate: section.read_int("AnimationRate", 0).clamp(0, 255) as u8,
            animation_probability_micros,
            // ObjectType `Armor=` (`0x005F94C8`): ReadString 0x80 ahead of the
            // armor-name lookup, over the constructor's Wood (`0x0071DBB6`).
            armor: section
                .read_name("Armor", 0x80)
                .unwrap_or("wood")
                .to_ascii_lowercase(),
            strength: if strength == -1 {
                tree_strength
            } else {
                strength
            },
            immune: section.read_bool("Immune", false),
            legal_target: section.read_bool("LegalTarget", false) || is_veinhole,
            insignificant: section.read_bool("Insignificant", true),
            radar_invisible: section.read_bool("RadarInvisible", true),
            water_bound: section.read_bool("WaterBound", false),
            is_veinhole,
            temperate_occupation_bits: section.read_int("TemperateOccupationBits", 7).clamp(0, 7)
                as u8,
            snow_occupation_bits: section.read_int("SnowOccupationBits", 7).clamp(0, 7) as u8,
            foundation: foundation::foundation_name("1x1").to_string(),
        }
    }

    pub fn merge_art_foundation(&mut self, foundation_name: &str) {
        self.foundation = foundation::foundation_name(foundation_name).to_string();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::ini_parser::IniFile;

    #[test]
    fn parse_tibtre_section_defaults() {
        let ini = IniFile::from_str(
            "[TIBTRE01]\nSpawnsTiberium=yes\nIsAnimated=yes\n\
             AnimationRate=3\nAnimationProbability=.003\n",
        );
        let section = ini.section("TIBTRE01").expect("section");
        let t = TerrainObjectType::from_ini_section("TIBTRE01", section);
        assert_eq!(t.name, "TIBTRE01");
        assert!(t.spawns_tiberium);
        assert!(t.is_animated);
        assert_eq!(t.animation_rate, 3);
        assert_eq!(t.animation_probability_micros, 3000);
        assert_eq!(t.armor, "wood");
        assert_eq!(
            t.strength,
            crate::rules::ruleset::GeneralRules::default().tree_strength
        );
        assert!(!t.immune);
        assert!(!t.legal_target);
        assert!(t.insignificant);
        assert!(t.radar_invisible);
        assert!(!t.water_bound);
        assert!(!t.is_veinhole);
        assert_eq!(t.temperate_occupation_bits, 7);
        assert_eq!(t.snow_occupation_bits, 7);
        assert_eq!(t.foundation, "1x1");
    }

    #[test]
    fn parse_non_spawning_terrain_section() {
        let ini = IniFile::from_str("[TREE01]\nIsAnimated=no\n");
        let section = ini.section("TREE01").expect("section");
        let t = TerrainObjectType::from_ini_section("TREE01", section);
        assert!(!t.spawns_tiberium);
        assert!(!t.is_animated);
        assert_eq!(t.animation_probability_micros, 0);
    }

    #[test]
    fn animation_probability_clamps_above_one() {
        let ini = IniFile::from_str("[X]\nAnimationProbability=2.5\n");
        let section = ini.section("X").expect("section");
        let t = TerrainObjectType::from_ini_section("X", section);
        assert_eq!(t.animation_probability_micros, 1_000_000);
    }

    #[test]
    fn parse_tibtre_lifecycle_fields_and_tree_strength_fallback() {
        let ini = IniFile::from_str(
            "[TIBTRE03]\nSpawnsTiberium=yes\nIsAnimated=yes\nImmune=yes\n\
             Armor=None\nIsVeinhole=true\nTemperateOccupationBits=4\nSnowOccupationBits=7\n",
        );
        let section = ini.section("TIBTRE03").expect("section");
        let t = TerrainObjectType::from_ini_section_with_tree_strength("TIBTRE03", section, 375);

        assert_eq!(t.strength, 375);
        assert_eq!(t.armor, "none");
        assert!(t.immune);
        assert!(t.is_veinhole);
        assert!(t.legal_target);
        assert_eq!(t.temperate_occupation_bits, 4);
        assert_eq!(t.snow_occupation_bits, 7);
    }

    #[test]
    fn gsi_04_10_strength_preserves_signed_explicit_and_fallback_values() {
        for (body, tree_strength, expected) in [
            ("AnimationRate=0\n", -375, -375),
            ("Strength=0\n", 200, 0),
            ("Strength=-27\n", 200, -27),
            ("Strength=450\n", -375, 450),
        ] {
            let ini = IniFile::from_str(&format!("[TREE01]\n{body}"));
            let section = ini.section("TREE01").expect("section");
            let terrain = TerrainObjectType::from_ini_section_with_tree_strength(
                "TREE01",
                section,
                tree_strength,
            );
            assert_eq!(terrain.strength, expected, "body={body:?}");
        }
    }

    #[test]
    fn bridge_tree_strength_reader_matches_original_constructor_and_fallback() {
        let corpus: serde_json::Value = serde_json::from_str(include_str!(
            "../../tools/spatial_oracle/terrain_strength.json"
        ))
        .unwrap();
        for row in corpus["cases"].as_array().unwrap() {
            // This test supplies a present Terrain section, so ObjectType's
            // absent-section refusal is native characterization only.
            if row["read_success"] == false {
                continue;
            }
            let raw = row["raw"].as_str();
            let strength = raw
                .map(|raw| format!("Strength={raw}\n"))
                .unwrap_or_default();
            let ini = IniFile::from_str(&format!(
                "[General]\nTreeStrength={}\n[TerrainTypes]\n0=TREE01\n[TREE01]\nFixture=1\n{strength}",
                row["tree_strength"].as_i64().unwrap()
            ));
            let rules = crate::rules::ruleset::RuleSet::from_ini(&ini).unwrap();
            let terrain = rules
                .terrain_object_type_case_insensitive("TREE01")
                .unwrap();
            assert_eq!(
                i64::from(terrain.strength),
                row["strength"].as_i64().unwrap(),
                "{row}"
            );
        }

        let Some(retail) = crate::rules::retail_ini_fixture::retail_ini("rulesmd.ini") else {
            return;
        };
        let rules = crate::rules::ruleset::RuleSet::from_ini(&retail).unwrap();
        let general = retail
            .section("General")
            .unwrap()
            .read_int("TreeStrength", 0);
        let raw = retail.section("TREE01").unwrap().get_for_test("Strength");
        let row = corpus["cases"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| {
                row["read_success"] == true
                    && row["tree_strength"] == general
                    && row["raw"].as_str() == raw
            })
            .expect("the native corpus must include the actual retail TREE01 inputs");
        assert_eq!(
            i64::from(
                rules
                    .terrain_object_type_case_insensitive("TREE01")
                    .unwrap()
                    .strength
            ),
            row["strength"].as_i64().unwrap()
        );
    }

    #[test]
    fn art_foundation_is_normalized() {
        let ini = IniFile::from_str("[TREE01]\nFixtureOnly=1\n");
        let section = ini.section("TREE01").expect("section");
        let mut t = TerrainObjectType::from_ini_section("TREE01", section);
        t.merge_art_foundation("2x2");
        assert_eq!(t.foundation, "2x2");
    }
}
