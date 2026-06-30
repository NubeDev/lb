//! Flow routes — the browser's `flows.*` surface over the gateway (flows-canvas + dashboard-binding
//! scopes, Wave 3). Each route mirrors a shipped `flows.*` host verb 1:1 and re-checks the
//! `mcp:flows.<verb>:call` capability server-side via `lb_host::call_tool` (the same MCP chokepoint
//! `routes/mcp.rs` uses). The workspace + principal come from the **token**, never the body (§7) — so
//! a flow is workspace-walled and un-spoofable. The UI cap-gate is convenience only; this is the
//! boundary.
//!
//! The host classifies a failure as a `lb_mcp::ToolError`; [`status`] maps it onto HTTP. A cyclic /
//! invalid DAG or a schema-invalid node config arrives as `BadInput(msg)` → `400` with the validation
//! message VERBATIM — the source of the canvas's inline edge error. `flows.patch_run` against an
//! executed node or a schema-mismatched config is likewise a `400`.

use std::convert::Infallible;

use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::Json;
use futures::stream::Stream;
use lb_mcp::ToolError;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::routes::stream::StreamAuth;
use crate::session::{authenticate, verify_token};
use crate::state::Gateway;

/// `GET /flows` — the workspace's flows (summaries via `flows.list`). Gated `flows.list`.
pub async fn list_flows(
    State(gw): State<Gateway>,
    headers: HeaderMap,
) -> Result<Json<Value>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    call(&gw, &p, "flows.list", &json!({})).await
}

/// `GET /flows/nodes` — the merged node registry (palette source). Gated `flows.nodes`.
pub async fn list_flow_nodes(
    State(gw): State<Gateway>,
    headers: HeaderMap,
) -> Result<Json<Value>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    call(&gw, &p, "flows.nodes", &json!({})).await
}

/// `GET /flows/{id}` — one flow (its typed graph). Gated `flows.get`; absent/tombstoned → `404`.
pub async fn get_flow(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    call(&gw, &p, "flows.get", &json!({ "id": id })).await
}

/// `POST /flows` — create/update a flow (DAG + every node config validated UPSERT). Body is the full
/// `Flow` minus the workspace (the host sets it from the token). Gated `flows.save`; an invalid DAG or
/// schema-invalid node config → `400` with the host's validation message (the canvas inline error).
/// Returns `{id, version}`.
pub async fn save_flow(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Json(mut body): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    // The `Flow` carries `workspace`; set it from the token so the body never spoofs it (§7).
    if let Some(obj) = body.as_object_mut() {
        obj.insert("workspace".into(), Value::String(p.ws().to_string()));
    }
    call(&gw, &p, "flows.save", &body).await
}

