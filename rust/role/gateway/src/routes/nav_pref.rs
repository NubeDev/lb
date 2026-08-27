//! `/nav/pref` — the **member-owned active pick**, its pinned favorites, and the force-built-in
//! escape hatch. The gateway twin of `lb_host`'s `nav/pref.rs`, split out of `nav.rs` so the nav
//! CRUD/share/resolve surface and the per-member preference surface are edited apart (FILE-LAYOUT).
//!
//! Both routes are keyed to the token's `sub`, never the body: a caller can neither read nor set
//! another member's pick or pins. Gated `nav.resolve` — member-level, like the resolver they steer.

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use serde::Deserialize;
use serde_json::Value;

use crate::session::authenticate;
use crate::state::Gateway;

use super::nav::status;

/// `GET /nav/pref` — read the caller's own active-nav pick. Gated `nav.resolve`; member-level.
pub async fn get_nav_pref(
    State(gw): State<Gateway>,
    headers: HeaderMap,
) -> Result<Json<Value>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let pref = lb_host::nav_pref_get(&gw.node.store, &p, p.ws())
        .await
        .map_err(status)?;
    Ok(Json(serde_json::to_value(pref).unwrap_or(Value::Null)))
}

/// `POST /nav/pref` body — set the caller's own active-nav pick (empty `id` clears it), their
/// pinned favorites (hide-and-pins scope), and/or the force-built-in escape-hatch override
/// (no-lockout scope). `pinned` absent = pins untouched; present = full replace. `forceBuiltin`
/// absent = override untouched; present = set/clear — and NEVER touches `id`, so the member's real
/// pick survives the "Show all pages" / "Use my menu" round-trip.
#[derive(Debug, Deserialize)]
pub struct SetNavPref {
    /// Absent = leave the active pick untouched (a pin-only write); `""` clears it.
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub pinned: Option<Vec<String>>,
    /// Absent = leave the force-built-in override untouched; present = set/clear it (the decoupled
    /// escape-hatch slot; the real pick in `id` is never written by this axis).
    #[serde(default)]
    pub force_builtin: Option<bool>,
}

/// `POST /nav/pref` — set the caller's own active-nav pick, pins, and/or force-built-in override.
/// Keyed to the token `sub` (a caller cannot set another user's pick or pins). Gated `nav.resolve`;
/// member-level.
pub async fn set_nav_pref(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Json(body): Json<SetNavPref>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let pref = match body.force_builtin {
        Some(force) => {
            lb_host::nav_pref_set_force_builtin(&gw.node.store, &p, p.ws(), force, gw.now())
                .await
                .map_err(status)?
        }
        None => lb_host::nav_pref_set(
            &gw.node.store,
            &p,
            p.ws(),
            body.id.as_deref(),
            body.pinned,
            gw.now(),
        )
        .await
        .map_err(status)?,
    };
    Ok(Json(serde_json::to_value(pref).unwrap_or(Value::Null)))
}
