//! Read-only INI inspection through production source selection and readers.
//!
//! Results describe an accessor on the retained compatibility projection, not
//! arbitrary final gameplay fields. Type constructors and post-passes remain
//! owned by rules processing; this tool never recreates them.

use std::path::Path;

use serde_json::{Value, json};

use crate::asset_tools::report::ErrorReport;
use crate::assets::asset_manager::AssetManager;
use crate::rules::ini_parser::{IniFile, IniSection};
use crate::rules::retail_sources::{RetailRulesSources, select_ini};

#[derive(Debug, Default)]
pub struct IniOptions {
    pub domain: Option<String>,
    pub reader: Option<String>,
    pub default: Option<String>,
    pub capacity: Option<usize>,
    pub map: Option<String>,
    pub mode_id: Option<i32>,
}

impl IniOptions {
    pub fn validate(&self) -> Result<(), String> {
        match self.domain.as_deref() {
            Some("rules") => {
                if self.map.as_deref().is_none_or(str::is_empty) || self.mode_id.is_none() {
                    return Err(
                        "ini-get --domain rules requires explicit --map and --mode-id".into(),
                    );
                }
            }
            Some("art") => {
                if self.map.is_some() || self.mode_id.is_some() {
                    return Err(
                        "ARTMD is fixed, not scenario-layered; omit --map and --mode-id".into(),
                    );
                }
            }
            _ => return Err("ini-get requires --domain rules|art".into()),
        }
        let reader = self.reader.as_deref().ok_or("ini-get requires --reader")?;
        if !matches!(
            reader,
            "raw" | "int" | "bool" | "double" | "string" | "range" | "speed" | "coord"
        ) {
            return Err("--reader must be raw|int|bool|double|string|range|speed|coord".into());
        }
        if reader == "raw" && self.default.is_some() {
            return Err("--default is not valid for the raw reader".into());
        }
        if reader != "raw" && self.default.is_none() {
            return Err(
                "typed readers require an explicit --default from the caller contract".into(),
            );
        }
        if (reader == "string") != self.capacity.is_some() {
            return Err("--capacity is required only for the string reader".into());
        }
        // Validate CLI values before mounting retail assets. These are supplied
        // typed defaults, not INI tokens, so strict CLI parsing is intentional.
        accessor(&IniFile::empty(), "", "", self)?;
        Ok(())
    }
}

fn cli_i32(text: &str) -> Result<i32, String> {
    text.parse()
        .map_err(|_| "--default requires a signed decimal i32 for this reader".into())
}

fn float_result(value: f64) -> Value {
    json!({
        "value": if value.is_finite() { json!(value) } else { Value::Null },
        "representation": value.to_string(),
        "binary64_bits": format!("0x{:016x}", value.to_bits()),
    })
}

fn accessor(
    ini: &IniFile,
    section: &str,
    key: &str,
    options: &IniOptions,
) -> Result<Value, String> {
    let empty = IniSection::new(section.to_owned());
    let section = ini.section(section).unwrap_or(&empty);
    let default = options.default.as_deref().unwrap_or("");
    Ok(match options.reader.as_deref() {
        Some("raw") => json!(raw_value(section, key)),
        Some("int") => json!(section.read_int(key, cli_i32(default)?)),
        Some("bool") => {
            let default = default
                .parse::<bool>()
                .map_err(|_| "--default for bool must be true or false")?;
            json!(section.read_bool(key, default))
        }
        Some("double") => {
            let default = default
                .parse::<f64>()
                .map_err(|_| "--default for double must be a binary64 number")?;
            float_result(section.read_double(key, default))
        }
        Some("string") => json!(
            section.read_string(
                key,
                default,
                options
                    .capacity
                    .ok_or("string reader requires --capacity")?
            )
        ),
        Some("range") => json!(section.read_range(key, cli_i32(default)?)),
        Some("speed") => json!(section.read_speed(key, cli_i32(default)?)),
        Some("coord") => {
            let values = default
                .split(',')
                .map(cli_i32)
                .collect::<Result<Vec<_>, _>>()?;
            let values: [i32; 3] = values
                .try_into()
                .map_err(|_| "--default for coord must be three signed decimal integers: x,y,z")?;
            json!(section.read_coord3(key, values))
        }
        _ => return Err("ini-get requires a supported --reader".into()),
    })
}

