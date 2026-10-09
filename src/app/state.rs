//! Application orchestration through eight explicit state owners.
//!
//! Platform, renderer, diagnostics, frontend, match, process assets, audio and
//! persistence each own their state and lifecycle. AppState coordinates these
//! owners; match-scoped input and presentation live inside MatchState.
//! The architecture guards keep unrelated flat fields from returning.

use super::{BTreeMap, OverlayTypeRegistry, ResolvedTerrainGrid};

mod platform;

pub(crate) use platform::PlatformState;

/// All initialized state. Created in `resumed()` when the window is available.
/// pub(crate) so the app presentation/render paths can access fields.
pub(crate) struct AppState {
    pub(crate) platform: PlatformState,
    /// Process-wide renderer owner (F12): GPU context, batch renderer,
    /// pools, passes, fonts, and rendering caches.
    pub(crate) renderer: crate::app::renderer_state::RendererState,
    /// Process diagnostics owner (F12): debug toggles, frame stepper,
    /// parity digest sink, dev-overlay bookkeeping.
    pub(crate) diag: crate::app::diagnostics::state::DiagnosticsState,
    /// Frontend owner (F12): shell/menu/score/dialog flow state.
    pub(crate) frontend: crate::app::frontend::state::FrontendState,
    /// Match owner (F12): everything scoped to the running (or last) match.
    pub(crate) match_state: crate::app::match_runtime::state::MatchState,
    /// Process-wide asset ownership (F11): the one retail MIX manager for
    /// the process, leased to the loading pipeline and always returned.
    pub(crate) process_assets: crate::app::process_assets::ProcessAssets,
    /// Process-wide audio owner (F12): players and registries.
    pub(crate) audio: crate::app::audio_runtime::AppAudioRuntime,
    /// Save repository, cached listing, and last save/load metadata.
    pub(crate) persistence: crate::app::persistence::PersistenceState,
}

/// Drop app-owned scenario-exit runtime after a successful world replacement.
/// Serialized HouseState remains the sole authority for any loaded SavourDelay;
/// wall waits are reconstructed from its expiry latch without replaying EVA.
pub(crate) fn reset_scenario_exit_runtime(state: &mut AppState) {
    state.match_state.scenario_outcome = None;
    state.match_state.scenario_exit = None;
    state.audio.cancel_scenario_theme_request();
    state.audio.set_master_output_scale(1.0);
}

impl AppState {
    /// Current projection: frontend/result use physical window pixels; tactical
    /// rendering and pending tactical installation use the optional upscaler.
    /// Loading artwork explicitly uses GPU/window dimensions in loading::pump.
    fn render_dimensions(&self) -> (u32, u32) {
        platform::render_dimensions(
            &self.frontend.screen,
            (
                self.renderer.gpu.config.width,
                self.renderer.gpu.config.height,
            ),
            self.renderer
                .upscale_pass
                .as_ref()
                .map(|up| (up.src_width(), up.src_height())),
        )
    }

    pub(crate) fn render_width(&self) -> u32 {
        self.render_dimensions().0
    }

    pub(crate) fn render_height(&self) -> u32 {
        self.render_dimensions().1
    }

    /// Modal shells use physical window pixels, while the retained tactical
    /// cursor stays in battlefield source pixels even during an upscaled match.
    pub(crate) fn window_cursor_position(&self) -> (f32, f32) {
        (
            self.match_state.input.cursor_x * self.renderer.gpu.config.width as f32
                / self.render_width().max(1) as f32,
            self.match_state.input.cursor_y * self.renderer.gpu.config.height as f32
                / self.render_height().max(1) as f32,
        )
    }

    /// Whether the software cursor (mouse.shp) should be active this frame.
    /// Asset-independent status cards and diagnostic modals use the OS cursor.
    pub(crate) fn use_software_cursor(&self) -> bool {
        !(self.frontend.screen == crate::ui::game_screen::GameScreen::MainMenu
            && self.frontend.main_menu_shell_error.is_some())
            && self
                .match_state
                .match_presentation
                .software_cursor
                .is_some()
            && (!self.match_state.paused()
                || crate::app::frontend::skirmish_shell_render::native_in_game_shell_active(self))
            && !self.main_menu_dialog_open()
    }

