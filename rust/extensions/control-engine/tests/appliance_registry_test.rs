//! S4 — the `ce_appliance` registry, end to end against a REAL gateway (the native-callback pattern,
//! copied from `ros/tests/crud_test.rs` / `role/gateway/tests/native_callback_test.rs`). No mocks
//! (CLAUDE rule 9 / testing §0): a real `Node`, a real `axum` gateway on a real TCP port, the real
//! `lb-sidecar-client` making real `reqwest` calls, the real MCP gate + the real generic
//! `store.write`/`store.query`/`store.delete` host verbs + embedded store. There is NO CE here at all —
//! the registry verbs are pure store CRUD, so no `ce_fake` is needed.
//!
//! The verbs are driven exactly as `main.rs` drives them — `tools::appliance::{add,list,remove}::run`
//! and `resolve::resolve` with a `HostCtx` built over the real gateway + the sidecar's grant. Proves
//! the S4 mandatory categories:
//!   - **capability deny (per verb):** a grant missing `mcp:control-engine.appliance.add:call` (the
//!     sidecar self-check) refuses BEFORE any store write; a grant missing the host-side
//!     `store:ce_appliance:write` is refused by the gateway's gate. Neither writes a record.
//!   - **workspace isolation:** an appliance registered in ws-A is invisible to a ws-B sidecar's
//!     `appliance.list`; a ws-B resolve of it → not-found; ws-B cannot remove it.
//!   - **resolution:** a known appliance resolves to its recorded base; an unknown/other-ws id →
//!     not-found (the isolation wall, no existence leak); the empty selector → the canonical local base.
//!   - **restart/statelessness:** the registry is in SurrealDB (read per call), so a fresh `HostCtx`
//!     over the same store still answers — no in-memory registry cache.

use std::net::SocketAddr;
use std::sync::Arc;

use control_engine::host::{HostCtx, HostError};
use control_engine::resolve;
use control_engine::tools::appliance;
use lb_auth::{mint, Claims, Role, SigningKey};
use lb_host::{Node, Role as NodeRole};
use lb_role_gateway::{router, Gateway};
use lb_sidecar_client::{Config, SidecarClient};
use serde_json::{json, Value};

const NOW: u64 = 1000;

