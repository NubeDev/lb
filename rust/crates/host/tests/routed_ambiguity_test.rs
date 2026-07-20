//! routed-node-dispatch (#81) — the hazard this scope exists to kill, and the guard that kills it.
//!
//! THE HAZARD: `mcp/{ext}/call` addresses an EXTENSION, not a node. Every node hosting `{ext}`
//! declares a queryable on that same key (`host/src/serve.rs`), and `lb_bus::query` takes the
//! FIRST reply ("a routed tool call has exactly one responder"). So with two nodes hosting one
//! ext, BOTH answer and the caller silently keeps whichever won the race — no error, no signal,
//! nondeterministic per call. A supervisor can provision the wrong physical box and get a
//! success reply.
//!
//! `nondeterminism_is_real_the_hazard` PROVES that, rather than arguing it: N sequential
//! untargeted calls against two live hosts do not all land on the same node. It is written to
//! FAIL (or flap) on the pre-#81 code — a regression test that cannot fail on the old code
//! proves nothing (scope: "Nondeterminism, proven not argued").
//!
//! SCOPE HONESTY (scope → Open questions → Finding A): neither `serve_ext` nor
//! `register_remote_extension` has any PRODUCTION caller today, so this two-host wiring is
//! reachable only from a test. That makes the defect **latent, not active** — real in the
//! library seam, not a live production misprovisioning. This file demonstrates the seam defect;
//! it is not evidence of a shipped bug, and must not be cited as one.
//!
//! Everything here is real: two real `Node`s, two real in-process Zenoh peers explicitly linked
//! over loopback TCP, real queryables, real dispatch (testing-scope §0 — no mocks, no fake
//! transport). The tool body is a `#[cfg(test)]` fixture that reports WHICH node ran it, which is
//! the only way to observe who answered — `hello.echo` returns the input verbatim and so cannot
//! identify its responder. A fixture tool is not a fake backend: the registry, the bus, the
//! queryable and the dispatch path are all the production ones.
//!
//! Discovery posture is inherited from `cross_node_routing_test.rs`: an explicit point-to-point
//! loopback endpoint instead of ambient multicast scouting, because under a parallel
//! `cargo test --workspace` gossip between a specific pair can stall past any timeout
//! (debugging/bus/routed-call-races-mesh-discovery.md). Unique workspace id per test, because
//! in-process peers share a workspace keyspace (debugging/bus/in-process-peers-share-the-keyspace.md).
//! Multi-thread flavor is required (boots a Zenoh peer).

use std::sync::Arc;
use std::time::Duration;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_bus::{Bus, NodeId};
use lb_host::{
    forget_remote_extension, register_remote_extension, serve_ext, Node, Role as NodeRole,
    ToolServer,
};
use lb_mcp::{call, call_on_node, ToolDescriptor, ToolError};
use lb_runtime::{CallContext, LocalDispatch, RuntimeError};
use tokio::sync::Mutex;

/// A real local dispatch target that answers `whoami` with the id of the node hosting it.
///
/// This is the observability the hazard proof needs: it makes "which node answered?" a fact in
/// the reply instead of an inference. It implements the SAME `LocalDispatch` trait a wasm
/// instance and a native sidecar implement, and is reached through the production registry and
/// the production `serve_call` — nothing about the transport or the routing is simulated.
struct WhoAmI {
    node: String,
}

