//! Result-render coverage (widget-platform scope, Slice C — closes G1) through the REAL MCP bridge
//! (`lb_host::call_tool`) and the direct shell path (`dashboard_pin`), against a real store/node —
//! no fakes. Slice C gives the **tabular** host tools a `descriptor.result = table` envelope so the
//! channel CAN render them descriptor-driven, the AI discovers the render via `tools.catalog`, and
//! Slice B's `dashboard.pin` can pin them with ZERO tool-specific code in the pin path (the mint
//! treats `source.tool` as opaque data — rule 10). Covers the mandatory categories:
//!   - **catalog visibility (the menu IS the permission model)**: a principal WITHOUT
//!     `mcp:federation.query:call` does NOT get the `federation.query` descriptor (or its NEW `result`
//!     envelope) in `tools.catalog`; the paired happy path (with the cap) sees it WITH the envelope.
//!     No new cap — the verb's own call gate decides the envelope's visibility too.
//!   - **workspace isolation**: a `federation.query` pin in ws-A produces a cell on a ws-A dashboard;
//!     a ws-B principal cannot read it. The cell's `source.tool` re-resolves under the viewer's grant
//!     at render (the wall is structural — federation/query.rs:42 already namespace-walls the alias).
//!   - **the HEADLINE**: pin `federation.query`'s NEW `result` envelope → a persisted
//!     `pin-federation-query` cell that reloads via `dashboard.get` and carries the envelope's
//!     `view`/`source`/`tools` intact, with ZERO federation-specific code in the pin path (the mint
//!     is GENERIC over the tool id — Slice B proven, re-asserted here for the new envelope).
//!   - **shell path AND headless `POST /mcp/call` parity**: the same envelope pinned via the direct
//!     `dashboard_pin` AND via `call_tool` → `dashboard.pin` produces the SAME cell.
//!   - **query.run envelope parity**: the same mint path produces a `pin-query-run` cell.
//!   - **idempotent re-pin**: re-pinning the same `federation.query` envelope REPLACES the cell, not
//!     duplicates (Slice B proven; re-asserted for the new envelope).

use std::sync::Arc;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{call_tool, dashboard_get, dashboard_pin, Node};
use lb_mcp::ToolError;
use serde_json::{json, Value};

fn principal(sub: &str, ws: &str, caps: &[&str]) -> Principal {
    let key = SigningKey::generate();
    let claims = Claims {
        sub: sub.into(),
        ws: ws.into(),
        role: Role::Member,
        caps: caps.iter().map(|s| s.to_string()).collect(),
        iat: 0,
        exp: u64::MAX,
        constraint: None,
        run_id: None,
    };
    let token = mint(&key, &claims);
    verify(&key, &token, 1).expect("token verifies")
}

const PIN: &str = "mcp:dashboard.pin:call";
const GET: &str = "mcp:dashboard.get:call";
const LIST: &str = "mcp:dashboard.list:call";
const TOOLS_CATALOG: &str = "mcp:tools.catalog:call";
const FEDERATION_QUERY: &str = "mcp:federation.query:call";
const QUERY_RUN: &str = "mcp:query.run:call";

async fn call(
    node: &Arc<Node>,
    p: &Principal,
    ws: &str,
    tool: &str,
    input: Value,
) -> Result<Value, ToolError> {
    let out = call_tool(node, p, ws, tool, &input.to_string()).await?;
    Ok(serde_json::from_str(&out).unwrap())
}

/// The `federation.query` declared `result` envelope — the exact shape
/// `federation/query.rs::query_result_render()` declares (the headline proof: pinning this needs
/// ZERO federation-specific code in the pin path — the envelope is opaque data). The host MINT
/// function (Slice B, unchanged by Slice C) treats `source.tool` as opaque data.
fn federation_query_envelope() -> Value {
    json!({
        "v": 2,
        "view": "table",
        "source": { "tool": "federation.query", "args": { "source": "warehouse", "sql": "SELECT 1" } },
        "tools": ["federation.query"]
    })
}

/// The `query.run` declared `result` envelope — the exact shape `query/descriptors.rs::run_result_render()`
/// declares. Carries `{id}` so a pinned cell re-runs the saved query by id (the "this query, live"
/// mental model — an edit to the saved query propagates to the dashboard).
fn query_run_envelope() -> Value {
    json!({
        "v": 2,
        "view": "table",
        "source": { "tool": "query.run", "args": { "id": "daily" } },
        "tools": ["query.run"]
    })
}

