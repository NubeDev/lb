//! Build and serve the registry-host's axum [`Router`]. Construction (`router`) is split from serving
//! (`serve`) so tests can drive the route with `tower::ServiceExt::oneshot` — no socket for the
//! request/response path — and a real bound port only for the round-trip-over-HTTP test (the same
//! split `lb-role-gateway` uses).

use axum::routing::{get, post};
use axum::Router;

use crate::catalog::ArtifactStore;
use crate::routes::{get_artifact, post_artifact};

/// The registry-host router: the read endpoint a node's `HttpSource` calls + the publish endpoint a
/// workspace admin pushes signed artifacts to.
///
/// - `GET  /artifacts/{ext_id}/{version}` → the signed artifact, or `404` on a miss/offline.
/// - `POST /artifacts` → publish a signed artifact (verify-before-store; `403` on an unsigned/foreign
///   upload, `204` on success).
pub fn router(store: ArtifactStore) -> Router {
    Router::new()
        .route("/artifacts/{ext_id}/{version}", get(get_artifact))
        .route("/artifacts", post(post_artifact))
        .with_state(store)
}

/// Serve the registry-host on `addr` (e.g. `127.0.0.1:9000`) until the process ends. A node's
/// `HttpSource` base URL points here.
pub async fn serve(store: ArtifactStore, addr: std::net::SocketAddr) -> std::io::Result<()> {
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, router(store)).await
}
