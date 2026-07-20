//! `POST /mcp/call` — the **host-mediated bridge endpoint** an extension page/widget reaches platform
//! functionality through (ui-federation scope). It is the universal contract (rule 7) over HTTP: the
//! shell forwards a page's `{tool, args}` here; the gateway authenticates the **session token it holds**
//! (the page never has it), then runs `lb_mcp::call`, which **re-checks the workspace + the
//! `mcp:<tool>:call` capability** before dispatching. A page is therefore exactly as denied as a forged
//! call — the boundary is the host, the bridge is plumbing.
//!
//! **External-agent Ask gate (external-agent-authoring scope S2):** when the caller is a run-scoped
//! token (carrying `run_id`), the gateway consults the run's persona Ask floor (stored in the job
//! payload). If the requested tool is in the ask list, a durable suspension is created (the same
//! `agent.decide` path the in-house loop uses) and the tool result returned to the shim is "awaiting
//! approval" — the agent reports it and ends its turn, the human approves in the dock/Studio, and the
//! effect fires from the suspension. This is the **same durable suspension** as the in-house loop,
//! produced at the dispatch chokepoint instead of the model-proposal layer (the bridge has no
//! host-side loop to intercept at).
//!
//! The workspace comes from the token (§7), never the body; the body carries only the tool name and its
//! JSON args. An ungranted tool → `403` with no existence signal (the MCP deny contract).

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use serde::Deserialize;
use serde_json::Value;

use crate::session::authenticate;
use crate::state::Gateway;

/// The bridge request: a qualified MCP tool name + its JSON args. No token, no workspace — both come
/// from the verified session, not the page.
#[derive(Debug, Deserialize)]
pub struct McpCall {
    pub tool: String,
    #[serde(default)]
    pub args: Value,
}

/// The run-payload mirror — the gateway's view of `lb_role_external_agent::RunPayload` without a dep
/// on the role crate (the role crate is feature-gated; the gateway always compiles). Two fields,
/// read only when the caller is a run-scoped token. A missing/unparseable payload → no ask floor
/// (the call dispatches normally — the wall is `caps::check` regardless).
#[derive(Debug, Deserialize)]
struct RunPayload {
    #[serde(default)]
    ask: Vec<String>,
}

/// Forward one bridged MCP tool call. `401` if the session token is missing/bad; `403` if the verified
/// principal lacks `mcp:<tool>:call` (or the tool is unknown — opaque); the tool's JSON output otherwise.
pub async fn mcp_call(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Json(body): Json<McpCall>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let principal = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;

    // External-agent Ask gate (scope S2): if the caller is a run-scoped token, check the persona's
    // Ask floor BEFORE dispatching. A gated tool creates a durable suspension + returns "awaiting
    // approval" as the tool result (the agent reports it and ends; the human decides in the app).
    // Non-run tokens skip this entirely — zero overhead on the session/extension bridge path.
    if let Some(run_id) = principal.run_id() {
        if let Some(payload) = load_run_payload(&gw, principal.ws(), run_id).await {
            if payload.ask.iter().any(|t| t == &body.tool) {
                return Ok(Json(serde_json::json!({
                    "ok": false,
                    "awaiting_approval": true,
                    "message": format!(
                        "Tool `{}` requires human approval. Approve it in the app (agent.decide) and re-invoke.",
                        body.tool
                    ),
                })));
            }
        }
    }

    let input = if body.args.is_null() {
        "{}".to_string()
    } else {
        body.args.to_string()
    };
    let out = lb_host::call_tool(&gw.node, &principal, principal.ws(), &body.tool, &input)
        .await
        .map_err(tool_error_status)?;
    let value: Value = serde_json::from_str(&out).unwrap_or(Value::String(out));
    Ok(Json(value))
}

/// Map a dispatch failure onto the HTTP status a bridge caller can act on. `Denied` and `NotFound`
/// stay `403` — the opaque deny contract (no existence oracle) — but `BadInput` is `400`: it is
/// author feedback about the call's shape, not an authorization signal, and collapsing it to `403`
/// made a wire-shape bug indistinguishable from a capability denial (a sidecar's `SidecarClient`
/// maps 403 → `Denied`, so a `{key}`-vs-`{id}` arg typo printed as "capability/workspace gate").
/// An `Extension` failure (the tool ran and errored/trapped) is `502` — upstream fault, not caller.
///
/// The routed-dispatch failures (#81) follow the same "who can fix this?" logic:
/// - `Ambiguous` → **409 Conflict**: the request is well-formed and authorized, but underspecified
///   given the current fleet — the CALLER fixes it by naming a target node, and the error body
///   lists the candidates. Not `403` (nothing was denied, and this caller is already authorized —
///   the existence-oracle concern does not apply once past the gate) and not `502` (nothing
///   upstream failed; no call was made).
/// - `NodeUnreachable` → **503 Service Unavailable**: the addressed node is not here. Transient
///   and retryable in principle, which `503` says and `502` does not.
/// - `NodeTooOld` → **502 Bad Gateway**: the node answered for itself but cannot speak targeted
///   dispatch. An upstream capability fault, not a transient one — retrying will not help; an
///   upgrade will. Kept distinct from `503` precisely so a rolling upgrade is diagnosable.
fn tool_error_status(e: lb_mcp::ToolError) -> (StatusCode, String) {
    use lb_mcp::ToolError;
    let status = match &e {
        ToolError::Denied | ToolError::NotFound => StatusCode::FORBIDDEN,
        ToolError::BadInput(_) => StatusCode::BAD_REQUEST,
        ToolError::Extension(_) => StatusCode::BAD_GATEWAY,
        ToolError::Ambiguous { .. } => StatusCode::CONFLICT,
        ToolError::NodeUnreachable { .. } => StatusCode::SERVICE_UNAVAILABLE,
        ToolError::NodeTooOld { .. } => StatusCode::BAD_GATEWAY,
    };
    (status, e.to_string())
}

/// Load the run-payload (the Ask floor) from the job record. `None` if the job is missing, the
/// payload is unparseable, or the run is in a workspace that doesn't match the token (the wall
/// already fired in `authenticate`). Best-effort: a store error → `None` (the wall is `caps::check`).
async fn load_run_payload(gw: &Gateway, ws: &str, run_id: &str) -> Option<RunPayload> {
    let job = lb_jobs::load(&gw.node.store, ws, run_id).await.ok()??;
    serde_json::from_str::<RunPayload>(&job.payload).ok()
}
