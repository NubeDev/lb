//! The receiver's one route: `POST /webhook` — verify the delivery's signature, then drive the
//! host's `ingest_via_bridge` (HMAC authenticity → the sandboxed `github-bridge.normalize` →
//! the must-deliver inbox write). The handler is thin: it owns the HTTP↔host translation and
//! nothing else (no normalization, no store access — those are the bridge's and the host's).
//!
//! Status mapping is deliberate and leaks nothing:
//!   - bad/absent signature           → `401 Unauthorized` (a bare body — no oracle, no secret);
//!   - authentic but the caps deny it  → `403 Forbidden`    (the host's gate refused; opaque);
//!   - a malformed payload the bridge can't normalize → `422 Unprocessable Entity`;
//!   - ingested (or idempotently re-ingested) → `200 OK`.
//!
//! Note the signature gate runs FIRST, on the RAW body, before any parse — re-serializing would
//! change bytes and never match (see [`crate::verify`]). The cap gates run AFTER, inside
//! `ingest_via_bridge`; an authentic-but-ungranted delivery is a `403`, distinct from a forgery.

use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};

use lb_host::{ingest_via_bridge, WorkflowError};

use crate::state::WebhookState;
use crate::verify::verify_signature;

/// GitHub's signature header (the SHA-256 variant; the legacy `X-Hub-Signature` SHA-1 header is
/// deliberately NOT accepted — SHA-1 is broken for this purpose).
const SIGNATURE_HEADER: &str = "x-hub-signature-256";

/// Handle one webhook delivery. The body is taken as raw [`Bytes`] (not parsed JSON) so the
/// signature is verified over the EXACT bytes GitHub signed.
pub(crate) async fn post_webhook(
    State(state): State<WebhookState>,
    headers: HeaderMap,
    body: Bytes,
) -> StatusCode {
    // 1. Transport authenticity: HMAC over the raw body. A failure is an opaque `401` — never a
    //    detail that would turn the front door into a signature oracle or leak the secret.
    let sig = headers.get(SIGNATURE_HEADER).and_then(|v| v.to_str().ok());
    if verify_signature(state.secret(), &body, sig).is_err() {
        return StatusCode::UNAUTHORIZED;
    }

    // 2. The body is authentic — hand the raw JSON to the host edge. The two capability gates and
    //    the idempotent inbox upsert all live in `ingest_via_bridge`; the receiver adds no
    //    authority and re-delivery of the same issue still produces one item.
    let raw = match std::str::from_utf8(&body) {
        Ok(s) => s,
        // A non-UTF-8 body passed the MAC (the secret-holder sent garbage) — unprocessable.
        Err(_) => return StatusCode::UNPROCESSABLE_ENTITY,
    };

    match ingest_via_bridge(&state.node, &state.principal, &state.ws, raw).await {
        Ok(_item) => StatusCode::OK,
        // The caps gate refused an authentic delivery (no grant, or the bridge isn't installed —
        // both opaque by design). Distinct from the `401`: the sender IS GitHub, but unauthorized.
        Err(WorkflowError::Denied) => StatusCode::FORBIDDEN,
        // The bridge could not normalize the payload (malformed webhook shape) — a client fault.
        Err(WorkflowError::Bridge(_)) => StatusCode::UNPROCESSABLE_ENTITY,
        // Any other host fault (store/bus) is ours, not the caller's.
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}
