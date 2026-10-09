//! Native signed-frame producer controls. Scalable list ownership is separate.
use super::*;
use crate::rules::{ini_parser::IniFile, projectile_type::ProjectileType};
use serde_json::{Value, json};

#[test]
fn projectile_trailer_launch_placement_matches_original_bullet_fire() {
    use crate::map::resolved_terrain::{test_flat_cell, test_grid};
    use crate::sim::world::Simulation;
    use crate::util::native_x87::NativeF64Bits;

    let Some(retail) = crate::rules::retail_ini_fixture::retail_battle_rules() else {
        return;
    };
    let kind = retail.rules.projectile("Torpedo").unwrap();
    let corpus: Value = serde_json::from_str(crate::test_fixture::text(
        "tools/projectile_oracle/projectile_trailer.json",
    ))
    .unwrap();
    let point = |value: &Value| {
        ProjectileCoord::new(
            value[0].as_i64().unwrap() as i32,
            value[1].as_i64().unwrap() as i32,
            value[2].as_i64().unwrap() as i32,
        )
    };
    let world = |input: &Value| {
        let mut sim = Simulation::new();
        sim.install_resolved_terrain_for_new_map(test_grid(25, 25, |x, y| {
            let mut cell = test_flat_cell(x, y);
            cell.level = input["level"].as_u64().unwrap() as u8;
            cell.slope_type = input["slope"].as_u64().unwrap() as u8;
            if input["live_bridge"].as_bool().unwrap() {
                cell.bridge_facts.raw_flags = 0x100;
            }
            cell
        }));
        sim
    };
    for row in corpus["placement_controls"]["fixup"].as_array().unwrap() {
        let input = &row["supplied"];
        let sim = world(input);
        assert_eq!(
            bullet_unlimbo_coord(
                sim.resolved_terrain.as_ref(),
                &sim.effective_shared_cell_dummy(),
                point(&input["origin"]),
            ),
            point(&row["adjusted"]),
            "{}",
            row["name"]
        );
    }
    let rows = corpus["placement_controls"]["fire"].as_array().unwrap();
    assert!(rows.iter().any(|row| row["name"] == "surface_below_far"));
    for row in rows {
        let input = &row["supplied"];
        let mut sim = world(input);
        let target = &input["target_cell"];
        let mut shot = super::tests::spawn(ProjectileTarget::Cell {
            rx: target[0].as_u64().unwrap() as u16,
            ry: target[1].as_u64().unwrap() as u16,
        });
        shot.source_id = 0;
        shot.origin = point(&input["origin"]);
        shot.initial_target_position = point(&row["target_coord"]);
        shot.payload = ProjectilePayload::new(
            0,
            sim.interner.intern("AP"),
            sim.interner.intern("SubTorpedo"),
        );
        shot.flat = kind.flat;
        shot.arm_frames = kind.arm;
        // This test supplies the already-normalized Fire velocity, just as
        // the existing launch owner does. It compares placement and detector
        // setup against full original Fire468670, not the preceding FireAt.
        shot.velocity = ProjectileVelocity::from_native(std::array::from_fn(|axis| {
            NativeF64Bits::from_bits(
                u64::from_str_radix(row["velocity"]["bits"][axis].as_str().unwrap(), 16).unwrap(),
            )
        }));
        let id = sim.allocate_stable_id();
        sim.admit_projectile(id, shot);
        let bullet = sim.projectiles.get(id).unwrap();
        assert_eq!(bullet.position, point(&row["location"]), "{}", row["name"]);
        assert_eq!(bullet.launch_origin, point(&row["fire_coord"]));
        assert_eq!(bullet.launch_target, point(&row["target_coord"]));
        assert_eq!(
            bullet.previous_cell,
            (
                row["last_cell"][0].as_i64().unwrap() as i16,
                row["last_cell"][1].as_i64().unwrap() as i16,
            )
        );
        assert_eq!(
            bullet.velocity, shot.velocity,
            "placement preserves velocity"
        );
        let detector = &row["detector"];
        assert_eq!(bullet.last_target_position, point(&detector["reference"]));
        assert_eq!(
            i64::from(bullet.last_distance_half),
            detector["distance_watermark"].as_i64().unwrap(),
            "{}: detector starts at placed Location, not FireCoord",
            row["name"]
        );
        assert_eq!(
            i64::from(bullet.arm_timer.start_frame()),
            detector["arm_start"].as_i64().unwrap()
        );
        assert_eq!(
            i64::from(bullet.arm_timer.duration()),
            detector["arm_duration"].as_i64().unwrap()
        );
    }
}

