//! The multi-tenant route: `POST /webhook/{tenant}` — resolve the tenant from the URL, verify the
//! delivery against **that tenant's** secret, then drive `ingest_via_bridge` into that tenant's
//! workspace as that tenant's principal. The single-tenant [`crate::routes`] handler's twin, with
//! the secret/ws/principal selected per request instead of fixed.
//!
//! Status mapping is identical to the single-tenant route, with one addition: an **unknown tenant**
//! is folded into the `401` (not a `404`) so the front door is not an enumeration oracle. So a `401`
//! means "not authentic OR not a tenant" — indistinguishable to a prober, by design. The cap+ws
//! gates inside `ingest_via_bridge` still re-check authority for the resolved principal (a verified
//! delivery to a known tenant can still be `403` if ungranted).

use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};

use lb_host::{ingest_via_bridge, WorkflowError};

use crate::routes::SIGNATURE_HEADER;
use crate::tenant::TenantRegistry;
use crate::verify::verify_signature;

/// Handle one delivery to `/webhook/{tenant}`. The body is raw [`Bytes`] so the signature is checked
/// over the EXACT bytes GitHub signed (re-serializing would change them — see [`crate::verify`]).
pub(crate) async fn post_tenant_webhook(
    State(registry): State<TenantRegistry>,
    Path(tenant): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> StatusCode {
    // Resolve the tenant from the URL FIRST — the per-tenant secret is needed before the HMAC check.
    // An unknown tenant is an opaque `401` (no `404` enumeration oracle), the same as a forgery.
    let Some(tenant) = registry.resolve(&tenant) else {
        return StatusCode::UNAUTHORIZED;
    };

    // Transport authenticity: HMAC over the raw body, with THIS tenant's secret. A delivery signed
    // with another tenant's secret fails here → `401`, never crossing into this workspace.
    let sig = headers.get(SIGNATURE_HEADER).and_then(|v| v.to_str().ok());
    if verify_signature(tenant.secret(), &body, sig).is_err() {
        return StatusCode::UNAUTHORIZED;
    }

    let raw = match std::str::from_utf8(&body) {
        Ok(s) => s,
        Err(_) => return StatusCode::UNPROCESSABLE_ENTITY,
    };

    // Authentic — hand off to the host edge under this tenant's principal + workspace. The two cap
    // gates, the workspace wall, and the idempotent inbox upsert all live in `ingest_via_bridge`.
    match ingest_via_bridge(&registry.node, &tenant.principal, &tenant.ws, raw).await {
        Ok(_item) => StatusCode::OK,
        Err(WorkflowError::Denied) => StatusCode::FORBIDDEN,
        Err(WorkflowError::Bridge(_)) => StatusCode::UNPROCESSABLE_ENTITY,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}
