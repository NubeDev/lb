//! Native sidecar **lifecycle** verbs — `stop` / `restart` / `status` (native-tier scope). `start`
//! is `install_native` (spawn-from-record); these are the operator controls over a running child.
//! Each is capability-gated (`mcp:native.<verb>:call`, workspace-first) and reaches only THIS
//! workspace's sidecar (the `SidecarMap` is keyed `(ws, ext_id)`) — a ws-B operator can never stop or
//! restart a ws-A child even with the grant (gate 1 refuses first; the map would miss anyway).
//!
//! `restart` here is the OPERATOR restart (cooperative stop → re-spawn), distinct from the
//! supervisor's automatic crash-restart policy (which fires on a dead child during a `call`). Both
//! re-derive the child from the durable records — no durable state is lost (§3.4).

use lb_auth::Principal;

use super::authorize::authorize_native;
use super::error::NativeServiceError;
use super::status::{read_status, record_status, Lifecycle, NativeStatus};
use crate::boot::Node;

/// Cooperatively stop the native sidecar `ext_id` in workspace `ws` as `caller`. Drains the child
/// (a `shutdown` notification, escalating to a process-group kill) and records `lifecycle = Stopped`.
/// Refuses without the grant; `NotRunning` if no sidecar is live here.
pub async fn stop_native(
    node: &Node,
    caller: &Principal,
    ws: &str,
    ext_id: &str,
    ts: u64,
) -> Result<(), NativeServiceError> {
    authorize_native(caller, ws, "stop")?;
    // The OPERATOR stop requires a live child — a missing sidecar is `NotRunning` (the contract the
    // native-tier isolation/lifecycle tests rely on). The host-internal cascade uses the idempotent
    // variant below.
    if !node.sidecars.is_running(ws, ext_id) {
        return Err(NativeServiceError::NotRunning);
    }
    stop_sidecar_internal(node, ws, ext_id, ts).await
}

/// Stop the live native sidecar `ext_id` in `ws` and record `Stopped` — WITHOUT a capability gate
/// and **idempotently** (a missing sidecar is a no-op success). For host-internal cascades
/// (`ext.disable`/`ext.uninstall`) where the caller was already authorized by the ext gate and a
/// not-running extension must not error; reuses the one shutdown+status path.
pub(crate) async fn stop_sidecar_internal(
    node: &Node,
    ws: &str,
    ext_id: &str,
    ts: u64,
) -> Result<(), NativeServiceError> {
    if let Some(handle) = node.sidecars.remove(ws, ext_id) {
        handle.lock().await.shutdown().await;
    }
    if let Some(mut status) = read_status(&node.store, ws, ext_id).await? {
        status.lifecycle = Lifecycle::Stopped;
        status.ts = ts;
        record_status(&node.store, ws, &status).await?;
    }
    Ok(())
}

/// Operator-restart the native sidecar `ext_id`: cooperative stop, then re-spawn from the SAME live
/// handle's spec (the sidecar's `restart` kills + re-launches + re-handshakes). The restart count in
/// the durable status increments. Refuses without the grant; `NotRunning` if not live here.
///
/// Generic over the launcher so tests inject a fake and the real path uses `OsLauncher` — the same
/// seam `install_native` uses.
pub async fn restart_native<L: lb_supervisor::Launcher>(
    node: &Node,
    launcher: &L,
    caller: &Principal,
    ws: &str,
    ext_id: &str,
    ts: u64,
) -> Result<u32, NativeServiceError> {
    authorize_native(caller, ws, "restart")?;

    let handle = node
        .sidecars
        .get(ws, ext_id)
        .ok_or(NativeServiceError::NotRunning)?;
    {
        let mut sidecar = handle.lock().await;
        sidecar.restart(launcher).await?;
    }
    let restarts = handle.lock().await.restarts();
    bump_restart_count(node, ws, ext_id, restarts, ts).await?;
    Ok(restarts)
}

/// Read the durable [`NativeStatus`] for `ext_id` in workspace `ws` as `caller`. `None` if not
/// installed here. The status reflects the last durable change (lifecycle intent + restart count);
/// the *live* running flag is on the runtime map (`is_running`) and merged by the MCP bridge.
pub async fn status_native(
    node: &Node,
    caller: &Principal,
    ws: &str,
    ext_id: &str,
) -> Result<Option<NativeStatus>, NativeServiceError> {
    authorize_native(caller, ws, "status")?;
    Ok(read_status(&node.store, ws, ext_id).await?)
}

/// Update the durable status's restart count after a (operator or crash) restart.
pub(crate) async fn bump_restart_count(
    node: &Node,
    ws: &str,
    ext_id: &str,
    restarts: u32,
    ts: u64,
) -> Result<(), NativeServiceError> {
    if let Some(mut status) = read_status(&node.store, ws, ext_id).await? {
        status.restart_count = restarts;
        status.lifecycle = Lifecycle::Started;
        status.ts = ts;
        record_status(&node.store, ws, &status).await?;
    }
    Ok(())
}