#[test]
fn projectile_trailer_header_matches_original_signed_cadence_and_wait_gates() {
    let corpus: Value = serde_json::from_str(crate::test_fixture::text(
        "tools/projectile_oracle/projectile_trailer.json",
    ))
    .unwrap();
    let mut compared = 0;
    let mut excluded = Vec::new();
    for row in corpus["rows"].as_array().unwrap() {
        let supplied = &row["supplied"];
        // The store represents only admitted/live Bullets. Allocation failure
        // and the unported Scalable global-list producer have no Rust receiver.
        if supplied["alive"] == false
            || supplied["allocation_failure"] == true
            || supplied["scaled_delay"] != 0
        {
            excluded.push(row["name"].as_str().unwrap());
            continue;
        }
        let mut art = String::from("[SUBT]\n");
        if supplied["trailer"] == true {
            art.push_str("Trailer=BBBLELRG\n");
        }
        if let Some(delay) = supplied["art_delay"].as_i64() {
            art.push_str(&format!("SpawnDelay={delay}\n"));
        }
        let art = IniFile::from_str(&art);
        let rules = IniFile::from_str("[Torpedo]\nImage=SUBT\n");
        let kind = ProjectileType::from_ini_section(
            "Torpedo",
            rules.section("Torpedo").unwrap(),
            art.section("SUBT"),
        );
        assert_eq!(
            i64::from(kind.spawn_delay),
            row["spawn_delay"].as_i64().unwrap()
        );
        let mut store = ProjectileStore::new();
        let mut spawn = super::tests::spawn(ProjectileTarget::Cell { rx: 16, ry: 20 });
        let p = &supplied["position"];
        spawn.origin = ProjectileCoord::new(
            p[0].as_i64().unwrap() as i32,
            p[1].as_i64().unwrap() as i32,
            p[2].as_i64().unwrap() as i32,
        );
        spawn.native_unique_id = row["bullet_native_id"].as_i64().unwrap() as i32;
        let id = store.spawn(1, spawn);
        let waiting = supplied["waiting"].as_str().unwrap();
        let projectile = store.projectiles.get_mut(&id).unwrap();
        projectile.awaiting_anim = waiting != "no";
        projectile.awaited_anim = (waiting == "attached").then_some(99);
        let head = store.begin_ai_visit(id, |anim| anim == 99).unwrap();
        assert_eq!(
            store.get(id).unwrap().awaiting_anim,
            row["waiting_after"].as_bool().unwrap(),
            "{}",
            row["name"]
        );
        let result = std::panic::catch_unwind(|| {
            head.trailer(&kind, supplied["frame"].as_i64().unwrap() as u32)
        });
        if row.get("divide_fault").is_some() {
            assert!(
                result.is_err(),
                "{}: native #DE has no silent cadence",
                row["name"]
            );
        } else {
            let actual: Vec<_> = result
                .unwrap()
                .into_iter()
                .map(|(name, p)| json!({"anim_type":name,"position":[p.x,p.y,p.z]}))
                .collect();
            let expected: Vec<_> = row["emissions"].as_array().unwrap().iter().map(|emission|
                json!({"anim_type":emission["anim_type"],"position":emission["position"]})).collect();
            assert_eq!(actual, expected, "{}", row["name"]);
        }
        compared += 1;
    }
    assert_eq!(
        excluded,
        [
            "scaled_hit",
            "scaled_overrides_base",
            "scaled_negative",
            "scaled_overrides_zero",
            "scaled_overflow_trap",
            "dead_before_divide",
            "allocation_failure"
        ]
    );
    assert_eq!(
        compared,
        corpus["rows"].as_array().unwrap().len() - excluded.len()
    );
}
