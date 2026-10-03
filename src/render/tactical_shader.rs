//! Assembly of shared tactical palette and stored-depth shader mechanisms.

pub(crate) fn source(body: &str) -> String {
    super::palette_light::shader_source(&format!("{}\n{}", include_str!("native_z.wgsl"), body))
}

/// World source blitters use the shared Camera + A binding. Packed destination
/// resolve and LineTrail keep their own binding/operation classes and include
/// only palette math and stored-Z helpers through `source` above.
pub(crate) fn world_source(body: &str) -> String {
    source(&format!("{}\n{}", include_str!("tactical_a.wgsl"), body))
}