// --- catalog visibility: the menu IS the permission model ---

/// The NEW `result` envelopes reach `tools.catalog` for a caller GRANTED the tool's cap. A principal
/// WITH `mcp:federation.query:call` sees `federation.query` WITH its `result = table` envelope (and
/// `query.run` WITH its envelope when granted that cap).
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn catalog_serves_the_new_result_envelopes_to_a_granted_caller() {
    let ws = "rr-catalog-yes";
    let node = Arc::new(Node::boot().await.unwrap());

    // A PLAIN member holding the catalog read + the two tool caps — sees BOTH descriptors WITH their
    // NEW `result` envelopes (proves the envelope reaches the catalog, not just the code constant).
    let member = principal(
        "user:ada",
        ws,
        &[TOOLS_CATALOG, FEDERATION_QUERY, QUERY_RUN],
    );
    let out = call(&node, &member, ws, "tools.catalog", json!({}))
        .await
        .expect("plain member reads the catalog");
    let tools = out["tools"].as_array().expect("tools array");

    let fed = tools
        .iter()
        .find(|t| t["name"] == "federation.query")
        .expect("federation.query is in the catalog for a granted caller");
    let fed_render = &fed["result"];
    assert_eq!(fed_render["v"], 2, "federation.query result envelope");
    assert_eq!(fed_render["view"], "table");
    assert_eq!(fed_render["source"]["tool"], "federation.query");
    assert!(
        fed_render["tools"]
            .as_array()
            .map(|a| a.contains(&json!("federation.query")))
            .unwrap_or(false),
        "tools[] includes the read itself"
    );

    let run = tools
        .iter()
        .find(|t| t["name"] == "query.run")
        .expect("query.run is in the catalog for a granted caller");
    let run_render = &run["result"];
    assert_eq!(run_render["v"], 2, "query.run result envelope");
    assert_eq!(run_render["view"], "table");
    assert_eq!(run_render["source"]["tool"], "query.run");
}

/// The menu IS the permission model: a principal WITHOUT `mcp:federation.query:call` does NOT get
/// the `federation.query` descriptor (so no command in the palette, no render, NO ENVELOPE LEAK).
/// `tools.catalog`'s per-tool `authorize_tool` (catalog.rs:51) already drops a tool the caller can't
/// call — Slice C adds no new cap; the verb's own call gate decides the envelope's visibility too.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn catalog_hides_the_result_envelope_when_the_tool_cap_is_absent() {
    let ws = "rr-catalog-no";
    let node = Arc::new(Node::boot().await.unwrap());

    // Granted the catalog read but NEITHER tool cap → the descriptors are ABSENT (not greyed; no
    // existence leak, no envelope leak). The member could be admin or not — what matters is the
    // specific tool caps are absent.
    let member = principal("user:eve", ws, &[TOOLS_CATALOG, QUERY_RUN]);
    let out = call(&node, &member, ws, "tools.catalog", json!({}))
        .await
        .expect("catalog read OK with the catalog cap alone");
    let tools = out["tools"].as_array().expect("tools array");
    let names: Vec<&str> = tools.iter().filter_map(|t| t["name"].as_str()).collect();
    assert!(
        !names.contains(&"federation.query"),
        "federation.query is ABSENT without mcp:federation.query:call (no envelope leak)"
    );
    // query.run IS present (the member holds its cap) — its envelope is served.
    let run = tools
        .iter()
        .find(|t| t["name"] == "query.run")
        .expect("query.run IS present (the member holds its cap)");
    assert_eq!(run["result"]["view"], "table");
}

