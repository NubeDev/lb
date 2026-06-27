//! `call_tool` — the host's generic MCP bridge entry, exposed so a *transport* (the gateway's
//! `POST /mcp/call`) can forward an extension page/widget's `{tool, args}` through the **one** MCP
//! contract (rule 7, ui-federation scope). It runs the workspace-first, then `mcp:<tool>:call`
//! authorize gate and then dispatches — so a bridged caller is denied exactly like any other.
//!
//! Two dispatch families share the one contract:
//!   - **extension tools** (`<ext>.<tool>`, wasm or routed native) — resolved in the runtime
//!     `Registry` and run via `lb_mcp::call`.
//!   - **host-native tools** (`series.*` / `ingest.*`) — NOT in the runtime registry (they are host
//!     verbs over the embedded store, not components), so `lb_mcp::call` alone would `NotFound` them.
//!     The bridge must reach them too: a federated page reads platform data through `series.find` /
//!     `series.latest` here exactly as it would any extension tool. We authorize with the SAME MCP
//!     gate first (opaque `Denied`), then delegate to `call_ingest_tool` (which re-checks its own
//!     store-surface gate). This is the seam the `proof-panel` page exercises end to end; before it,
//!     `/mcp/call` could not dispatch a host-native verb at all (see
//!     debugging/extensions/bridge-cannot-dispatch-host-native-series.md).

use std::sync::Arc;

use lb_assets::read_install;
use lb_auth::Principal;
use lb_inbox::Decision;
use lb_mcp::{authorize_tool, ToolError};
use lb_runtime::CallContext;
use serde_json::{json, Value};

use crate::boot::Node;
use crate::callback::Bridge;
use crate::ingest::call_ingest_tool;
use crate::{list_inbox, outbox_status, resolve_inbox};

/// The host-native verb prefixes the bridge dispatches over the embedded store (not the runtime
/// registry). Kept narrow on purpose — the read-only series surface a federated page reads, `ingest.*`
/// for symmetry, plus the durable-workflow surface (`outbox.status`, `inbox.list`, `inbox.resolve`)
/// the proof-panel demo exercises. Each still passes the per-verb MCP gate first (the bridge scope
/// filter is only defense in depth).
fn is_host_native(qualified_tool: &str) -> bool {
    qualified_tool.starts_with("series.")
        || qualified_tool.starts_with("ingest.")
        || qualified_tool.starts_with("outbox.")
        || qualified_tool.starts_with("inbox.")
}

/// Call `qualified_tool` as `principal` in `ws` with a JSON input string, returning the tool's JSON
/// output. Authorization runs first; the workspace is the caller's (the gateway derives it from the
/// token, never the page). Host-native `series.*`/`ingest.*` verbs dispatch over the store; everything
/// else (`<ext>.<tool>`) routes through the runtime registry / bus.
///
/// This is the outermost (depth-0) entry — the page bridge (`POST /mcp/call`) and the gateway reach
/// it. A re-entrant guest→host callback re-enters at [`call_tool_at_depth`] one level deeper.
pub async fn call_tool(
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
    qualified_tool: &str,
    input_json: &str,
) -> Result<String, ToolError> {
    call_tool_at_depth(node, principal, ws, qualified_tool, input_json, 0).await
}

/// The depth-tracked core of [`call_tool`]. `depth` is 0 for an outermost call and incremented by
/// the host callback on each guest→host hop ([`crate::callback`]). For a **wasm extension** target,
/// the host derives the guest's effective principal (`caller ∩ install-grant`) and installs a
/// [`Bridge`] into the instance so the guest's `host.call-tool` import can re-enter HERE — under that
/// narrowed authority, in this workspace. Host-native verbs and routed/remote targets need no
/// callback (they carry no guest).
pub async fn call_tool_at_depth(
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
    qualified_tool: &str,
    input_json: &str,
    depth: u32,
) -> Result<String, ToolError> {
    if is_host_native(qualified_tool) {
        // Same MCP gate as any tool (workspace-first, then `mcp:<tool>:call`) so a denied bridged
        // caller is opaque and indistinguishable from a missing tool — then delegate to the host verb.
        authorize_tool(principal, ws, qualified_tool)?;
        let input: Value = serde_json::from_str(input_json)
            .map_err(|e| ToolError::BadInput(format!("input json: {e}")))?;
        let out = if qualified_tool.starts_with("outbox.") || qualified_tool.starts_with("inbox.") {
            call_workflow_tool(node, principal, ws, qualified_tool, &input).await?
        } else {
            call_ingest_tool(&node.store, principal, ws, qualified_tool, &input).await?
        };
        return serde_json::to_string(&out).map_err(|e| ToolError::Extension(e.to_string()));
    }

    // An `<ext>.<tool>` target: build the guest's host-callback context so its backend can call
    // host tools under its DELEGATED, INTERSECTED authority (host-callback scope).
    let ctx = build_call_context(node, principal, ws, qualified_tool, depth).await;

    // depth > 0 means this call ORIGINATED from a guest's host-callback (re-entrant): dispatch must
    // not block on the instance lock (it may be the in-flight guest's own) — fail fast instead.
    lb_mcp::call_with_ctx(
        &node.registry,
        &node.bus,
        principal,
        ws,
        qualified_tool,
        input_json,
        ctx,
        depth > 0,
    )
    .await
}