#[async_trait::async_trait]
impl LocalDispatch for WhoAmI {
    async fn call_tool(
        &mut self,
        _ws: &str,
        tool: &str,
        _input_json: &str,
        _ctx: Option<CallContext>,
    ) -> Result<String, RuntimeError> {
        match tool {
            "whoami" => Ok(format!(r#"{{"node":"{}"}}"#, self.node)),
            other => Err(RuntimeError::Tool(format!("unknown tool: {other}"))),
        }
    }
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

/// Register the `whoami` fixture as a LOCAL ext on `node`, labelled with `label` so its replies
/// identify it. Goes through the production Tier-agnostic entry (`register_local_dispatch`) —
/// the same one a native sidecar adapter uses.
fn host_whoami(node: &Node, ext: &str, label: &str) {
    node.registry.register_local_dispatch(
        ext,
        vec![ToolDescriptor::name_only("whoami")],
        Arc::new(Mutex::new(WhoAmI {
            node: label.to_string(),
        })),
    );
}

/// Two hubs, both hosting AND serving the same ext `fleet`, plus a caller edge that knows the ext
/// is remote. This is the fleet shape: two distinct physical boxes running one extension.
///
/// Returns (caller, label_a, label_b, servers) — the servers are kept alive for the test.
async fn two_hosts_one_ext(
    ext: &str,
    workspaces: &[&str],
) -> (Node, NodeId, NodeId, Vec<ToolServer>, Node, Node) {
    let (label_a, label_b) = ("node-a", "node-b");
    // Node ids are namespaced by the caller's (unique-per-test) ext id, for the SAME reason each
    // test uses a unique workspace: in-process Zenoh peers share a keyspace
    // (debugging/bus/in-process-peers-share-the-keyspace.md), so concurrently-running tests would
    // otherwise both declare `…/node:gw-01/call` and answer each other's queries. That is a real
    // duplicate-node-id collision, and the new `MultipleResponders` check correctly fires on it —
    // see debugging/mcp/duplicate-node-ids-across-concurrent-tests.md.
    let (id_a, id_b) = (
        NodeId::new(format!("node:{ext}-gw-01")).expect("key-safe id"),
        NodeId::new(format!("node:{ext}-gw-02")).expect("key-safe id"),
    );

    // One shared loopback endpoint both hubs listen on is not possible (one port, one listener),
    // so each hub listens on its own and the caller connects to BOTH — a deterministic star.
    let port_a = free_port();
    let port_b = free_port();
    let ep_a = format!("tcp/127.0.0.1:{port_a}");
    let ep_b = format!("tcp/127.0.0.1:{port_b}");

    let bus_a = Bus::peer_with(&[ep_a.clone()], &[])
        .await
        .expect("hub A listens");
    let hub_a = Node::boot_on_bus(bus_a, NodeRole::Hub)
        .await
        .expect("hub A boots");
    hub_a.install_node_id(id_a.clone());
    host_whoami(&hub_a, ext, label_a);
    let server_a = serve_ext(&hub_a.bus, hub_a.registry.clone(), ext, &id_a, workspaces)
        .await
        .expect("hub A serves");

    let bus_b = Bus::peer_with(&[ep_b.clone()], &[])
        .await
        .expect("hub B listens");
    let hub_b = Node::boot_on_bus(bus_b, NodeRole::Hub)
        .await
        .expect("hub B boots");
    hub_b.install_node_id(id_b.clone());
    host_whoami(&hub_b, ext, label_b);
    let server_b = serve_ext(&hub_b.bus, hub_b.registry.clone(), ext, &id_b, workspaces)
        .await
        .expect("hub B serves");

    // The caller connects to both hubs — so both queryables are reachable from it, which is
    // exactly the fan-in the hazard needs.
    let caller_bus = Bus::peer_with(&[], &[ep_a, ep_b])
        .await
        .expect("caller connects to both hubs");
    let caller = Node::boot_on_bus(caller_bus, NodeRole::Edge)
        .await
        .expect("caller boots");
    // The caller knows the ext lives on BOTH hubs — two routing entries, no local instance. This
    // is the fleet the ambiguity guard exists for: two candidates for one ext id. (Without any
    // entry, resolve fails locally with `NotFound` and no call reaches the bus.)
    register_remote_extension(&caller, ext, id_a.clone(), &["whoami".to_string()]);
    register_remote_extension(&caller, ext, id_b.clone(), &["whoami".to_string()]);

    // The hubs are RETURNED (not leaked) so each test owns them and they shut down when it ends.
    //
    // This used to `Box::leak` them, on the reasoning that dropping a hub mid-test would retract
    // its queryable. True — but returning them keeps them alive just as well, and leaking made
    // every hub outlive its test: with 7 tests × 3 peers, ~21 Zenoh peers stayed live and
    // gossiping for the whole binary. That exhausted the substrate near the end of a run and
    // produced BOTH failure modes seen here — a reachability timeout in
    // `a_targeted_call_lands_on_the_named_node`, and a hang in whichever test ran last
    // (alphabetically `untargeted_call_…`) — reproducibly, and *even under
    // `--test-threads=1`*, which is what ruled out cross-test port/id collisions and pointed at
    // accumulated peers instead. See debugging/mcp/leaked-zenoh-peers-exhaust-the-test-binary.md.
    (caller, id_a, id_b, vec![server_a, server_b], hub_a, hub_b)
}

/// A loopback port no other test in this binary will be handed.
///
/// **Why not just bind `:0` and read the port back.** That is a TOCTOU race: the probe socket is
/// dropped before Zenoh binds, so the OS is free to hand the same ephemeral port to a *concurrently
/// running* test in the moment between. When it does, two hubs from different tests fight over one
/// port — one loses its listener, its queryable never becomes reachable, and the victim fails with
/// "queryable not yet reachable" after burning its full retry deadline. That reproduced here as a
/// ~1-in-2 failure of `a_targeted_call_lands_on_the_named_node` once a third port-consuming test
/// joined the file, and it is the *same class* as the duplicate-node-id flake recorded in
/// debugging/mcp/duplicate-node-ids-across-concurrent-tests.md: a per-test resource that is only
/// unique by luck.
///
/// Instead each caller takes a **disjoint slice of the port space** via a process-wide counter, so
/// no two tests can ever be handed the same number — no probe socket, no window, no luck involved.
/// The base is high enough to sit above the usual ephemeral range, and the probe below only skips
/// a port that is genuinely occupied (by something outside this binary).
fn free_port() -> u16 {
    use std::sync::atomic::{AtomicU16, Ordering};
    static NEXT: AtomicU16 = AtomicU16::new(41_000);
    loop {
        let port = NEXT.fetch_add(1, Ordering::Relaxed);
        assert!(port < 60_000, "ran out of test ports");
        // Bind-and-drop is fine HERE because the counter — not the OS — guarantees uniqueness
        // within this binary; this only skips a port some *other* process already holds.
        if std::net::TcpListener::bind(("127.0.0.1", port)).is_ok() {
            return port;
        }
    }
}

/// Poll a TARGETED routed call until the addressed node's queryable is reachable, returning the
/// label of whoever answered. Mirrors `cross_node_routing_test::route_until_reachable`'s rationale:
/// a `get` issued before the queryable has propagated finds no responder, so we retry the REAL call
/// until it converges (nothing mocked, no fixed sleep).
///
/// Note it retries `NodeUnreachable` — during convergence that is genuinely indistinguishable from
/// "not yet propagated", which is the honest reading of the zero-queryable signal.
async fn ask_node(caller: &Node, p: &Principal, ws: &str, ext: &str, node: &NodeId) -> String {
    let tool = format!("{ext}.whoami");
    let deadline = std::time::Instant::now() + Duration::from_secs(20);
    let mut last_err = None;
    while std::time::Instant::now() < deadline {
        match tokio::time::timeout(
            Duration::from_millis(500),
            call_on_node(&caller.registry, &caller.bus, p, ws, &tool, "{}", node),
        )
        .await
        {
            Ok(Ok(out)) => {
                let v: serde_json::Value = serde_json::from_str(&out).expect("whoami json");
                return v["node"].as_str().expect("node label").to_string();
            }
            Ok(Err(e)) => last_err = Some(format!("{e:?}")),
            Err(_) => last_err = Some("attempt timed out (queryable not yet reachable)".into()),
        }
    }
    panic!("targeted call to {node} never became reachable; last: {last_err:?}");
}

// ─────────────────────────────────────────────────────────────────────────────────────────────
// THE GUARD
// ─────────────────────────────────────────────────────────────────────────────────────────────

/// THE REGRESSION THIS SCOPE EXISTS FOR. Two nodes host one ext; an UNTARGETED call is refused
/// with a structured `Ambiguous` naming both candidates — instead of the pre-#81 coin flip that
/// silently answered from whichever node replied first.
///
/// Before this change the same wiring produced a *successful* call with a nondeterministic
/// responder (measured: a 25/15 split over 40 identical calls across the two hubs — recorded in
/// the session doc). So this assertion genuinely could not have passed on the old code.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn untargeted_call_to_a_multiply_hosted_ext_is_ambiguous() {
    let ws = "ambig-guard";
    let ext = "fleet-guard";
    let (caller, id_a, id_b, _servers, _hub_a, _hub_b) = two_hosts_one_ext(ext, &[ws]).await;
    let p = principal(ws, &[&format!("mcp:{ext}.whoami:call")]);

    let err = call(
        &caller.registry,
        &caller.bus,
        &p,
        ws,
        &format!("{ext}.whoami"),
        "{}",
    )
    .await
    .expect_err("an untargeted call to a two-host ext must be refused, not coin-flipped");

    match err {
        ToolError::Ambiguous { ext: e, candidates } => {
            assert_eq!(e, ext);
            // Sorted and complete: the caller can act on this without parsing prose.
            assert_eq!(
                candidates,
                vec![id_a.to_string(), id_b.to_string()],
                "both hosts named, deterministically ordered"
            );
        }
        other => panic!("expected Ambiguous, got {other:?}"),
    }
}

/// The other half: naming a node resolves it unambiguously, and the call lands on THAT box. This
/// is what a supervisor needs — and what the pre-#81 code could not express at all.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_targeted_call_lands_on_the_named_node() {
    let ws = "ambig-target";
    let ext = "fleet-target";
    let (caller, id_a, id_b, _servers, _hub_a, _hub_b) = two_hosts_one_ext(ext, &[ws]).await;
    let p = principal(ws, &[&format!("mcp:{ext}.whoami:call")]);

    // Each target is asked repeatedly: one correct answer could be luck, ten cannot.
    for _ in 0..10 {
        assert_eq!(
            ask_node(&caller, &p, ws, ext, &id_a).await,
            "node-a",
            "a call targeted at {id_a} must always run on that node"
        );
        assert_eq!(
            ask_node(&caller, &p, ws, ext, &id_b).await,
            "node-b",
            "a call targeted at {id_b} must always run on that node"
        );
    }
}

/// A disconnected/unknown target is a REFUSAL — never a queue, and never a fallback to the other
/// node that also hosts the ext. The fallback IS the misprovisioning bug this scope removes.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_unknown_target_is_refused_and_never_falls_back() {
    let ws = "ambig-unreachable";
    let ext = "fleet-unreach";
    let (caller, _id_a, _id_b, _servers, _hub_a, _hub_b) = two_hosts_one_ext(ext, &[ws]).await;
    let p = principal(ws, &[&format!("mcp:{ext}.whoami:call")]);
    let ghost = NodeId::new("node:gw-99-never-existed").unwrap();

