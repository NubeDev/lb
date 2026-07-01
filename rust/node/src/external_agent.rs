//! The node binary's **external-agent registration hook** (external-agent runtime-seam #1). This is
//! the ONE place the `external-agent` cargo feature changes node behaviour: with the feature ON it
//! registers the external `AcpRuntime` entries into the node's runtime registry; with it OFF the
//! function is absent and the role crate is not even compiled. That difference is the whole of rule
//! 1's "feature + config, never an `if cloud {…}`" for this topic — a feature-off and a
//! feature-on-but-unconfigured node both behave as default-only.
//!
//! The registry itself is built by the wiring layer that serves the agent (`serve_agent`, host):
//! `RuntimeRegistry::with_default(model)` for the in-house loop, then this hook for the external
//! entries. TODO(serve-wiring): boot `serve_agent(node, Arc::new(registry), agent_caps)` where the
//! gateway wires the agent surface, passing a registry this hook has populated. Until then the seam
//! is exercised by the role crate's own registry/swap tests (no fake — a real `Node` + registry).

/// Register the external-agent runtimes into `registry`, if the feature is on. `model` is the node's
/// configured model endpoint (provider/model/api-key-env NAME — never a key value); `scratch_base`
/// is the node's scratch root (defaults to the OS temp dir when `None`).
#[cfg(feature = "external-agent")]
pub fn register_external_runtimes(
    registry: &mut lb_host::RuntimeRegistry,
    model: lb_role_external_agent::ModelEndpoint,
    scratch_base: Option<std::path::PathBuf>,
) {
    lb_role_external_agent::register(registry, model, scratch_base);
}
