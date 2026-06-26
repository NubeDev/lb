//! Hot-reload an extension: swap a live component for a new version with NO loss of durable
//! state (the stateless-extension guarantee, §3.4).
//!
//! Why this is safe — and the whole point of the principle: an extension instance holds no
//! durable state. Everything that must survive lives in the store or on the bus. So replacing
//! the instance (even with a different version) cannot drop state: there is none in the
//! instance to drop. This verb re-parses the (possibly new) manifest, re-instantiates the
//! component, and replaces the registry entry — the store the registry sits beside is never
//! touched. A caller's in-flight channel history, presence, etc. are all in `lb_store`/`lb_bus`
//! and ride straight through.
//!
//! `register` already overwrites an existing id, so a reload of the same id is a true swap.

use lb_ext_loader::{grant, Manifest};

use crate::boot::Node;
use crate::load::{LoadError, Loaded};

/// Reload `manifest_toml` + `wasm_bytes` for an already-hosted extension, replacing its live
/// instance in place. Returns the freshly granted caps + tools, exactly like `load_extension`.
/// The extension id in the manifest must already be hosted (it is a *re*-load) — loading a
/// brand-new id is `load_extension`'s job.
pub async fn reload_extension(
    node: &mut Node,
    manifest_toml: &str,
    wasm_bytes: &[u8],
    admin_approved: &[String],
) -> Result<Loaded, LoadError> {
    let manifest =
        Manifest::parse(manifest_toml).map_err(|e| LoadError::Manifest(e.to_string()))?;

    if !node.registry.is_hosted(&manifest.id) {
        return Err(LoadError::Manifest(format!(
            "reload of '{}' which is not currently hosted (use load_extension to install)",
            manifest.id
        )));
    }

    let granted = grant(&manifest, admin_approved);
    let instance = node
        .engine
        .load(wasm_bytes)
        .await
        .map_err(|e| LoadError::Runtime(e.to_string()))?;

    let tools: Vec<String> = manifest.tools.iter().map(|t| t.name.clone()).collect();
    // Overwrites the existing entry — the live swap. The store is untouched (durable state
    // survives), which is exactly the stateless-extension guarantee.
    node.registry
        .register(manifest.id.clone(), tools.clone(), instance);

    Ok(Loaded {
        granted_caps: granted,
        tools,
    })
}