    /// Optional GUI visibility, shared by input routing and capture admission.
    /// Native/world debug overlays remain owned by their existing toggles.
    pub(crate) fn diagnostic_gui_visible(&self) -> bool {
        #[cfg(feature = "dev-ui")]
        {
            self.frontend.screen == crate::ui::game_screen::GameScreen::InGame
                && !crate::app::frontend::skirmish_shell_render::native_in_game_shell_active(self)
                && (self.match_state.debug_pause
                    || self.diag.debug_show_pathgrid
                    || self.diag.debug_unit_inspector
                    || self.match_state.match_presentation.show_hotkey_help)
        }
        #[cfg(not(feature = "dev-ui"))]
        {
            false
        }
    }

    /// Whether any main-menu modal dialog (exit confirm, options, keyboard)
    /// is currently open.
    pub(crate) fn main_menu_dialog_open(&self) -> bool {
        self.frontend.exit_confirm_modal.is_some()
            || self.frontend.options_dialog.is_some()
            || self.frontend.keyboard_dialog.is_some()
    }

    /// Return the building-placement section name if the targeting mode
    /// is set to `BuildingPlacement`, else `None`.
    pub(crate) fn armed_building_type(&self) -> Option<&str> {
        self.match_state
            .input
            .targeting_mode
            .as_ref()
            .and_then(crate::app::types::TargetingMode::as_building_placement)
    }

    /// Return the SW section name if the targeting mode is set to
    /// `SuperWeapon`, else `None`.
    pub(crate) fn armed_super_weapon_type(&self) -> Option<&str> {
        self.match_state
            .input
            .targeting_mode
            .as_ref()
            .and_then(crate::app::types::TargetingMode::as_super_weapon)
    }
}

impl AppState {
    /// Immutable view of the running simulation (F10): the read boundary
    /// presentation cones consume. `None` outside a match. Sites that also
    /// hold `&mut` app fields keep the `sim_runtime` field chain and call
    /// `rt.view()` directly for split borrows.
    pub(crate) fn sim_view(&self) -> Option<crate::sim::runtime::SimView<'_>> {
        self.match_state.sim_runtime.as_ref().map(|rt| rt.view())
    }

    /// Load-time cell levels the input and presentation click resolution
    /// reads (see `MatchPresentationState::height_map`).
    pub(crate) fn height_map(&self) -> &BTreeMap<(u16, u16), u8> {
        &self.match_state.match_presentation.height_map
    }

    /// Load-time high-bridge deck levels for the same click resolution.
    pub(crate) fn bridge_height_map(&self) -> &BTreeMap<(u16, u16), u8> {
        &self.match_state.match_presentation.bridge_height_map
    }
}

impl AppState {
    /// The overlay registry: runtime-bound during a match, shell-retained
    /// (last loaded) otherwise — exactly the old field's lifecycle.
    pub(crate) fn overlay_registry(&self) -> Option<&OverlayTypeRegistry> {
        self.match_state
            .sim_runtime
            .as_ref()
            .map(|rt| &rt.resources.overlay_registry)
            .or(self.frontend.shell_preview_overlay_registry.as_ref())
    }
}

impl AppState {
    /// The active rules: runtime-bound during a match, startup-shell rules
    /// otherwise. Matches the old field's Option shape at every consumer.
    pub(crate) fn rules(&self) -> Option<&crate::rules::ruleset::RuleSet> {
        self.match_state
            .sim_runtime
            .as_ref()
            .map(|rt| &rt.resources.rules)
            .or(self.frontend.frontend_rules.as_ref())
    }
}

impl AppState {
    /// The immutable base resolved-terrain template for the active match
    /// (static rendering + restore); never the live sim grid.
    pub(crate) fn terrain_template(&self) -> Option<&ResolvedTerrainGrid> {
        self.match_state
            .sim_runtime
            .as_ref()
            .and_then(|rt| rt.resources.terrain_template.as_ref())
    }
}
