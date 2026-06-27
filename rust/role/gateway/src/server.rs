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
    add_team_member, archive_workspace, assign_grant, channel_stream, create_channel, create_team,
    create_user, create_workspace, delete_team, delete_user, disable_extension, disable_user,
    enable_extension, enable_user, get_history, get_outbox_status, list_channels, list_extensions,
    list_grants, list_inbox, list_roles, list_team_members, list_teams, list_users,
    list_workspaces, login, post_message, publish_extension, purge_workspace, remove_team_member,
    rename_team, rename_workspace, resolve_inbox, revoke_grant, uninstall_extension,
};
use crate::state::Gateway;
use axum::routing::delete;

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
        .route("/teams/{team}/members/{user}", delete(remove_team_member))
        .route("/inbox/{channel}", get(list_inbox))
        .route("/inbox/{item}/resolve", post(resolve_inbox))
        .route("/outbox", get(get_outbox_status))
        // admin-crud: the destructive/admin surface (admin-console scope). Every verb re-checks the
        // capability server-side — the UI cap-gate is convenience only.
        .route("/admin/users", get(list_users).post(create_user))
        .route("/admin/users/{user}", delete(delete_user))
        .route("/admin/users/{user}/disable", post(disable_user))
        .route("/admin/users/{user}/enable", post(enable_user))
        .route("/admin/teams", get(list_teams).post(create_team))
        .route("/admin/teams/{team}", delete(delete_team))
        .route("/admin/teams/{team}/rename", post(rename_team))
        .route("/admin/workspaces/{ws}/rename", post(rename_workspace))
        .route("/admin/workspaces/{ws}/archive", post(archive_workspace))
        .route("/admin/workspaces/{ws}/purge", post(purge_workspace))
        .route("/admin/grants", get(list_grants).post(assign_grant))
        .route("/admin/grants/revoke", post(revoke_grant))
        .route("/admin/roles", get(list_roles))
        // extension lifecycle (lifecycle-management scope) — the browser's `ext.*` surface.
        .route("/extensions", get(list_extensions).post(publish_extension))
        .route("/extensions/{ext}", delete(uninstall_extension))
        .route("/extensions/{ext}/enable", post(enable_extension))
        .route("/extensions/{ext}/disable", post(disable_extension))
        .layer(CorsLayer::permissive())
        .with_state(gw)
}

/// Serve the gateway on `addr` (e.g. `127.0.0.1:8080`) until the process ends. The browser app's
/// transport points here.
pub async fn serve(gw: Gateway, addr: std::net::SocketAddr) -> std::io::Result<()> {
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, router(gw)).await
}
