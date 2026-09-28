//! CRT `atoi`/`strtok` and ReadString/ReadInt/ReadBool against the shared
//! readers. Native rows: tools/rules_oracle/ini_token_readers.{py,json,meta.json}.

use super::ini_parser::IniSection;
use super::ini_value::{crt_atoi, strtok};

#[derive(Debug, serde::Deserialize)]
struct Native {
    atoi: Vec<AtoiRow>,
    strtok: Vec<StrtokRow>,
    read_string: Vec<ReadStringRow>,
    read_int: Vec<ReadIntRow>,
    read_bool: Vec<ReadBoolRow>,
}

#[derive(Debug, serde::Deserialize)]
struct AtoiRow {
    raw: String,
    output: i32,
}

#[derive(Debug, serde::Deserialize)]
struct StrtokRow {
    raw: String,
    tokens: Vec<String>,
}

#[derive(Debug, serde::Deserialize)]
struct ReadStringRow {
    raw: Option<String>,
    default: String,
    capacity: usize,
    output: String,
}

#[derive(Debug, serde::Deserialize)]
struct ReadIntRow {
    raw: Option<String>,
    default: i32,
    output: i32,
}

#[derive(Debug, serde::Deserialize)]
struct ReadBoolRow {
    raw: Option<String>,
    default: bool,
    output: bool,
}

fn native() -> Native {
    serde_json::from_str(include_str!(
        "../../tools/rules_oracle/ini_token_readers.json"
    ))
    .unwrap()
}

/// The oracle's cached entry holds the value exactly as given, as `set` does.
fn section(raw: Option<&str>) -> IniSection {
    let mut section = IniSection::new("TEST".to_string());
    if let Some(raw) = raw {
        section.set("Key", raw);
    }
    section
}

#[test]
fn crt_atoi_matches_original() {
    for row in native().atoi {
        assert_eq!(crt_atoi(&row.raw), row.output, "{row:?}");
    }
}

#[test]
fn strtok_matches_original() {
    for row in native().strtok {
        assert_eq!(
            strtok(&row.raw, &[',']).collect::<Vec<_>>(),
            row.tokens,
            "{row:?}"
        );
    }
}

#[test]
fn read_string_matches_original() {
    for row in native().read_string {
        let section = section(row.raw.as_deref());
        assert_eq!(
            section.read_string("Key", &row.default, row.capacity),
            row.output,
            "{row:?}"
        );
    }
}

#[test]
fn read_int_matches_original() {
    for row in native().read_int {
        let section = section(row.raw.as_deref());
        assert_eq!(section.read_int("Key", row.default), row.output, "{row:?}");
    }
}

#[test]
fn read_bool_matches_original() {
    for row in native().read_bool {
        let section = section(row.raw.as_deref());
        assert_eq!(section.read_bool("Key", row.default), row.output, "{row:?}");
    }
}
