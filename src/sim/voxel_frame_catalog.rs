//! GPU-independent HVA frame-count catalog (F09).
//!
//! Voxel animation frame counts are authoritative sim metadata — the
//! `VoxelAnimation` component advances by them — but were historically read
//! back from the renderer's unit atlas after texture building. This module
//! owns the assets/rules half with no GPU involvement: it resolves each voxel
//! type's model variants and layers exactly as the atlas seeding does and
//! parses `.hva` frame counts directly. The GPU-free construction path (app
//! and headless) and the renderer's atlas seeding consume the same functions,
//! so presentation and simulation cannot disagree on a frame count.
//!
//! Depends on `assets/`, `rules/`, and sim component/store types only.

use std::collections::{BTreeMap, BTreeSet};

use crate::assets::asset_manager::AssetManager;
use crate::assets::hva_file::HvaFile;
use crate::rules::art_data;
use crate::rules::object_type::ObjectCategory;
use crate::rules::ruleset::RuleSet;
use crate::sim::components::VxlLayer;

pub(crate) const NO_SPAWN_ALT_SUFFIX: &str = "WO";

/// Every voxel model an object of `type_id` can select at presentation time:
/// its own, its `UnloadingClass=` and its `%sWO`.
///
/// Initial atlas construction and incremental coverage checks must enumerate
/// the same set. Otherwise an already-valid base model can hide a missing
/// UnloadingClass or `%sWO` auxiliary model until the draw lookup fails.
pub(crate) fn unit_atlas_variants(type_id: &str, rules: Option<&RuleSet>) -> Vec<String> {
    let object = rules.and_then(|rules| rules.object(type_id));
    let mut variants = vec![type_id.to_string()];

    if let Some(unloading_type) = object
        .and_then(|object| object.unloading_class.as_deref())
        .filter(|unloading_type| rules.is_some_and(|rules| rules.object(unloading_type).is_some()))
    {
        variants.push(unloading_type.to_string());
    }

    if object.is_some_and(|object| object.no_spawn_alt) {
        // Native stores the `%sWO` pair in the same AuxVoxel slot used by
        // turrets. Stock NoSpawnAlt types therefore render it as one
        // composite body and cannot also own a turret.
        variants.push(format!("{type_id}{NO_SPAWN_ALT_SUFFIX}"));
    }

    variants
}

/// The image a voxel model's files are named from: `type_id`'s rules `Image=`,
/// which defaults to the type's ID. The constructor copies the ID into `+0x1F8`
/// (`0x005F724F..0x005F7276`), `ObjectTypeClass::ReadINI` reads `Image=` over
/// it (`0x005F9335`), and the voxel loader `0x005F8110` formats every model
/// name from that field. An art `Image=` does not redirect it: gamemd reads
/// that art key for buildings (`0x0045F93A`), not for vehicles or aircraft. A
/// name that is not a rules type is its own image.
pub(crate) fn voxel_image_id(type_id: &str, rules: Option<&RuleSet>) -> String {
    rules
        .and_then(|rules| rules.object(type_id))
        .map_or(type_id, |object| object.image.as_str())
        .trim()
        .to_uppercase()
}

/// The indexed gun-model count admitted by the UnitType voxel loader and
/// Unit draw: `TurretCount > 0 && !IsGattling` (5F8640, 73B8D9).
/// Aircraft and building turret models never use the UnitType gun arrays.
/// Native controls: tools/spatial_oracle/ifv_turret_switching.json.
pub(crate) fn indexed_turret_count(type_id: &str, rules: Option<&RuleSet>) -> i32 {
    rules
        .and_then(|rules| rules.object(type_id))
        .filter(|object| object.category == ObjectCategory::Vehicle && !object.is_gattling)
        .map_or(0, |object| object.turret_count.max(0))
}

