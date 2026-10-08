//! Startup errors and development loading presentation. The skirmish menu's
//! country and start choices are the launch record's `LaunchCountry` and
//! `LaunchStartPosition`.
//! Normal match setup belongs to the retail shell in `skirmish_shell`.

use crate::ui::client_theme;

/// Display an actionable startup failure. This surface cannot launch a match.
/// Returns true when the player chooses to quit.
pub(crate) fn draw_shell_error(
    ctx: &egui::Context,
    error: &str,
    asset_root: Option<&std::path::Path>,
) -> bool {
    let palette = client_theme::apply_client_theme(ctx);
    let mut quit = false;
    egui::CentralPanel::default()
        .frame(egui::Frame::new().fill(palette.bg).inner_margin(24.0))
        .show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.heading("Unable to load the game menu");
                ui.add_space(12.0);
                ui.label("VERA20k needs the original Red Alert 2: Yuri's Revenge game files.");
                if let Some(root) = asset_root {
                    ui.add_space(12.0);
                    ui.strong("Game files searched in:");
                    ui.label(root.display().to_string());
                }
                ui.add_space(12.0);
                ui.label(error);
                ui.add_space(12.0);
                ui.label("Set [paths] ra2_dir in config.toml to your game installation, then restart. The configuration can be beside the executable or in the folder you launch from.");
                ui.add_space(20.0);
                quit = ui.button("Quit").clicked();
            });
        });
    quit
}

/// Draw the loading screen shown while map data is being parsed.
pub fn draw_loading_screen(ctx: &egui::Context, map_name: &str) {
    let palette = client_theme::apply_client_theme(ctx);

    egui::CentralPanel::default()
        .frame(egui::Frame::new().fill(palette.bg))
        .show(ctx, |ui| {
            client_theme::paint_background(ui, palette);
            let panel = ui.max_rect();

            if let Some(texture) = loading_screen_texture(ctx) {
                let image_size = texture.size_vec2();
                let scale = (panel.width() / image_size.x).min(panel.height() / image_size.y);
                let desired = image_size * (scale * 0.96);
                let image_rect = egui::Align2::CENTER_CENTER.align_size_within_rect(desired, panel);
                ui.put(image_rect, egui::Image::new((texture.id(), desired)));
                ui.painter().rect_filled(
                    image_rect.expand(2.0),
                    20.0,
                    egui::Color32::from_rgba_unmultiplied(0, 0, 0, 36),
                );
            }

            let overlay_size = egui::vec2(430.0, 132.0);
            let overlay_rect = egui::Rect::from_min_size(
                egui::pos2(panel.left() + 28.0, panel.bottom() - overlay_size.y - 28.0),
                overlay_size,
            );
            ui.painter().rect_filled(
                overlay_rect,
                18.0,
                egui::Color32::from_rgba_premultiplied(9, 14, 20, 214),
            );
            ui.painter().rect_stroke(
                overlay_rect,
                18.0,
                egui::Stroke::new(1.0_f32, palette.line.gamma_multiply(0.85)),
                egui::StrokeKind::Middle,
            );

            let text_origin = overlay_rect.min + egui::vec2(18.0, 16.0);
            ui.painter().text(
                text_origin,
                egui::Align2::LEFT_TOP,
                "Mission deployment",
                egui::FontId::proportional(14.0),
                palette.accent,
            );
            ui.painter().text(
                text_origin + egui::vec2(0.0, 24.0),
                egui::Align2::LEFT_TOP,
                "Loading...",
                egui::FontId::proportional(32.0),
                palette.text,
            );
            ui.painter().text(
                text_origin + egui::vec2(0.0, 68.0),
                egui::Align2::LEFT_TOP,
                format!("Map: {}", map_name),
                egui::FontId::proportional(16.0),
                palette.text_muted,
            );
            ui.painter().text(
                text_origin + egui::vec2(0.0, 92.0),
                egui::Align2::LEFT_TOP,
                "Parsing map, spawning actors, and preparing assets.",
                egui::FontId::proportional(14.0),
                palette.text_muted,
            );
        });
}

fn loading_screen_texture(ctx: &egui::Context) -> Option<egui::TextureHandle> {
    let texture_id = egui::Id::new("loading_screen_texture");
    if let Some(existing) = ctx.data(|d| d.get_temp::<egui::TextureHandle>(texture_id)) {
        return Some(existing);
    }

    let image = loading_screen_image()?;
    let texture = ctx.load_texture(
        "loading_screen_texture",
        image.clone(),
        egui::TextureOptions::LINEAR,
    );
    ctx.data_mut(|d| d.insert_temp(texture_id, texture.clone()));
    Some(texture)
}

fn loading_screen_image() -> Option<&'static egui::ColorImage> {
    // Loading screen image removed — the caller gracefully handles None.
    None
}
