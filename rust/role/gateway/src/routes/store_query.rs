//! Read-only SQL routes — the browser's `store.query`/`store.schema` surface over the gateway
//! (widget-builder Slice A). The "direct SurrealDB" widget source + the visual SQL builder's schema
//! feed. Each route mirrors a `lb_host::store_*` verb 1:1 and re-runs the host's gate server-side
//! (workspace-first → `mcp:store.query|schema:call`), then — for `store.query` — the **parse-allowlist
//! to a single SELECT** (the load-bearing read-only gate, in the host). The workspace + principal come
//! from the **token**, never the request (the hard wall, §7); the SQL can never name a namespace.
//!
//! These are the same verbs a widget cell reaches over `POST /mcp/call` (leashed by `cell.tools ∩
//! grant`); the gateway routes are the convenience surface for the builder UI. Read-only by design —
//! a write statement is refused at parse, returned as `400` (author feedback), not run.

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use lb_host::{QueryResult, Schema, StoreQueryError};
use serde::Deserialize;
use serde_json::{Map, Value};

use crate::session::authenticate;
use crate::state::Gateway;

/// `POST /store/query` body — a read-only SurrealQL string + optional `$`-bound vars.
#[derive(Debug, Deserialize)]
pub struct RunQuery {
    pub sql: String,
    #[serde(default)]
    pub vars: Option<Map<String, Value>>,
}

/// `POST /store/query` — run a parse-allowlisted, bounded, read-only `SELECT` in the caller's
/// workspace, returning `{ columns, rows }`. Gated `store.query`; a write/multi/namespace statement
/// is `400` (the parse rejection reason), a missing cap `403`-opaque.
pub async fn run_query(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Json(body): Json<RunQuery>,
) -> Result<Json<QueryResult>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers).map_err(|e| e.into_response())?;
    let vars = body
        .vars
        .unwrap_or_default()
        .into_iter()
        .collect::<Vec<(String, Value)>>();
    let result = lb_host::store_query_run(&gw.node.store, &p, p.ws(), &body.sql, vars)
        .await
        .map_err(status)?;
    Ok(Json(result))
}

/// `GET /store/schema` — the workspace's tables + columns (the visual SQL builder's dropdowns).
/// Gated `store.schema`; workspace-walled (ws-B sees only ws-B's tables).
pub async fn read_schema(
    State(gw): State<Gateway>,
    headers: HeaderMap,
) -> Result<Json<Schema>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers).map_err(|e| e.into_response())?;
    let schema = lb_host::store_schema_read(&gw.node.store, &p, p.ws())
        .await
        .map_err(status)?;
    Ok(Json(schema))
}

/// Map a read-only SQL gate outcome onto an HTTP status. `Denied` is `403` (opaque — no existence
/// signal); a parse `Rejected`/`Parse` is `400` (author feedback for the editor); a store fault is
/// `403`-opaque like the other gateway routes.
fn status(e: StoreQueryError) -> (StatusCode, String) {
    match e {
        StoreQueryError::Denied => (StatusCode::FORBIDDEN, e.to_string()),
        StoreQueryError::Rejected(m) => (StatusCode::BAD_REQUEST, m),
        StoreQueryError::Parse(m) => (StatusCode::BAD_REQUEST, format!("parse error: {m}")),
        StoreQueryError::Store(s) => (StatusCode::FORBIDDEN, s.to_string()),
    }
}
