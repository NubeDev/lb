//! Dispatch one tool call to a live sidecar, with the shared **crash-restart-on-fault** discipline
//! (native-tier scope). Both the typed lifecycle path (`call_sidecar`) and the Tier-agnostic
//! registry adapter ([`SidecarDispatch`]) go through [`call_once_or_restart`] so the fault handling
//! lives in ONE place, not two.
//!
//! The fault path is: attempt the call; if the child DIED mid-call (`Transport`/`Child`), run the
//! caller-supplied `on_fault` recovery (re-spawn + bump the durable restart count) and retry once.
//! `call_sidecar` supplies a recovery that restarts via the `Launcher` and bumps the store record —
//! the supervision proof. The registry adapter supplies a NO-OP recovery: it is stored node-global
//! with no `Launcher` in hand (the `Launcher` trait is `impl Future`, not object-safe, so the
//! adapter cannot own one), so a transport fault surfaces cleanly to the routed caller rather than
//! silently swallowing supervision. A subsequent lifecycle `restart`/`call` (which DO carry the
//! launcher) then recovers the child — no supervision is lost, it is just driven by the typed path.

use std::future::Future;
use std::sync::Arc;

use lb_runtime::{CallContext, LocalDispatch, RuntimeError};
use lb_supervisor::{Caller, Sidecar, SupervisorError};
use tokio::sync::Mutex as AsyncMutex;

use super::registry::SidecarMap;

/// Call `tool` on `handle`'s sidecar; on a mid-call fault, run `on_fault` (recovery) and retry once.
/// The recovery is what differs between the typed path (restart + store bump) and the adapter
/// (no-op) — the attempt/fault/retry shape is shared. `on_fault` runs with the handle unlocked so it
/// may re-lock to restart.
pub(super) async fn call_once_or_restart<F, Fut>(
    handle: &Arc<AsyncMutex<Sidecar>>,
    tool: &str,
    input: &str,
    caller: Option<Caller>,
    on_fault: F,
) -> Result<String, SupervisorError>
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = Result<(), SupervisorError>>,
{
    let first = {
        let mut sidecar = handle.lock().await;
        sidecar.call_with_caller(tool, input, caller.clone()).await
    };
    match first {
        Ok(out) => Ok(out),
        // The child died mid-call (no reply came back): run the recovery, then retry once.
        // `Child(_)` is NOT a fault — it is the child's ordinary error REPLY over a healthy line
        // (a failed SQL query, a bad arg). Restarting on it burned the whole restart budget on
        // five failed queries and took federation dark mid-run (live); the child was never down.
        Err(SupervisorError::Transport(_)) => {
            on_fault().await?;
            let mut sidecar = handle.lock().await;
            // Re-stamp the same caller on the retry — the restarted child is a fresh process that
            // never saw the first frame, so it must learn the caller again (identity is per-call).
            sidecar.call_with_caller(tool, input, caller).await
        }
        Err(other) => Err(other),
    }
}

/// The Tier-agnostic **native-sidecar adapter**: a [`LocalDispatch`] that forwards a routed/local
/// tool call to the live sidecar for `(ws, ext_id)`. Registered into `lb_mcp::Registry` at install so
/// `resolve`/`dispatch`/`serve_call` reach a native sidecar through the ONE trait — no `if native`
/// branch (§3.1).
///
/// It holds `Arc<SidecarMap>` + `ext_id` (NOT a single ws): the registry is node-global, so the ws
/// is resolved **per call** from the `ws` the trait passes — `SidecarMap.get(ws, ext_id)`. A ws-B
/// routed call thus resolves ws-B's child or `None`; a ws-B call can never reach a ws-A sidecar (the
/// workspace wall stays structural for Tier 2, mirroring the map key).
pub struct SidecarDispatch {
    sidecars: Arc<SidecarMap>,
    ext_id: String,
}

impl SidecarDispatch {
    pub(super) fn new(sidecars: Arc<SidecarMap>, ext_id: impl Into<String>) -> Self {
        Self {
            sidecars,
            ext_id: ext_id.into(),
        }
    }
}

#[async_trait::async_trait]
impl LocalDispatch for SidecarDispatch {
    async fn call_tool(
        &mut self,
        ws: &str,
        tool: &str,
        input_json: &str,
        ctx: Option<CallContext>,
    ) -> Result<String, RuntimeError> {
        // Resolve the live child for THIS workspace — `None` means not running here (or a ws-mismatch
        // that structurally cannot cross the wall).
        let handle = self
            .sidecars
            .get(ws, &self.ext_id)
            .ok_or_else(|| RuntimeError::Call(format!("no running sidecar for {}", self.ext_id)))?;

        // The registry passed the BARE tool name (`dispatch`/`serve_call` unqualify). Re-qualify it
        // as `<ext_id>.<tool>` for the sidecar's control-line ABI, which dispatches on the qualified
        // name (its manifest declares tools qualified). This mirrors the direct `call_sidecar` path,
        // which passes the qualified name through. Generic: `ext_id` + `.` + the bare tool.
        let qualified = format!("{}.{}", self.ext_id, tool);

        // Stamp the authorized caller into the frame so the child can enforce per-caller row
        // visibility (native-caller-identity scope). Projected from `ctx` — a READ of the principal
        // the host already gated (`mcp:<tool>:call` fired first, workspace-first); NOT a token. A
        // routed cross-node call carries no `ctx` (the serve side passes `None`), so the frame is the
        // old shape — the caller-in-frame guarantee is single-node this slice, matching the scope's
        // non-goal. `LB_EXT_TOKEN` remains the child's identity for its OWN callbacks; the frame
        // caller is a separate, inert projection the child reads to attribute a decision.
        let caller = ctx.and_then(|c| c.caller).map(project);

        // No-op recovery: the adapter carries no `Launcher` (see module doc), so a transport fault
        // surfaces; the typed lifecycle path drives supervised restart.
        call_once_or_restart(&handle, &qualified, input_json, caller, || async { Ok(()) })
            .await
            .map_err(|e| RuntimeError::Tool(e.to_string()))
    }
}

/// Map the runtime's [`lb_runtime::Caller`] (carried on `CallContext`, shared with the wasm path)
/// onto the supervisor's wire [`Caller`] (serialized into the sidecar frame). Two identical minimal
/// projections, one per crate boundary: `lb-runtime` stays free of `lb-supervisor` and vice versa —
/// the host is the one place that bridges the wasm-runtime and native-supervisor worlds.
fn project(c: lb_runtime::Caller) -> Caller {
    Caller {
        sub: c.sub,
        ws: c.ws,
        role: c.role,
        delegated: c.delegated,
        admin: c.admin,
    }
}
