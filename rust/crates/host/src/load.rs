//! Load an extension into a booted node: parse its manifest, verify the WIT world, compute
//! the granted caps, instantiate the component, and register its declared tools.
//!
//! The grant computation (`requested ∩ admin_approved`) happens here, before the instance is
//! ever callable — nothing requested is live unless the workspace admin approved it
//! (extensions scope, §6.4). In S1 the approved set is passed in by the caller; the install
//! flow that persists it lands at S4/S7.

use lb_ext_loader::{grant, Manifest};
use lb_mcp::ToolDescriptor;
use thiserror::Error;

use crate::boot::Node;

#[derive(Debug, Error)]
pub enum LoadError {
    #[error("manifest invalid: {0}")]
    Manifest(String),
    #[error("runtime failed to load component: {0}")]
    Runtime(String),
}

/// The result of loading: the granted capability strings (for the caller to surface/audit)
/// and the registered tool names.
#[derive(Debug)]
pub struct Loaded {
    pub granted_caps: Vec<String>,
    pub tools: Vec<String>,
}

/// Load `wasm_bytes` described by `manifest_toml` into `node`, granting only the intersection
/// of requested caps with `admin_approved`. Registers the extension's declared tools in the
/// MCP registry so they become callable (after caps).
pub async fn load_extension(
    node: &Node,
    manifest_toml: &str,
    wasm_bytes: &[u8],
    admin_approved: &[String],
) -> Result<Loaded, LoadError> {
    let manifest =
        Manifest::parse(manifest_toml).map_err(|e| LoadError::Manifest(e.to_string()))?;
    let granted = grant(&manifest, admin_approved);

    let instance = node
        .engine
        .load(wasm_bytes)
        .await
        .map_err(|e| LoadError::Runtime(e.to_string()))?;

    let tools: Vec<String> = manifest.tools.iter().map(|t| t.name.clone()).collect();
    let descriptors = descriptors_from(&manifest);
    node.registry
        .register_descriptors(manifest.id.clone(), descriptors, instance);

    Ok(Loaded {
        granted_caps: granted,
        tools,
    })
}

/// Build the schema-bearing tool descriptors for `manifest`: name + title (the manifest
/// description) + group (the ext id) + the optional `input_schema` (channels-command-palette
/// scope). Shared by `load_extension` and `reload_extension` so the two paths cannot drift.
pub(crate) fn descriptors_from(manifest: &Manifest) -> Vec<ToolDescriptor> {
    manifest
        .tools
        .iter()
        .map(|t| ToolDescriptor {
            name: t.name.clone(),
            title: t.description.clone(),
            group: manifest.id.clone(),
            input_schema: t.input_schema.clone(),
            // The self-declared exfiltration taint, carried verbatim (opaque data — rule 10).
            emits_external: t.emits_external,
            // Extension manifests declare no response render (yet); a manifest tool is a plain call.
            result: None,
        })
        .collect()
}
