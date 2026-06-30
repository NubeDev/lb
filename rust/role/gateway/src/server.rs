//! Build and serve the gateway's axum [`Router`]. Construction (`router`) is split from serving
//! (`serve`) so tests can drive the routes with `tower::ServiceExt::oneshot` — no socket needed
//! for the request/response paths, and a real bound port only for the SSE test.
//!
//! CORS is permissive here for the dev UI (the Vite browser app on a different origin). A real
//! deployment tightens this to the served origin — config, not code.

use axum::routing::{delete, get, patch, post, put};
use axum::Router;
use tower_http::cors::CorsLayer;

use crate::routes::{
    add_datasource, add_team_member, archive_workspace, assign_grant, bus_stream, channel_stream,
    convert_unit, create_apikey, create_channel, create_team, create_user, create_workspace,
    define_role, delete_chain, delete_dashboard, delete_message, delete_role, delete_rule,
    delete_team, delete_user, disable_extension, disable_user, edit_message, enable_extension,
    enable_user, find_series, format_datetime, format_number, format_quantity, get_apikey,
    get_chain, get_chain_run, get_dashboard, get_doc, get_history, get_outbox_status, get_prefs,
    get_rule, grant_skill, latest_sample, link_doc, list_apikeys, list_chains, list_channels,
    list_dashboards, list_datasources, list_docs, list_extensions, list_grants, list_inbox,
    list_roles, list_rules, list_series, list_tables, list_team_members, list_teams, list_users,
    list_workspaces, load_skill, login, mcp_call, mcp_catalog, post_message, publish_extension,
    publish_message, purge_workspace, put_doc, put_skill, read_graph, read_samples, read_schema,
    remove_datasource, remove_team_member, rename_team, rename_workspace, request_approval,
    resolve_caps, resolve_inbox, resolve_prefs, resolve_workflow_approval, revoke_apikey,
    revoke_grant, revoke_tokens_route, rotate_apikey, run_chain, run_query, run_rule, run_stream,
    save_chain, save_dashboard, save_rule, scan_table, series_stream, serve_ext_ui,
    set_default_prefs, set_prefs, share_dashboard, share_doc, start_job, system_acp,
    system_overview, system_subsystem, system_tools, system_topology, test_datasource,
    uninstall_extension, write_samples,
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
        .route(
            "/channels/{cid}/messages/{id}",
            patch(edit_message).delete(delete_message),
        )
        .route("/channels/{cid}/stream", get(channel_stream))
        // agent-run live feed (agent-run scope Part 3) — the SSE analog of the channel stream for a
        // run: snapshot-then-deltas of the `RunEvent` projection. Auth via `?token=`; ws from token.
        .route("/runs/{job}/stream", get(run_stream))
        .route(
            "/teams/{team}/members",
            get(list_team_members).post(add_team_member),
        )
        .route("/teams/{team}/members/{user}", delete(remove_team_member))
        .route("/inbox/{channel}", get(list_inbox))
        .route("/inbox/{item}/resolve", post(resolve_inbox))
        .route("/outbox", get(get_outbox_status))
        // prefs + formatting (prefs scope). `prefs.*` are gated tenant verbs; `format.*`/`convert.*`
        // are the grant-free utility tier (pure CLDR/unit math, authenticated for identity only).
        .route("/prefs", get(get_prefs).put(set_prefs))
        .route("/prefs/resolve", post(resolve_prefs))
        .route("/prefs/default", put(set_default_prefs))
        .route("/format/datetime", post(format_datetime))
        .route("/format/number", post(format_number))
        .route("/format/quantity", post(format_quantity))
        .route("/convert/unit", post(convert_unit))
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
        // access-console scope — the access-graph gaps: resolved effective caps WITH provenance
        // (read), the live-token revoke lever (composes with the shipped grant-revoke), and the
        // missing roles.delete cascade. Each re-checks its admin cap server-side; ws from token.
        .route("/admin/authz/resolve", get(resolve_caps))
        .route("/admin/authz/revoke-tokens", post(revoke_tokens_route))
        .route("/admin/roles/{name}", delete(delete_role))
        // api-keys (api-keys scope) — the machine-credential admin surface: list (no hash/secret),
        // create (returns the one-time bearer), get (full resolved caps), revoke (instant local
        // revoke), rotate (new secret, old dead). Each re-checks `mcp:apikey.manage:call`
        // server-side; ws + principal from the token.
        .route("/admin/apikeys", get(list_apikeys).post(create_apikey))
        .route("/admin/apikeys/{id}", get(get_apikey))
        .route("/admin/apikeys/{id}/revoke", post(revoke_apikey))
        .route("/admin/apikeys/{id}/rotate", post(rotate_apikey))
        // extension lifecycle (lifecycle-management scope) — the browser's `ext.*` surface.
        .route("/extensions", get(list_extensions).post(publish_extension))
        .route("/extensions/{ext}", delete(uninstall_extension))
        .route("/extensions/{ext}/ui/{*path}", get(serve_ext_ui))
        .route("/mcp/call", post(mcp_call))
        .route("/mcp/catalog", get(mcp_catalog))
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
        // tool-catalog scope: the reachable MCP tool catalog (host-native + extension) behind the MCP
        // service page, and the ACP adapter's static facts behind the ACP service page. Same admin gate.
        .route("/system/tools", get(system_tools))
        .route("/system/acp", get(system_acp))
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
        // rules (rules-workbench scope, Phase 1) — the browser's `rules.*` Playground CRUD + run.
        // Each route re-checks `mcp:rules.<verb>:call` server-side; ws + principal from the token.
        .route("/rules/run", post(run_rule))
        .route("/rules", get(list_rules).post(save_rule))
        .route("/rules/{id}", get(get_rule).delete(delete_rule))
        // datasources (rules-workbench scope, Phase 3) — the browser's `datasource.*` admin surface.
        // Each route re-checks `mcp:datasource.<verb>:call` server-side via `call_tool`; ws +
        // principal from the token. The DSN is supplied only on the Add POST and never returned.
        .route("/datasources", get(list_datasources).post(add_datasource))
        .route("/datasources/{name}", delete(remove_datasource))
        .route("/datasources/{name}/test", post(test_datasource))
        // chains (rules-workbench scope, Phase 2) — the browser's `chains.*` DAG-canvas CRUD + run +
        // the per-step run snapshot poll. Each route re-checks `mcp:chains.<verb>:call` server-side;
        // ws + principal from the token. An invalid DAG at save → `400` (the canvas inline error).
        .route("/chains", get(list_chains).post(save_chain))
        .route("/chains/{id}", get(get_chain).delete(delete_chain))
        .route("/chains/{id}/run", post(run_chain))
        .route("/chains/{id}/runs/{run_id}", get(get_chain_run))
        .route("/series/{series}/stream", get(series_stream))
        // bus (widget-config-vars "Platform fix") — generic workspace-walled pub/sub. `POST /bus/publish`
        // is the fire-and-forget motion sink; `GET /bus/{subject}/stream?token=` is the live subscribe
        // (the motion analog of the series stream, for non-series subjects). Subject walled from the token.
        .route("/bus/publish", post(publish_message))
        .route("/bus/stream", get(bus_stream))
        .layer(CorsLayer::permissive())
        .with_state(gw)
}

/// Serve the gateway on `addr` (e.g. `127.0.0.1:8080`) until the process ends. The browser app's
/// transport points here.
pub async fn serve(gw: Gateway, addr: std::net::SocketAddr) -> std::io::Result<()> {
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, router(gw)).await
}