    let err = call_on_node(
        &caller.registry,
        &caller.bus,
        &p,
        ws,
        &format!("{ext}.whoami"),
        "{}",
        &ghost,
    )
    .await
    .expect_err("targeting a node that does not host this ext must be refused");

    match err {
        ToolError::NodeUnreachable { node } => assert_eq!(node, ghost.to_string()),
        // The critical negative: anything that SUCCEEDED would mean the call silently ran on some
        // other node — exactly the bug.
        other => panic!("expected NodeUnreachable, got {other:?}"),
    }
}

// ─────────────────────────────────────────────────────────────────────────────────────────────
// MANDATORY: capability deny (testing-scope) — and the sharp version the scope calls for
// ─────────────────────────────────────────────────────────────────────────────────────────────

/// MANDATORY capability-deny, plus the SHARP property: the deny is byte-identical whether or not
/// the named node exists, so a capless caller cannot use targeting to enumerate a fleet.
///
/// This is why `authorize` strictly precedes `resolve` (`call/mod.rs`): resolve is what knows
/// whether a node is real, and an unauthorized caller must never reach it.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_capless_targeted_call_is_denied_indistinguishably_for_real_and_fake_nodes() {
    let ws = "ambig-deny";
    let ext = "fleet-deny";
    let (caller, id_a, _id_b, _servers, _hub_a, _hub_b) = two_hosts_one_ext(ext, &[ws]).await;
    let capless = principal(ws, &[]); // no caps at all
    let ghost = NodeId::new("node:gw-99-never-existed").unwrap();

    let real = call_on_node(
        &caller.registry,
        &caller.bus,
        &capless,
        ws,
        &format!("{ext}.whoami"),
        "{}",
        &id_a, // a node that genuinely hosts this ext
    )
    .await
    .expect_err("capless call is denied");

    let fake = call_on_node(
        &caller.registry,
        &caller.bus,
        &capless,
        ws,
        &format!("{ext}.whoami"),
        "{}",
        &ghost, // a node that does not exist
    )
    .await
    .expect_err("capless call is denied");

    assert_eq!(real, ToolError::Denied, "denied before any node lookup");
    assert_eq!(fake, ToolError::Denied, "denied before any node lookup");
    // The oracle test: identical error AND identical rendering. If a real node produced `Denied`
    // and a fake one produced `NodeUnreachable`, an unauthorized caller could probe the fleet's
    // shape one guess at a time.
    assert_eq!(
        real.to_string(),
        fake.to_string(),
        "the deny must not reveal whether the named node exists"
    );
}

