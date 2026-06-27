//! `ext.uninstall` — stop/unload the instance + delete the durable `Install`, one logical op
//! (lifecycle-management scope). Gated `mcp:ext.uninstall:call`, **workspace-first** (the workspace
//! is resolved before any record is touched, so a ws-B caller can never uninstall a ws-A extension).
//! Idempotent: uninstalling an absent extension is a success, never a cross-workspace delete.
//!
//! Order matters — stop the live instance first (a native sidecar is cooperatively shut down via the
//! host-internal stop; a wasm component has no separate process), THEN tombstone the `Install` so
//! `ext.list`/the loader read it as absent. The cached binary eviction is the registry cache's
//! concern (a follow-up); the load-bearing delete here is the durable record (the loader consults it
//! on the next load, so the extension does not come back).

use lb_assets::{delete_install, read_install, Tier};
use lb_auth::Principal;
use lb_mcp::authorize_tool;

use super::error::ExtError;
use crate::boot::Node;
use crate::native::stop_sidecar_internal;

/// Uninstall `ext_id` in workspace `ws` for `caller`: stop the instance, delete the install record.
/// Idempotent; workspace-first.
pub async fn ext_uninstall(
    node: &Node,
    caller: &Principal,
    ws: &str,
    ext_id: &str,
    ts: u64,
) -> Result<(), ExtError> {
    authorize_tool(caller, ws, "ext.uninstall").map_err(|_| ExtError::Denied)?;
    // Stop a running native child first (no-op for wasm / not-running) — host-internal, the
    // ext.uninstall gate already authorized this caller.
    if let Some(install) = read_install(&node.store, ws, ext_id).await? {
        if install.tier == Tier::Native {
            stop_sidecar_internal(node, ws, ext_id, ts)
                .await
                .map_err(|e| ExtError::Native(e.to_string()))?;
        }
    }
    // Tombstone the durable install — read-as-absent everywhere; survives sync (no resurrection).
    delete_install(&node.store, ws, ext_id).await?;
    Ok(())
}
