//! Map cell-tag parsing.
//!
//! `[CellTags]` binds map cells to tag identifiers. In RA2/YR this lets later
//! trigger logic attach events/actions to authored locations on the map.

use std::collections::HashMap;

use crate::rules::ini_parser::IniFile;

/// Cell coordinate -> tag id mapping parsed from `[CellTags]`.
pub type CellTagMap = HashMap<(u16, u16), String>;

/// Parse `[CellTags]`.
///
/// Keys use the standard packed-cell encoding `ry * 1000 + rx`.
/// Values are tag identifiers that later resolve through `[Tags]`.
pub fn parse_cell_tags(ini: &IniFile) -> CellTagMap {
    let Some(section) = ini.section("CellTags") else {
        return HashMap::new();
    };

    let mut tags: CellTagMap = HashMap::new();
    // `Read_Map_Section_And_IsoMapPacks @ 0x004AD1CF`: the tag a 0x80-byte
    // ReadString, the cell `atoi` of the entry name. Rust skips a negative
    // cell.
    for key in section.keys() {
        let Ok(packed) = u32::try_from(crate::rules::ini_value::crt_atoi(key)) else {
            continue;
        };
        let Some(tag_id) = section.read_name(key, 0x80) else {
            continue;
        };
        let ry = (packed / 1000) as u16;
        let rx = (packed % 1000) as u16;
        tags.insert((rx, ry), tag_id.to_string());
    }

    if !tags.is_empty() {
        log::info!("Parsed {} cell tags from [CellTags]", tags.len());
    }
    tags
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_cell_tags() {
        let ini = IniFile::from_str("[CellTags]\n35042=TRIG_A\n72015=OBJ_01\n");
        let tags = parse_cell_tags(&ini);
        assert_eq!(tags.get(&(42, 35)).map(String::as_str), Some("TRIG_A"));
        assert_eq!(tags.get(&(15, 72)).map(String::as_str), Some("OBJ_01"));
    }

    #[test]
    fn test_missing_cell_tags_is_empty() {
        let ini = IniFile::from_str("[Map]\nTheater=TEMPERATE\n");
        assert!(parse_cell_tags(&ini).is_empty());
    }
}