/// `DELETE /flows/{id}` — guarded, ordered teardown (disarm sources, cancel runs, drop cron). Gated
/// `flows.delete`. `204` on success.
pub async fn delete_flow(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let _ = call(&gw, &p, "flows.delete", &json!({ "id": id })).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// `POST /flows/{id}/run` body — optional run params + optional explicit run id.
#[derive(Debug, Default, Deserialize)]
pub struct RunFlow {
    #[serde(default)]
    pub params: Value,
    #[serde(default)]
    pub run_id: Option<String>,
}

/// `POST /flows/{id}/run` — start a flow run (a durable job). Gated `flows.run`. Returns `{run_id}`.
pub async fn run_flow(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<RunFlow>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let mut input = json!({ "id": id, "params": body.params, "ts": gw.now });
    if let Some(rid) = body.run_id {
        input["run_id"] = Value::String(rid);
    }
    call(&gw, &p, "flows.run", &input).await
}

/// `POST /flows/runs/{run_id}/{op}` — the run lifecycle (`suspend` | `resume` | `cancel`). Each is its
/// own cap (`mcp:flows.<op>:call`); the path segment selects the verb. Returns `{ok:true}`.
pub async fn lifecycle_flow(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path((run_id, op)): Path<(String, String)>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let verb = match op.as_str() {
        "suspend" => "flows.suspend",
        "resume" => "flows.resume",
        "cancel" => "flows.cancel",
        other => {
            return Err((
                StatusCode::NOT_FOUND,
                format!("unknown lifecycle op: {other}"),
            ));
        }
    };
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let input = json!({ "run_id": run_id, "ts": gw.now });
    call(&gw, &p, verb, &input).await
}

/// `POST /flows/runs/{run_id}/patch` body — a config-only patch to one node.
#[derive(Debug, Deserialize)]
pub struct PatchRun {
    pub node: String,
    pub config: Value,
}

/// `POST /flows/runs/{run_id}/patch` — `flows.patch_run` (config-only, UNEXECUTED node, validated
/// against the run's PINNED schema — Decision 12). Gated `flows.patch_run`. Returns `{ok:true}`.
pub async fn patch_flow_run(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path(run_id): Path<String>,
    Json(body): Json<PatchRun>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let input = json!({ "run_id": run_id, "node": body.node, "config": body.config });
    call(&gw, &p, "flows.patch_run", &input).await
}

/// `GET /flows/runs/{run_id}` — the per-node run snapshot the canvas colours from (gated
/// `flows.runs.get`). The records are the source of truth: a late open rebuilds the same colours.
pub async fn get_flow_run(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path(run_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    call(&gw, &p, "flows.runs.get", &json!({ "run_id": run_id })).await
}

/// `GET /flows/{id}/runs?status=` — the runs of a flow (the **reattach** surface: a reopened canvas
/// finds the active `run_id`). Gated `flows.runs.list`. Never another workspace's runs.
pub async fn list_flow_runs(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path(id): Path<String>,
    raw: axum::extract::RawQuery,
) -> Result<Json<Value>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let status = raw
        .0
        .as_deref()
        .and_then(|q| q.split('&').find(|p| p.starts_with("status=")))
        .and_then(|p| p.split('=').nth(1))
        .map(|s| s.to_string());
    let mut input = json!({ "flow_id": id });
    if let Some(s) = status {
        input["status"] = Value::String(s);
    }
    call(&gw, &p, "flows.runs.list", &input).await
}

/// `POST /flows/{id}/enable` body — flip the durable lifecycle flags (triggers-lifecycle scope).
#[derive(Debug, Default, Deserialize)]
pub struct EnableFlow {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub start_on_boot: bool,
}
fn default_true() -> bool {
    true
}

/// `POST /flows/{id}/enable` — `flows.enable` (enable/disable + start_on_boot). Gated `flows.enable`.
pub async fn enable_flow(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<EnableFlow>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let input = json!({ "id": id, "enabled": body.enabled, "start_on_boot": body.start_on_boot });
    call(&gw, &p, "flows.enable", &input).await
}

/// `POST /flows/{id}/inject` body — set a node's retained value (Decision 9).
#[derive(Debug, Deserialize)]
pub struct InjectFlow {
    pub node: String,
    pub value: Value,
}

/// `POST /flows/{id}/inject` — `flows.inject` (sets a retained input OR fires a one-shot run for a
/// firing trigger). Gated `mcp:flows.inject:call`, re-checked per call like any control write. Returns
/// `{fired_run}`.
pub async fn inject_flow(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<InjectFlow>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let input = json!({ "id": id, "node": body.node, "value": body.value, "ts": gw.now });
    call(&gw, &p, "flows.inject", &input).await
}

/// `GET /flows/node/{id}/{node}` — read one node's config from the saved flow. Gated `flows.node.get`.
pub async fn get_flow_node(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path((id, node)): Path<(String, String)>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    call(
        &gw,
        &p,
        "flows.node.get",
        &json!({ "id": id, "node": node }),
    )
    .await
}

/// `POST /flows/node/{id}/{node}` body — the replacement config for one node.
#[derive(Debug, Deserialize)]
pub struct UpdateFlowNode {
    pub config: Value,
}

/// `POST /flows/node/{id}/{node}` — replace one node's config on the SAVED flow (validated against
/// its descriptor schema, bumps the flow version — flow-runtime-control-scope). Gated
/// `flows.node.update`. A schema-invalid config → `400` with the host's message (the inline panel
/// error). This is the per-node write that replaces re-posting the whole `Flow` on a config tweak.
pub async fn update_flow_node(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path((id, node)): Path<(String, String)>,
    Json(body): Json<UpdateFlowNode>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let input = json!({ "id": id, "node": node, "config": body.config });
    call(&gw, &p, "flows.node.update", &input).await
}

/// `GET /flows/runs/{run_id}/stream?token=` — the **live settle feed** the canvas watches
/// (flow-runtime-control-scope). Mirrors `run_stream` / `channel_stream`: open once with
/// `EventSource`, receive a `snapshot` frame (the run as of attach) then `flow` frames (each a
/// `node-settled` / `run-finished` delta) instead of polling `runs.get`. Auth is the `?token=` query
/// param (`EventSource` can't set headers); `watch_flow_run` runs the `mcp:flows.watch:call` gate
/// (a `403` before any body) and the bus subject is workspace-walled, so a ws-B session can never
/// observe a ws-A run.
pub async fn flow_run_stream(
    State(gw): State<Gateway>,
    Path(run_id): Path<String>,
    Query(auth): Query<StreamAuth>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, (StatusCode, String)> {
    let principal = verify_token(&gw, &auth.token)
        .await
        .map_err(|e| e.into_response())?;
    let ws = principal.ws().to_string();
    // Authorize + read the snapshot + declare the live feed up front (workspace-first). A denial here
    // is a 403 before any stream body.
    let watch = lb_host::watch_flow_run(&gw.node.store, &gw.node.bus, &principal, &ws, &run_id)
        .await
        .map_err(|e| (StatusCode::FORBIDDEN, e.to_string()))?;

    // Phase 1: emit the catch-up snapshot as one `snapshot` frame. Phase 2: fold the live settle feed
    // (`flow` frames) until it closes (a `run-finished` delta tells the client to stop).
    let stream = futures::stream::unfold(
        (Some(watch.snapshot), watch.stream),
        |(snapshot, live)| async move {
            if let Some(snap) = snapshot {
                let ev = Event::default()
                    .event("snapshot")
                    .json_data(&snap)
                    .unwrap_or_else(|_| Event::default().comment("encode error"));
                return Some((Ok(ev), (None, live)));
            }
            live.recv().await.map(|event| {
                let ev = Event::default()
                    .event("flow")
                    .json_data(&event)
                    .unwrap_or_else(|_| Event::default().comment("encode error"));
                (Ok(ev), (None, live))
            })
        },
    );
    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}

/// Forward one `flows.*` MCP call through the host (re-checking the cap), returning its JSON output.
async fn call(
    gw: &Gateway,
    p: &lb_auth::Principal,
    tool: &str,
    input: &Value,
) -> Result<Json<Value>, (StatusCode, String)> {
    let out = lb_host::call_tool(&gw.node, p, p.ws(), tool, &input.to_string())
        .await
        .map_err(status)?;
    let value: Value = serde_json::from_str(&out).unwrap_or(Value::String(out));
    Ok(Json(value))
}

/// Map an MCP gate outcome onto an HTTP status. `Denied` → `403` (opaque "not permitted"); `BadInput`
/// → `400` with the message VERBATIM (the canvas inline DAG/schema error + the patch_run mismatch);
/// `NotFound` → `404`; an extension/store fault → `500`.
fn status(e: ToolError) -> (StatusCode, String) {
    match e {
        ToolError::Denied => (StatusCode::FORBIDDEN, "not permitted".into()),
        ToolError::BadInput(m) => (StatusCode::BAD_REQUEST, m),
        ToolError::NotFound => (StatusCode::NOT_FOUND, "no such flow".into()),
        ToolError::Extension(m) => (StatusCode::INTERNAL_SERVER_ERROR, m),
    }
}