/// Ordering, pinned: a capless caller on an AMBIGUOUS ext gets `Denied`, never `Ambiguous`.
/// `Ambiguous` names the fleet's nodes, so reaching it without a capability would leak exactly
/// what the deny path is meant to hide.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn authorize_precedes_resolve_so_ambiguity_never_leaks_to_the_unauthorized() {
    let ws = "ambig-order";
    let ext = "fleet-order";
    let (caller, _id_a, _id_b, _servers, _hub_a, _hub_b) = two_hosts_one_ext(ext, &[ws]).await;
    let capless = principal(ws, &[]);

    let err = call(
        &caller.registry,
        &caller.bus,
        &capless,
        ws,
        &format!("{ext}.whoami"),
        "{}",
    )
    .await
    .expect_err("capless untargeted call on an ambiguous ext");

    assert_eq!(
        err,
        ToolError::Denied,
        "must be Denied — an unauthorized caller must never learn the fleet's shape"
    );
}

// ─────────────────────────────────────────────────────────────────────────────────────────────
// MANDATORY: workspace isolation
// ─────────────────────────────────────────────────────────────────────────────────────────────

/// MANDATORY workspace-isolation on the routed path, asserted in its STRONG form because the
/// node-qualified key is declared PER WORKSPACE (scope, open question 6 = per-workspace
/// declaration, not the `ws/*` wildcard).
///
/// The hubs serve ws-A only. A ws-B principal targeting one of them is refused with
/// `NodeUnreachable` — not merely "refused downstream" but genuinely unreachable: the hubs
/// declared nothing matching `ws/{ws_b}/…`, so there is no queryable for the call to land on. That
/// is the key-space wall, and it is the assertion the weaker wildcard design could NOT support.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_ws_b_caller_cannot_reach_a_node_that_serves_only_ws_a() {
    let ws_a = "ambig-iso-a";
    let ws_b = "ambig-iso-b";
    let ext = "fleet-iso";
    // The hubs declare their node key for ws_a ONLY.
    let (caller, id_a, _id_b, _servers, _hub_a, _hub_b) = two_hosts_one_ext(ext, &[ws_a]).await;

    // A principal legitimately scoped to ws_b, with a real capability IN ws_b — so this is not a
    // capability deny, it is the workspace wall.
    let intruder = principal(ws_b, &[&format!("mcp:{ext}.whoami:call")]);

    let err = call_on_node(
        &caller.registry,
        &caller.bus,
        &intruder,
        ws_b,
        &format!("{ext}.whoami"),
        "{}",
        &id_a,
    )
    .await
    .expect_err("a ws-B call must not execute on a node serving only ws-A");

    match err {
        // Unreachable, because the ws-A hub declared no ws-B key — the wall is the key space.
        ToolError::NodeUnreachable { node } => assert_eq!(node, id_a.to_string()),
        other => panic!("expected NodeUnreachable (the ws-B key has no responder), got {other:?}"),
    }
}

