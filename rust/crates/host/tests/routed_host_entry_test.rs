//! routed-dispatch-sidecar-bridge scope — the CALLER-facing half of routed-node-dispatch (#81).
//!
//! `routed_ambiguity_test.rs` proves the *engine* (`lb_mcp::call_on_node`) routes correctly. This
//! file proves the *seam above it*: `lb_host::call_tool_on_node`, the chokepoint the `POST /mcp/call`
//! bridge (and hence every native sidecar) funnels through. Before this scope the engine had zero
//! non-test callers — the routed-dispatch ERROR was reachable over HTTP but the routed-dispatch
//! SUCCESS was not.
//!
//! THE HAZARD THIS FILE GUARDS: `call_tool` has a wide fan-out (agent loop, gateway routes, reach
//! path), so the node axis was added additively. The failure mode that buys is a `Some(node)` that
//! silently degrades to `None` somewhere in the thread
//! (`call_tool_on_node` → `call_tool_at_depth_on_node` → `dispatch_at_depth` → `call_on_node`) —
//! the call then runs UNTARGETED and returns success from whichever node won the race. That is
//! exactly the misprovisioning bug #81 exists to kill, reintroduced one layer up and invisible: the
//! caller sees `200 OK`. A test that only asserts "a targeted call succeeds" would pass on the
//! degraded code, so the determinism test below asserts WHICH node ran it, N times.
//!
//! Everything is real (testing-scope §0): two real `Node`s, two real in-process Zenoh peers linked
//! over loopback TCP, real queryables, real dispatch, real capability checks. The `whoami` fixture
//! is `#[cfg(test)]` and exists only because a tool must report its own host for "who answered?" to
//! be a fact rather than an inference — the registry, bus, queryable and dispatch path are all
//! production. Harness shape (unique ws + ext per test, explicit loopback endpoints, multi-thread
//! flavor) is inherited from `routed_ambiguity_test.rs`; see its header for the why.

use std::sync::Arc;
use std::time::Duration;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_bus::{Bus, NodeId};
use lb_host::{
    call_tool, call_tool_on_node, register_remote_extension, serve_ext, Node, Role as NodeRole,
    ToolServer,
};
use lb_mcp::{ToolDescriptor, ToolError};
use lb_runtime::{CallContext, LocalDispatch, RuntimeError};
use tokio::sync::Mutex;

/// A real local dispatch target that answers `whoami` with the id of the node hosting it — the only
/// way to observe WHICH node answered. Implements the same `LocalDispatch` a wasm instance and a
/// native sidecar implement, reached through the production registry and `serve_call`.
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

fn host_whoami(node: &Node, ext: &str, label: &str) {
    node.registry.register_local_dispatch(
        ext,
        vec![ToolDescriptor::name_only("whoami")],
        Arc::new(Mutex::new(WhoAmI {
            node: label.to_string(),
        })),
    );
}

fn free_port() -> u16 {
    let probe = std::net::TcpListener::bind("127.0.0.1:0").expect("grab a free loopback port");
    probe.local_addr().expect("probe addr").port()
}

/// Two hubs both hosting AND serving the same ext, plus a caller edge that knows the ext is remote
/// on both. The fleet shape the addressing axis exists for. Node ids are namespaced by the (unique
/// per test) ext id because in-process Zenoh peers share a keyspace — two concurrent tests declaring
/// the same node key is a real duplicate-id collision
/// (debugging/bus/in-process-peers-share-the-keyspace.md).
async fn two_hosts_one_ext(
    ext: &str,
    workspaces: &[&str],
) -> (Node, NodeId, NodeId, Vec<ToolServer>) {
    let (id_a, id_b) = (
        NodeId::new(format!("node:{ext}-gw-01")).expect("key-safe id"),
        NodeId::new(format!("node:{ext}-gw-02")).expect("key-safe id"),
    );

    let ep_a = format!("tcp/127.0.0.1:{}", free_port());
    let ep_b = format!("tcp/127.0.0.1:{}", free_port());

    let bus_a = Bus::peer_with(&[ep_a.clone()], &[])
        .await
        .expect("hub A listens");
    let hub_a = Node::boot_on_bus(bus_a, NodeRole::Hub)
        .await
        .expect("hub A boots");
    hub_a.install_node_id(id_a.clone());
    host_whoami(&hub_a, ext, "node-a");
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
    host_whoami(&hub_b, ext, "node-b");
    let server_b = serve_ext(&hub_b.bus, hub_b.registry.clone(), ext, &id_b, workspaces)
        .await
        .expect("hub B serves");

    let caller_bus = Bus::peer_with(&[], &[ep_a, ep_b])
        .await
        .expect("caller connects to both hubs");
    let caller = Node::boot_on_bus(caller_bus, NodeRole::Edge)
        .await
        .expect("caller boots");
    register_remote_extension(&caller, ext, id_a.clone(), &["whoami".to_string()]);
    register_remote_extension(&caller, ext, id_b.clone(), &["whoami".to_string()]);

    // Dropping the hubs mid-test would retract their queryables and make the run meaningless; the
    // ToolServer holds the task but the Node owns the registry it dispatches against. Test-only.
    Box::leak(Box::new(hub_a));
    Box::leak(Box::new(hub_b));

    (caller, id_a, id_b, vec![server_a, server_b])
}

