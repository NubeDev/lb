//! `POST /agent/invoke` — the browser path for the dashboard "AI widget" (genui author flow). The
//! widget calls the `agent_invoke` command; this is the transport that was missing (it was wired on
//! NO transport → "unknown command: agent_invoke"). It resolves the workspace's ACTIVE agent (the run
//! passes NO runtime) and drives it to completion synchronously, returning the durable job id + the
//! agent's final answer (active-agent-wiring Slice 5).
//!
//!   POST /agent/invoke  -> the active runtime (no runtime arg → workspace default → registry default)
//!
//! The workspace + caps come from the token (the hard wall, §7), NEVER the body — the body carries only
//! the goal (and an optional client `job_id`/substrate refs). An ungranted caller → opaque `403`.
//!
//! **Which host entry.** The scope text says "→ `lb_host::invoke`", but `invoke` takes an explicit
//! `&M: ModelAccess` and does NOT resolve the workspace runtime/model — it cannot honour "passes no
//! runtime → resolves the active agent". So this route drives [`lb_host::invoke_via_runtime`] with
//! `runtime = None`, exactly as the in-channel `/agent` worker does: ONE resolution seam (explicit arg
//! → workspace `agent.config.default_runtime` → registry default), the SAME invoke gate
//! (`mcp:agent.invoke:call`, workspace-first) firing inside it, and the SAME per-workspace in-house
//! model wiring. It is the honest equivalent of "invoke the active agent", not a second path.

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use lb_host::{invoke_via_runtime, reachable_tools, AgentError, Substrate};
use serde::{Deserialize, Serialize};

use crate::session::authenticate;
use crate::state::Gateway;

/// The invoke request: the goal the agent runs, plus an optional client `job_id` (lets the caller
/// correlate the run's SSE stream — the genui author flow mints one) and optional substrate refs. No
/// token, no workspace — both come from the verified session, not the page.
#[derive(Debug, Deserialize)]
pub struct InvokeRequest {
    pub goal: String,
    #[serde(default)]
    pub job_id: Option<String>,
    #[serde(default)]
    pub skill: Option<String>,
    #[serde(default)]
    pub doc: Option<String>,
    /// Optional **persona** selector (agent-personas scope #1) — a per-invoke override of the
    /// workspace's `agent.config.active_persona`. `#[serde(default)]` so an omitting caller resolves
    /// the workspace-active persona (or none). Lets a surface (Data Studio → `builtin.widget-builder`)
    /// pick a focus per run without changing the workspace default. Opaque id (rule 10).
    #[serde(default)]
    pub persona: Option<String>,
    /// Optional **page context** (agent-dock scope) — the client-reported `{ surface, path, search }`
    /// object the run fences into its goal as untrusted context (parity with the channel `kind:"agent"`
    /// payload). `#[serde(default)]` so an omitting caller is byte-identical to today; oversize (>4 KB
    /// serialized) is rejected as a `400` by the fence. Never a workspace/cap source — those stay
    /// token-derived (§7).
    #[serde(default)]
    pub context: Option<serde_json::Value>,
    /// Optional **runtime** selector (external-agent-authoring scope S4): the Studio "Generate with
    /// agent" card names the external runtime (`open-interpreter-default` etc) here. Absent ⇒ the
    /// workspace default (the genui/dashboard path). Opaque id (rule 10) — resolved by the registry.
    #[serde(default)]
    pub runtime: Option<String>,
}

/// The run's result — the UI's `AgentResult` shape (`agent.types.ts`): the agent's final answer + the
/// durable job/session id (survives the edge disconnecting; resumable on the hub).
#[derive(Debug, Serialize)]
pub struct InvokeReply {
    pub answer: String,
    #[serde(rename = "jobId")]
    pub job_id: String,
}

/// `POST /agent/invoke` — drive the workspace's active agent for the caller's goal and return the
/// final answer. `401` if the session token is missing/bad; a self-gate refusal (the caller lacks
/// `mcp:agent.invoke:call`) → opaque `403`; a bad input → `400`; a missing session → `404`.
pub async fn agent_invoke(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Json(req): Json<InvokeRequest>,
) -> Result<Json<InvokeReply>, (StatusCode, String)> {
    let principal = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let ws = principal.ws();

    // A client-supplied job_id lets the caller correlate the run's stream; absent, derive a STABLE id
    // from (ws, goal) — deterministic (idempotent on re-invoke of the same goal), never wall-clock/rng.
    let job_id = req
        .job_id
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| stable_job_id(ws, &req.goal));

    // Surface the caller's REACHABLE tool menu (default-agent-wiring #3): the same `tools.catalog` gate
    // the `/`-palette reads computes "every tool the caller may run" — so the model has real tools to
    // propose. The wall re-checks every proposed call under `agent ∩ caller`, so the menu is not a
    // widening. Best-effort: a catalog read failure yields an empty menu, never an error here.
    let tools = reachable_tools(&gw.node, &principal, ws).await;

    // Drive the ACTIVE agent: `runtime = None` → the ONE resolution seam picks the workspace default (or
    // the registry default). The invoke gate fires inside; the caller's own caps bound the run
    // (`agent ∩ caller`). The registry is the node's installed one (routed + in-channel share it).
    // The Studio "Generate with agent" card passes an explicit `runtime` (scope S4); absent → default.
    let answer = invoke_via_runtime(
        &gw.node,
        &gw.node.runtimes(),
        req.runtime.as_deref(),
        req.persona.as_deref(),
        &principal,
        &principal.caps().to_vec(),
        ws,
        &job_id,
        &req.goal,
        Substrate {
            skill: req.skill.as_deref(),
            doc: req.doc.as_deref(),
        },
        req.context.as_ref(),
        &tools,
        gw.now(),
    )
    .await
    .map_err(agent_status)?;

    Ok(Json(InvokeReply { answer, job_id }))
}

/// A deterministic job id from `(ws, goal)` — the idempotent fallback when the caller supplies none.
/// A stable hash (not wall-clock/rng) so re-invoking the same goal in the same workspace reuses the
/// SAME durable session (the loop is idempotent on `job_id`).
fn stable_job_id(ws: &str, goal: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    ws.hash(&mut h);
    goal.hash(&mut h);
    format!("agent-{:016x}", h.finish())
}

/// Map an agent error to an HTTP status. A gate refusal is an opaque `403` (no capability/existence
/// leak — the MCP deny contract); a bad input is `400`; a missing session is `404`.
fn agent_status(e: AgentError) -> (StatusCode, String) {
    match e {
        AgentError::BadInput(m) => (StatusCode::BAD_REQUEST, m),
        AgentError::NotFound => (StatusCode::NOT_FOUND, "no such session".into()),
        _ => (StatusCode::FORBIDDEN, "denied".into()),
    }
}
