//! Transient VXL slope-transition sprite cache.
//!
//! Stable unit sprites stay in `UnitAtlas`. This cache only materializes
//! gamemd's short 3-frame blended slope sprites that are actually visible,
//! uploading each new sprite's rectangle into free shelf space of its page.

use std::collections::{BTreeMap, HashMap};

use crate::assets::asset_manager::AssetManager;
use crate::assets::vpl_file::VplFile;
use crate::render::atlas_growth::{self, ShelfCursor};
use crate::render::batch::{BatchRenderer, BatchTexture};
use crate::render::gpu::GpuContext;
use crate::render::unit_atlas::{UnitModel, UnitSpriteEntry, UnitSpriteKey, VxlLayer};
use crate::render::vxl_raster::VxlSlopeBlend;
use crate::rules::ruleset::RuleSet;

const PAGE_SIZE: u32 = 2048;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TransitionUnitSpriteKey {
    pub type_id: String,
    pub turret_index: i32,
    pub facing: u8,
    pub layer: VxlLayer,
    pub frame: u32,
    pub from_slope: u8,
    pub to_slope: u8,
    pub phase_num: i32,
    pub phase_den: u8,
    /// The barrel's pitch step, as [`UnitSpriteKey::barrel_pitch`].
    pub barrel_pitch: i8,
}

#[derive(Debug, Clone, Copy)]
pub struct TransitionUnitSpriteEntry {
    pub page: usize,
    pub entry: UnitSpriteEntry,
}

pub struct TransitionAtlasPage {
    pub texture: BatchTexture,
    shelf: ShelfCursor,
}

#[derive(Default)]
pub struct VxlSlopeTransitionCache {
    entries: HashMap<TransitionUnitSpriteKey, TransitionUnitSpriteEntry>,
    pages: Vec<TransitionAtlasPage>,
    /// VOXELS.VPL, parsed on the first miss (`None` inside when unavailable).
    vpl: Option<Option<VplFile>>,
    /// Shared parsed models; reset with this cache on each scenario load.
    models: BTreeMap<String, Option<UnitModel>>,
}

impl VxlSlopeTransitionCache {
    pub fn page_count(&self) -> usize {
        self.pages.len()
    }

    pub fn page_texture(&self, page: usize) -> Option<&BatchTexture> {
        self.pages.get(page).map(|p| &p.texture)
    }

    pub fn get_or_render(
        &mut self,
        gpu: &GpuContext,
        batch: &BatchRenderer,
        asset_manager: &AssetManager,
        rules: Option<&RuleSet>,
        key: TransitionUnitSpriteKey,
    ) -> Option<TransitionUnitSpriteEntry> {
        if let Some(entry) = self.entries.get(&key).copied() {
            return Some(entry);
        }

        let vpl = self.vpl.get_or_insert_with(|| {
            asset_manager
                .get_ref("VOXELS.VPL")
                .and_then(|data| VplFile::from_bytes(data).ok())
        });
        let render_key = UnitSpriteKey {
            type_id: key.type_id.clone(),
            turret_index: key.turret_index,
            facing: key.facing,
            layer: key.layer,
            frame: key.frame,
            slope_type: key.to_slope,
            barrel_pitch: key.barrel_pitch,
        };
        let blend = VxlSlopeBlend {
            from_slope: key.from_slope,
            to_slope: key.to_slope,
            phase_num: key.phase_num,
            phase_den: key.phase_den,
        };
        let model = self
            .models
            .entry(key.type_id.clone())
            .or_insert_with(|| UnitModel::load(asset_manager, &key.type_id, rules))
            .as_ref()?;
        let (sprite, native_draw_bounds) =
            model.render(&render_key, vpl.as_ref(), Some(blend), &mut None)?;

        let size = [sprite.width, sprite.height];
        let placed = self
            .pages
            .iter_mut()
            .enumerate()
            .find_map(|(index, page)| Some((index, page.shelf.place(size[0], size[1])?)));
        let (page_index, [px, py]) = match placed {
            Some(placed) => placed,
            None => {
                let page_size = PAGE_SIZE.min(gpu.device.limits().max_texture_dimension_2d);
                let mut shelf = ShelfCursor::new(page_size, page_size);
                let origin = shelf.place(size[0], size[1])?;
                let texture =
                    batch.create_blank_unit_atlas_texture(&gpu.device, page_size, page_size);
                self.pages.push(TransitionAtlasPage { texture, shelf });
                (self.pages.len() - 1, origin)
            }
        };
        let page = &self.pages[page_index];
        atlas_growth::write_texels(
            &gpu.queue,
            page.texture.view.texture(),
            [px, py],
            size,
            1,
            &sprite.palette_indices,
        );
        let page_width = page.shelf.width() as f32;
        let page_height = page.shelf.height() as f32;
        let entry = TransitionUnitSpriteEntry {
            page: page_index,
            entry: UnitSpriteEntry {
                uv_origin: [px as f32 / page_width, py as f32 / page_height],
                uv_size: [
                    sprite.width as f32 / page_width,
                    sprite.height as f32 / page_height,
                ],
                pixel_size: [sprite.width as f32, sprite.height as f32],
                offset_x: sprite.offset_x,
                offset_y: sprite.offset_y,
                native_draw_bounds,
                page: page_index,
            },
        };
        self.entries.insert(key, entry);
        Some(entry)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(from_slope: u8, to_slope: u8, phase_num: i32) -> TransitionUnitSpriteKey {
        TransitionUnitSpriteKey {
            type_id: "CMIN".to_string(),
            turret_index: 0,
            facing: 64,
            layer: VxlLayer::Composite,
            frame: 0,
            from_slope,
            to_slope,
            phase_num,
            phase_den: crate::sim::movement::slope_transition::SLOPE_TRANSITION_FRAMES,
            barrel_pitch: 0,
        }
    }

    #[test]
    fn vxl_slope_transition_cache_key_distinguishes_from_to_phase() {
        assert_ne!(key(1, 2, 0), key(2, 1, 0));
        assert_ne!(key(1, 2, 0), key(1, 2, 1));
        let mut other_turret = key(1, 2, 0);
        other_turret.turret_index = 1;
        assert_ne!(key(1, 2, 0), other_turret);
    }

    #[test]
    fn vxl_slope_transition_cache_starts_empty() {
        let cache = VxlSlopeTransitionCache::default();
        assert_eq!(cache.page_count(), 0);
        assert!(cache.page_texture(0).is_none());
    }
}
