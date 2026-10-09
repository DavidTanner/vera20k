//! Diagnostics owner (F12): process debug/capture UI plus the hidden
//! shell/tactical capture harnesses. Match-lifetime diagnostic replay lives
//! in `app::match_diagnostics`.

pub(crate) mod debug_overlays;
pub(crate) mod state;
#[cfg(feature = "dev-ui")]
pub(crate) mod debug_panel;
#[cfg(feature = "dev-ui")]
pub(crate) mod dev_overlay;
pub(crate) mod shell_capture;
pub(crate) mod tactical_capture;
