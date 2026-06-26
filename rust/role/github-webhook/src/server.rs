//! Build and serve the receiver's axum [`Router`]. Construction (`router`) is split from serving
//! (`serve`) so tests can drive the route with `tower::ServiceExt::oneshot` — no socket for the
//! deny/bad-signature/isolation paths — and a real bound port only for the round-trip-over-HTTP
//! test (the same split `lb-role-registry-host` and `lb-role-gateway` use).

use axum::routing::post;
use axum::Router;

use crate::route_tenant::post_tenant_webhook;
use crate::routes::post_webhook;
use crate::state::WebhookState;
use crate::tenant::TenantRegistry;

/// The single-tenant receiver's router: one inbound endpoint, one `(ws, principal, secret)`.
///
/// - `POST /webhook` → `200` ingested · `401` bad signature · `403` denied · `422` malformed.
pub fn router(state: WebhookState) -> Router {
    Router::new()
        .route("/webhook", post(post_webhook))
        .with_state(state)
}

/// The **multi-tenant front door**'s router: one process, many workspaces, routed by URL slug.
///
/// - `POST /webhook/{tenant}` → `200` ingested · `401` bad signature **or unknown tenant** ·
///   `403` denied · `422` malformed. Each repo points its Payload URL at its own `/webhook/{tenant}`.
pub fn tenant_router(registry: TenantRegistry) -> Router {
    Router::new()
        .route("/webhook/{tenant}", post(post_tenant_webhook))
        .with_state(registry)
}

/// Serve the single-tenant receiver on `addr` until the process ends. The repo's webhook config
/// points its Payload URL here (`https://<host>/webhook`).
pub async fn serve(state: WebhookState, addr: std::net::SocketAddr) -> std::io::Result<()> {
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, router(state)).await
}

/// Serve the multi-tenant front door on `addr` until the process ends.
pub async fn serve_tenants(
    registry: TenantRegistry,
    addr: std::net::SocketAddr,
) -> std::io::Result<()> {
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, tenant_router(registry)).await
}