// ─────────────────────────────────────────────────────────────────────────────────────────────
// The unchanged fast path
// ─────────────────────────────────────────────────────────────────────────────────────────────

/// The overwhelmingly common case must be untouched: ONE host, no target named, resolves and runs
/// exactly as before. This is the guard against making the fleet feature cost everyone else
/// something (scope: "Unaddressed calls keep working, unchanged").
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_single_host_untargeted_call_still_just_works() {
    let ws = "ambig-single";
    let ext = "solo";
    let port = free_port();
    let ep = format!("tcp/127.0.0.1:{port}");
    let id = NodeId::new("node:only-one").unwrap();

    let hub_bus = Bus::peer_with(&[ep.clone()], &[]).await.unwrap();
    let hub = Node::boot_on_bus(hub_bus, NodeRole::Hub).await.unwrap();
    hub.install_node_id(id.clone());
    host_whoami(&hub, ext, "the-only-node");
    let _server = serve_ext(&hub.bus, hub.registry.clone(), ext, &id, &[ws])
        .await
        .unwrap();

    let caller_bus = Bus::peer_with(&[], &[ep]).await.unwrap();
    let caller = Node::boot_on_bus(caller_bus, NodeRole::Edge).await.unwrap();
    register_remote_extension(&caller, ext, id.clone(), &["whoami".to_string()]);
    Box::leak(Box::new(hub));

    let p = principal(ws, &[&format!("mcp:{ext}.whoami:call")]);
    // No target named, and none needed — one host is unambiguous.
    let deadline = std::time::Instant::now() + Duration::from_secs(20);
    loop {
        match tokio::time::timeout(
            Duration::from_millis(500),
            call(
                &caller.registry,
                &caller.bus,
                &p,
                ws,
                &format!("{ext}.whoami"),
                "{}",
            ),
        )
        .await
        {
            Ok(Ok(out)) => {
                let v: serde_json::Value = serde_json::from_str(&out).unwrap();
                assert_eq!(v["node"], "the-only-node");
                return;
            }
            _ if std::time::Instant::now() < deadline => continue,
            other => panic!("single-host untargeted call must still work; got {other:?}"),
        }
    }
}

