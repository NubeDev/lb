//! The server's one route: `GET /artifacts/{ext_id}/{version}` → the signed [`Artifact`] as JSON, or
//! `404` if the origin lacks it / is offline. No auth, no signing, no verification here — the origin
//! is dumb and the artifact is self-authenticating (the client verifies its signature on arrival).
//! A real deployment adds TLS + a read token (config, a follow-up); the bytes' trust never rides the
//! transport, so an unauthenticated read leaks only already-public signed artifacts.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use lb_registry::Artifact;

use crate::catalog::ArtifactStore;

/// Serve the artifact for `(ext_id, version)`. `404` (not `500`) on a miss: a missing version and an
/// offline origin are indistinguishable to the client by design (the `Source` contract → both become
/// `NotAvailable`). The body is the artifact verbatim — including the publisher's signature — so the
/// client can re-establish trust without trusting the wire.
pub(crate) async fn get_artifact(
    State(store): State<ArtifactStore>,
    Path((ext_id, version)): Path<(String, String)>,
) -> Result<Json<Artifact>, StatusCode> {
    store
        .get(&ext_id, &version)
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

/// `POST /artifacts` — the **publish** endpoint (lifecycle-management scope). Verify the uploaded
/// artifact against the publisher allow-list **before** storing it (authenticity before authority):
/// a tampered/unsigned/foreign-key upload is `403` and **nothing is stored**. `204` on success;
/// idempotent on `(ext_id, version)`. This is the producer side the registry never had — install
/// then proceeds through the existing verified pull, unchanged.
pub(crate) async fn post_artifact(
    State(store): State<ArtifactStore>,
    Json(artifact): Json<Artifact>,
) -> StatusCode {
    match store.publish(artifact) {
        Ok(()) => StatusCode::NO_CONTENT,
        // Opaque: an unverified upload learns nothing about the allow-list (mirrors the read miss).
        Err(_) => StatusCode::FORBIDDEN,
    }
}
