//! Brand routes — the browser's `brand.*` surface over the gateway (reports scope, "Brand
//! profiles"). Each route mirrors a `lb_host::brand_*` verb 1:1 and re-runs the host's gates
//! server-side (workspace-first → `mcp:brand.<verb>:call`). A brand is workspace-shared (no
//! visibility tiers); the owner comes from the token, never the body (§7).

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use lb_host::{BrandColors, BrandError, BrandFonts};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::session::authenticate;
use crate::state::Gateway;

/// `GET /brands` — the workspace's brand roster (summaries). Gated `brand.list`.
pub async fn list_brands(
    State(gw): State<Gateway>,
    headers: HeaderMap,
) -> Result<Json<Value>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let rows = lb_host::brand_list(&gw.node.store, &p, p.ws())
        .await
        .map_err(status)?;
    Ok(Json(json!({ "brands": rows })))
}

/// `GET /brands/{id}` — one brand profile. Gated `brand.get`.
pub async fn get_brand(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let brand = lb_host::brand_get(&gw.node.store, &p, p.ws(), &id)
        .await
        .map_err(status)?;
    Ok(Json(serde_json::to_value(brand).unwrap_or(Value::Null)))
}

/// `POST /brands` body — create/update a brand (UPSERT on `id`). Owner is the token's principal.
#[derive(Debug, Deserialize)]
pub struct SaveBrand {
    pub id: String,
    pub name: String,
    #[serde(default, rename = "logoAssetId")]
    pub logo_asset_id: String,
    #[serde(default)]
    pub colors: BrandColors,
    #[serde(default)]
    pub fonts: BrandFonts,
    #[serde(default, rename = "headerText")]
    pub header_text: String,
    #[serde(default, rename = "footerText")]
    pub footer_text: String,
}

/// `POST /brands` — idempotent UPSERT (owner-forced create, owner-only update). Gated `brand.save`.
pub async fn save_brand(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Json(body): Json<SaveBrand>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let brand = lb_host::brand_save(
        &gw.node.store,
        &p,
        p.ws(),
        &body.id,
        &body.name,
        &body.logo_asset_id,
        body.colors,
        body.fonts,
        &body.header_text,
        &body.footer_text,
        gw.now(),
    )
    .await
    .map_err(status)?;
    Ok(Json(serde_json::to_value(brand).unwrap_or(Value::Null)))
}

/// `DELETE /brands/{id}` — idempotent tombstone (owner-only). Gated `brand.delete`.
pub async fn delete_brand(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    lb_host::brand_delete(&gw.node.store, &p, p.ws(), &id, gw.now())
        .await
        .map_err(status)?;
    Ok(StatusCode::NO_CONTENT)
}

/// Map a brand gate outcome onto an HTTP status. `Denied`/`Store` are `403` (opaque); `NotFound`
/// `404`; `BadInput` `400`.
fn status(e: BrandError) -> (StatusCode, String) {
    match e {
        BrandError::Denied => (StatusCode::FORBIDDEN, e.to_string()),
        BrandError::NotFound => (StatusCode::NOT_FOUND, e.to_string()),
        BrandError::BadInput(m) => (StatusCode::BAD_REQUEST, m),
        BrandError::Store(s) => (StatusCode::FORBIDDEN, s.to_string()),
    }
}
