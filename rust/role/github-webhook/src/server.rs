//! Build and serve the receiver's axum [`Router`]. Construction (`router`) is split from serving
//! (`serve`) so tests can drive the route with `tower::ServiceExt::oneshot` — no socket for the
//! deny/bad-signature/isolation paths — and a real bound port only for the round-trip-over-HTTP
//! test (the same split `lb-role-registry-host` and `lb-role-gateway` use).

use axum::routing::post;
use axum::Router;

use crate::routes::post_webhook;
use crate::state::WebhookState;

/// The receiver's router: the one inbound endpoint GitHub POSTs deliveries to.
///
/// - `POST /webhook` → `200` ingested · `401` bad signature · `403` denied · `422` malformed.
pub fn router(state: WebhookState) -> Router {
    Router::new()
        .route("/webhook", post(post_webhook))
        .with_state(state)
}

/// Serve the receiver on `addr` (e.g. `0.0.0.0:8080`) until the process ends. The repo's webhook
/// config points its Payload URL here (`https://<host>/webhook`).
pub async fn serve(state: WebhookState, addr: std::net::SocketAddr) -> std::io::Result<()> {
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, router(state)).await
}
