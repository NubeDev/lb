//! `ext.enable` / `ext.disable` — the durable lifecycle **intent**, distinct from `start`/`stop`
//! (lifecycle-management scope, the load-bearing distinction). `disable` means "do not run this, and
//! do not auto-start it on boot": it flips the `Install.enabled` flag false AND stops a running
//! native sidecar now; `enable` flips it true — it spawns nothing itself, so bring it up now with
//! [`ext.start`](super::ext_start) or leave it to the boot reconciler. A
//! `stop` (native lifecycle) only stops the live instance — boot would restart it; `disable` would
//! not. Conflating them lets a "disabled" extension silently return after a restart — the bug this
//! split prevents.
//!
//! Gated `mcp:ext.disable:call`, workspace-first. Idempotent: disabling an already-disabled (or
//! absent) extension is a no-op success; never crosses the workspace wall.

use lb_assets::{read_install, record_install, Tier};
use lb_auth::Principal;
use lb_mcp::authorize_tool;

use super::error::ExtError;
use crate::boot::Node;
use crate::native::stop_sidecar_internal;

/// Mark `ext_id` enabled in workspace `ws` (eligible to run / auto-start). Idempotent.
pub async fn ext_enable(
    node: &Node,
    caller: &Principal,
    ws: &str,
    ext_id: &str,
    ts: u64,
) -> Result<(), ExtError> {
    set_enabled(node, caller, ws, ext_id, true, ts).await
}

/// Mark `ext_id` disabled in workspace `ws` and stop any running native instance now. Idempotent.
pub async fn ext_disable(
    node: &Node,
    caller: &Principal,
    ws: &str,
    ext_id: &str,
    ts: u64,
) -> Result<(), ExtError> {
    set_enabled(node, caller, ws, ext_id, false, ts).await
}

/// Flip the durable `enabled` intent for `ext_id`; on `disable`, stop a running native sidecar.
async fn set_enabled(
    node: &Node,
    caller: &Principal,
    ws: &str,
    ext_id: &str,
    enabled: bool,
    ts: u64,
) -> Result<(), ExtError> {
    authorize_tool(caller, ws, "ext.disable").map_err(|_| ExtError::Denied)?;
    let Some(mut install) = read_install(&node.store, ws, ext_id).await? else {
        return Ok(()); // absent → nothing to flip; idempotent success.
    };
    install.enabled = enabled;
    install.ts = ts;
    record_install(&node.store, ws, &install).await?;
    // Disabling a native extension stops its live child now (start is re-derivable from the record).
    // Host-internal stop (no extra cap) — the ext.disable gate already authorized this caller.
    if !enabled && install.tier == Tier::Native && node.sidecars.is_running(ws, ext_id) {
        stop_sidecar_internal(node, ws, ext_id, ts)
            .await
            .map_err(|e| ExtError::Native(e.to_string()))?;
    }
    Ok(())
}