/// Self-targeting resolves LOCAL with no bus hop: a node addressing itself runs in-process. Proven
/// by giving the caller a local instance and NO bus link to anything — if this took the bus it
/// could not possibly succeed.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_node_targeting_itself_runs_locally_with_no_bus_hop() {
    let ws = "ambig-self";
    let ext = "selfhost";
    let node = Node::boot().await.unwrap();
    host_whoami(&node, ext, "me");
    let p = principal(ws, &[&format!("mcp:{ext}.whoami:call")]);

    // Untargeted: the local host wins outright, no bus involved.
    let out = call(
        &node.registry,
        &node.bus,
        &p,
        ws,
        &format!("{ext}.whoami"),
        "{}",
    )
    .await
    .expect("a locally-hosted ext resolves local");
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["node"], "me");
}

/// A LOCAL host wins an untargeted call even when remote hosts also exist — it is unambiguously
/// *here*, needs no hop, and is what an untargeted call already did before #81. Refusing this as
/// ambiguous would break every single-node caller the moment a fleet peer appeared.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_local_host_wins_an_untargeted_call_over_remote_peers() {
    let ws = "ambig-localwins";
    let ext = "mixed";
    let node = Node::boot().await.unwrap();
    host_whoami(&node, ext, "local");
    // Two remote peers ALSO host it — without the local-wins rule this would be `Ambiguous`.
    register_remote_extension(
        &node,
        ext,
        NodeId::new("node:peer-1").unwrap(),
        &["whoami".to_string()],
    );
    register_remote_extension(
        &node,
        ext,
        NodeId::new("node:peer-2").unwrap(),
        &["whoami".to_string()],
    );

    let p = principal(ws, &[&format!("mcp:{ext}.whoami:call")]);
    let out = call(
        &node.registry,
        &node.bus,
        &p,
        ws,
        &format!("{ext}.whoami"),
        "{}",
    )
    .await
    .expect("a local host resolves unambiguously despite remote peers");
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["node"], "local");
}

