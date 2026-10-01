//! Replay each retained Display layer without sorting by texture or depth.
//! Adjacent native destination edits share the existing overlap scheduler;
//! ordinary GPU draw policies fence those spans in the same parent order.

use crate::render::batch::{BatchRenderer, BatchTexture};
use crate::render::overlay_atlas::OverlayAtlas;
use crate::render::palette_textures::PaletteSet;
use crate::render::sprite_atlas::SpriteAtlas;
use crate::render::unit_atlas::UnitAtlas;
use crate::render::unit_slope_transition_cache::VxlSlopeTransitionCache;

use super::draw_plan_lowering::{ObjectLayerPass, ObjectTexture};
use crate::render::tactical_draw_plan::RenderZPolicy;

/// Dispatch the already-lowered native Display sequence without re-sorting it.
///
/// Every run is a contiguous slice of one flat instance buffer. Texture page,
/// atlas family, and `SpriteInstance.depth` select only GPU state; none can
/// change the retained Display parent order carried by `TacticalDrawPlan`.
#[allow(clippy::too_many_arguments)]
pub(super) fn draw_native_object_pass<'a>(
    encoder: &mut wgpu::CommandEncoder,
    color: &wgpu::TextureView,
    depth: &wgpu::TextureView,
    terrain: &mut crate::render::terrain_draw::TerrainDrawRenderer,
    tactical: [u32; 4],
    batch: &'a BatchRenderer,
    buffer: Option<(&'a wgpu::Buffer, u32)>,
    ground: &ObjectLayerPass,
    overlay_atlas: Option<&'a OverlayAtlas>,
    unit_atlas: Option<&'a UnitAtlas>,
    pose_texture: Option<&'a BatchTexture>,
    transition_cache: &'a VxlSlopeTransitionCache,
    sprite_atlas: Option<&'a SpriteAtlas>,
    palette_set: Option<&'a PaletteSet>,
    zshape: &'a wgpu::BindGroup,
) -> crate::render::terrain_draw::TerrainBatchStats {
    let Some((buffer, count)) = buffer else {
        return Default::default();
    };
    let mut terrain_stats = crate::render::terrain_draw::TerrainBatchStats::default();
    assert_eq!(count as usize, ground.instances.len());
    let mut cursor = 0;
    while cursor < ground.runs.len() {
        let run = &ground.runs[cursor];
        if destination_edit(run.target).is_some() {
            let start = cursor;
            while cursor < ground.runs.len()
                && destination_edit(ground.runs[cursor].target).is_some()
            {
                cursor += 1;
            }
            // Atlas changes preserve parent order. All destination edits in
            // this span share overlap dependencies; ordinary draws are fences.
            let commands = ground.runs[start..cursor].iter().flat_map(|run| {
                let (piece, atlas_slot) =
                    destination_edit(run.target).expect("destination edit span");
                (run.start..run.start + run.count).map(move |index| {
                    crate::render::terrain_draw::DestinationEditCommand {
                        index,
                        piece,
                        render_z: run.render_z,
                        atlas_slot,
                    }
                })
            });
            terrain_stats.accumulate(terrain.draw_span(
                encoder,
                color,
                depth,
                batch,
                |slot| {
                    if slot == 0 {
                        overlay_atlas.map(|atlas| &atlas.texture)
                    } else {
                        sprite_atlas
                            .and_then(|atlas| atlas.page(slot - 1))
                            .map(|page| &page.texture)
                    }
                },
                buffer,
                &ground.instances,
                commands,
                tactical,
            ));
            continue;
        }
        let mut pass =
            crate::app::presentation::sidebar_render::begin_main_load_pass(encoder, color, depth);
        pass.set_scissor_rect(tactical[0], tactical[1], tactical[2], tactical[3]);
        while cursor < ground.runs.len() && destination_edit(ground.runs[cursor].target).is_none() {
            let run = &ground.runs[cursor];
            match run.target {
                ObjectTexture::TerrainShp(_) | ObjectTexture::ProjectileShp(_, _) => {
                    unreachable!("native destination edits split normal runs")
                }
                ObjectTexture::OverlayAtlas => {
                    if let Some(atlas) = overlay_atlas {
                        batch.draw_passthrough_range(
                            &mut pass,
                            &atlas.texture,
                            buffer,
                            run.start,
                            run.count,
                        );
                    }
                }
                ObjectTexture::UnitAtlasPage(page) => {
                    if let (Some(palette), Some(texture)) = (
                        palette_set,
                        unit_atlas.and_then(|atlas| atlas.page_texture(page)),
                    ) {
                        batch.draw_voxel_sprites_range(
                            &mut pass,
                            texture,
                            &palette.bind_group,
                            buffer,
                            run.start,
                            run.count,
                        );
                    }
                }
                ObjectTexture::UnitPose => {
                    if let (Some(texture), Some(palette)) = (pose_texture, palette_set) {
                        batch.draw_voxel_sprites_range(
                            &mut pass,
                            texture,
                            &palette.bind_group,
                            buffer,
                            run.start,
                            run.count,
                        );
                    }
                }
                ObjectTexture::UnitTransitionPage(page) => {
                    if let (Some(texture), Some(palette)) =
                        (transition_cache.page_texture(page), palette_set)
                    {
                        batch.draw_voxel_sprites_range(
                            &mut pass,
                            texture,
                            &palette.bind_group,
                            buffer,
                            run.start,
                            run.count,
                        );
                    }
                }
                ObjectTexture::ShpPage(page) => {
                    if let Some(texture) = sprite_atlas.and_then(|atlas| atlas.page(page)) {
                        // The plan's Z policy selects the native leaf family:
                        // buildings write (`0x6E00` -> `0x004990e0`), everything
                        // else tests only (`0x2E00` -> `0x00494b60`).
                        match run.render_z {
                            RenderZPolicy::None => batch.draw_passthrough_range(
                                &mut pass,
                                &texture.texture,
                                buffer,
                                run.start,
                                run.count,
                            ),
                            RenderZPolicy::ReadOnly => batch.draw_zsprite_range(
                                &mut pass,
                                &texture.texture,
                                zshape,
                                buffer,
                                run.start,
                                run.count,
                                false,
                            ),
                            RenderZPolicy::ReadWrite | RenderZPolicy::AlphaReadWrite => batch
                                .draw_zsprite_range(
                                    &mut pass,
                                    &texture.texture,
                                    zshape,
                                    buffer,
                                    run.start,
                                    run.count,
                                    true,
                                ),
                        }
                    }
                }
            }
            cursor += 1;
        }
        // Ordinary draws are submission boundaries as well as ordering
        // fences. Many alternating Bullet/Anim parents must not fill Metal's
        // native command-buffer pool before the frame reaches submission.
        drop(pass);
        terrain.note_external_passes(encoder, 1);
    }
    terrain_stats
}

fn destination_edit(
    target: ObjectTexture,
) -> Option<(crate::render::terrain_draw::TerrainPiece, usize)> {
    match target {
        ObjectTexture::TerrainShp(piece) => Some((piece, 0)),
        ObjectTexture::ProjectileShp(page, piece) => Some((piece, page + 1)),
        _ => None,
    }
}

#[cfg(test)]
#[path = "terrain_ground_gpu_tests.rs"]
mod terrain_ground_gpu_tests;

#[cfg(test)]
#[path = "naval_sinking_gpu_tests.rs"]
mod naval_sinking_gpu_tests;

#[cfg(test)]
#[path = "projectile_shape_gpu_tests.rs"]
mod projectile_shape_gpu_tests;

#[cfg(test)]
#[path = "projectile_submission_gpu_tests.rs"]
mod projectile_submission_gpu_tests;
