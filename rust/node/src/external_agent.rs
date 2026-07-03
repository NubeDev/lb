//! The node binary's **external-agent registration hook** (external-agent runtime-seam #1). This is
//! the ONE place the `external-agent` cargo feature changes node behaviour: with the feature ON it
//! registers the external `AcpRuntime` entries into a runtime registry the caller is building; with it
//! OFF the function is a no-op and the role crate is not even compiled. That difference is the whole of
//! rule 1's "feature + config, never an `if cloud {…}`" for this topic — a feature-off and a
//! feature-on-but-unconfigured node both behave as default-only.
//!
//! The registry itself is built by [`crate::agent::mount`] (default-agent-wiring): it constructs the
//! in-house `default` over the node's configured model, calls [`register_external`] here to add the
//! external entries, installs the registry on the node, and serves the agent. So this crate no longer
//! builds its own placeholder registry — it only contributes the external half.

/// Register the external-agent runtimes into `registry`, if the feature is on. The node's configured
/// external model endpoint (provider/model/api-key-env NAME — never a key value) comes from
/// `default_model_endpoint()`; the scratch root defaults to the OS temp dir.
///
/// **Feature-ON:** add the external `AcpRuntime` entries (Open Interpreter → Z.AI GLM-4.6 default, VT
/// Code, Codex) so a routed/in-channel `runtime:"open-interpreter-default"` reaches a real agent.
/// **Feature-OFF:** a no-op — the registry keeps only the in-house default the caller built.
#[cfg(feature = "external-agent")]
pub fn register_external(registry: &mut lb_host::RuntimeRegistry) {
    let model = lb_role_external_agent::profiles::default_model_endpoint();
    lb_role_external_agent::register(registry, model, None);
}

/// Feature-OFF: no external runtimes to register (the registry keeps only the in-house default).
#[cfg(not(feature = "external-agent"))]
pub fn register_external(_registry: &mut lb_host::RuntimeRegistry) {}