/// A fleet that SHRINKS back to one host stops refusing untargeted calls — no restart needed. This
/// is what makes the guard usable with live discovery: hosting announcements come and go, and the
/// candidate set must follow them down as well as up.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn forgetting_a_host_makes_an_ambiguous_ext_callable_again() {
    let ws = "ambig-shrink";
    let ext = "shrinking";
    let node = Node::boot().await.unwrap();
    let p = principal(ws, &[&format!("mcp:{ext}.whoami:call")]);
    let (peer1, peer2) = (
        NodeId::new("node:peer-1").unwrap(),
        NodeId::new("node:peer-2").unwrap(),
    );
    register_remote_extension(&node, ext, peer1.clone(), &["whoami".to_string()]);
    register_remote_extension(&node, ext, peer2.clone(), &["whoami".to_string()]);

    // Two hosts → ambiguous.
    let err = call(
        &node.registry,
        &node.bus,
        &p,
        ws,
        &format!("{ext}.whoami"),
        "{}",
    )
    .await
    .expect_err("two hosts is ambiguous");
    assert!(matches!(err, ToolError::Ambiguous { .. }));

    // One drops out → unambiguous again. (It then fails to REACH peer-1, since no such node is on
    // this bus — but the important part is that it resolved to a single target instead of refusing.)
    forget_remote_extension(&node, ext, &peer2);
    let err = call(
        &node.registry,
        &node.bus,
        &p,
        ws,
        &format!("{ext}.whoami"),
        "{}",
    )
    .await
    .expect_err("peer-1 is not actually on this bus");
    assert!(
        matches!(err, ToolError::NodeUnreachable { .. }),
        "must resolve to the ONE remaining host (then fail to reach it), not stay Ambiguous; got {err:?}"
    );
}

// ─────────────────────────────────────────────────────────────────────────────────────────────
// MANDATORY: hot-reload
// ─────────────────────────────────────────────────────────────────────────────────────────────

/// MANDATORY hot-reload: swapping the instance behind a targeted node keeps that node addressable,
/// and in-flight targeting recovers — the new instance answers on the SAME node id.
///
/// This is the routed-path half of the stateless-extension guarantee (§3.4): a reload swaps the
/// instance inside the registry the serving loop already holds an `Arc` to, so the node's declared
/// queryables are untouched by the swap. The test proves the *observable* consequence — the
/// caller's target stays valid across a reload and starts seeing the new instance's answers —
/// rather than asserting on the declaration internals.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_reload_keeps_the_node_addressable_and_targeting_recovers() {
    let ws = "ambig-reload";
    let ext = "fleet-reload";
    let (caller, id_a, id_b, _servers, hub_a, _hub_b) = two_hosts_one_ext(ext, &[ws]).await;
    let p = principal(ws, &[&format!("mcp:{ext}.whoami:call")]);

    // Converge, and confirm the pre-reload identity.
    assert_eq!(ask_node(&caller, &p, ws, ext, &id_a).await, "node-a");

    // Reload hub A's instance in place — same ext id, same node id, NEW instance. The serving loop
    // holds the same `Arc<Registry>`, so it dispatches to whatever is registered now.
    hub_a.registry.register_local_dispatch(
        ext,
        vec![ToolDescriptor::name_only("whoami")],
        Arc::new(Mutex::new(WhoAmI {
            node: "node-a-reloaded".to_string(),
        })),
    );

    // Targeting the same node still resolves and now reaches the reloaded instance. Retried,
    // because a reload is not instantaneous from the caller's side.
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    loop {
        let who = ask_node(&caller, &p, ws, ext, &id_a).await;
        if who == "node-a-reloaded" {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "after a reload the targeted node must answer from the NEW instance; still got {who:?}"
        );
    }

    // And the OTHER host is untouched by A's reload — a reload is per-node, not fleet-wide.
    assert_eq!(ask_node(&caller, &p, ws, ext, &id_b).await, "node-b");
}

