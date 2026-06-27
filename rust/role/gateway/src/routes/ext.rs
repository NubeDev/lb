//! Extension lifecycle routes — the browser's `ext.*` surface over the gateway (lifecycle-management
//! scope: THE biggest real gap — the host had the verbs but only the Tauri shell reached them, so a
//! browser threw `unknown command`). Mirror `lb_host::ext_*` 1:1; gated server-side on
//! `mcp:ext.list:call` / `mcp:ext.disable:call` / `mcp:ext.uninstall:call`.

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use lb_host::{ExtError, ExtRow};
use lb_registry::{Artifact, Visibility};

use crate::session::authenticate;
use crate::state::Gateway;

/// `GET /extensions` — every installed extension (both tiers) with live state.
pub async fn list_extensions(
    State(gw): State<Gateway>,
    headers: HeaderMap,
) -> Result<Json<Vec<ExtRow>>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers).map_err(|e| e.into_response())?;
    let rows = lb_host::ext_list(&gw.node, &p, p.ws())
        .await
        .map_err(forbid)?;
    Ok(Json(rows))
}

/// `POST /extensions/{ext}/enable` — durable enable (eligible to auto-start on boot).
pub async fn enable_extension(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path(ext): Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    let p = authenticate(&gw, &headers).map_err(|e| e.into_response())?;
    lb_host::ext_enable(&gw.node, &p, p.ws(), &ext, gw.now)
        .await
        .map_err(forbid)?;
    Ok(StatusCode::NO_CONTENT)
}

/// `POST /extensions/{ext}/disable` — durable disable (stop now + do-not-auto-start).
pub async fn disable_extension(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path(ext): Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    let p = authenticate(&gw, &headers).map_err(|e| e.into_response())?;
    lb_host::ext_disable(&gw.node, &p, p.ws(), &ext, gw.now)
        .await
        .map_err(forbid)?;
    Ok(StatusCode::NO_CONTENT)
}

/// `DELETE /extensions/{ext}` — uninstall (stop/unload + delete the install record).
pub async fn uninstall_extension(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path(ext): Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    let p = authenticate(&gw, &headers).map_err(|e| e.into_response())?;
    lb_host::ext_uninstall(&gw.node, &p, p.ws(), &ext, gw.now)
        .await
        .map_err(forbid)?;
    Ok(StatusCode::NO_CONTENT)
}

/// `POST /extensions` — **publish** (upload) a signed extension artifact (lifecycle-management scope:
/// the admin console's "publish an extension" path). Body is the [`Artifact`] verbatim (the same wire
/// shape the registry-host `POST /artifacts` accepts), including the publisher signature. The workspace
/// comes from the token, never the body (the hard wall, §7). Gated server-side on `mcp:ext.publish:call`
/// inside the host verb; verify-before-store inside it too. `204` on publish, `403` on a capability
/// deny, `422` on a verification failure (tampered/unsigned/foreign-key — nothing stored).
pub async fn publish_extension(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Json(artifact): Json<Artifact>,
) -> Result<StatusCode, (StatusCode, String)> {
    let p = authenticate(&gw, &headers).map_err(|e| e.into_response())?;
    lb_host::ext_publish(
        &gw.node,
        &p,
        p.ws(),
        artifact,
        &gw.trusted,
        Visibility::Private,
        gw.now,
    )
    .await
    .map_err(publish_status)?;
    Ok(StatusCode::NO_CONTENT)
}

fn forbid(e: impl std::fmt::Display) -> (StatusCode, String) {
    (StatusCode::FORBIDDEN, e.to_string())
}

/// Map a publish error to a status: a capability/workspace deny is `403`; a verification failure is
/// `422` (the upload was well-formed but its signature/digest did not check out — distinct from "you
/// may not"); any store fault is `403`-opaque like the other ext routes.
fn publish_status(e: ExtError) -> (StatusCode, String) {
    match e {
        ExtError::Unverified => (StatusCode::UNPROCESSABLE_ENTITY, e.to_string()),
        other => forbid(other),
    }
}
