//! Render-agnostic UI models plus egui-backed menus and dialogs.
//!
//! Some screens use egui when they do not need pixel-perfect RA2 art. Pixel
//! parity shells and the in-game sidebar keep layout and state here, while
//! app/render layers draw them.
//!
//! ## Dependency rules
//! - ui/ depends on: sim/ (reads game state, produces commands)
//! - ui/ does NOT depend on: assets/, render/, audio/, net/

pub mod campaign_shell;
pub mod client_theme;
pub mod gadget;
pub mod game_screen;
pub mod main_menu;
pub mod main_menu_dialogs;
pub mod main_menu_shell;
pub mod messages;
pub mod mission_status;
pub mod movies_credits_shell;
pub mod pause_menu;
pub mod score_shell;
pub mod shell;
pub mod sidebar;
pub mod single_player_shell;
pub mod skirmish_shell;
pub mod tooltips;
pub mod wol_shell;
// pub mod skirmish;
// pub mod dialog;
// pub mod settings;