/// Build the [`CallContext`] for a wasm guest call: derive the effective principal
/// `caller ∩ install-grant` and wrap it in a [`Bridge`] the guest's `host.call-tool` dispatches
/// through. Returns `None` when the target isn't an installed extension in this workspace (a routed
/// remote, or an ext with no install record) — the callback is simply unavailable, never widened.
async fn build_call_context(
    node: &Arc<Node>,
    caller: &Principal,
    ws: &str,
    qualified_tool: &str,
    depth: u32,
) -> Option<CallContext> {
    let ext_id = qualified_tool.split_once('.').map(|(e, _)| e)?;
    // The install grant (`requested ∩ admin_approved`, persisted at install) for THIS ext in THIS
    // workspace — the upper bound on what the guest's callback may reach.
    let install = read_install(&node.store, ws, ext_id).await.ok().flatten()?;
    // effective = caller ∩ install-grant: `derive` sets the grant as caps and the caller's caps as
    // the constraint, so `caps::check` enforces the intersection both ways. The sub records the ext
    // acted on the caller's behalf; the workspace is inherited (delegation never crosses the wall).
    let effective = caller.derive(format!("ext:{ext_id}"), install.granted.clone());
    let bridge = Bridge::new(Arc::clone(node), effective, ws);
    Some(CallContext {
        bridge: Arc::new(bridge),
        depth,
    })
}

/// Dispatch the durable-workflow host verbs a federated page reaches through the bridge:
/// `outbox.status` (read), `inbox.list` (read), `inbox.resolve` (the page's first workflow write).
/// Each host verb re-authorizes internally (workspace-first, then `mcp:<verb>:call`); a denial is
/// opaque (`ToolError::Denied`), indistinguishable from a missing tool. The `actor` of a resolve is
/// forced to the principal's `sub` inside `resolve_inbox` — never caller-supplied.
async fn call_workflow_tool(
    node: &Node,
    principal: &Principal,
    ws: &str,
    qualified_tool: &str,
    input: &Value,
) -> Result<Value, ToolError> {
    match qualified_tool {
        "outbox.status" => {
            let status = outbox_status(&node.store, principal, ws)
                .await
                .map_err(|_| ToolError::Denied)?;
            serde_json::to_value(status).map_err(|e| ToolError::Extension(e.to_string()))
        }
        "inbox.list" => {
            let channel = input
                .get("channel")
                .and_then(|v| v.as_str())
                .ok_or_else(|| ToolError::BadInput("missing arg: channel".into()))?;
            let items = list_inbox(&node.store, principal, ws, channel)
                .await
                .map_err(|_| ToolError::Denied)?;
            Ok(json!({ "items": items }))
        }
        "inbox.resolve" => {
            let item_id = input
                .get("item_id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| ToolError::BadInput("missing arg: item_id".into()))?;
            let decision: Decision =
                serde_json::from_value(input.get("decision").cloned().unwrap_or(Value::Null))
                    .map_err(|e| ToolError::BadInput(format!("decision: {e}")))?;
            // Logical ordering timestamp (no wall-clock in core); the page supplies it. Idempotent on
            // `item_id` regardless — re-resolving upserts, last decision wins.
            let ts = input.get("ts").and_then(|v| v.as_u64()).unwrap_or(0);
            resolve_inbox(&node.store, principal, ws, item_id, decision, ts)
                .await
                .map_err(|_| ToolError::Denied)?;
            Ok(json!({ "ok": true }))
        }
        _ => Err(ToolError::NotFound),
    }
}