// ─────────────────────────────────────────────────────────────────────────────────────────────
// MANDATORY: offline / sync
// ─────────────────────────────────────────────────────────────────────────────────────────────

/// MANDATORY offline: a node that was LIVE and then DROPS becomes a prompt `NodeUnreachable`, and
/// nothing is executed anywhere.
///
/// Deliberately distinct from `an_unknown_target_is_refused_and_never_falls_back`, which targets a
/// node that never existed. That case never had a queryable; this one exercises **retraction of a
/// real, previously-answering one** — the actual production shape (a gateway loses its WAN link),
/// and the only version that can catch a caller caching reachability from a successful call.
///
/// The hub is owned by this test rather than leaked, so dropping it really does retract the
/// queryable — the drop IS the disconnection, nothing is simulated.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_live_node_that_drops_becomes_unreachable_promptly_and_runs_nothing() {
    let ws = "ambig-offline";
    let ext = "fleet-offline";
    let id = NodeId::new("node:fleet-offline-gw").unwrap();
    let port = free_port();
    let ep = format!("tcp/127.0.0.1:{port}");

    let caller_bus = Bus::peer_with(&[], &[ep.clone()]).await.unwrap();
    let caller = Node::boot_on_bus(caller_bus, NodeRole::Edge).await.unwrap();
    register_remote_extension(&caller, ext, id.clone(), &["whoami".to_string()]);
    let p = principal(ws, &[&format!("mcp:{ext}.whoami:call")]);

    // Phase 1 — the hub is LIVE and answering. Scoped so the drop is explicit.
    {
        let hub_bus = Bus::peer_with(&[ep], &[]).await.unwrap();
        let hub = Node::boot_on_bus(hub_bus, NodeRole::Hub).await.unwrap();
        hub.install_node_id(id.clone());
        host_whoami(&hub, ext, "the-gateway");
        let _server = serve_ext(&hub.bus, hub.registry.clone(), ext, &id, &[ws])
            .await
            .unwrap();

        assert_eq!(
            ask_node(&caller, &p, ws, ext, &id).await,
            "the-gateway",
            "precondition: the node is genuinely live and answering before it drops"
        );
        // hub + _server drop here → Zenoh retracts the queryable. This is a real disconnection.
    }

    // Phase 2 — the same target, now gone. Must refuse PROMPTLY (a bounded wait, not the query's
    // default ~10s timeout), and must never fall back to anything else.
    let started = std::time::Instant::now();
    let mut last = None;
    let deadline = std::time::Instant::now() + Duration::from_secs(20);
    let err = loop {
        match call_on_node(
            &caller.registry,
            &caller.bus,
            &p,
            ws,
            &format!("{ext}.whoami"),
            "{}",
            &id,
        )
        .await
        {
            Err(ToolError::NodeUnreachable { node }) => break node,
            // Retraction propagates a beat after the drop; a lingering success is the stale-route
            // window, not a wrong answer. Retry until it settles or the deadline fails us loudly.
            other => {
                last = Some(format!("{other:?}"));
                assert!(
                    std::time::Instant::now() < deadline,
                    "a dropped node must become NodeUnreachable; last was {last:?}"
                );
            }
        }
    };

    assert_eq!(err, id.to_string(), "names the node the caller asked for");
    // "Promptly" made concrete: once retraction has propagated, a zero-queryable `get` completes
    // fast rather than running to the query timeout. Generous bound — it guards the *class*
    // (fast-fail, not timeout), not a tight number, so it can't flake on a loaded box.
    assert!(
        started.elapsed() < Duration::from_secs(15),
        "refusal must not wait out the full query timeout; took {:?}",
        started.elapsed()
    );
}
