//! S3 EXIT-GATE (part): a tool call on node A resolves to an extension hosted on node B and
//! routes over a Zenoh queryable — callers and `authorize` UNCHANGED, `caps::check` on the
//! CALLING node, workspace-first. Plus the mandatory categories across TWO nodes:
//!   - capability-deny: an ungranted call is refused on node A, the request never leaves it;
//!   - workspace-isolation across nodes: a node-A principal in workspace B can NEVER reach
//!     node B's tool acting in workspace A via the routing seam.
//!
//! The two nodes are two in-process Zenoh peers, but they are **explicitly linked over a TCP
//! endpoint** rather than left to ambient multicast scouting (`Bus::peer_with` / `Bus::locators`).
//! Why: under a full parallel `cargo test --workspace`, hundreds of in-process peers share one
//! multicast scout domain and gossip between a *specific* pair can stall past any test timeout —
//! the original flake (debugging/bus/routed-call-races-mesh-discovery.md). A point-to-point
//! endpoint makes this pair's discovery deterministic, independent of the scout noise, and is the
//! production-faithful posture (real edge↔hub links are configured endpoints). Each test still uses
//! a UNIQUE workspace id: in-process peers share a workspace's keyspace (debugging/bus/in-process-
//! peers-share-the-keyspace.md), so a shared id would let concurrent tests cross-talk. Multi-thread
//! flavor is required (boots a Zenoh peer; debugging/bus/zenoh-needs-multi-thread-runtime.md).

use std::time::Duration;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_bus::Bus;
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
        constraint: None,
        run_id: None,
    };
    let token = mint(&key, &claims);
    verify(&key, &token, 1).expect("token verifies")
}

/// Build a node on an explicit `bus` and `role`. Same wiring as `Node::boot_as`, but we own the
/// bus so the hub and edge can be **point-to-point linked** (see `edge_and_hub`). Mirrors the
/// direct-construction pattern in `ext_publish_test.rs` (a custom store handle there; a custom bus
/// here) — both legitimate, since `Node`'s fields are the booted spine and nothing here is mocked.
async fn node_on_bus(bus: Bus, role: NodeRole) -> Node {
    Node::boot_on_bus(bus, role)
        .await
        .expect("node boots on the given bus")
}

/// Stand up an edge (node A, the caller) and a hub (node B, hosting `hello` and serving it),
/// **explicitly linked over a loopback TCP endpoint** so discovery is deterministic regardless of
/// how many other in-process peers are scouting concurrently (the root-cause fix — see the module
/// doc and debugging/bus/routed-call-races-mesh-discovery.md). The hub listens on an OS-assigned
/// loopback port; the edge connects to that exact locator. Returns both nodes and the live tool
/// server (kept alive for the test's duration).
async fn edge_and_hub() -> (Node, Node, ToolServer) {
    // Pick a concrete free loopback port up front so the hub LISTENS on it and the edge CONNECTS to
    // exactly it — a deterministic point-to-point link. (We bind a throwaway socket to `:0`, read
    // the OS-assigned port, and drop it; Zenoh re-binds the same port on loopback. We choose the
    // port ourselves rather than read it back from Zenoh because `Session::info().locators()` is
    // behind zenoh's `unstable` feature, which we don't want to take on the whole workspace.)
    let port = {
        let probe = std::net::TcpListener::bind("127.0.0.1:0").expect("grab a free loopback port");
        probe.local_addr().expect("probe addr").port()
    };
    let endpoint = format!("tcp/127.0.0.1:{port}");

    let hub_bus = Bus::peer_with(&[endpoint.clone()], &[])
        .await
        .expect("hub bus listens on the chosen endpoint");
    let hub = node_on_bus(hub_bus, NodeRole::Hub).await;
    load_extension(&hub, MANIFEST, &hello_wasm(), &[])
        .await
        .expect("hub loads hello");
    let server = serve_ext(&hub.bus, hub.registry.clone(), "hello")
        .await
        .expect("hub serves hello");

    // Edge connects straight to the hub's endpoint — discovery is now deterministic, not multicast.
    let edge_bus = Bus::peer_with(&[], &[endpoint])
        .await
        .expect("edge bus connects to hub");
    let edge = node_on_bus(edge_bus, NodeRole::Edge).await;
    // The edge knows hello lives elsewhere — a routing entry, no local instance.
    register_remote_extension(&edge, "hello", &["echo".to_string()]);

    (edge, hub, server)
}