/// Poll the REAL host entry until the addressed node's queryable has propagated, returning the label
/// of whoever answered. A `get` issued before propagation finds no responder, so we retry the real
/// call rather than sleeping a fixed beat or mocking convergence. Retrying `NodeUnreachable` is
/// honest here: during convergence it is genuinely indistinguishable from "not yet propagated".
async fn ask_node_via_host(
    caller: &Arc<Node>,
    p: &Principal,
    ws: &str,
    ext: &str,
    node: &NodeId,
) -> String {
    let tool = format!("{ext}.whoami");
    let deadline = std::time::Instant::now() + Duration::from_secs(20);
    let mut last_err = None;
    while std::time::Instant::now() < deadline {
        match tokio::time::timeout(
            Duration::from_millis(500),
            call_tool_on_node(caller, p, ws, &tool, "{}", Some(node)),
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
    panic!("targeted call to {node} never converged; last error: {last_err:?}");
}

// ─────────────────────────────────────────────────────────────────────────────────────────────
// The thread holds: Some(node) reaches the engine, and is never dropped on the way
// ─────────────────────────────────────────────────────────────────────────────────────────────

/// THE central guard. 40 calls alternating between two named nodes, through the real host entry —
/// 100% must land on the node named, 0% may fall back. A single correct answer proves nothing (the
/// untargeted path would produce one half the time by luck); the count is what distinguishes
/// "routed" from "raced". This is the test that fails if the `Option` is lost anywhere in the
/// four-hop thread down to `lb_mcp::call_on_node`.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_host_entry_lands_every_targeted_call_on_the_node_named() {
    let ws = "host-entry-determinism";
    let ext = "fleet-hostdet";
    let (caller, id_a, id_b, _servers) = two_hosts_one_ext(ext, &[ws]).await;
    let caller = Arc::new(caller);
    let p = principal(ws, &[&format!("mcp:{ext}.whoami:call")]);

    let mut landed_a = 0;
    let mut landed_b = 0;
    for _ in 0..20 {
        if ask_node_via_host(&caller, &p, ws, ext, &id_a).await == "node-a" {
            landed_a += 1;
        }
        if ask_node_via_host(&caller, &p, ws, ext, &id_b).await == "node-b" {
            landed_b += 1;
        }
    }
    assert_eq!(
        (landed_a, landed_b),
        (20, 20),
        "every call targeted through lb_host::call_tool_on_node must run on the node named; \
         anything less means the target degraded to an untargeted (raced) dispatch"
    );
}

/// The other half of the additive contract: `None` must behave EXACTLY as before — so a
/// multiply-hosted ext is still `Ambiguous`, not silently routed to some default. This is what
/// keeps the change backwards compatible for the wide `call_tool` fan-out.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_untargeted_host_call_is_still_ambiguous_and_dispatches_nothing() {
    let ws = "host-entry-none";
    let ext = "fleet-hostnone";
    let (caller, id_a, id_b, _servers) = two_hosts_one_ext(ext, &[ws]).await;
    let caller = Arc::new(caller);
    let p = principal(ws, &[&format!("mcp:{ext}.whoami:call")]);

    // Both the explicit `None` and the untouched legacy `call_tool` must agree — they are one path.
    for label in ["explicit None", "legacy call_tool"] {
        let err = match label {
            "explicit None" => {
                call_tool_on_node(&caller, &p, ws, &format!("{ext}.whoami"), "{}", None).await
            }
            _ => call_tool(&caller, &p, ws, &format!("{ext}.whoami"), "{}").await,
        }
        .expect_err("a multiply-hosted ext must not be coin-flipped");

        match err {
            ToolError::Ambiguous { candidates, .. } => {
                assert!(
                    candidates.contains(&id_a.to_string())
                        && candidates.contains(&id_b.to_string()),
                    "{label}: candidates must name both hosts so a caller can pick one, got \
                     {candidates:?}"
                );
            }
            other => panic!("{label}: expected Ambiguous, got {other:?}"),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────────────────────
// Mandatory: capability deny + workspace isolation, on the TARGETED path
// ─────────────────────────────────────────────────────────────────────────────────────────────

/// Capability deny-test (mandatory). Addressing is not authorization: a capless caller is `Denied`
/// through the host entry, and identically so whether the node it names is real or invented — so a
/// targeted call is not an oracle for enumerating the fleet. Asserts it is NOT `409/503`, which
/// would leak that the named node does (or does not) exist.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_capless_targeted_host_call_is_denied_with_no_existence_signal() {
    let ws = "host-entry-deny";
    let ext = "fleet-hostdeny";
    let (caller, id_a, _id_b, _servers) = two_hosts_one_ext(ext, &[ws]).await;
    let caller = Arc::new(caller);
    // A real principal in the right workspace, holding an UNRELATED capability — so this is a
    // capability denial, not an authentication or workspace failure.
    let capless = principal(ws, &["mcp:something.else:call"]);
    let ghost = NodeId::new("node:never-existed-anywhere").unwrap();

    for (which, node) in [("a real node", &id_a), ("an invented node", &ghost)] {
        let err = call_tool_on_node(
            &caller,
            &capless,
            ws,
            &format!("{ext}.whoami"),
            "{}",
            Some(node),
        )
        .await
        .expect_err("a caller without the tool's capability must be refused");

        assert!(
            matches!(err, ToolError::Denied),
            "{which}: a capless targeted call must be Denied (never Ambiguous/NodeUnreachable, \
             which would reveal whether {node} exists), got {err:?}"
        );
    }
}

/// Workspace isolation (mandatory), on the targeted path. The node-qualified key is declared PER
/// WORKSPACE, so a targeted call from ws B to a node serving only ws A has nowhere to land — the
/// key space IS the wall. It must refuse, not leak, and not fall back to any node reachable in B.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_ws_b_caller_cannot_target_a_node_serving_only_ws_a() {
    let ws_a = "host-entry-iso-a";
    let ws_b = "host-entry-iso-b";
    let ext = "fleet-hostiso";
    // The hubs declare their node keys for ws_a ONLY.
    let (caller, id_a, _id_b, _servers) = two_hosts_one_ext(ext, &[ws_a]).await;
    let caller = Arc::new(caller);
    // Legitimately scoped to ws_b, WITH a real capability in ws_b — so what stops this is the
    // workspace wall, not the capability gate.
    let intruder = principal(ws_b, &[&format!("mcp:{ext}.whoami:call")]);

    let err = call_tool_on_node(
        &caller,
        &intruder,
        ws_b,
        &format!("{ext}.whoami"),
        "{}",
        Some(&id_a),
    )
    .await
    .expect_err("a ws-B call must not execute on a node serving only ws-A");

    match err {
        ToolError::NodeUnreachable { node } => assert_eq!(node, id_a.to_string()),
        other => panic!("expected NodeUnreachable (the ws-B key has no responder), got {other:?}"),
    }
}

/// A named-but-absent node is a REFUSAL, never a fallback to the other node that also hosts the ext.
/// The fallback is the misprovisioning bug; through the host entry it would look like plain success.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_unknown_target_through_the_host_entry_never_falls_back() {
    let ws = "host-entry-ghost";
    let ext = "fleet-hostghost";
    let (caller, _id_a, _id_b, _servers) = two_hosts_one_ext(ext, &[ws]).await;
    let caller = Arc::new(caller);
    let p = principal(ws, &[&format!("mcp:{ext}.whoami:call")]);
    let ghost = NodeId::new("node:gw-99-never-existed").unwrap();

    let err = call_tool_on_node(
        &caller,
        &p,
        ws,
        &format!("{ext}.whoami"),
        "{}",
        Some(&ghost),
    )
    .await
    .expect_err("a targeted call to an absent node must refuse, never run elsewhere");

    match err {
        ToolError::NodeUnreachable { node } => assert_eq!(node, ghost.to_string()),
        other => panic!(
            "expected NodeUnreachable, got {other:?} (a success here means it fell back \
                         to a node that was NOT named — the misprovisioning bug)"
        ),
    }
}
