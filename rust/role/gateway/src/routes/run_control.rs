//! `POST /runs/{job}/{op}` — the **run lifecycle controls** the agent dock drives (agent-dock scope,
//! run controls). One op-param handler for the three actions, mirroring `/flows/runs/{run_id}/{op}`:
//!
//!   POST /runs/{job}/cancel   → stop  (durable, non-restartable)
//!   POST /runs/{job}/pause    → pause (Suspended, restartable)
//!   POST /runs/{job}/resume   → resume a paused run (re-drives from the cursor)
//!
//! The workspace + caps come from the verified session token (§7), never the path — the `{job}` is a
//! deep-link hint, and the host verbs (`stop_run`/`pause_run`/`resume_run`) each re-check
//! `mcp:agent.control:call` **workspace-first**, so a ws-B token can neither authorize for ws-A nor
//! reach a ws-A run job. A caller lacking the cap gets an **opaque `403`** (the MCP deny contract — no
//! leak of whether the run exists); an unknown op is a `400`; a bad state (pausing a finished run) is a
//! `400` with the store's honest message.

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use lb_host::{pause_run, resume_run, stop_run, AgentError};

use crate::session::authenticate;
use crate::state::Gateway;

/// Handle one run-control op. Returns `204 No Content` on success.
pub async fn run_control(
    State(gw): State<Gateway>,
    Path((job, op)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<StatusCode, (StatusCode, String)> {
    let principal = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let ws = principal.ws();

    let result = match op.as_str() {
        // `cancel` is the canonical stop verb name (matches `lb_jobs::cancel` / ACP `session/cancel`);
        // `stop` is accepted as a friendly alias so the UI can say "Stop".
        "cancel" | "stop" => stop_run(&gw.node, &principal, ws, &job).await,
        "pause" => pause_run(&gw.node, &principal, ws, &job).await,
        "resume" => resume_run(&gw.node, &principal, ws, &job).await,
        other => {
            return Err((
                StatusCode::BAD_REQUEST,
                format!("unknown run control op: {other}"),
            ))
        }
    };

    result
        .map(|_| StatusCode::NO_CONTENT)
        .map_err(control_status)
}

/// Map a control error to an HTTP status. A gate refusal is an opaque `403` (the MCP deny contract —
/// no capability/existence leak); a bad state (pause a finished run, etc.) is `400`.
fn control_status(e: AgentError) -> (StatusCode, String) {
    match e {
        AgentError::Denied => (StatusCode::FORBIDDEN, "denied".into()),
        AgentError::BadInput(m) => (StatusCode::BAD_REQUEST, m),
        AgentError::NotFound => (StatusCode::NOT_FOUND, "no such run".into()),
        _ => (StatusCode::FORBIDDEN, "denied".into()),
    }
}
