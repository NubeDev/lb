//! `GET /mcp/catalog` — the command-palette's catalog read (channels-command-palette scope). Returns
//! the caller's **authorized** MCP tool set (registered tools ∩ caps held), each with a title, group,
//! and standard JSON-Schema `input_schema` so the palette renders a guided argument rail. The menu IS
//! the permission model rendered: a denied tool is absent, never greyed (no existence leak).
//!
//! Member-level — every UI-capable principal holds `mcp:tools.catalog:call` (it leaks only the tool
//! *shapes* the caller may already run, never data). The workspace + principal come from the
//! **verified session token** (the hard wall, §7), never the request. A denial is `403`-opaque.

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use lb_host::ToolsCatalog;

use crate::session::authenticate;
use crate::state::Gateway;

/// `GET /mcp/catalog` — the authorized tool catalog for the caller's workspace. Member cap
/// (`mcp:tools.catalog:call`); `401` if the session token is missing/bad, `403` if the host's gate
/// denies (opaque).
pub async fn mcp_catalog(
    State(gw): State<Gateway>,
    headers: HeaderMap,
) -> Result<Json<ToolsCatalog>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let catalog = lb_host::tools_catalog(gw.node.as_ref(), &p, p.ws())
        .await
        .map_err(|e| (StatusCode::FORBIDDEN, e.to_string()))?;
    Ok(Json(catalog))
}