/// The full grant a fully-authorized control-engine sidecar holds for the registry surface — its
/// manifest `request`, resolved to caps (the per-verb `mcp:…:call` self-checks + the host-side store
/// verb gates + the per-table `store:ce_appliance:*` grants).
fn full_caps() -> Vec<String> {
    [
        "mcp:control-engine.appliance.add:call",
        "mcp:control-engine.appliance.list:call",
        "mcp:control-engine.appliance.remove:call",
        "mcp:store.write:call",
        "mcp:store.query:call",
        "mcp:store.delete:call",
        "store:ce_appliance:read",
        "store:ce_appliance:write",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

fn child_token(key: &SigningKey, ws: &str, caps: &[String]) -> String {
    let claims = Claims {
        sub: "ext:control-engine".into(),
        ws: ws.into(),
        role: Role::Member,
        caps: caps.to_vec(),
        iat: NOW - 1,
        exp: NOW + 10_000,
        constraint: None,
        run_id: None,
    };
    mint(key, &claims)
}

/// Boot a real node + real gateway on a real ephemeral port. The node's key IS the gateway's key, so a
/// token minted with `key` verifies on `/mcp/call`.
async fn serve() -> (Arc<Node>, SigningKey, String) {
    let node = Arc::new(Node::boot_as(NodeRole::Hub).await.expect("node boots"));
    let key = SigningKey::generate();
    let gw = Gateway::new(node.clone(), key.clone(), NOW);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let addr: SocketAddr = listener.local_addr().unwrap();
    let app = router(gw);
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    (node, key, format!("http://{addr}"))
}

/// A HostCtx whose SidecarClient carries a REAL signed child token (so callbacks authenticate), and
/// whose self-check caps match that token's grant.
fn host_for(key: &SigningKey, base: &str, ws: &str, caps: &[String]) -> HostCtx {
    let token = child_token(key, ws, caps);
    HostCtx::with_parts(
        SidecarClient::with_config(Config::new(base, token, ws, "control-engine")),
        caps.to_vec(),
        ws,
    )
}

fn add_args(id: &str, node: &str, base: &str) -> Value {
    json!({ "id": id, "name": format!("Appliance {id}"), "mode": "appliance", "node": node, "base": base })
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn add_list_resolve_remove_round_trip() {
    let (_node, key, gw) = serve().await;
    let ws = "ce-reg";
    let caps = full_caps();
    let host = host_for(&key, &gw, ws, &caps);

    // add
    let added = appliance::add::run(
        &host,
        &add_args("plant-1", "edge-7", "http://10.0.0.2:7979"),
        NOW,
    )
    .await
    .expect("add ok");
    assert_eq!(added["id"], "plant-1");

    // list — the record is present with its recorded fields
    let listed = appliance::list::run(&host).await.expect("list ok");
    let arr = listed["appliances"].as_array().expect("appliances array");
    assert_eq!(arr.len(), 1, "one appliance: {listed}");
    assert_eq!(arr[0]["id"], "plant-1");
    assert_eq!(arr[0]["node"], "edge-7");
    assert_eq!(arr[0]["base"], "http://10.0.0.2:7979");
    assert_eq!(arr[0]["mode"], "appliance");

    // resolve — a known appliance resolves to its recorded base
    let resolved = resolve::resolve(&host, "plant-1")
        .await
        .expect("resolve ok");
    assert_eq!(resolved.base, "http://10.0.0.2:7979");

    // resolve — the empty selector is the canonical local base (no lookup)
    let local = resolve::resolve(&host, "").await.expect("empty resolves");
    assert_eq!(local.base, "", "empty selector → canonical local");

    // resolve — an unknown id is NOT-FOUND (the isolation wall; no literal-base fallback with a real store)
    let unknown = resolve::resolve(&host, "nope")
        .await
        .expect_err("unknown → not-found");
    assert!(
        matches!(unknown, HostError::NotFound),
        "unknown: {unknown:?}"
    );

    // remove — idempotent; after remove the list is empty and resolve is not-found
    appliance::remove::run(
        &host,
        &control_engine::watch::WatchRegistry::new(),
        &json!({ "id": "plant-1" }),
    )
    .await
    .expect("remove ok");
    let after = appliance::list::run(&host)
        .await
        .expect("list after remove");
    assert_eq!(
        after["appliances"].as_array().unwrap().len(),
        0,
        "empty after remove"
    );
    appliance::remove::run(
        &host,
        &control_engine::watch::WatchRegistry::new(),
        &json!({ "id": "plant-1" }),
    )
    .await
    .expect("second remove is idempotent");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn add_without_its_cap_is_denied_before_any_write() {
    let (_node, key, gw) = serve().await;
    let ws = "ce-deny-add";
    // Everything EXCEPT the appliance.add self-check cap.
    let caps: Vec<String> = full_caps()
        .into_iter()
        .filter(|c| c != "mcp:control-engine.appliance.add:call")
        .collect();
    let host = host_for(&key, &gw, ws, &caps);

    let err = appliance::add::run(&host, &add_args("x", "n", "http://h:1"), NOW)
        .await
        .expect_err("add denied without its cap");
    assert!(matches!(err, HostError::Denied), "opaque deny: {err:?}");

    // The self-check ran BEFORE any callback — a full-cap sidecar sees no record.
    let full = host_for(&key, &gw, ws, &full_caps());
    let listed = appliance::list::run(&full).await.expect("list ok");
    assert_eq!(
        listed["appliances"].as_array().unwrap().len(),
        0,
        "no record written on a denied add"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn add_without_the_store_table_grant_is_denied_by_the_host() {
    let (_node, key, gw) = serve().await;
    let ws = "ce-deny-store";
    // Holds the self-check cap + the store.write MCP verb, but NOT the per-table store:ce_appliance:write
    // — so the sidecar's self-check passes but the HOST gate refuses the store.write callback.
    let caps: Vec<String> = full_caps()
        .into_iter()
        .filter(|c| c != "store:ce_appliance:write")
        .collect();
    let host = host_for(&key, &gw, ws, &caps);

    let err = appliance::add::run(&host, &add_args("x", "n", "http://h:1"), NOW)
        .await
        .expect_err("add denied by the host store gate");
    assert!(
        matches!(err, HostError::Denied),
        "opaque deny from host: {err:?}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn remove_without_its_cap_is_denied_and_nothing_is_erased() {
    let (_node, key, gw) = serve().await;
    let ws = "ce-deny-remove";
    let full = host_for(&key, &gw, ws, &full_caps());
    appliance::add::run(&full, &add_args("keep", "n", "http://h:1"), NOW)
        .await
        .expect("seed a record");

    let caps: Vec<String> = full_caps()
        .into_iter()
        .filter(|c| c != "mcp:control-engine.appliance.remove:call")
        .collect();
    let nocap = host_for(&key, &gw, ws, &caps);
    let err = appliance::remove::run(
        &nocap,
        &control_engine::watch::WatchRegistry::new(),
        &json!({ "id": "keep" }),
    )
    .await
    .expect_err("remove denied without its cap");
    assert!(matches!(err, HostError::Denied), "opaque deny: {err:?}");

    // Survives — a denied remove erases nothing.
    let listed = appliance::list::run(&full).await.expect("list ok");
    assert_eq!(
        listed["appliances"].as_array().unwrap().len(),
        1,
        "record survives"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_ws_a_appliance_is_invisible_to_ws_b() {
    let (_node, key, gw) = serve().await;
    let caps = full_caps();
    let a = host_for(&key, &gw, "ws-a", &caps);
    let b = host_for(&key, &gw, "ws-b", &caps);

    appliance::add::run(&a, &add_args("plant-a", "edge-a", "http://a:7979"), NOW)
        .await
        .expect("ws-a add");

    // ws-B's list does not see it (structural namespace wall).
    let b_list = appliance::list::run(&b).await.expect("ws-b list");
    assert_eq!(
        b_list["appliances"].as_array().unwrap().len(),
        0,
        "ws-b sees no ws-a appliance: {b_list}"
    );

    // ws-B resolve of the ws-a id → not-found (no existence leak).
    let b_resolve = resolve::resolve(&b, "plant-a")
        .await
        .expect_err("ws-b resolve → not-found");
    assert!(
        matches!(b_resolve, HostError::NotFound),
        "ws-b: {b_resolve:?}"
    );

    // ws-B remove of the ws-a id is idempotent (nothing there) and does NOT touch ws-a's record.
    appliance::remove::run(
        &b,
        &control_engine::watch::WatchRegistry::new(),
        &json!({ "id": "plant-a" }),
    )
    .await
    .expect("ws-b remove is a no-op in its own ns");
    let a_list = appliance::list::run(&a).await.expect("ws-a list");
    assert_eq!(
        a_list["appliances"].as_array().unwrap().len(),
        1,
        "ws-a appliance untouched by ws-b remove"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_registry_is_stateless_a_fresh_ctx_still_answers() {
    let (_node, key, gw) = serve().await;
    let ws = "ce-stateless";
    let caps = full_caps();

    // Write with one HostCtx…
    let writer = host_for(&key, &gw, ws, &caps);
    appliance::add::run(
        &writer,
        &add_args("plant-1", "edge-7", "http://x:7979"),
        NOW,
    )
    .await
    .expect("add");
    drop(writer);

    // …read with a BRAND-NEW HostCtx (mirrors a killed + respawned sidecar): the record is in
    // SurrealDB, reread on demand — no in-memory registry cache.
    let reader = host_for(&key, &gw, ws, &caps);
    let listed = appliance::list::run(&reader)
        .await
        .expect("fresh ctx lists");
    assert_eq!(
        listed["appliances"].as_array().unwrap().len(),
        1,
        "reread from the store"
    );
    let resolved = resolve::resolve(&reader, "plant-1")
        .await
        .expect("fresh ctx resolves");
    assert_eq!(resolved.base, "http://x:7979");
}
