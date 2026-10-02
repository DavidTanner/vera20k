//! Voxel type -> production constructor -> body draw -> the layers the unit
//! atlas seeds for the drawn model.
//!
//! `UnitClass::DrawVoxelBody` (`0x0073B470`) takes its turret arm on the draw
//! type's `Turret=` (`0x0073B7A3`), and `UnitClass::DrawIt` makes a
//! `Harvester=` unit's `UnloadingClass=` the draw type while `Unit+0x6D1` is
//! set (`0x0073D29C..0x0073D2C4`). `AircraftClass::Draw_It` (`0x004144B0`)
//! draws the main voxel alone.
use super::*;
use crate::assets::asset_manager::AssetManager;
use crate::map::source::test_support::TestDirectory;
use crate::rules::ini_parser::IniFile;
use crate::rules::retail_ini_fixture::{retail_assets, retail_battle_rules};
use crate::rules::ruleset::RuleSet;
use crate::sim::voxel_frame_catalog::{
    detect_hva_frame_count, seed_layers_for, unit_atlas_variants, voxel_image_id,
};
use crate::sim::world::Simulation;

/// One body draw of `id`: its model, and whether it asks the atlas for
/// separate hull and turret sprites. A model the atlas does not seed for the
/// object's type, or seeds without the sprite asked for, is an `Err`.
fn body_draw_against_seed(
    sim: &Simulation,
    assets: &AssetManager,
    rules: &RuleSet,
    id: u64,
) -> Result<(String, bool), String> {
    let entity = sim.entities().get(id).expect("the object was just built");
    let base_type = sim.interner.resolve(entity.type_ref());
    let (model, body) = unit_body_draw(
        entity,
        &sim.interner,
        Some(rules),
        EntityDrawBand::Ground,
        sim.session.binary_frame,
    );
    if !unit_atlas_variants(base_type, Some(rules)).contains(&model.to_string()) {
        return Err(format!(
            "{base_type} draws {model}, which the atlas never seeds"
        ));
    }
    let seeded = seed_layers_for(assets, &model, Some(rules));
    let (asked, parts) = match body {
        BodyDraw::Turret { .. } => (VxlLayer::Body, true),
        BodyDraw::Composite => (VxlLayer::Composite, false),
        BodyDraw::CrashPose(_) => return Err(format!("{base_type} takes the crash pose")),
    };
    if !seeded.contains(&asked) {
        return Err(format!(
            "{base_type} draws {model} from its {asked:?} sprite, but the atlas seeds {seeded:?}"
        ));
    }
    Ok((model.into_owned(), parts))
}

/// The dock latch's presentation write (`refinery_dock::set_unload_latch`).
fn latch_unloading_image(sim: &mut Simulation, id: u64, image: &str) {
    let image = sim.interner.intern(image);
    sim.entities_mut()
        .get_mut(id)
        .expect("the object was just built")
        .display_type_override = Some(image);
}

#[test]
fn the_turret_split_follows_the_drawn_model_not_the_objects_turret() {
    let rules = RuleSet::from_ini(&IniFile::from_str(
        "\
[VehicleTypes]
0=MINER
1=EMPTY
2=PLAIN
3=GUNNED
[AircraftTypes]
0=JET
[MINER]
Harvester=yes
Turret=yes
UnloadingClass=EMPTY
[EMPTY]
[PLAIN]
Harvester=yes
UnloadingClass=GUNNED
[GUNNED]
Turret=yes
[JET]
Turret=yes
",
    ))
    .expect("the draw-type rules parse");
    let directory = TestDirectory::new("unit-body-draw");
    let assets = AssetManager::from_loose_root_for_test(directory.path());
    let mut sim = Simulation::new();
    let spawn = |sim: &mut Simulation, type_id: &str| {
        sim.spawn_object_limbo_at_height(type_id, "Americans", 10, 10, 0x40, 0, &rules)
            .unwrap_or_else(|| panic!("build {type_id}"))
    };
    let draw = |sim: &Simulation, id: u64| body_draw_against_seed(sim, &assets, &rules, id);

    // A turreted miner draws its turret until it unloads as a turretless type.
    let miner = spawn(&mut sim, "MINER");
    assert!(sim.entities().get(miner).unwrap().barrel_facing.is_some());
    assert_eq!(draw(&sim, miner), Ok(("MINER".to_string(), true)));
    latch_unloading_image(&mut sim, miner, "EMPTY");
    assert_eq!(draw(&sim, miner), Ok(("EMPTY".to_string(), false)));

    // An aircraft keeps a Secondary facing and draws one voxel, `Turret=` or not.
    let jet = spawn(&mut sim, "JET");
    assert!(sim.entities().get(jet).unwrap().barrel_facing.is_some());
    assert_eq!(draw(&sim, jet), Ok(("JET".to_string(), false)));

    // RESIDUAL pinned: an object with no turret facing of its own draws a
    // turreted model's turret at its hull's facing.
    let plain = spawn(&mut sim, "PLAIN");
    assert!(sim.entities().get(plain).unwrap().barrel_facing.is_none());
    assert_eq!(draw(&sim, plain), Ok(("PLAIN".to_string(), false)));
    latch_unloading_image(&mut sim, plain, "GUNNED");
    assert_eq!(draw(&sim, plain), Ok(("GUNNED".to_string(), true)));
    let entity = sim.entities().get(plain).unwrap();
    let frame = sim.session.binary_frame;
    let ground = EntityDrawBand::Ground;
    let (_, body) = unit_body_draw(entity, &sim.interner, Some(&rules), ground, frame);
    let hull = entity.body_facing_current(frame);
    assert!(matches!(body, BodyDraw::Turret { turret, .. } if turret == hull));
}

