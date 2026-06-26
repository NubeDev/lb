//! Build and serve the gateway's axum [`Router`]. Construction (`router`) is split from serving
//! (`serve`) so tests can drive the routes with `tower::ServiceExt::oneshot` — no socket needed
//! for the request/response paths, and a real bound port only for the SSE test.
//!
//! CORS is permissive here for the dev UI (the Vite browser app on a different origin). A real
//! deployment tightens this to the served origin — config, not code.

use axum::routing::get;
use axum::Router;
use tower_http::cors::CorsLayer;

use crate::routes::{channel_stream, get_history, post_message};
use crate::state::Gateway;

/// The gateway router: the channel verbs the browser calls, mirroring the host one-to-one.
///
/// - `GET  /channels/{cid}/messages` → durable history
/// - `POST /channels/{cid}/messages` → post a message
/// - `GET  /channels/{cid}/stream`   → SSE: live messages + presence
pub fn router(gw: Gateway) -> Router {
    Router::new()
        .route(
            "/channels/{cid}/messages",
            get(get_history).post(post_message),
        )
        .route("/channels/{cid}/stream", get(channel_stream))
        .layer(CorsLayer::permissive())
        .with_state(gw)
}

/// Serve the gateway on `addr` (e.g. `127.0.0.1:8080`) until the process ends. The browser app's
/// transport points here (the only UI file that changes for S3 is `lib/ipc/invoke.ts`).
pub async fn serve(gw: Gateway, addr: std::net::SocketAddr) -> std::io::Result<()> {
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, router(gw)).await
}
