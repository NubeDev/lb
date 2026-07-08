//! `POST /agent/runs/{id}/token/refresh` — re-mint a run-scoped token for an external-agent run
//! (agent-key-lifecycle D2). The shim calls this proactively at 60% TTL and as a one-shot self-heal
//! on a 401. The route authenticates the CURRENT run token (the still-known bearer), re-checks the
//! run is live (D3 — a terminal run's refresh is refused), and re-mints under the node key with a
//! fresh TTL. The fresh token REPLACES the one the shim holds.
//!
//! **Why a dedicated route (not `POST /mcp/call`).** Refresh is a run-lifecycle verb, not a tool
//! call — it mints a credential, not a tool result. Riding `/mcp/call` would require a host verb
//! (`agent.token.refresh`) that returns a fresh token carrying `agent.decide`-tier semantics; a
//! dedicated REST route is the simpler, honest surface (scope: one verb, no CRUD). The route is
//! the ONE place a run-scoped token is re-minted outside the role crate's run-start mint.
//!
//! **The path-vs-claim id check.** The `{id}` path segment must equal the verified principal's
//! `run_id` claim. A mismatch is `400` (not `403` — no oracle on whether the run exists; the
//! caller already authenticated, so the mismatch is a client bug, not an attack surface).

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use lb_auth::{mint, Claims, Role};

use crate::session::authenticate;
use crate::state::Gateway;

/// The TTL stamped into a refreshed token (D1: 5 minutes). Matches the role crate's run-start mint.
const REFRESH_TOKEN_TTL_SECS: u64 = 5 * 60;

/// The refresh reply: the new bearer + the next 60%-TTL proactive-refresh timestamp (D2).
#[derive(Debug, serde::Serialize)]
pub struct RefreshReply {
    pub token: String,
    pub refresh_at_sec: u64,
}

/// Handle one refresh. `401` if the bearer is missing/bad/expired/revoked/terminal-run; `400` if
/// the path id does not match the token's `run_id` claim; `200` with the fresh token otherwise.
pub async fn refresh_run_token(
    State(gw): State<Gateway>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<RefreshReply>, (StatusCode, String)> {
    let principal = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    // The token MUST be run-scoped (carry a run_id) AND the run_id MUST match the path. A
    // non-run token (an ordinary session) hitting this route is a client bug → 400.
    let Some(run_id) = principal.run_id() else {
        return Err((StatusCode::BAD_REQUEST, "token is not run-scoped".into()));
    };
    if run_id != id {
        return Err((
            StatusCode::BAD_REQUEST,
            "run id mismatch (path ≠ token claim)".into(),
        ));
    }
    let ws = principal.ws();
    let now = gw.now();
    // The run-status gate already fired inside `authenticate` (D3) — if we reach here, the run is
    // live. But the window between authenticate and the mint is negligible and the gate is the
    // load-bearing one; we don't re-read (the job record's status is monotonic for our purposes —
    // a run that was live a moment ago cannot have been terminal before).
    let exp_sec = now.saturating_add(REFRESH_TOKEN_TTL_SECS);
    let refresh_at_sec = now.saturating_add(REFRESH_TOKEN_TTL_SECS * 3 / 5);
    let claims = Claims {
        sub: principal.sub().to_string(),
        ws: ws.to_string(),
        role: Role::Member,
        caps: principal.caps().to_vec(),
        iat: now,
        exp: exp_sec,
        // Preserve the delegation bound + run scope — the refreshed token is the same shape as
        // the run-start one, just with a fresh `exp`.
        constraint: principal.constraint().map(|c| c.to_vec()),
        run_id: Some(run_id.to_string()),
    };
    let token = mint(&gw.key, &claims);
    Ok(Json(RefreshReply {
        token,
        refresh_at_sec,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refresh_reply_serializes_to_token_and_refresh_at() {
        let r = RefreshReply {
            token: "abc".into(),
            refresh_at_sec: 123,
        };
        let j = serde_json::to_value(&r).unwrap();
        assert_eq!(j["token"], "abc");
        assert_eq!(j["refresh_at_sec"], 123);
        // No `exp_sec` leaks — the reply carries only what the shim reads.
        assert!(j.get("exp_sec").is_none());
    }

    #[test]
    fn ttl_is_five_minutes() {
        assert_eq!(REFRESH_TOKEN_TTL_SECS, 300);
        // 60% of 300 = 180 — the proactive-refresh lead.
        assert_eq!(REFRESH_TOKEN_TTL_SECS * 3 / 5, 180);
    }

    #[test]
    fn reply_carries_only_token_and_refresh_at_sec() {
        // The shim reads `token` + `refresh_at_sec`; pin the field names so a rename on either side
        // is caught here, not at runtime over the wire. The shim's own `Refreshed` mirror is the
        // authoritative reader; this asserts the gateway's reply shape matches that contract.
        let r = RefreshReply {
            token: "t".into(),
            refresh_at_sec: 9,
        };
        let j = serde_json::to_value(&r).unwrap();
        let keys: Vec<&str> = j.as_object().unwrap().keys().map(|s| s.as_str()).collect();
        assert_eq!(keys, vec!["token", "refresh_at_sec"]);
    }
}
