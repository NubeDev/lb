//! Report routes — the browser's `report.*` surface over the gateway (reports scope). Each CRUD
//! route mirrors a `lb_host::report_*` verb 1:1 and re-runs the host's three gates server-side
//! (workspace-first → `mcp:report.<verb>:call` → membership/visibility). The workspace + owner come
//! from the **token**, never the body (§7). The export route is the one binary path: it returns raw
//! `%PDF` bytes (the `ext_ui` bytes tuple) but — unlike `ext_ui` — authenticates first.

use axum::extract::{Path, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::IntoResponse;
use axum::Json;
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use lb_host::{ReportBlock, ReportError, ReportVisibility};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::session::authenticate;
use crate::state::Gateway;

/// `GET /reports` — the roster the caller can reach (own + team-shared + workspace), summaries only.
/// Gated `report.list`.
pub async fn list_reports(
    State(gw): State<Gateway>,
    headers: HeaderMap,
) -> Result<Json<Value>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let rows = lb_host::report_list(&gw.node.store, &p, p.ws())
        .await
        .map_err(status)?;
    Ok(Json(json!({ "reports": rows })))
}

/// `GET /reports/{id}` — one report (three-gate read, panel blocks hydrated). Gated `report.get`.
pub async fn get_report(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let report = lb_host::report_get(&gw.node.store, &p, p.ws(), &id)
        .await
        .map_err(status)?;
    Ok(Json(serde_json::to_value(report).unwrap_or(Value::Null)))
}

/// `POST /reports` body — create/update a report (UPSERT on `id`). Owner is the token's principal;
/// visibility is set via `/share`, never here.
#[derive(Debug, Deserialize)]
pub struct SaveReport {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub blocks: Vec<ReportBlock>,
    #[serde(default, rename = "brandId")]
    pub brand_id: String,
    #[serde(default)]
    pub toolbar: Value,
}

/// `POST /reports` — idempotent UPSERT (create on a fresh id, owner-only update). Gated `report.save`.
pub async fn save_report(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Json(body): Json<SaveReport>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let report = lb_host::report_save(
        &gw.node.store,
        &p,
        p.ws(),
        &body.id,
        &body.title,
        body.blocks,
        &body.brand_id,
        body.toolbar,
        gw.now(),
    )
    .await
    .map_err(status)?;
    Ok(Json(serde_json::to_value(report).unwrap_or(Value::Null)))
}

/// `DELETE /reports/{id}` — idempotent tombstone (owner-only). Gated `report.delete`.
pub async fn delete_report(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    lb_host::report_delete(&gw.node.store, &p, p.ws(), &id, gw.now())
        .await
        .map_err(status)?;
    Ok(StatusCode::NO_CONTENT)
}

/// `POST /reports/{id}/share` body — set visibility (`private|team|workspace`) + optional team.
#[derive(Debug, Deserialize)]
pub struct ShareReport {
    pub visibility: String,
    #[serde(default)]
    pub team: Option<String>,
}

pub async fn share_report(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<ShareReport>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let visibility = parse_visibility(&body.visibility).ok_or((
        StatusCode::BAD_REQUEST,
        format!("bad visibility: {}", body.visibility),
    ))?;
    let report = lb_host::report_share(
        &gw.node.store,
        &p,
        p.ws(),
        &id,
        visibility,
        body.team.as_deref(),
        gw.now(),
    )
    .await
    .map_err(status)?;
    Ok(Json(serde_json::to_value(report).unwrap_or(Value::Null)))
}

/// One client-captured panel snapshot in the export payload: the block's `cell.i` (`cellId`) and a
/// base64-encoded PNG. Decoded to raw bytes and handed to `report_export` as `(cellId, png_bytes)`.
#[derive(Debug, Deserialize)]
pub struct Snapshot {
    #[serde(rename = "cellId")]
    pub cell_id: String,
    pub png: String,
}

/// `POST /reports/{id}/export.pdf` body — the snapshot payload. The client renders each panel block
/// to a PNG (the DOM-capture seam) and posts them; the host assembles blocks + brand + snapshots
/// into a branded PDF.
#[derive(Debug, Deserialize)]
pub struct ExportBody {
    #[serde(default)]
    pub snapshots: Vec<Snapshot>,
}

/// `POST /reports/{id}/export.pdf` — assemble → branded PDF bytes. Authenticated (unlike `ext_ui`),
/// gated `report.export` inside the host. Returns raw `application/pdf` bytes.
pub async fn export_report(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<ExportBody>,
) -> impl IntoResponse {
    let p = match authenticate(&gw, &headers).await {
        Ok(p) => p,
        Err(e) => return e.into_response().into_response(),
    };
    // Decode each snapshot's base64 PNG → raw bytes. A bad base64 is a client error (400).
    let mut snapshots: Vec<(String, Vec<u8>)> = Vec::with_capacity(body.snapshots.len());
    for s in body.snapshots {
        match B64.decode(s.png.as_bytes()) {
            Ok(bytes) => snapshots.push((s.cell_id, bytes)),
            Err(_) => {
                return (StatusCode::BAD_REQUEST, "bad base64 snapshot").into_response();
            }
        }
    }
    match lb_host::report_export(&gw.node.store, &p, p.ws(), &id, snapshots, gw.now()).await {
        Ok(pdf) => (
            StatusCode::OK,
            [(
                header::CONTENT_TYPE,
                HeaderValue::from_static("application/pdf"),
            )],
            pdf,
        )
            .into_response(),
        Err(e) => status(e).into_response(),
    }
}

fn parse_visibility(s: &str) -> Option<ReportVisibility> {
    match s {
        "private" => Some(ReportVisibility::Private),
        "team" => Some(ReportVisibility::Team),
        "workspace" => Some(ReportVisibility::Workspace),
        _ => None,
    }
}

/// Map a report gate outcome onto an HTTP status. `Denied`/`Store` are `403` (opaque); `NotFound`
/// `404`; `BadInput` `400`; a `Render` fault is `500` (a server-side compile failure).
fn status(e: ReportError) -> (StatusCode, String) {
    match e {
        ReportError::Denied => (StatusCode::FORBIDDEN, e.to_string()),
        ReportError::NotFound => (StatusCode::NOT_FOUND, e.to_string()),
        ReportError::BadInput(m) => (StatusCode::BAD_REQUEST, m),
        ReportError::Render(m) => (StatusCode::INTERNAL_SERVER_ERROR, m),
        ReportError::Store(s) => (StatusCode::FORBIDDEN, s.to_string()),
    }
}
