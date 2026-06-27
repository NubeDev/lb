//! The system-map service, headless (system-map scope). Proves the mandatory categories against a
//! **real** booted `Node` with **real** seeded records (no mocks, no fakes — CLAUDE §9): the fixed
//! service set is always present; `tables`-derived counts match the seeds; an enabled-but-stopped
//! native extension and a dead-lettered effect each yield `Degraded`; an empty workspace yields all
//! `Ok`/`Idle` (never `Down`/`Degraded`); the topology never dangles (every edge endpoint is a
//! present node and the node set ⊇ the overview ids); plus the mandatory capability-deny (a token
//! without `mcp:system.*:call` is refused) and two-workspace isolation (B's snapshot never shows A's
//! rows/effects/extensions).

use lb_assets::{record_install, Install, Tier};
use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{system_overview, system_subsystem, system_topology, Health, Node, ServiceStatus};
use lb_outbox::{enqueue, mark_failed, Effect};
use lb_store::write;
use serde_json::json;

/// A principal `sub` in workspace `ws` holding `caps`.
fn principal(sub: &str, ws: &str, caps: &[&str]) -> Principal {
    let key = SigningKey::generate();
    let claims = Claims {
        sub: sub.into(),
        ws: ws.into(),
        role: Role::Member,
        caps: caps.iter().map(|s| s.to_string()).collect(),
        iat: 0,
        exp: u64::MAX,
    };
    verify(&key, &mint(&key, &claims), 1).expect("token verifies")
}

const OVERVIEW: &str = "mcp:system.overview:call";
const TOPOLOGY: &str = "mcp:system.topology:call";
const SUBSYSTEM: &str = "mcp:system.subsystem:call";
const ALL: &[&str] = &[OVERVIEW, TOPOLOGY, SUBSYSTEM];

/// The fixed subsystem set every workspace must always report (a missing card means "we forgot it",
/// never "it happens to be empty").
const FIXED_IDS: &[&str] = &[
    "gateway",
    "bus",
    "mcp",
    "extensions",
    "registry",
    "store",
    "ingest",
    "inbox",
    "outbox",
    "jobs",
];

fn card<'a>(services: &'a [ServiceStatus], id: &str) -> &'a ServiceStatus {
    services
        .iter()
        .find(|s| s.id == id)
        .unwrap_or_else(|| panic!("missing card {id}"))
}

