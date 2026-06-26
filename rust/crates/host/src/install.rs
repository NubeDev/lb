//! Install an extension into a workspace — the S4 durable install flow (README §6.4,
//! extensions scope). S1 passed `admin_approved` in on every `load_extension`; S4 **persists**
//! the computed grant set as an [`Install`] record so it survives a restart and becomes the
//! workspace's durable answer to "what is this extension allowed here?" (the extensions-scope
//! open question: "where the admin-approval set is stored").
//!
//! Flow: parse the manifest, compute `granted = requested ∩ admin_approved` (the loader's
//! enforcement point), **record the install** in the workspace namespace, then load the
//! component into the runtime. Persist-before-load mirrors the channel persist-before-publish
//! discipline — the durable approval is the source of truth, the running instance follows it.

use lb_assets::{record_install, Install};
use lb_ext_loader::{grant, Manifest};
use lb_store::StoreError;

use crate::boot::Node;
use crate::load::{load_extension, LoadError, Loaded};

/// Install `wasm_bytes` (described by `manifest_toml`) into `node` for workspace `ws`: persist
/// the `requested ∩ admin_approved` grant set as a durable install record, then load. `ts` is a
/// caller-injected logical timestamp (testing §3 determinism). Returns the loaded result.
pub async fn install_extension(
    node: &Node,
    ws: &str,
    manifest_toml: &str,
    wasm_bytes: &[u8],
    admin_approved: &[String],
    ts: u64,
) -> Result<Loaded, LoadError> {
    let manifest =
        Manifest::parse(manifest_toml).map_err(|e| LoadError::Manifest(e.to_string()))?;
    let granted = grant(&manifest, admin_approved);

    // STATE: persist the approved grant set first — the durable record of what was allowed.
    let install = Install::new(
        manifest.id.clone(),
        manifest.version.clone(),
        granted.clone(),
        ts,
    );
    record_install(&node.store, ws, &install)
        .await
        .map_err(store_to_load)?;

    // Then bring the component online with exactly that approved set.
    load_extension(node, manifest_toml, wasm_bytes, admin_approved).await
}

fn store_to_load(e: StoreError) -> LoadError {
    LoadError::Manifest(format!("persisting install record: {e}"))
}
