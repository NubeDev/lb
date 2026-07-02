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

/// Update the durable status's restart count after a (operator or crash) restart. A restart re-opens
/// the fault window, so `healthy_since` is reset to `ts`: the cool-off clock for decay restarts from
/// the fresh child (a sidecar must serve cleanly for the whole window AFTER its last restart).
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
        status.healthy_since = Some(ts);
        status.ts = ts;
        record_status(&node.store, ws, &status).await?;
    }
    Ok(())
}

/// **Reset** the native sidecar `ext_id`: re-arm its restart budget and force a fresh child even if
/// it has already crash-looped past `max_restarts` (native-tier resilience — the operator rescue for
/// a permanently-exhausted sidecar, distinct from `restart` which is bounded by the budget and
/// refuses when exhausted). Gated `mcp:native.reset:call`, workspace-first, reaching only THIS
/// workspace's sidecar (the `SidecarMap` is keyed `(ws, ext_id)`).
///
/// Unlike `restart_native`, this recovers a sidecar whose handle is present-but-dead (the exhausted
/// state leaves the handle in the map with a closed channel): it `rearm`s that handle. If no handle
/// exists at all (never started / already removed) it is `NotRunning` — use `ext.enable`/install to
/// start a stopped extension. Resets the durable `restart_count` to 0 and re-opens the cool-off clock.
pub async fn reset_native<L: lb_supervisor::Launcher>(
    node: &Node,
    launcher: &L,
    caller: &Principal,
    ws: &str,
    ext_id: &str,
    ts: u64,
) -> Result<(), NativeServiceError> {
    authorize_native(caller, ws, "reset")?;

    let handle = node
        .sidecars
        .get(ws, ext_id)
        .ok_or(NativeServiceError::NotRunning)?;
    {
        let mut sidecar = handle.lock().await;
        sidecar.rearm(launcher).await?;
    }
    // The budget is re-armed and the child is fresh — zero the durable count and re-open the cool-off
    // clock so the record matches the in-memory sidecar (restart_count 0, healthy from now).
    if let Some(mut status) = read_status(&node.store, ws, ext_id).await? {
        status.restart_count = 0;
        status.lifecycle = Lifecycle::Started;
        status.healthy_since = Some(ts);
        status.ts = ts;
        record_status(&node.store, ws, &status).await?;
    }
    Ok(())
}

/// The decay path: after a sidecar has served a call cleanly, clear its restart accounting if it has
/// been healthy for the whole cool-off window since its last restart (a transient crash no longer
/// permanently poisons the budget). Reads the durable `healthy_since` + the sidecar's `cooloff`; if
/// elapsed and the count is non-zero, zeroes BOTH the in-memory counter (so a later fault gets the
/// full budget) and the durable `restart_count`. Best-effort and cheap: called on the success branch
/// of a sidecar call; any read/write hiccup is swallowed (decay is an optimization, not correctness).
pub(crate) async fn decay_if_healthy(
    node: &Node,
    handle: &std::sync::Arc<tokio::sync::Mutex<lb_supervisor::Sidecar>>,
    ws: &str,
    ext_id: &str,
    now: u64,
) {
    let cooloff = {
        let sidecar = handle.lock().await;
        if sidecar.restarts() == 0 {
            return; // nothing to decay
        }
        sidecar.cooloff().as_millis() as u64
    };
    if cooloff == 0 {
        return; // decay disabled by config; only an explicit reset clears the count
    }
    let Ok(Some(mut status)) = read_status(&node.store, ws, ext_id).await else {
        return;
    };
    if status.restart_count == 0 {
        return;
    }
    let healthy_since = status.healthy_since.unwrap_or(now);
    if now.saturating_sub(healthy_since) < cooloff {
        return; // not healthy long enough yet
    }
    // Sustained-healthy: re-arm the budget in memory and persist the cleared count.
    {
        let mut sidecar = handle.lock().await;
        sidecar.reset_restarts();
    }
    status.restart_count = 0;
    status.healthy_since = Some(now);
    status.ts = now;
    let _ = record_status(&node.store, ws, &status).await;
}
