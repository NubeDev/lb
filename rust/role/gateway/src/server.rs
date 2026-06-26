//! Build and serve the gateway's axum [`Router`]. Construction (`router`) is split from serving
//! (`serve`) so tests can drive the routes with `tower::ServiceExt::oneshot` — no socket needed
//! for the request/response paths, and a real bound port only for the SSE test.
//!
//! CORS is permissive here for the dev UI (the Vite browser app on a different origin). A real
//! deployment tightens this to the served origin — config, not code.

use axum::routing::{get, post};
use axum::Router;
use tower_http::cors::CorsLayer;

use crate::routes::{
    add_team_member, channel_stream, create_channel, create_workspace, get_history,
    get_outbox_status, list_channels, list_inbox, list_team_members, list_workspaces, login,
    post_message, resolve_inbox,
};
use crate::state::Gateway;

/// The gateway router: every browser verb, mirroring the host one-to-one. Each guarded route reads
/// the session token (the `login` route issues it); the workspace + caps come from the token.
///
/// Session:        `POST /login`
/// Workspaces:     `GET|POST /workspaces`
/// Channel msgs:   `GET|POST /channels/{cid}/messages`, `GET /channels/{cid}/stream`
/// Channel registry:`GET|POST /channels`
/// Members:        `GET|POST /teams/{team}/members`
/// Inbox:          `GET /inbox/{channel}`, `POST /inbox/{item}/resolve`
/// Outbox status:  `GET /outbox`
pub fn router(gw: Gateway) -> Router {
    Router::new()
        .route("/login", post(login))
        .route("/workspaces", get(list_workspaces).post(create_workspace))
        .route("/channels", get(list_channels).post(create_channel))
        .route(
            "/channels/{cid}/messages",
            get(get_history).post(post_message),
        )
        .route("/channels/{cid}/stream", get(channel_stream))
        .route(
            "/teams/{team}/members",
            get(list_team_members).post(add_team_member),
        )
        .route("/inbox/{channel}", get(list_inbox))
        .route("/inbox/{item}/resolve", post(resolve_inbox))
        .route("/outbox", get(get_outbox_status))
        .layer(CorsLayer::permissive())
        .with_state(gw)
}

/// Serve the gateway on `addr` (e.g. `127.0.0.1:8080`) until the process ends. The browser app's
/// transport points here.
pub async fn serve(gw: Gateway, addr: std::net::SocketAddr) -> std::io::Result<()> {
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, router(gw)).await
}