/// The stored text, shown as it is: the tool's raw view, not a reader.
fn raw_value<'a>(section: &'a IniSection, key: &str) -> Option<&'a str> {
    section
        .raw_entries()
        .find_map(|(entry, value)| (entry == key).then_some(value))
}

fn presence(ini: &IniFile, section: &str, key: &str) -> Value {
    let selected = ini.section(section);
    json!({
        "section_present": selected.is_some(),
        "key_present": selected.is_some_and(|s| s.is_present(key)),
        "raw_value": selected.and_then(|s| raw_value(s, key)),
    })
}

fn layer(kind: &str, source: Value, ini: Option<&IniFile>, section: &str, key: &str) -> Value {
    json!({
        "layer": kind,
        "source": source,
        "source_present": ini.is_some(),
        "authored": ini.map(|ini| presence(ini, section, key)),
    })
}

fn assemble(
    ini: &IniFile,
    section: &str,
    key: &str,
    options: &IniOptions,
    layers: Vec<Value>,
) -> Result<Value, String> {
    Ok(json!({
        "schema_version": 1,
        "domain": options.domain,
        "section": section,
        "key": key,
        "lookup": "exact_case",
        "reader": options.reader,
        "supplied_default": options.default,
        "string_capacity_bytes": options.capacity,
        "authored_layers": layers,
        "processed": presence(ini, section, key),
        "accessor_result": accessor(ini, section, key, options)?,
        "result_kind": "production_ini_accessor",
        "limits": [
            "An accessor result is not a final gameplay field: constructors, type allocation timing, field-specific defaults/clamps, art indirection and post-read passes may change it.",
            "Rules inspection executes the supported noncampaign skirmish path with the special TMCJ4F flag absent; campaign passes, LANGRULE Digest handling and that flagged extra pass are not modeled.",
            "Authored values reflect the production byte parser: empty values are omitted and duplicate-key resolution retains its documented stock-inert residual.",
            "Reader residuals remain: malformed ReadDouble/coord values use deterministic Rust recovery; ReadRange overflow differs from native. No new native equivalence is claimed."
        ],
    }))
}

pub fn run(
    assets: &mut AssetManager,
    retail_dir: &Path,
    section: &str,
    key: &str,
    options: &IniOptions,
) -> Result<Value, ErrorReport> {
    run_inner(assets, retail_dir, section, key, options).map_err(|error| ErrorReport { error, hint: Some("Use asset ini-get --help; typed results require the native caller's default and reader.".into()) })
}