/// Every voxel body in the stock Hills/Battle fixture must ask the atlas for
/// the sprites its drawn model was seeded with: the unloading War Miner for
/// HORV's one body sprite, an aircraft for its one body sprite. Other map,
/// mode and campaign layers are not covered by this fixture.
#[test]
fn retail_voxel_bodies_draw_the_sprites_their_model_is_seeded_with() {
    let Some(battle) = retail_battle_rules() else {
        return;
    };
    let (_, assets) = retail_assets().expect("the battle rules came from RA2_DIR");
    let rules = &battle.rules;
    let mut sim = Simulation::new();
    let mut failures = Vec::new();
    let mut voxel_vehicles = 0;
    let mut aircraft = Vec::new();
    let mut unloading = Vec::new();
    for type_id in rules.vehicle_ids.iter().chain(&rules.aircraft_ids) {
        let object = rules.object(type_id).expect("a listed type");
        let Some(id) = sim.spawn_object_limbo_at_height(type_id, "Americans", 10, 10, 0, 0, rules)
        else {
            continue;
        };
        let entity = sim.entities().get(id).expect("the object was just built");
        if !entity.is_voxel {
            continue;
        }
        let is_aircraft = entity.category == EntityCategory::Aircraft;
        match body_draw_against_seed(&sim, &assets, rules, id) {
            Ok((model, parts)) => {
                assert_eq!(&model, type_id);
                if is_aircraft {
                    aircraft.push((type_id.clone(), parts));
                } else {
                    voxel_vehicles += 1;
                }
            }
            Err(failure) => failures.push(failure),
        }
        let Some(image) = object
            .unloading_class
            .as_deref()
            .filter(|image| object.harvester && rules.object(image).is_some())
        else {
            continue;
        };
        latch_unloading_image(&mut sim, id, image);
        match body_draw_against_seed(&sim, &assets, rules, id) {
            Ok((model, parts)) => unloading.push((type_id.clone(), model, parts)),
            Err(failure) => failures.push(format!("unloading: {failure}")),
        }
    }
    assert_eq!(failures, Vec::<String>::new());
    assert!(
        voxel_vehicles > 40,
        "{voxel_vehicles} retail voxel vehicles"
    );
    // Every aircraft in this stock Hills/Battle fixture is one body sprite.
    assert_eq!(aircraft.len(), 12, "{aircraft:?}");
    assert!(aircraft.iter().all(|(_, parts)| !parts), "{aircraft:?}");
    // The War Miner's HARV has `Turret=yes`; its unloading HORV has none.
    unloading.sort();
    assert_eq!(
        unloading,
        [
            ("CMIN".to_string(), "CMON".to_string(), false),
            ("HARV".to_string(), "HORV".to_string(), false),
        ]
    );
    // Only a vehicle model loads `%sTUR` and `%sBARL`. No other retail model
    // ships one: not an aircraft, whose `AircraftClass::Draw_It` would not
    // draw it, and not a building's voxel `TurretAnim=` model, of which
    // YAGGUN's is named as its building.
    let turret_models: Vec<&str> = rules
        .building_ids
        .iter()
        .filter_map(|type_id| rules.object(type_id))
        .filter(|object| object.turret_anim_is_voxel)
        .filter_map(|object| object.turret_anim.as_deref())
        .collect();
    assert_eq!(turret_models.len(), 8, "{turret_models:?}");
    assert!(turret_models.contains(&"YAGGUN"), "{turret_models:?}");
    let aircraft_models = rules.aircraft_ids.iter().map(String::as_str);
    for model in aircraft_models.chain(turret_models) {
        let image = voxel_image_id(model, Some(rules));
        for part in ["TUR", "BARL"] {
            let file = format!("{image}{part}.VXL");
            assert!(assets.get_ref(&file).is_none(), "{file}");
        }
    }
    // The body's HVA frame is `Unit+0x538` modulo the draw type's frame count
    // (`0x0073B4DA..0x0073B4E7`); VERA draws an unloading body's frame 0. The
    // two agree while both retail unloading models are single-frame.
    for model in ["HORV", "CMON"] {
        let frames = detect_hva_frame_count(&assets, model, VxlLayer::Composite, Some(rules));
        assert_eq!(frames, 1, "{model}");
    }
    // The turret arm's second admission (`0x0073B7B1..0x0073B7C5`: a
    // `TurretCount=` type's current turret) is not represented. It is dormant
    // in these stock Hills/Battle rules: every `TurretCount=` vehicle also
    // sets `Turret=yes`. This does not cover other INI-layer combinations.
    for type_id in &rules.vehicle_ids {
        let object = rules.object(type_id).expect("a listed type");
        assert!(
            object.turret_count <= 0 || object.has_turret,
            "{type_id} has TurretCount= without Turret="
        );
    }
}