/// Model selection at 73B8D9..73B94F. The signed current index is retained
/// for indexed types; ordinary and Gattling models use their base gun pair.
/// Native does not bounds-check an indexed access. The renderer deliberately
/// omits an invalid part instead of reproducing an unchecked pointer alias.
pub(crate) fn voxel_turret_index(type_id: &str, rules: Option<&RuleSet>, current: i32) -> i32 {
    if indexed_turret_count(type_id, rules) > 0 {
        current
    } else {
        0
    }
}

/// Unit DrawVoxelBody73B7A3..73B7C5 admits its gun arm on the drawn type's
/// `Turret=` or on `TurretCount > 0` with a current index other than -1.
/// The type may be UnloadingClass or a disguise. Aircraft draw only their
/// main voxel (4144B0), and building voxel turrets have their own loader.
pub(crate) fn draws_turret_parts(type_id: &str, rules: Option<&RuleSet>, current: i32) -> bool {
    rules
        .and_then(|rules| rules.object(type_id))
        .is_some_and(|object| {
            object.category == ObjectCategory::Vehicle
                && (object.has_turret || (object.turret_count > 0 && current != -1))
        })
}

/// Shared native part naming (5F7A90 / 5F7DB0): index zero has no numeric
/// suffix; positive indices append their decimal value after TUR or BARL.
pub(crate) fn voxel_gun_basename(image: &str, layer: VxlLayer, index: i32) -> String {
    let part = match layer {
        VxlLayer::Turret => "TUR",
        VxlLayer::Barrel => "BARL",
        _ => unreachable!("only gun layers have a gun basename"),
    };
    if index == 0 {
        format!("{image}{part}")
    } else {
        format!("{image}{part}{index}")
    }
}

/// BuildingType's voxel loader45FA90 searches for `TUR` starting at byte4
/// of TurretAnim. A match loads the B8 turret and replaces that suffix with
/// `BARL` for C0; without a match the unchanged name loads C0 alone. Asset
/// lookup is case-insensitive. Original execution:
/// tools/voxel_oracle/building_barrel.json.
/// The retail layers leave BarrelAnimIsVoxel/VoxelBarrelFile unset; that
/// custom C0-only override is not part of this retained filename projection.
pub(crate) fn building_voxel_names(
    type_id: &str,
    rules: Option<&RuleSet>,
) -> Option<(Option<String>, String)> {
    let object = rules?.object(type_id)?;
    if object.category != ObjectCategory::Building || !object.turret_anim_is_voxel {
        return None;
    }
    let image = object.turret_anim.as_deref()?.to_uppercase();
    let split = image.get(4..).and_then(|suffix| suffix.find("TUR"));
    Some(match split {
        Some(offset) => {
            let barrel = format!("{}BARL", &image[..offset + 4]);
            (Some(image), barrel)
        }
        None => (None, image),
    })
}

/// Layers and gun indices the model can draw. Body and shadow storage stay
/// independent of the selected gun, while every indexed gun has its own HVA.
/// Missing optional barrels never create atlas work. A missing indexed turret
/// ends enumeration, matching the loader's bounded prefix walk; UnitModel
/// rejects the entire invalid model, as native's failure cleanup does.
pub(crate) fn seed_layers_for(
    asset_manager: &AssetManager,
    type_id: &str,
    rules: Option<&RuleSet>,
) -> Vec<(VxlLayer, i32)> {
    if let Some((turret, barrel)) = building_voxel_names(type_id, rules) {
        return [(VxlLayer::Turret, turret), (VxlLayer::Barrel, Some(barrel))]
            .into_iter()
            .filter_map(|(layer, image)| {
                asset_manager
                    .get_ref(&format!("{}.VXL", image?))
                    .map(|_| (layer, 0))
            })
            .collect();
    }
    if !draws_turret_parts(type_id, rules, 0) {
        return vec![(VxlLayer::Composite, 0)];
    }
    let image = voxel_image_id(type_id, rules);
    let indexed_count = indexed_turret_count(type_id, rules);
    let has_turret = rules
        .and_then(|rules| rules.object(type_id))
        .is_some_and(|object| object.has_turret);
    let mut layers = vec![(VxlLayer::Body, 0)];
    for index in 0..indexed_count.max(i32::from(has_turret)) {
        let turret = voxel_gun_basename(&image, VxlLayer::Turret, index);
        if indexed_count > 0
            && ["VXL", "HVA"].into_iter().any(|extension| {
                asset_manager
                    .get_ref(&format!("{turret}.{extension}"))
                    .is_none()
            })
        {
            break;
        }
        layers.push((VxlLayer::Turret, index));
        // 5F8844 bypasses the entire barrel route for a Turret=no UnitType,
        // even when its indexed turret loop ran.
        if has_turret {
            let barrel = voxel_gun_basename(&image, VxlLayer::Barrel, index);
            if asset_manager.get_ref(&format!("{barrel}.VXL")).is_some() {
                layers.push((VxlLayer::Barrel, index));
            }
        }
    }
    layers
}