// --- the HEADLINE: pin federation.query's declared result, generic over the tool id ---

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn pin_federation_query_envelope_persists_and_reloads_intact() {
    let ws = "rr-headline";
    let node = Arc::new(Node::boot().await.unwrap());
    let ada = principal("user:ada", ws, &[PIN, GET, LIST, FEDERATION_QUERY]);

    // Pin `federation.query`'s declared `result` envelope — ZERO federation-specific code in the pin
    // path (the mint function treats the tool id as opaque data; the envelope is a normal
    // `x-lb-render`). Pin through the headless `POST /mcp/call` path; then RELOAD via
    // `dashboard.get` to prove the persisted cell survives intact (it was persisted host-side).
    let _pinned = call(
        &node,
        &ada,
        ws,
        "dashboard.pin",
        json!({ "dashboard": "ops", "title": "Ops", "envelope": federation_query_envelope(), "now": 10 }),
    )
    .await
    .expect("pin federation.query");

    // Reload the dashboard — the minted cell survives intact (it was persisted).
    let got = dashboard_get(&node.store, &ada, ws, "ops")
        .await
        .expect("get");
    assert_eq!(got.cells.len(), 1);
    let c = &got.cells[0];
    assert_eq!(c.i, "pin-federation-query");
    assert_eq!(c.view, "table");
    assert_eq!(c.widget_type, "table");
    assert_eq!(c.v, 3);
    // The envelope's `source.tool` + `source.args` ride onto the cell — the cell RE-RUNS
    // `federation.query` under the viewer's grant at render with the captured args.
    assert_eq!(c.source.tool, "federation.query");
    assert_eq!(c.source.args["source"], "warehouse");
    assert_eq!(c.source.args["sql"], "SELECT 1");
    // A pure read has NO row-control write verbs → no hidden extra `sources[]` (the `tools` fold
    // drops the source.tool, leaving nothing). The bridge leash covers just the read.
    assert!(
        c.sources.is_empty(),
        "a pure-read envelope has no extra tools to fold into sources[]"
    );
}

/// The mint is GENERIC over the tool id (rule 10) — already proven by Slice B for `__test__.*`; this
/// re-asserts it for a tabular envelope sourced at a tool that DOESN'T EXIST (no federation sidecar
/// involved — the mint happens entirely off the descriptor's envelope shape, never calling the tool).
/// The render-time call would fail honestly under the viewer's grant; the MINT+PERSIST is generic.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn pin_path_is_generic_over_an_arbitrary_tabular_tool_id() {
    let ws = "rr-generic";
    let node = Arc::new(Node::boot().await.unwrap());
    let ada = principal("user:ada", ws, &[PIN, GET]);

    let env = json!({
        "v": 2,
        "view": "table",
        "source": { "tool": "__test__.warehouse_read", "args": { "q": "shipments" } },
        "tools": ["__test__.warehouse_read"]
    });
    let d = call(
        &node,
        &ada,
        ws,
        "dashboard.pin",
        json!({ "dashboard": "d", "title": "D", "envelope": env, "now": 10 }),
    )
    .await
    .expect("arbitrary tabular tool id mints");
    assert_eq!(d["cells"][0]["i"], "pin-test-warehouse-read");
    assert_eq!(d["cells"][0]["source"]["tool"], "__test__.warehouse_read");
    assert_eq!(d["cells"][0]["source"]["args"]["q"], "shipments");
    assert_eq!(d["cells"][0]["view"], "table");
}

// --- workspace isolation (mandatory) ---

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn pin_in_ws_a_is_invisible_to_ws_b_for_federation_query() {
    let node = Arc::new(Node::boot().await.unwrap());
    let (wa, wb) = ("rr-iso-a", "rr-iso-b");

    // Ada in ws-A pins federation.query to a ws-A dashboard "ops".
    let ada = principal("user:ada", wa, &[PIN, GET, LIST, FEDERATION_QUERY]);
    call(
        &node,
        &ada,
        wa,
        "dashboard.pin",
        json!({ "dashboard": "ops", "title": "Ops", "envelope": federation_query_envelope(), "now": 10 }),
    )
    .await
    .expect("ada pins in ws-A");

    // Bob in ws-B cannot read ws-A's dashboard "ops" — the workspace wall (gate 1).
    let bob = principal("user:bob", wb, &[GET]);
    let err = dashboard_get(&node.store, &bob, wb, "ops")
        .await
        .expect_err("ws-B cannot read ws-A's dashboard");
    assert!(
        matches!(
            err,
            lb_host::DashboardError::Denied | lb_host::DashboardError::NotFound
        ),
        "ws-B sees neither ws-A's dashboard nor a 404-existence leak"
    );

    // Bob pins the same envelope in ws-B → a SEPARATE cell on a ws-B "ops" dashboard (a different
    // record in a different namespace). The two dashboards are independent; the cell `source` re-runs
    // `federation.query` at render under the viewer's grant, and `federation_query` resolves the
    // source alias in the VIEWER's workspace (federation/query.rs:42) — ws-B's source (or none),
    // never ws-A's.
    let bob = principal("user:bob", wb, &[PIN, GET, FEDERATION_QUERY]);
    let d = call(
        &node,
        &bob,
        wb,
        "dashboard.pin",
        json!({ "dashboard": "ops", "title": "Ops", "envelope": federation_query_envelope(), "now": 10 }),
    )
    .await
    .expect("bob pins in ws-B");
    assert_eq!(d["cells"][0]["i"], "pin-federation-query");
    assert_eq!(d["cells"][0]["source"]["tool"], "federation.query");
}

