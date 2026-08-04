//! The wall every upload route passes: resolve the **opaque** sink name, authenticate, and enforce
//! the sink's OWN capability — on every call in the sequence, not only at `begin`, so a session that
//! loses its grant mid-upload stops there.
//!
//! The core names NO sink (rule 10). `{sink}` is a key into the embedder's registry and nothing
//! else; a sink called `"package"` on one host and `"firmware"` on another needs no lb change, and
//! lb never learns whose bytes it moved.

use std::sync::Arc;

use axum::http::{HeaderMap, StatusCode};
use lb_auth::Principal;
use lb_host::{holds_cap, UploadError, UploadSink};

use crate::session::authenticate;
use crate::state::Gateway;

/// Resolve `{sink}` and prove the caller holds its declared capability. Returns the sink and the
/// verified principal.
///
/// An unknown sink is `404` and a missing capability is `403`-opaque — deliberately in that order,
/// because which sinks an embedder registered is a static, public fact about the build (the route is
/// not even mounted without one), while what a caller may do with them is not.
pub async fn resolve(
    gw: &Gateway,
    headers: &HeaderMap,
    sink: &str,
) -> Result<(Arc<dyn UploadSink>, Principal), (StatusCode, String)> {
    let found = gw
        .upload_sinks
        .iter()
        .find(|(name, _)| name == sink)
        .map(|(_, s)| Arc::clone(s))
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                format!("no such upload sink: {sink}"),
            )
        })?;
    let p = authenticate(gw, headers)
        .await
        .map_err(|e| e.into_response())?;
    // THE SINK CHOOSES; LB ENFORCES. The cap string is opaque to the gateway — it is checked through
    // the same wall every other grant passes, in the caller's own workspace (from the token).
    if !holds_cap(&p, p.ws(), found.required_cap()) {
        return Err((StatusCode::FORBIDDEN, "denied".into()));
    }
    Ok((found, p))
}

/// Map a sink's typed refusal onto HTTP. The one mapping, shared by all five routes.
///
/// [`UploadError::Offset`] is the load-bearing case: **409 carrying the correct offset**, so a
/// client that lost track resumes without guessing. The body is JSON (not prose) precisely because a
/// client must parse the number.
pub fn status(e: &UploadError) -> (StatusCode, String) {
    match e {
        UploadError::NotFound => (StatusCode::NOT_FOUND, "no such upload".into()),
        UploadError::Offset { expected } => (
            StatusCode::CONFLICT,
            serde_json::json!({ "error": "offset", "offset": expected }).to_string(),
        ),
        UploadError::TooLarge { limit } => (
            StatusCode::PAYLOAD_TOO_LARGE,
            serde_json::json!({ "error": "too_large", "limit": limit }).to_string(),
        ),
        UploadError::Rejected(m) => (StatusCode::BAD_REQUEST, m.clone()),
        UploadError::Backend(m) => (StatusCode::BAD_GATEWAY, m.clone()),
    }
}
