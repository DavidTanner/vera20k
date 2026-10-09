//! Original candidate geometry observations, not a second search implementation.
use super::*;
use serde_json::Value;

#[test]
fn original_approach_candidate_geometry_and_order() {
    let native: Value = serde_json::from_str(crate::test_fixture::text(
        "tools/spatial_oracle/fv_cell_attack/approach_vectors.json",
    ))
    .unwrap();
    assert_eq!(
        native["native_sha256"],
        "1cdd1180e49024fbda8ad568caac2e86e856063ff67ab38f62b7d2c7bb84298c"
    );
    let original_offsets = native["original_constants"]
        .as_array()
        .unwrap()
        .iter()
        .find_map(|row| row.get("signed_offsets"))
        .unwrap();
    assert_eq!(serde_json::json!(OFFSETS), *original_offsets);
    for (i, offset) in native["numeric"]["neighbor_offsets"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
    {
        assert_eq!(serde_json::json!(cell_delta_unchecked(i as u8)), *offset);
    }
    let mut count = 0;
    for pose in native["numeric"]["poses"].as_array().unwrap() {
        let source: [i32; 2] = serde_json::from_value(pose["source_xy"].clone()).unwrap();
        let target: [i32; 2] = serde_json::from_value(pose["target_xy"].clone()).unwrap();
        let facing = facing16_between(target, source);
        assert_eq!(u64::from(facing), pose["facing_word"].as_u64().unwrap());
        let base = ((u32::from(facing) + 128) >> 8) as u8;
        assert_eq!(u64::from(base), pose["base_direction"].as_u64().unwrap());
        for row in pose["candidates"].as_array().unwrap() {
            let offset = row["offset"].as_i64().unwrap() as i16;
            let direction = (i16::from(base).wrapping_add(offset) as u16) << 8;
            let radius = row["radius"].as_i64().unwrap() as i32;
            let actual = facing_step_world_xy(target, direction, radius);
            let expected: [i32; 3] = serde_json::from_value(row["unsnapped_xyz"].clone()).unwrap();
            assert_eq!(
                actual,
                [expected[0], expected[1]],
                "source={source:?} target={target:?} offset={offset} radius={radius}"
            );
            count += 1;
        }
    }
    assert_eq!(count, 465);
}

/// Retail rulesmd.ini through the production reader: the Kirov (`ZEP`) and
/// the Floating Disc (`DISK`) are `BalloonHover=`; the Kirov's bombs, rookie
/// and elite, and the Disc's drain fall `Vertical=`, so the arm sends them
/// over their Target; the Disc's laser does not, so it goes on to the Foot
/// approach.
#[test]
fn retail_balloons_take_their_target_with_vertical_weapons() {
    use crate::sim::combat::combat_weapon::{primary_for_tier, secondary_for_tier};
    use crate::sim::combat::veterancy::RANK_ELITE_U16;
    let Some((rules_ini, art_ini)) = crate::rules::retail_ini_fixture::retail_rules_and_art()
    else {
        return;
    };
    let rules = RuleSet::from_ini_with_fixed_art_for_test(&rules_ini, &art_ini).unwrap();
    let zep = rules.object("ZEP").unwrap();
    let disk = rules.object("DISK").unwrap();
    assert!(zep.balloon_hover && disk.balloon_hover);
    for (weapon, vertical) in [
        (primary_for_tier(zep, 0), true),
        (primary_for_tier(zep, RANK_ELITE_U16), true),
        (primary_for_tier(disk, 0), false),
        (secondary_for_tier(disk, 0), true),
    ] {
        let weapon = rules.weapon(weapon.unwrap()).unwrap();
        assert_eq!(
            balloon_hover_destination(Some(weapon), TargetKind::Entity(7), &rules),
            vertical.then_some(NavTargetRef::object(7)),
            "{}",
            weapon.id
        );
    }
}