// --- shell path AND headless POST /mcp/call parity (Slice A/B pattern) ---

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn shell_path_and_headless_mcp_call_produce_the_same_federation_query_cell() {
    let ws = "rr-parity";
    let node = Arc::new(Node::boot().await.unwrap());

    // The shell path — direct `dashboard_pin` into dashboard "shell".
    let ada_shell = principal("user:ada", ws, &[PIN, GET, FEDERATION_QUERY]);
    dashboard_pin(
        &node.store,
        &ada_shell,
        ws,
        "shell",
        "Shell",
        &federation_query_envelope(),
        10,
    )
    .await
    .expect("shell path pin");

    // The headless path — the same call over `POST /mcp/call` (`call_tool` → `dashboard.pin`).
    call(
        &node,
        &ada_shell,
        ws,
        "dashboard.pin",
        json!({ "dashboard": "mcp", "title": "Mcp", "envelope": federation_query_envelope(), "now": 10 }),
    )
    .await
    .expect("headless path pin");

    let shell = dashboard_get(&node.store, &ada_shell, ws, "shell")
        .await
        .expect("shell get");
    let mcp = dashboard_get(&node.store, &ada_shell, ws, "mcp")
        .await
        .expect("mcp get");
    // The two paths produce the SAME cell shape (view/source/tools-fold/options/i).
    assert_eq!(shell.cells[0].i, mcp.cells[0].i);
    assert_eq!(shell.cells[0].view, mcp.cells[0].view);
    assert_eq!(shell.cells[0].source, mcp.cells[0].source);
    assert_eq!(shell.cells[0].sources, mcp.cells[0].sources);
}

// --- query.run envelope parity (the second tabular tool) ---

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn query_run_envelope_mints_a_table_cell_with_the_captured_id() {
    let ws = "rr-query-run";
    let node = Arc::new(Node::boot().await.unwrap());
    let ada = principal("user:ada", ws, &[PIN, GET, QUERY_RUN]);

    // Pin `query.run`'s declared `result` envelope. The `source.args = {id:"daily"}` is captured at
    // pin time → the pinned cell re-runs the SAVED query by id (so an edit to "daily" propagates to
    // the dashboard — "the daily query, live"). Same generic mint path as federation.query.
    let _pinned = call(
        &node,
        &ada,
        ws,
        "dashboard.pin",
        json!({ "dashboard": "ops", "title": "Ops", "envelope": query_run_envelope(), "now": 10 }),
    )
    .await
    .expect("pin query.run");

    let got = dashboard_get(&node.store, &ada, ws, "ops")
        .await
        .expect("get");
    let c = &got.cells[0];
    assert_eq!(c.i, "pin-query-run");
    assert_eq!(c.view, "table");
    assert_eq!(c.source.tool, "query.run");
    assert_eq!(c.source.args["id"], "daily");
}

// --- idempotent re-pin (Slice B proven; re-asserted for the federation.query envelope) ---

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn re_pin_federation_query_replaces_in_place_not_duplicates() {
    let ws = "rr-idem";
    let node = Arc::new(Node::boot().await.unwrap());
    let ada = principal("user:ada", ws, &[PIN, GET, FEDERATION_QUERY]);

    call(
        &node,
        &ada,
        ws,
        "dashboard.pin",
        json!({ "dashboard": "ops", "title": "Ops", "envelope": federation_query_envelope(), "now": 10 }),
    )
    .await
    .expect("first pin");

    // Direct shell path: re-pin the SAME envelope (same source.tool → same `i`). It should REPLACE
    // the cell, not append a duplicate.
    dashboard_pin(
        &node.store,
        &ada,
        ws,
        "ops",
        "Ops",
        &federation_query_envelope(),
        20,
    )
    .await
    .expect("re-pin (shell path)");

    let got = dashboard_get(&node.store, &ada, ws, "ops")
        .await
        .expect("get");
    assert_eq!(got.cells.len(), 1, "re-pin replaces, not duplicates");
    assert_eq!(got.cells[0].i, "pin-federation-query");
    assert_eq!(got.updated_ts, 20);
}
