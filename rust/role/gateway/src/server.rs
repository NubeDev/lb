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
    create_user, create_workspace, define_role, delete_dashboard, delete_team, delete_user,
    disable_extension, disable_user, enable_extension, enable_user, find_series, get_dashboard,
    get_doc, get_history, get_outbox_status, grant_skill, latest_sample, link_doc, list_channels,
    list_dashboards, list_docs, list_extensions, list_grants, list_inbox, list_roles, list_series,
    list_tables, list_team_members, list_teams, list_users, list_workspaces, load_skill, login,
    mcp_call, post_message, publish_extension, purge_workspace, put_doc, put_skill, read_graph,
    read_samples, read_schema, remove_team_member, rename_team, rename_workspace, request_approval,
    resolve_inbox, resolve_workflow_approval, revoke_grant, run_query, save_dashboard, scan_table,
    series_stream, serve_ext_ui, share_dashboard, share_doc, start_job, system_overview,
    system_subsystem, system_topology, uninstall_extension, write_samples,
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
        .route("/admin/roles", get(list_roles).post(define_role))
        // extension lifecycle (lifecycle-management scope) — the browser's `ext.*` surface.
        .route("/extensions", get(list_extensions).post(publish_extension))
        .route("/extensions/{ext}", delete(uninstall_extension))
        .route("/extensions/{ext}/ui/{*path}", get(serve_ext_ui))
        .route("/mcp/call", post(mcp_call))
        .route("/extensions/{ext}/enable", post(enable_extension))
        .route("/extensions/{ext}/disable", post(disable_extension))
        // shared assets (files/skills scope) — the browser's `assets.*` surface, finally reachable
        // over the gateway (was Tauri-only → `unknown command` in the browser). Each re-checks the
        // S4 gates server-side; the workspace + owner come from the token.
        .route("/docs", get(list_docs).post(put_doc))
        .route("/docs/{id}", get(get_doc))
        .route("/docs/{id}/share", post(share_doc))
        .route("/docs/{id}/link", post(link_doc))
        .route("/skills", post(put_skill))
        .route("/skills/{id}", get(load_skill))
        .route("/skills/{id}/grant", post(grant_skill))
        // coding workflow (coding-workflow scope) — the browser's `workflow.*` surface. The headline
        // S6 approval gate runs server-side; reading the outbox is `GET /outbox` above.
        .route("/approvals/{id}/request", post(request_approval))
        .route("/approvals/{id}/resolve", post(resolve_workflow_approval))
        .route("/approvals/{id}/start", post(start_job))
        // DB browser (data-console scope) — the browser's admin, READ-ONLY `store.*` lens: table
        // picker + counts, paged raw rows, relation graph for react-flow. Each re-checks the
        // **admin** cap server-side (gate-3-relaxed → admin-only). No write routes by design.
        .route("/store/tables", get(list_tables))
        .route("/store/tables/{table}/rows", get(scan_table))
        .route("/store/graph", get(read_graph))
        // Read-only SQL (widget-builder Slice A) — the "direct SurrealDB" widget source + the visual
        // SQL builder's schema feed. `POST /store/query` runs a parse-allowlisted, bounded SELECT;
        // `GET /store/schema` lists the workspace's tables + columns. Each re-checks its cap
        // server-side; ws from the token; the SQL can never name a namespace (read-only, walled).
        .route("/store/query", post(run_query))
        .route("/store/schema", get(read_schema))
        // System map (system-map scope) — the admin, READ-ONLY workspace topology + status console:
        // a per-subsystem status grid + a react-flow wiring graph, both from one live snapshot. Each
        // route re-checks the **admin** cap server-side; ws + principal from the token. No writes.
        .route("/system/overview", get(system_overview))
        .route("/system/topology", get(system_topology))
        .route("/system/subsystem/{id}", get(system_subsystem))
        // ingest / series (data-console scope) — the browser's `ingest.*`/`series.*` surface (the S8
        // verbs, finally reachable over the gateway). Manual write + series list/find + latest/recent.
        .route("/ingest", post(write_samples))
        .route("/series", get(list_series))
        .route("/series/find", post(find_series))
        .route("/series/{series}/latest", get(latest_sample))
        .route("/series/{series}/samples", get(read_samples))
        // dashboard (dashboard scope) — the browser's `dashboard.*` CRUD + the live **series** SSE
        // feed widgets watch. Each route re-checks the three gates server-side; ws+owner from the
        // token. `GET /series/{series}/stream` is the motion analog of the channel stream.
        .route("/dashboards", get(list_dashboards).post(save_dashboard))
        .route(
            "/dashboards/{id}",
            get(get_dashboard).delete(delete_dashboard),
        )
        .route("/dashboards/{id}/share", post(share_dashboard))
        .route("/series/{series}/stream", get(series_stream))
        .layer(CorsLayer::permissive())
        .with_state(gw)
}

/// Serve the gateway on `addr` (e.g. `127.0.0.1:8080`) until the process ends. The browser app's
/// transport points here.
pub async fn serve(gw: Gateway, addr: std::net::SocketAddr) -> std::io::Result<()> {
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, router(gw)).await
}
