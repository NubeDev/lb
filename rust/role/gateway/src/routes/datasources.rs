//! Datasource routes — the browser's `datasource.*` admin surface over the gateway (rules-workbench
//! scope, Phase 3). Each route mirrors a shipped `datasource.*` host verb 1:1 and re-checks the
//! `mcp:datasource.<verb>:call` capability server-side via `lb_host::call_tool` — exactly like the
//! `/mcp/call` bridge, but as a named REST surface the first-party admin page drives. The workspace +
//! principal come from the **token**, never the body (§7).
//!
//! The DSN is the only secret material: it is supplied on the Add submit, forwarded to the host (which
//! writes it to `lb_secrets` and stores only a ref on the record), and **never** returned to the page —
//! `datasource.list`/`datasource.test` responses carry no DSN. A redaction test proves it.

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use lb_mcp::ToolError;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::session::authenticate;
use crate::state::Gateway;

/// `GET /datasources` — the workspace's registered sources (name + kind + endpoint + redacted secret
/// ref, NEVER a DSN). Mirrors `datasource.list`.
pub async fn list_datasources(
    State(gw): State<Gateway>,
    headers: HeaderMap,
) -> Result<Json<Value>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let out = lb_host::call_tool(&gw.node, &p, p.ws(), "datasource.list", "{}")
        .await
        .map_err(status)?;
    Ok(Json(parse(&out)))
}

/// The `POST /datasources` body — register a source. The DSN exists only here (the Add submit); it is
/// forwarded to the host and never read back.
#[derive(Debug, Deserialize)]
pub struct AddDatasource {
    pub name: String,
    pub kind: String,
    pub endpoint: String,
    pub dsn: String,
}

/// `POST /datasources` — register a datasource (DSN → `lb_secrets` host-side; only the ref persisted).
/// Mirrors `datasource.add`. Returns `{ok:true}`.
pub async fn add_datasource(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Json(body): Json<AddDatasource>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let input = json!({
        "name": body.name,
        "kind": body.kind,
        "endpoint": body.endpoint,
        "dsn": body.dsn,
        "ts": gw.now,
    });
    let out = lb_host::call_tool(&gw.node, &p, p.ws(), "datasource.add", &input.to_string())
        .await
        .map_err(status)?;
    Ok(Json(parse(&out)))
}

/// `DELETE /datasources/{name}` — drop a source record. Mirrors `datasource.remove`. `204`.
pub async fn remove_datasource(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path(name): Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let input = json!({ "name": name });
    lb_host::call_tool(
        &gw.node,
        &p,
        p.ws(),
        "datasource.remove",
        &input.to_string(),
    )
    .await
    .map_err(status)?;
    Ok(StatusCode::NO_CONTENT)
}

/// `POST /datasources/{name}/test` — a real connectivity probe via the supervised federation sidecar.
/// Mirrors `datasource.test`. A `200` with `{ok:true}` is a GREEN probe; a non-`200` (a sidecar fault
/// → `Extension`/500, a missing source → `400`, a refused endpoint → `403`) is an honest RED probe —
/// never a fabricated green.
pub async fn test_datasource(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path(name): Path<String>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let input = json!({ "source": name, "ts": gw.now });
    let out = lb_host::call_tool(&gw.node, &p, p.ws(), "datasource.test", &input.to_string())
        .await
        .map_err(status)?;
    Ok(Json(parse(&out)))
}

/// Parse a host tool's JSON string output, falling back to a wrapped string on a non-JSON body.
fn parse(out: &str) -> Value {
    serde_json::from_str(out).unwrap_or_else(|_| Value::String(out.to_string()))
}

/// Map a `ToolError` onto an HTTP status. `Denied` → `403` (opaque); `BadInput(m)` → `400` verbatim
/// (author feedback, e.g. "no such datasource"); `NotFound` → `404`; an `Extension` fault (a sidecar
/// fault — the unavailable-probe case) → `500`. Mirrors the MCP→HTTP contract.
fn status(e: ToolError) -> (StatusCode, String) {
    match e {
        ToolError::Denied => (StatusCode::FORBIDDEN, "denied".into()),
        ToolError::BadInput(m) => (StatusCode::BAD_REQUEST, m),
        ToolError::NotFound => (StatusCode::NOT_FOUND, e.to_string()),
        ToolError::Extension(m) => (StatusCode::INTERNAL_SERVER_ERROR, m),
    }
}
