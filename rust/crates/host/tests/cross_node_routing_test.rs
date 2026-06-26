//! S3 EXIT-GATE (part): a tool call on node A resolves to an extension hosted on node B and
//! routes over a Zenoh queryable — callers and `authorize` UNCHANGED, `caps::check` on the
//! CALLING node, workspace-first. Plus the mandatory categories across TWO nodes:
//!   - capability-deny: an ungranted call is refused on node A, the request never leaves it;
//!   - workspace-isolation across nodes: a node-A principal in workspace B can NEVER reach
//!     node B's tool acting in workspace A via the routing seam.
//!
//! The two nodes are two in-process `Node::boot()`s — separate Zenoh sessions that auto-discover
//! into one network (that is the multi-node substrate). Each test uses a UNIQUE workspace id:
//! in-process peers share a workspace's keyspace (debugging/bus/in-process-peers-share-the-
//! keyspace.md), so a shared id would let concurrent tests cross-talk. Multi-thread flavor is
//! required (boots a Zenoh peer; debugging/bus/zenoh-needs-multi-thread-runtime.md).

use std::time::Duration;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{
    load_extension, register_remote_extension, serve_ext, Node, Role as NodeRole, ToolServer,
};
use lb_mcp::{call, ToolError};

const MANIFEST: &str = include_str!("../../../extensions/hello/extension.toml");

fn hello_wasm() -> Vec<u8> {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../extensions/hello/target/wasm32-wasip2/release/hello_ext.wasm");
    std::fs::read(&path).unwrap_or_else(|e| {
        panic!(
            "missing hello component at {} ({e}).\nBuild it first:\n  \
             (cd rust/extensions/hello && cargo build --target wasm32-wasip2 --release)",
            path.display()
        )
    })
}

fn principal(ws: &str, caps: &[&str]) -> Principal {
    let key = SigningKey::generate();
    let claims = Claims {
        sub: "user:test".into(),
        ws: ws.into(),
        role: Role::Member,
        caps: caps.iter().map(|s| s.to_string()).collect(),
        iat: 0,
        exp: u64::MAX,
    };
    let token = mint(&key, &claims);
    verify(&key, &token, 1).expect("token verifies")
}

/// Stand up an edge (node A, the caller) and a hub (node B, hosting `hello` and serving it).
/// Returns both nodes and the live tool server (kept alive for the test's duration).
async fn edge_and_hub() -> (Node, Node, ToolServer) {
    let hub = Node::boot_as(NodeRole::Hub).await.expect("hub boots");
    load_extension(&hub, MANIFEST, &hello_wasm(), &[])
        .await
        .expect("hub loads hello");
    let server = serve_ext(&hub.bus, hub.registry.clone(), "hello")
        .await
        .expect("hub serves hello");

    let edge = Node::boot_as(NodeRole::Edge).await.expect("edge boots");
    // The edge knows hello lives elsewhere — a routing entry, no local instance.
    register_remote_extension(&edge, "hello", &["echo".to_string()]);

    (edge, hub, server)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_call_on_the_edge_routes_to_the_extension_on_the_hub() {
    let ws = "xnode-routes";
    let (edge, _hub, _server) = edge_and_hub().await;
    let p = principal(ws, &["mcp:hello.echo:call"]);

    // The call site is IDENTICAL to a local call — the edge has no local hello, so dispatch
    // routes over the bus to the hub, which runs the tool and replies.
    let out = tokio::time::timeout(
        Duration::from_secs(5),
        call(
            &edge.registry,
            &edge.bus,
            &p,
            ws,
            "hello.echo",
            r#"{"msg":"routed"}"#,
        ),
    )
    .await
    .expect("a routed call returns in time")
    .expect("routed call succeeds");

    let value: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(
        value["echo"], "routed",
        "the hub's hello answered the edge's call"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_routed_call_is_denied_without_the_grant_and_never_leaves_the_edge() {
    // MANDATORY capability-deny across nodes: authorize runs on the EDGE first. Without the
    // grant the call is refused there — it never routes to the hub at all.
    let ws = "xnode-deny";
    let (edge, _hub, _server) = edge_and_hub().await;
    let p = principal(ws, &[]); // no caps

    let err = call(
        &edge.registry,
        &edge.bus,
        &p,
        ws,
        "hello.echo",
        r#"{"msg":"x"}"#,
    )
    .await
    .expect_err("ungranted routed call is refused on the calling node");
    assert_eq!(err, ToolError::Denied, "denied on the edge, before routing");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_principal_in_ws_b_cannot_route_into_ws_a() {
    // MANDATORY workspace-isolation across nodes via the routing seam. The principal is scoped
    // to ws_b but TRIES to call targeting ws_a. Gate 1 (workspace isolation) fires on the edge
    // — principal.ws (ws_b) != request.ws (ws_a) — so the call is denied BEFORE any bus hop.
    // Even if it somehow emitted a request, it would land on `ws/ws_b/...` (its own ws), which
    // the hub answers only for ws_b — never ws_a's data. Belt and suspenders.
    let ws_a = "xnode-iso-a";
    let ws_b = "xnode-iso-b";
    let (edge, _hub, _server) = edge_and_hub().await;
    let intruder = principal(ws_b, &["mcp:hello.echo:call"]);

    let err = call(
        &edge.registry,
        &edge.bus,
        &intruder,
        ws_a, // targeting workspace A while scoped to B
        "hello.echo",
        r#"{"msg":"cross"}"#,
    )
    .await
    .expect_err("cross-workspace routed call is refused");
    assert_eq!(err, ToolError::Denied, "isolation gate fires on the edge");
}