/// Poll the routed call until the hub's queryable is actually reachable, then return its output.
///
/// ROOT CAUSE of the old flake (debugging/bus/routed-call-races-mesh-discovery.md): the two
/// in-process Zenoh peers used to rely on ambient multicast scouting to find each other, but that
/// discovery is **asynchronous AND best-effort** — when the edge issues `query` (a Zenoh `get`)
/// before its peer has learned of the hub's queryable, the query reaches no responder and the reply
/// channel blocks until Zenoh's default ~10s query timeout; a late-joining queryable does not
/// retroactively answer an in-flight `get`. Under a full parallel `cargo test --workspace` (hundreds
/// of peers in one scout domain) that discovery could stall past *any* fixed timeout, so the old
/// single-call-in-a-5s-`timeout` test hit `Elapsed`. That was a real discovery race, not a tight
/// number — bumping the timeout did not fix it (verified: a 30s retry loop still failed in the
/// workspace storm because the two peers never discovered each other at all).
///
/// PRIMARY FIX (`edge_and_hub`): link the pair over an explicit loopback TCP endpoint, so discovery
/// is deterministic and independent of the scout noise — the link forms in milliseconds.
///
/// This barrier is the SECONDARY belt-and-suspenders: even with a deterministic link, the
/// queryable *declaration* still propagates to the edge a beat after the link forms. Retrying the
/// real call until the first `Ok` converges on actual reachability (nothing mocked) instead of
/// hoping a fixed sleep was long enough. With the TCP link it returns in well under a second.
async fn route_until_reachable(edge: &Node, p: &Principal, ws: &str, input_json: &str) -> String {
    // With the deterministic loopback link this converges in <1s; the deadline only guards against a
    // genuinely-broken routing path (then it fails loudly), not slow ambient mesh convergence. The
    // headroom (20s) is free — the loop returns the instant a call succeeds — and covers a CPU-
    // starved box where even the direct link + queryable propagation is briefly slow to schedule.
    let deadline = std::time::Instant::now() + Duration::from_secs(20);
    let mut last_err = None;
    while std::time::Instant::now() < deadline {
        // Bound each individual attempt so a `get` that blocks on the (not-yet-discovered)
        // queryable's full query timeout doesn't eat the whole budget — we retry instead.
        match tokio::time::timeout(
            Duration::from_millis(500),
            call(&edge.registry, &edge.bus, p, ws, "hello.echo", input_json),
        )
        .await
        {
            Ok(Ok(out)) => return out,
            Ok(Err(e)) => last_err = Some(format!("{e:?}")), // reachable but errored: surface it
            Err(_) => last_err = Some("attempt timed out (queryable not yet reachable)".into()),
        }
    }
    panic!("routed call never became reachable within the deadline; last: {last_err:?}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_call_on_the_edge_routes_to_the_extension_on_the_hub() {
    let ws = "xnode-routes";
    let (edge, _hub, _server) = edge_and_hub().await;
    let p = principal(ws, &["mcp:hello.echo:call"]);

    // The call site is IDENTICAL to a local call — the edge has no local hello, so dispatch routes
    // over the bus to the hub. We poll until the hub's queryable is reachable (the readiness
    // barrier above) rather than wrapping one call in a fixed timeout and hoping the mesh converged.
    let out = route_until_reachable(&edge, &p, ws, r#"{"msg":"routed"}"#).await;

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
