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
    add_datasource, add_member, add_team_member, agent_invoke, archive_workspace, assign_grant,
    bus_stream, channel_stream, convert_unit, create_apikey, create_channel, create_def,
    create_identity, create_team, create_user, create_workspace, define_role, delete_dashboard,
    delete_def, delete_flow, delete_message, delete_role, delete_rule, delete_team, delete_user,
    disable_extension, disable_user, edit_message, enable_extension, enable_flow, enable_user,
    find_series, flow_node_state, flow_run_stream, format_datetime, format_number, format_quantity,
    get_agent_config_route, get_apikey, get_catalog, get_dashboard, get_def, get_doc, get_flow,
    get_flow_node, get_flow_run, get_history, get_identity, get_outbox_status, get_prefs, get_rule,
    grant_skill, identity_workspaces_route, inject_flow, latest_sample, lifecycle_flow, link_doc,
    list_apikeys, list_channels, list_dashboards, list_datasources, list_defs, list_docs,
    list_extensions, list_flow_nodes, list_flow_runs, list_flows, list_grants, list_identities,
    list_inbox, list_members, list_roles, list_rules, list_series, list_tables, list_team_members,
    list_navs, list_teams, list_users, list_workspaces, load_skill, login, mcp_call, mcp_catalog,
    native_call, delete_nav, get_nav, get_nav_pref, resolve_nav, save_nav, set_default_nav,
    set_nav_pref, share_nav,
    patch_flow_run, post_message, publish_extension, publish_message, purge_workspace, put_doc,
    put_skill, read_graph, read_samples, read_schema, remove_datasource, remove_member,
    remove_team_member, rename_team, rename_workspace, render_catalog_message, request_approval,
    reset_extension, resolve_caps, resolve_inbox, resolve_prefs, resolve_workflow_approval,
    revoke_apikey, revoke_grant, revoke_tokens_route, rotate_apikey, run_flow, run_query, run_rule,
    run_stream, save_dashboard, save_flow, save_rule, scan_table, series_stream, serve_ext_ui,
    set_agent_config_route, set_catalog, set_default_prefs, set_prefs, share_dashboard, share_doc,
    start_job, system_acp, system_overview, system_subsystem, system_tools, system_topology,
    telemetry_stream, test_active_def, test_datasource, test_def, uninstall_extension, update_def,
    update_flow_node, write_samples,
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
        // agent-config scope: the per-workspace default-runtime + model-endpoint record.
        // `GET` is member-level; `PUT` is admin-gated by the host (re-checked server-side).
        .route(
            "/agent/config",
            get(get_agent_config_route).put(set_agent_config_route),
        )
        // agent-catalog scope: the definition catalog. `GET`/list is member-level; create/update/delete
        // are admin-gated by the host (re-checked server-side). Routes mirror the `agent.def.*` verbs.
        .route("/agent/defs", get(list_defs).post(create_def))
        // agent-catalog test-and-secrets scope: the context-proving diagnostic. `POST /agent/defs/test`
        // tests the active `agent.config` pick; `POST /agent/defs/{id}/test` tests one definition. Both
        // admin-gated by the host (`mcp:agent.def.test:call`, re-checked server-side).
        .route("/agent/defs/test", post(test_active_def))
        .route("/agent/defs/{id}/test", post(test_def))
        .route(
            "/agent/defs/{id}",
            get(get_def).patch(update_def).delete(delete_def),
        )
        // active-agent-wiring Slice 5: the dashboard "AI widget" (genui author flow) drives the
        // workspace's ACTIVE agent. The run passes NO runtime → `invoke_via_runtime` resolves the
        // workspace default (self-gates on `mcp:agent.invoke:call`); ws + caps from the token.
        .route("/agent/invoke", post(agent_invoke))
        .route("/format/datetime", post(format_datetime))
        .route("/format/number", post(format_number))
        .route("/format/quantity", post(format_quantity))
        .route("/convert/unit", post(convert_unit))
        // i18n catalogs (i18n-catalogs scope, prefs Phase 2). All three are GATED tenant verbs (a
        // catalog carries workspace overrides): `message.render` (member for self, +fan-out grant for
        // another recipient), `prefs.catalog` (member — the merged override-over-builtin map), and
        // `message.set_catalog` (admin — writes a workspace override + publishes the "changed" hint).
        .route("/message/render", post(render_catalog_message))
        .route("/prefs/catalog", post(get_catalog))
        .route("/message/catalog", put(set_catalog))
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
        // global-identity scope — the global identity directory + per-workspace membership roster.
        // `identity.*` are gated `mcp:identity.manage:call`; `membership.*` are gated
        // `mcp:members.manage:call`. ws + principal from the token; the People tab reads
        // `GET /admin/members` (decision #9), the switcher reads `GET /admin/identities/{sub}/workspaces`.
        .route(
            "/admin/identities",
            get(list_identities).post(create_identity),
        )
        .route("/admin/identities/{sub}", get(get_identity))
        .route(
            "/admin/identities/{sub}/workspaces",
            get(identity_workspaces_route),
        )
        .route("/admin/members", get(list_members).post(add_member))
        .route("/admin/members/{sub}", delete(remove_member))
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
        // native-tier bridge: a browser page drives its extension's sidecar tools (ros.*, point.write,
        // …), the peer of /mcp/call for the native tier (native-tier scope).
        .route("/native/call", post(native_call))
        .route("/mcp/catalog", get(mcp_catalog))
        .route("/extensions/{ext}/enable", post(enable_extension))
        .route("/extensions/{ext}/disable", post(disable_extension))
        // native-tier resilience: re-arm an exhausted restart budget + force a fresh child (the
        // Extensions console Reset button). Gated `mcp:native.reset:call` inside the host verb.
        .route("/extensions/{ext}/reset", post(reset_extension))
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
        // nav (nav scope) — the browser's `nav.*` CRUD + the composite `nav.resolve` menu NavRail
        // renders + the member-owned pick + the workspace-default pointer. Each route re-checks the
        // gates server-side; ws + owner from the token; the per-user pick keyed to the token `sub`.
        .route("/navs", get(list_navs).post(save_nav))
        .route("/navs/{id}", get(get_nav).delete(delete_nav))
        .route("/navs/{id}/share", post(share_nav))
        .route("/nav/resolve", get(resolve_nav))
        .route("/nav/default", post(set_default_nav))
        .route("/nav/pref", get(get_nav_pref).post(set_nav_pref))
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
        // flows (flows-canvas + dashboard-binding scopes, Wave 3) — the browser's `flows.*` typed-node
        // canvas CRUD + run + the per-node run snapshot + reattach, plus enable/inject. Each route
        // re-checks `mcp:flows.<verb>:call` server-side; ws + principal from the token. An invalid DAG
        // or schema-invalid node config at save → `400` (the canvas inline error). `flows` is the one
        // DAG engine (chains retired — chains-retirement scope).
        .route("/flows", get(list_flows).post(save_flow))
        .route("/flows/nodes", get(list_flow_nodes))
        .route("/flows/{id}", get(get_flow).delete(delete_flow))
        .route("/flows/{id}/run", post(run_flow))
        .route("/flows/{id}/enable", post(enable_flow))
        .route("/flows/{id}/inject", post(inject_flow))
        .route("/flows/{id}/runs", get(list_flow_runs))
        .route("/flows/{id}/node_state", get(flow_node_state))
        .route("/flows/runs/{run_id}", get(get_flow_run))
        // The live settle feed (flow-runtime-control-scope): `EventSource` opens this and folds a
        // `snapshot` frame then `flow` deltas as each node settles + a terminal `run-finished` — the
        // replacement for polling `runs.get`. GET, so it never collides with the POST `{op}` route.
        .route("/flows/runs/{run_id}/stream", get(flow_run_stream))
        .route("/flows/runs/{run_id}/{op}", post(lifecycle_flow))
        .route("/flows/runs/{run_id}/patch", post(patch_flow_run))
        // Per-node config CRUD on the SAVED flow (flow-runtime-control-scope) — read/replace one
        // node's config without re-posting the whole `Flow`. Gated `flows.node.get`/`flows.node.update`.
        .route(
            "/flows/node/{id}/{node}",
            get(get_flow_node).post(update_flow_node),
        )
        .route("/series/{series}/stream", get(series_stream))
        // telemetry console (telemetry-console scope) — the live tail the in-browser console watches
        // scroll. `GET /telemetry/stream?token=` folds a catch-up snapshot then the live ws-walled
        // feed (`event: snapshot` then `event: telemetry`). 403 before any body if the grant is
        // missing; the bus subject is ws-walled so a ws-B session never observes ws-A. There is NO
        // telemetry.write route — writes come from the SurrealCappedLayer only.
        .route("/telemetry/stream", get(telemetry_stream))
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
