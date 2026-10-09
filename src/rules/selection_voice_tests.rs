//! VoiceSelect's production fixed-catalog binding and reached Rules passes.
//! Reader525430 / TechnoReadINI712B1D..712B87; constructor710D74..710D9A.

use std::sync::Arc;

use super::NativeRulesProcessOwner;
use crate::rules::ini_parser::IniFile;
use crate::rules::sound_ini::SoundRegistry;

fn catalog() -> Arc<SoundRegistry> {
    Arc::new(SoundRegistry::from_ini(&IniFile::from_str(
        "[SoundList]\n0=First\n1=Second\n2=Third\n[NotRegistered]\nSounds=sample\n",
    )))
}

#[test]
fn voice_select_keeps_the_full_ordered_resolved_list_across_reached_passes() {
    let ini = IniFile::from_str;
    let root = ini(
        "[InfantryTypes]\n0=KEEP\n1=REPLACE\n2=CLEAR\n3=DELIMITERS\n4=DEFAULT\n\
         [KEEP]\nVoiceSelect=First,Unknown,Second,First\n\
         [REPLACE]\nVoiceSelect=First\n\
         [CLEAR]\nVoiceSelect=First\n\
         [DELIMITERS]\nVoiceSelect=First\n\
         [DEFAULT]\nVoiceSelectWrong=First\n\
         [LATE]\nVoiceSelect=First\n",
    );
    let lang = ini("[KEEP]\nVoiceSelect=\nvoiceselect=Third\n\
         [REPLACE]\nVoiceSelect=Second\n\
         [LATE]\nVoiceSelect=Second\n");
    let mode = ini("[InfantryTypes]\n5=LATE\n\
         [KEEP]\nVoiceSelect= \n\
         [REPLACE]\nVoiceSelect=Third,sEcOnD,Third\n\
         [CLEAR]\nVoiceSelect=Unknown,NotRegistered\n\
         [DELIMITERS]\nVoiceSelect=,,,\n");
    let map = ini("[KEEP]\nvoiceselect=Third\n\
         [REPLACE]\nVoiceSelect=\n\
         [CLEAR]\nVoiceSelect=\n\
         [DELIMITERS]\nVoiceSelect=\n");
    let sounds = catalog();
    let mut owner = NativeRulesProcessOwner::from_cold_start_sources(
        root,
        Some(lang),
        ini(""),
        Arc::clone(&sounds),
    )
    .unwrap();
    let (rules, _, _, _) = owner
        .load_noncampaign_scenario(Some(&mode), &map)
        .unwrap()
        .into_parts();
    assert!(Arc::ptr_eq(owner.fixed_sounds(), &sounds));
    assert_eq!(
        rules.object("KEEP").unwrap().voice_select,
        ["First", "Second", "First"]
    );
    assert_eq!(
        rules.object("REPLACE").unwrap().voice_select,
        ["Third", "Second", "Third"]
    );
    for name in ["CLEAR", "DELIMITERS", "DEFAULT", "LATE"] {
        assert!(
            rules.object(name).unwrap().voice_select.is_empty(),
            "{name}"
        );
    }
}

#[test]
fn voice_select_readstring128_and_untrimmed_tokens_precede_sound_lookup() {
    let sounds = catalog();
    let mut owner = NativeRulesProcessOwner::from_cold_start_sources(
        IniFile::from_str(&format!(
            "[VehicleTypes]\n0=SPACES\n1=BOUNDARY\n\
             [SPACES]\nVoiceSelect=First, Second,Second ,sEcOnD,,First\n\
             [BOUNDARY]\nVoiceSelect={},First,SecondX\n",
            "x".repeat(114),
        )),
        None,
        IniFile::from_str(""),
        sounds,
    )
    .unwrap();
    let (rules, _, _, _) = owner
        .load_noncampaign_scenario(None, &IniFile::from_str(""))
        .unwrap()
        .into_parts();
    assert_eq!(
        rules.object("SPACES").unwrap().voice_select,
        ["First", "Second", "First"]
    );
    // ReadString's 127 payload bytes retain Second but cut its final X;
    // the long unresolved token is discarded only after tokenization.
    assert_eq!(
        rules.object("BOUNDARY").unwrap().voice_select,
        ["First", "Second"]
    );
}
