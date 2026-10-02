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

use std::collections::BTreeMap;

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

/// Whether the voxel model `type_id` draws `%sTUR` and `%sBARL` gun parts,
/// each at its own facing: a vehicle type with `Turret=`. Every other model
/// is its body alone. Atlas seeding, model loading and the body draw all ask
/// this of the model being drawn, so they cannot disagree.
///
/// `UnitClass::DrawVoxelBody @ 0x0073B470` takes its turret arm on the draw
/// type's `Turret=` byte (`[ebx+0xCA1]` at `0x0073B7A3`). The draw type is
/// `Unit+0x6C4` as its caller left it (`0x0073B494`): `UnitClass::DrawIt`
/// writes a `Harvester=` type's `UnloadingClass=` there while `Unit+0x6D1` is
/// set (`0x0073D29C..0x0073D2C4`) and restores the type after the body
/// (`0x0073D39B..0x0073D3A2`). So an unloading War Miner draws HORV, which has
/// no `Turret=`, as a hull without the HARV turret.
/// `AircraftClass::Draw_It @ 0x004144B0` draws the main voxel alone
/// (`0x004149D7`), whatever the type's `Turret=`.
///
/// The voxel loader `0x005F8110` agrees for vehicles: `TechnoTypeClass::ReadINI`
/// runs it after reading `Turret=` (`0x0071609C`, behind the voxel flag
/// `+0x236`), and for a vehicle type whose `Turret=` is clear it jumps past
/// `%sTUR` (`0x005F8277..0x005F828D`) and past `%sBARL`
/// (`0x005F8844..0x005F8856`). A `TurretCount=` vehicle loads one pair per
/// turret index, the plain names for index 0 (`0x005F7A90`, `0x005F7DB0`). The
/// loader formats no `%sBARREL` name.
///
/// A name that is not a rules type has none. A `%sWO` model stands in for its
/// type's body. A building's voxel `TurretAnim=` model has none either, even
/// where its name is a rules type (the retail YAGGUN): the building's own
/// loader loads that model's barrel, rewriting `TUR` to `BARL` in the name
/// (`0x0045FC73..0x0045FC82`); VERA does not port that yet.
///
/// RESIDUALS (named trigger, effect, frequency):
/// - The turret arm also admits a `Turret=no` draw type whose `TurretCount=`
///   is positive while the unit's current turret (`TechnoClass+0x124`) is set
///   (`0x0070DC60`, `0x0070DCE0` at `0x0073B7B1..0x0073B7C5`). Not represented:
///   such a unit draws its hull alone. Frequency: zero in retail, where every
///   `TurretCount=` vehicle sets `Turret=yes`.
/// - The turretless arm draws a loaded barrel voxel alone, at the barrel's
///   pitch (`0x0073B7CB..0x0073B8D4`). The loader gives a `Turret=no` vehicle
///   none, so only a type whose `Turret=` a later INI pass clears keeps one.
///   Not represented. Frequency: zero in retail, where no layer does.
pub(crate) fn draws_turret_parts(type_id: &str, rules: Option<&RuleSet>) -> bool {
    rules
        .and_then(|rules| rules.object(type_id))
        .is_some_and(|object| object.category == ObjectCategory::Vehicle && object.has_turret)
}

/// The layer set to seed atlas keys for a voxel model.
///
/// A model with [`draws_turret_parts`] gets separate Body/Turret layers, and a
/// Barrel layer **only when a barrel voxel actually exists**. Most turreted
/// units model the gun as part of the turret and ship no `…BARL.VXL`: the
/// Soviet War Miner is one. Seeding a Barrel key for those produced a key that
/// could never be satisfied: the Barrel branch of the renderer rebuilds the
/// body and turret sprites, finds no barrel, and returns `None`, so nothing is
/// cached and the whole attempt repeats on the next frame, forever. A single
/// such unit on screen logged ~135k render failures in four minutes of play and
/// paid for two discarded voxel rasterisations every frame.
pub(crate) fn seed_layers_for(
    asset_manager: &AssetManager,
    type_id: &str,
    rules: Option<&RuleSet>,
) -> &'static [VxlLayer] {
    if !draws_turret_parts(type_id, rules) {
        return &[VxlLayer::Composite];
    }
    if has_barrel_voxel(asset_manager, type_id, rules) {
        &[VxlLayer::Body, VxlLayer::Turret, VxlLayer::Barrel]
    } else {
        &[VxlLayer::Body, VxlLayer::Turret]
    }
}

/// Whether this turreted type ships a separate barrel voxel. Resolves the image
/// id exactly as the render path does, so the seeding decision and the lookup
/// can never disagree.
fn has_barrel_voxel(asset_manager: &AssetManager, type_id: &str, rules: Option<&RuleSet>) -> bool {
    let image = voxel_image_id(type_id, rules);
    asset_manager.get_ref(&format!("{image}BARL.VXL")).is_some()
}

/// Detect the HVA animation frame count for a given (type_id, layer) combo.
///
/// Loads the HVA file from the asset manager and returns `frame_count`.
/// Returns 1 if no HVA is found or if parsing fails (single-frame default).
pub(crate) fn detect_hva_frame_count(
    asset_manager: &AssetManager,
    type_id: &str,
    layer: VxlLayer,
    rules: Option<&RuleSet>,
) -> u32 {
    let image = voxel_image_id(type_id, rules);
    let hva_name: String = match layer {
        VxlLayer::Composite | VxlLayer::Body => art_data::voxel_asset_names(&image).1,
        VxlLayer::Turret => format!("{}TUR.HVA", image),
        VxlLayer::Barrel => format!("{}BARL.HVA", image),
        // The shadow is rendered from motion frame 0 regardless of the body's
        // HVA length (`Get_Layer_Matrix(layer, 0)` in the TS shadow path, and
        // the RA2 shadow key folds only slope and facing: 0x0055A7D0).
        VxlLayer::Shadow => return 1,
    };

    let frame_count: u32 = asset_manager
        .get_ref(&hva_name)
        .and_then(|data| HvaFile::from_bytes(data).ok())
        .map(|h| h.frame_count)
        .unwrap_or(1);
    frame_count.max(1)
}

/// Build the frame-count catalog for every voxel entity in the store, keyed
/// by `(type_id, layer)` — the same enumeration (variants, then layers) the
/// unit-atlas seeding walks, minus the sprite keys.
pub(crate) fn build_voxel_frame_catalog(
    entities: &crate::sim::entity_store::EntityStore,
    interner: &crate::sim::intern::StringInterner,
    asset_manager: &AssetManager,
    rules: Option<&RuleSet>,
) -> BTreeMap<(String, VxlLayer), u32> {
    let mut frame_counts: BTreeMap<(String, VxlLayer), u32> = BTreeMap::new();
    for entity in entities.values() {
        if !entity.is_voxel {
            continue;
        }
        let type_str = interner.resolve(entity.type_ref());
        for variant in unit_atlas_variants(type_str, rules) {
            for &layer in seed_layers_for(asset_manager, &variant, rules) {
                frame_counts
                    .entry((variant.clone(), layer))
                    .or_insert_with(|| {
                        detect_hva_frame_count(asset_manager, &variant, layer, rules)
                    });
            }
        }
    }
    frame_counts
}