/// Detect the HVA animation frame count for a given (type_id, layer) combo.
///
/// Loads the HVA file from the asset manager and returns `frame_count`.
/// Returns 1 if no HVA is found or if parsing fails (single-frame default).
pub(crate) fn detect_hva_frame_count(
    asset_manager: &AssetManager,
    type_id: &str,
    layer: VxlLayer,
    turret_index: i32,
    rules: Option<&RuleSet>,
) -> u32 {
    let hva_name: String = if let Some((turret, barrel)) = building_voxel_names(type_id, rules) {
        let image = match layer {
            VxlLayer::Turret => turret,
            VxlLayer::Barrel => Some(barrel),
            _ => None,
        };
        let Some(image) = image else {
            return 1;
        };
        format!("{image}.HVA")
    } else {
        let image = voxel_image_id(type_id, rules);
        match layer {
            VxlLayer::Composite | VxlLayer::Body => art_data::voxel_asset_names(&image).1,
            VxlLayer::Turret | VxlLayer::Barrel => {
                format!("{}.HVA", voxel_gun_basename(&image, layer, turret_index))
            }
            // The shadow is rendered from motion frame 0 regardless of the body's
            // HVA length (`Get_Layer_Matrix(layer, 0)` in the TS shadow path, and
            // the RA2 shadow key folds only slope and facing: 0x0055A7D0).
            VxlLayer::Shadow => return 1,
        }
    };

    let frame_count: u32 = asset_manager
        .get_ref(&hva_name)
        .and_then(|data| HvaFile::from_bytes(data).ok())
        .map(|h| h.frame_count)
        .unwrap_or(1);
    frame_count.max(1)
}

/// Build the frame-count catalog for every voxel entity in the store, keyed
/// by `(type_id, layer, turret_index)` — the same enumeration (variants, then layers) the
/// unit-atlas seeding walks, minus the sprite keys.
pub(crate) fn build_voxel_frame_catalog(
    entities: &crate::sim::entity_store::EntityStore,
    interner: &crate::sim::intern::StringInterner,
    asset_manager: &AssetManager,
    rules: Option<&RuleSet>,
) -> BTreeMap<(String, VxlLayer, i32), u32> {
    let mut frame_counts: BTreeMap<(String, VxlLayer, i32), u32> = BTreeMap::new();
    let mut seen = BTreeSet::new();
    for entity in entities.values() {
        let type_str = interner.resolve(entity.type_ref());
        if !entity.is_voxel && building_voxel_names(type_str, rules).is_none() {
            continue;
        }
        for variant in unit_atlas_variants(type_str, rules) {
            if !seen.insert(variant.clone()) {
                continue;
            }
            for (layer, index) in seed_layers_for(asset_manager, &variant, rules) {
                frame_counts
                    .entry((variant.clone(), layer, index))
                    .or_insert_with(|| {
                        detect_hva_frame_count(asset_manager, &variant, layer, index, rules)
                    });
            }
        }
    }
    frame_counts
}
