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
// Not yet invoked from `main`: `serve_agent` (the routed agent surface this registry feeds) is booted
// where the gateway wires the agent, which is the serve-wiring TODO above. The function is the real,
// compiled registration path the role-crate tests exercise; wiring the boot call is the next step.
#[cfg(feature = "external-agent")]
pub fn register_external_runtimes(
    registry: &mut lb_host::RuntimeRegistry,
    model: lb_role_external_agent::ModelEndpoint,
    scratch_base: Option<std::path::PathBuf>,
) {
    lb_role_external_agent::register(registry, model, scratch_base);
}

/// Install the node's agent runtime registry from boot (external-agent runtime-seam #1). **Feature-ON:**
/// build a registry with the in-house `default` (over the [`UnconfiguredModel`] placeholder — the
/// in-house model provider is the separate `agent_invoke` gap) PLUS the external `AcpRuntime` entries
/// (Open Interpreter default over Z.AI GLM-4.6, VT Code, Codex), then install it on the node — so the
/// in-channel agent worker can drive `runtime:"open-interpreter-default"` for real. **Feature-OFF:** a
/// no-op; the node keeps the default-only registry `Node::boot` installed. This is the ONE place the
/// feature changes node behaviour (rule 1: feature + config, never `if cloud {…}`).
///
/// The default model endpoint comes from `default_model_endpoint()` (Z.AI coding endpoint, key env
/// `ZAI_API_KEY` — the *name*, never a value); a real deployment overrides it by config.
#[cfg(feature = "external-agent")]
pub fn install(node: &lb_host::Node) {
    use std::sync::Arc;
    let model = lb_role_external_agent::profiles::default_model_endpoint();
    let mut registry = lb_host::RuntimeRegistry::with_default(Arc::new(lb_host::UnconfiguredModel));
    register_external_runtimes(&mut registry, model, None);
    let ids = registry.ids();
    node.install_runtimes(registry);
    println!("external-agent: runtimes installed = {ids:?}");
}

/// Feature-OFF: installing external runtimes is a no-op (the node keeps its default-only registry).
#[cfg(not(feature = "external-agent"))]
pub fn install(_node: &lb_host::Node) {}