fn run_inner(
    assets: &mut AssetManager,
    retail_dir: &Path,
    section: &str,
    key: &str,
    options: &IniOptions,
) -> Result<Value, String> {
    options.validate()?;
    if options.domain.as_deref() == Some("art") {
        let art = select_ini(assets, "artmd.ini")?;
        let layers = vec![layer(
            "fixed_art",
            json!(art.source),
            Some(&art.ini),
            section,
            key,
        )];
        return assemble(&art.ini, section, key, options, layers);
    }
    let audio_definitions = crate::rules::audio_sources::AudioDefinitions::select(assets);
    let sources = RetailRulesSources::select(assets)?;
    // The app retains these registrations between cold Rules selection and
    // roster/map lookup. A shell-discovered YRO may supply either later input.
    // Preserve the app's tolerant failure policy and partially registered VFS.
    if let Err(error) = assets.register_neutral_archives() {
        log::warn!("Could not register retail neutral shell archives: {error:#}");
    }
    if let Err(error) = crate::map::scenario_sources::list_skirmish_scenario_records_with_assets(
        retail_dir, assets, None,
    ) {
        log::warn!("Could not enumerate retail skirmish scenarios: {error:#}");
    }
    let roster = select_ini(assets, "MPModesMD.ini")?;
    let modes = crate::skirmish_modes::skirmish_modes_from_selected_ini(
        assets,
        &roster.ini,
        &roster.source.source_archive,
    )
    .map_err(|e| e.to_string())?;
    let id = options.mode_id.expect("validated mode id");
    let mode = crate::skirmish_modes::mode_by_id(&modes, id)
        .ok_or_else(|| format!("mode id {id} is absent from MPModesMD.ini"))?;
    if !mode.class.lists_in_skirmish() {
        return Err(format!(
            "mode id {id} is not in the supported noncampaign skirmish list"
        ));
    }
    let map = crate::map::source::load_map_by_name_or_path_with_assets(
        retail_dir,
        options.map.as_deref().expect("validated map"),
        assets,
    )
    .map_err(|e| e.to_string())?;
    // App match loading activates theater archives after selecting startup
    // sources and before resolving the mode override. Follow that route;
    // headless's older pre-theater mode lookup is not the claimed path.
    crate::map::theater::load_theater(assets, &map.map.header.theater).ok_or_else(|| {
        format!(
            "load theater {} for app rule-source selection",
            map.map.header.theater
        )
    })?;
    let selected_mode = if mode.override_file.trim().is_empty() {
        None
    } else {
        Some(select_ini(assets, mode.override_file.trim())?)
    };
    let layers = vec![
        layer(
            "rulesmd",
            json!(sources.rulesmd.source),
            Some(&sources.rulesmd.ini),
            section,
            key,
        ),
        layer(
            "langrule",
            sources
                .langrule
                .as_ref()
                .map_or(Value::Null, |ini| json!(ini.source)),
            sources.langrule.as_ref().map(|ini| &ini.ini),
            section,
            key,
        ),
        layer(
            "game_mode",
            selected_mode
                .as_ref()
                .map_or(Value::Null, |ini| json!(ini.source)),
            selected_mode.as_ref().map(|ini| &ini.ini),
            section,
            key,
        ),
        layer(
            "scenario",
            json!(map.source),
            Some(&map.map.ini),
            section,
            key,
        ),
    ];
    // Retain provenance before moving the selected snapshots into their owner.
    let fixed_art_source = json!(sources.artmd.source);
    let fixed_sound_source = audio_definitions.sound_source().map(|source| json!(source));
    let (_, _, mut owner) = sources
        .into_startup(std::sync::Arc::clone(audio_definitions.sounds()))?
        .into_parts();
    let (_, processed, _, _) = owner
        .load_noncampaign_scenario(selected_mode.as_ref().map(|ini| &ini.ini), &map.map.ini)
        .map_err(|e| e.to_string())?
        .into_parts();
    let mut report = assemble(&processed, section, key, options, layers)?;
    report["mode_roster_source"] = json!(roster.source);
    report["mode"] = json!({"id": mode.id, "class": format!("{:?}", mode.class), "override_file": mode.override_file});
    report["source_selection_route"] =
        json!("app_noncampaign_startup_then_shell_registration_then_map_then_theater_then_mode");
    report["fixed_art_source"] = fixed_art_source;
    report["fixed_sound_source"] = json!(fixed_sound_source);
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::process_owner::NativeRulesProcessOwner;

    fn options(reader: &str, default: &str) -> IniOptions {
        IniOptions {
            domain: Some("art".into()),
            reader: Some(reader.into()),
            default: (reader != "raw").then(|| default.into()),
            ..Default::default()
        }
    }

    #[test]
    fn absent_sections_still_use_native_string_default_capacity_and_trim() {
        let mut query = options("string", "  default  ");
        query.capacity = Some(6);
        assert_eq!(
            accessor(&IniFile::empty(), "missing", "key", &query).unwrap(),
            "def"
        );
    }

    #[test]
    fn exact_case_and_omitted_empty_values_remain_visible() {
        let ini = IniFile::from_str("[Case]\nKey=42\nEmpty=\n");
        assert_eq!(presence(&ini, "Case", "key")["key_present"], false);
        assert_eq!(presence(&ini, "Case", "Key")["raw_value"], "42");
        assert_eq!(presence(&ini, "Case", "Empty")["key_present"], false);
        assert_eq!(
            accessor(&ini, "Case", "Empty", &options("int", "73")).unwrap(),
            73
        );
    }

    #[test]
    fn nonfinite_double_keeps_bits_in_valid_json() {
        let result = accessor(
            &IniFile::from_str("[S]\nK=1e39%\n"),
            "S",
            "K",
            &options("double", "0"),
        )
        .unwrap();
        assert_eq!(result["value"], Value::Null);
        assert_eq!(result["binary64_bits"], "0x7ff0000000000000");
        serde_json::from_str::<Value>(&serde_json::to_string(&result).unwrap()).unwrap();
    }

    #[test]
    fn accessor_keeps_layered_default_retention() {
        let mut owner = NativeRulesProcessOwner::from_cold_start_sources(
            IniFile::from_str("[General]\nTreeStrength=42\n"),
            None,
            IniFile::empty(),
            std::sync::Arc::default(),
        )
        .unwrap();
        let (_, processed, _, _) = owner
            .load_noncampaign_scenario(None, &IniFile::from_str("[General]\nTreeStrength=$xyz\n"))
            .unwrap()
            .into_parts();
        assert_eq!(
            accessor(&processed, "General", "TreeStrength", &options("int", "7")).unwrap(),
            42
        );
        assert_eq!(
            presence(&processed, "General", "TreeStrength")["raw_value"],
            "$xyz"
        );
    }

    #[test]
    fn raw_weapon_speed_is_not_claimed_as_final_postpass_field() {
        // Same production reader/postpass as rules::weapon_speed_tests; no
        // replacement implementation of the projectile-dependent calculation.
        let root = IniFile::from_str(
            "[VehicleTypes]\n0=UNIT\n[UNIT]\nPrimary=GUN\n[GUN]\nSpeed=40\nRange=5\nProjectile=SHOT\n[SHOT]\nROT=0\n",
        );
        let authored = presence(&root, "GUN", "Speed");
        let mut owner = NativeRulesProcessOwner::from_cold_start_sources(
            root,
            None,
            IniFile::empty(),
            std::sync::Arc::default(),
        )
        .unwrap();
        let (rules, processed, _, _) = owner
            .load_noncampaign_scenario(None, &IniFile::empty())
            .unwrap()
            .into_parts();
        let report = assemble(&processed, "GUN", "Speed", &options("raw", ""), vec![]).unwrap();
        assert_eq!(authored["raw_value"], "40");
        assert_eq!(report["accessor_result"], "40");
        assert_ne!(rules.weapon("GUN").unwrap().speed, 40);
        assert_eq!(report["result_kind"], "production_ini_accessor");
        assert!(
            report["limits"][0]
                .as_str()
                .unwrap()
                .contains("not a final gameplay field")
        );
    }
    #[test]
    #[ignore = "requires retail YR installation containing RiverRam.yro; run explicitly"]
    fn retail_yro_scenario_inspection_uses_retained_discovery() {
        let retail = std::path::PathBuf::from(std::env::var("RA2_DIR").expect("set RA2_DIR"));
        let mut assets =
            crate::asset_tools::root::open_manager(&retail, false).expect("retail manager");
        let query = IniOptions {
            domain: Some("rules".into()),
            reader: Some("speed".into()),
            default: Some("0".into()),
            map: Some("RiverRam.MAP".into()),
            mode_id: Some(1),
            ..Default::default()
        };
        let report = run_inner(&mut assets, &retail, "HoverMissile", "Speed", &query)
            .expect("inspect map registered by production scenario discovery");
        assert_eq!(report["accessor_result"], 102);
        let source = &report["authored_layers"][3]["source"];
        assert_eq!(source["kind"], "mix");
        assert!(
            source["source_archive"]
                .as_str()
                .unwrap()
                .eq_ignore_ascii_case("RiverRam.yro")
        );
        let selected = assets
            .resolve_ref("RiverRam.MAP")
            .expect("retained scenario archive");
        assert_eq!(
            source["source_sha256"],
            crate::util::sha256::sha256_hex(selected.bytes)
        );
    }
}
