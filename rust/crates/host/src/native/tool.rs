//! The `native.*` MCP bridge + the child tool-dispatch verb (native-tier scope, README §6.5: the
//! supervisor control plane is itself reached as MCP tools under the one contract). The UI and other
//! extensions reach a sidecar the SAME way they reach any tool — a qualified `native.<verb>` call.
//!
//! Split, like the registry's bridge: the **store-only read verb** (`status`) is bridged here for
//! the UI to drive directly; `install`/`stop`/`restart`/`call` are TYPED entries (in `install`/
//! `lifecycle`/here) because they need the `Launcher` seam (environment, not JSON args) — exactly
//! like `workflow::triage` needs a `ModelAccess`. The MCP gate runs first in every case
//! (`authorize_native` — workspace-first, then `mcp:native.<verb>:call`).
//!
//! `call_sidecar` is the child-dispatch verb: it resolves the live sidecar for `(ws, ext_id)` and
//! sends one control-line `call`. If the child has DIED (a transport fault), it applies the restart
//! policy ON DEMAND — re-spawn via the launcher, then retry once. That is the supervision crash-path
//! proven by the exit gate: a killed sidecar is restarted cleanly and the call still answers, with no
//! durable state lost (the child held none).

use lb_auth::Principal;
use lb_mcp::ToolError;
use lb_supervisor::{Launcher, SupervisorError};
use serde_json::{json, Value};

use super::authorize::authorize_native;
use super::error::NativeServiceError;
use super::lifecycle::{bump_restart_count, status_native};
use crate::boot::Node;

/// Dispatch a `native.<verb>` MCP call for the **store-only read verbs** (the UI path). `status`
/// merges the durable record with the live running flag. `install`/`stop`/`restart`/`call` are not
/// here — they need the launcher seam and have typed entries. The MCP gate runs first.
pub async fn call_native_tool(
    node: &Node,
    principal: &Principal,
    ws: &str,
    qualified_tool: &str,
    input: &Value,
) -> Result<Value, ToolError> {
    let verb = qualified_tool
        .split_once('.')
        .map(|(_, v)| v)
        .unwrap_or(qualified_tool);

    match verb {
        "status" => {
            let ext_id = str_arg(input, "ext_id")?;
            // authorize_native is called inside status_native (workspace-first) — but call it here
            // too so the deny path is identical to the other bridged surfaces. Cheap + explicit.
            authorize_native(principal, ws, "status").map_err(ns_to_tool)?;
            let status = status_native(node, principal, ws, ext_id)
                .await
                .map_err(ns_to_tool)?;
            let running = node.sidecars.is_running(ws, ext_id);
            Ok(json!({ "status": status, "running": running }))
        }
        _ => Err(ToolError::NotFound),
    }
}

/// Dispatch tool `tool` of native extension `ext_id` to its live child, as `caller` in `ws`. Gated
/// `mcp:native.call:call`. On a transport fault (the child died), restart-on-demand and retry once.
/// Generic over the launcher (the seam tests inject a fake into and the real path uses `OsLauncher`).
pub async fn call_sidecar<L: Launcher>(
    node: &Node,
    launcher: &L,
    caller: &Principal,
    ws: &str,
    ext_id: &str,
    tool: &str,
    input: &str,
    ts: u64,
) -> Result<String, NativeServiceError> {
    authorize_native(caller, ws, "call")?;

    let handle = node
        .sidecars
        .get(ws, ext_id)
        .ok_or(NativeServiceError::NotRunning)?;

    // Attempt-then-restart-and-retry is shared with the registry adapter via `call_once_or_restart`
    // (one place owns the fault shape). Here the recovery IS the supervision proof: apply the
    // crash-restart policy via the launcher and bump the durable restart count, so a killed sidecar
    // is restarted cleanly and the call still answers.
    let out = super::call::call_once_or_restart(&handle, tool, input, || async {
        let restarts = {
            let mut sidecar = handle.lock().await;
            sidecar.restart(launcher).await?;
            sidecar.restarts()
        };
        bump_restart_count(node, ws, ext_id, restarts, ts)
            .await
            .map_err(|e| SupervisorError::Transport(e.to_string()))?;
        Ok(())
    })
    .await?;
    // The call answered — the child is alive. Decay its restart accounting if it has been healthy for
    // the cool-off window (a transient crash no longer permanently exhausts the budget). Best-effort:
    // no launcher needed, and a hiccup here never fails the call the caller already got an answer to.
    super::lifecycle::decay_if_healthy(node, &handle, ws, ext_id, ts).await;
    Ok(out)
}

/// Map the native service error onto the MCP tool error. `Denied` stays opaque; the rest surface as
/// distinguishable client errors (a missing/faulted sidecar is not a hidden resource).
pub fn ns_to_tool(e: NativeServiceError) -> ToolError {
    match e {
        NativeServiceError::Denied => ToolError::Denied,
        NativeServiceError::NotRunning => ToolError::BadInput("sidecar not running".into()),
        NativeServiceError::NotNative(m) => ToolError::BadInput(format!("not native: {m}")),
        NativeServiceError::Supervisor(s) => ToolError::Extension(s.to_string()),
        NativeServiceError::Store(s) => ToolError::Extension(s.to_string()),
        NativeServiceError::Load(l) => ToolError::Extension(l.to_string()),
    }
}

fn str_arg<'a>(input: &'a Value, key: &str) -> Result<&'a str, ToolError> {
    input
        .get(key)
        .and_then(|v| v.as_str())
        .ok_or_else(|| ToolError::BadInput(format!("missing string arg: {key}")))
}