/// Seed a dead-lettered outbox effect: enqueue with a 1-attempt ceiling, then fail it once.
async fn dead_letter(node: &Node, ws: &str) {
    let effect =
        Effect::new("e1", "github-target", "open_pr", "{}", "idem-1", 1).with_max_attempts(1);
    enqueue(&node.store, ws, "change", "c1", &json!({"k": "v"}), &effect)
        .await
        .unwrap();
    mark_failed(&node.store, ws, "e1", 1).await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn fixed_service_set_always_present_and_empty_ws_is_ok_or_idle() {
    let node = Node::boot().await.unwrap();
    let ws = "empty";
    let ada = principal("user:ada", ws, ALL);

    let ov = system_overview(&node, &ada, ws).await.unwrap();
    let ids: Vec<&str> = ov.services.iter().map(|s| s.id.as_str()).collect();
    for want in FIXED_IDS {
        assert!(ids.contains(want), "fixed card {want} missing");
    }
    // An empty workspace is never a fault: every card is Ok or Idle, never Down/Degraded.
    for s in &ov.services {
        assert!(
            matches!(s.health, Health::Ok | Health::Idle),
            "empty ws card {} should be Ok/Idle, got {:?}",
            s.id,
            s.health
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn seeded_counts_match_and_degraded_on_dead_letter_and_stopped_ext() {
    let node = Node::boot().await.unwrap();
    let ws = "acme";
    let ada = principal("user:ada", ws, ALL);

    // A native extension, enabled by default but with no running sidecar → Degraded.
    record_install(
        &node.store,
        ws,
        &Install::new("coder", "v1", vec![], 1).with_tier(Tier::Native),
    )
    .await
    .unwrap();
    // Two real inbox rows + one job row → those cards report exact counts.
    write(&node.store, ws, "inbox", "i1", &json!({"x": 1}))
        .await
        .unwrap();
    write(&node.store, ws, "inbox", "i2", &json!({"x": 2}))
        .await
        .unwrap();
    write(&node.store, ws, "job", "j1", &json!({"x": 1}))
        .await
        .unwrap();
    // A dead-lettered effect → outbox Degraded.
    dead_letter(&node, ws).await;

    let ov = system_overview(&node, &ada, ws).await.unwrap();

    // Counts match the seeds (tables-derived, substring-matched by card).
    let inbox = card(&ov.services, "inbox");
    assert_eq!(inbox.metrics[0].value, "2");
    let jobs = card(&ov.services, "jobs");
    assert_eq!(jobs.metrics[0].value, "1");

    // Dead-lettered effect → outbox Degraded.
    assert_eq!(card(&ov.services, "outbox").health, Health::Degraded);
    // Enabled-but-stopped native extension → extensions Degraded.
    assert_eq!(card(&ov.services, "extensions").health, Health::Degraded);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn topology_never_dangles_and_covers_overview() {
    let node = Node::boot().await.unwrap();
    let ws = "acme";
    let ada = principal("user:ada", ws, ALL);

    let ov = system_overview(&node, &ada, ws).await.unwrap();
    let topo = system_topology(&node, &ada, ws).await.unwrap();

    let node_ids: std::collections::HashSet<&str> =
        topo.nodes.iter().map(|n| n.id.as_str()).collect();
    // Nodes ⊇ overview ids.
    for s in &ov.services {
        assert!(
            node_ids.contains(s.id.as_str()),
            "topology missing {}",
            s.id
        );
    }
    // Every edge endpoint is a present node — no dangling edge.
    for e in &topo.edges {
        assert!(
            node_ids.contains(e.from.as_str()),
            "dangling edge from {}",
            e.from
        );
        assert!(
            node_ids.contains(e.to.as_str()),
            "dangling edge to {}",
            e.to
        );
    }
    assert!(!topo.edges.is_empty(), "the platform has wiring");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn both_verbs_denied_without_their_cap() {
    let node = Node::boot().await.unwrap();
    let ws = "acme";
    let nobody = principal("user:mallory", ws, &[]);

    assert!(system_overview(&node, &nobody, ws).await.is_err());
    assert!(system_topology(&node, &nobody, ws).await.is_err());

    // Holding ONE cap does not grant the other.
    let only_ov = principal("user:ov", ws, &[OVERVIEW]);
    assert!(system_overview(&node, &only_ov, ws).await.is_ok());
    assert!(system_topology(&node, &only_ov, ws).await.is_err());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn workspace_isolation_b_never_sees_a() {
    let node = Node::boot().await.unwrap();

    // Seed ws-A with an extension, rows, and a dead-lettered effect.
    record_install(
        &node.store,
        "ws-a",
        &Install::new("coder", "v1", vec![], 1).with_tier(Tier::Native),
    )
    .await
    .unwrap();
    write(&node.store, "ws-a", "inbox", "i1", &json!({"x": 1}))
        .await
        .unwrap();
    dead_letter(&node, "ws-a").await;

    // ws-B's admin reads its own (empty) namespace.
    let ben = principal("user:ben", "ws-b", ALL);
    let ov = system_overview(&node, &ben, "ws-b").await.unwrap();
    assert_eq!(ov.ws, "ws-b");

    // None of A's state leaks: B's extensions/outbox are not Degraded, and B's counts are zero.
    assert_ne!(card(&ov.services, "extensions").health, Health::Degraded);
    assert_ne!(card(&ov.services, "outbox").health, Health::Degraded);
    assert_eq!(card(&ov.services, "inbox").metrics[0].value, "0");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn subsystem_returns_the_right_card_and_bus_extra_lists_zids() {
    let node = Node::boot().await.unwrap();
    let ws = "acme";
    let ada = principal("user:ada", ws, ALL);

    // A no-page card with no extra: gateway returns its own card and an empty `extra` object.
    let gw = system_subsystem(&node, &ada, ws, "gateway").await.unwrap();
    assert_eq!(gw.service.id, "gateway");
    assert_eq!(gw.ws, "acme");
    assert_eq!(gw.extra, json!({}));

    // The bus card's extra carries the live peer/router zid lists (the detail behind the counts) —
    // present as arrays of zid strings. We assert the *shape* (present arrays of strings), NOT exact
    // equality with the card's count: the metric and the extra are two independent live-session reads,
    // and the shared in-proc test mesh's peer count drifts between them as sibling test nodes join and
    // leave. The single-node guarantee (the extra exists for `bus`, `{}` elsewhere) is the invariant.
    let bus = system_subsystem(&node, &ada, ws, "bus").await.unwrap();
    assert_eq!(bus.service.id, "bus");
    let peer_zids = bus.extra["peer_zids"].as_array().expect("peer_zids array");
    let router_zids = bus.extra["router_zids"]
        .as_array()
        .expect("router_zids array");
    for z in peer_zids.iter().chain(router_zids) {
        assert!(z.is_string(), "each zid is a string, got {z:?}");
    }
    // The bus card exposes its own peer/router count metrics (the summary the detail expands on).
    assert!(bus.service.metrics.iter().any(|m| m.label == "peers"));
    assert!(bus.service.metrics.iter().any(|m| m.label == "routers"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn subsystem_unknown_id_is_opaque_not_a_panic() {
    let node = Node::boot().await.unwrap();
    let ws = "acme";
    let ada = principal("user:ada", ws, ALL);

    // An id that is not a subsystem is refused opaquely (the same answer a no-cap caller gets), never
    // a panic / 500.
    assert!(system_subsystem(&node, &ada, ws, "nope").await.is_err());
    assert!(system_subsystem(&node, &ada, ws, "").await.is_err());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn subsystem_denied_without_its_own_cap() {
    let node = Node::boot().await.unwrap();
    let ws = "acme";

    // No caps at all → denied.
    let nobody = principal("user:mallory", ws, &[]);
    assert!(system_subsystem(&node, &nobody, ws, "bus").await.is_err());

    // Holding overview/topology does NOT grant subsystem — its own cap is required.
    let no_sub = principal("user:ov", ws, &[OVERVIEW, TOPOLOGY]);
    assert!(system_subsystem(&node, &no_sub, ws, "bus").await.is_err());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn subsystem_workspace_isolation_b_never_sees_a() {
    let node = Node::boot().await.unwrap();

    // Seed ws-A with a dead-lettered effect → A's outbox detail is Degraded.
    dead_letter(&node, "ws-a").await;
    let ada = principal("user:ada", "ws-a", ALL);
    let a_outbox = system_subsystem(&node, &ada, "ws-a", "outbox")
        .await
        .unwrap();
    assert_eq!(a_outbox.service.health, Health::Degraded);

    // B's outbox detail reflects only B's (empty) namespace — none of A's dead-letter leaks.
    let ben = principal("user:ben", "ws-b", ALL);
    let b_outbox = system_subsystem(&node, &ben, "ws-b", "outbox")
        .await
        .unwrap();
    assert_eq!(b_outbox.ws, "ws-b");
    assert_ne!(b_outbox.service.health, Health::Degraded);
    assert_eq!(
        b_outbox
            .service
            .metrics
            .iter()
            .find(|m| m.label == "dead-letter")
            .unwrap()
            .value,
        "0"
    );
}
