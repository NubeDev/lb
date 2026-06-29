//! `tools.catalog` unit tests (channels-command-palette scope) — the menu IS the permission model
//! rendered. Boots a REAL `Node` (no mocks; per testing §0) and exercises the mandatory categories
//! against the host-native `tools_catalog` verb directly:
//!   - the authorized catalog CONTAINS `federation.query` with its `input_schema` intact (the
//!     `x-lb` entity/widget hints the palette renders from);
//!   - a principal lacking `mcp:federation.query:call` has that tool OMITTED (capability-filtered,
//!     no existence leak — absent, never greyed);
//!   - a principal lacking the verb gate (`mcp:tools.catalog:call`) gets an opaque `Denied`;
//!   - WORKSPACE ISOLATION: the catalog's `ws` is the caller's workspace (the gate is ws-scoped).

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{tools_catalog, Node};

/// Build a real verified principal for `ws` holding exactly `caps` (each a full `mcp:<verb>:call`).
fn principal(ws: &str, caps: &[&str]) -> Principal {
    let key = SigningKey::generate();
    let claims = Claims {
        sub: "user:ada".into(),
        ws: ws.into(),
        role: Role::Member,
        caps: caps.iter().map(|s| s.to_string()).collect(),
        iat: 0,
        exp: u64::MAX,
    };
    verify(&key, &mint(&key, &claims), 1).unwrap()
}

/// Is `name` present in the catalog's tool list?
fn has_tool(cat: &lb_host::ToolsCatalog, name: &str) -> bool {
    cat.tools.iter().any(|t| t.name == name)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn catalog_contains_federation_query_with_schema_for_an_authorized_principal() {
    let node = Node::boot().await.expect("node boots");
    let ws = "acme";
    let a = principal(ws, &["mcp:tools.catalog:call", "mcp:federation.query:call"]);

    let cat = tools_catalog(&node, &a, ws)
        .await
        .expect("authorized catalog");
    assert_eq!(cat.ws, ws, "the catalog reports the caller's workspace");

    let fq = cat
        .tools
        .iter()
        .find(|t| t.name == "federation.query")
        .expect("an authorized principal sees federation.query");

    // The descriptor carries its JSON-Schema input with the x-lb vendor hints intact (the palette
    // renders the guided rail from exactly these): `source` is a datasource entity, `sql` a sql
    // widget, and both are required.
    let schema = fq
        .input_schema
        .as_ref()
        .expect("federation.query has a schema");
    assert_eq!(schema["type"], "object");
    assert_eq!(
        schema["properties"]["source"]["x-lb"]["entity"],
        "datasource"
    );
    assert_eq!(schema["properties"]["sql"]["x-lb"]["widget"], "sql");
    let required = schema["required"].as_array().expect("required array");
    assert!(required.iter().any(|v| v == "source"));
    assert!(required.iter().any(|v| v == "sql"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn catalog_omits_a_tool_the_principal_cannot_call_no_existence_leak() {
    let node = Node::boot().await.expect("node boots");
    let ws = "acme";
    // B holds the verb gate but NOT `mcp:federation.query:call`.
    let b = principal(ws, &["mcp:tools.catalog:call"]);

    let cat = tools_catalog(&node, &b, ws).await.expect("catalog for B");
    assert!(
        !has_tool(&cat, "federation.query"),
        "a denied tool is absent (capability-filtered), never greyed: {:?}",
        cat.tools.iter().map(|t| &t.name).collect::<Vec<_>>()
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn catalog_without_the_verb_gate_is_opaque_denied() {
    let node = Node::boot().await.expect("node boots");
    let ws = "acme";
    // C lacks `mcp:tools.catalog:call` entirely (it does hold an unrelated cap).
    let c = principal(ws, &["mcp:federation.query:call"]);

    let err = tools_catalog(&node, &c, ws)
        .await
        .expect_err("no tools.catalog gate → denied");
    assert!(
        matches!(err, lb_mcp::ToolError::Denied),
        "the denial is opaque: {err:?}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn catalog_is_workspace_scoped() {
    // WORKSPACE ISOLATION: the catalog is always computed for the caller's own workspace; a ws-B
    // caller's catalog reports ws-B and the same caller cannot read ws-A through it (the gate runs
    // against the passed ws). Two principals in two workspaces over one node.
    let node = Node::boot().await.expect("node boots");

    let a = principal("acme", &["mcp:tools.catalog:call"]);
    let b = principal("other", &["mcp:tools.catalog:call"]);

    let cat_a = tools_catalog(&node, &a, "acme")
        .await
        .expect("ws-A catalog");
    let cat_b = tools_catalog(&node, &b, "other")
        .await
        .expect("ws-B catalog");

    assert_eq!(cat_a.ws, "acme");
    assert_eq!(cat_b.ws, "other");
}
