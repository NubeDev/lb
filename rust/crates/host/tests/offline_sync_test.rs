//! S3 EXIT-GATE (part) + the FIRST mandatory offline/sync tests (testing §2.3): a message
//! posted on the edge **while the hub is not yet syncing** (offline) must apply **idempotently**
//! on reconnect, per the README §6.8 authority/merge rules.
//!
//! Model of "offline then reconnect", honestly, with two in-process nodes:
//!   1. The edge posts items. The hub has NOT started its `ChannelSync` yet, so it misses the
//!      live bus push — exactly what an offline/disconnected hub experiences (state is on the
//!      edge's store; the motion was not observed).
//!   2. "Reconnect": the hub starts a `ChannelSync` (subscribe + apply), and the edge
//!      `replay_history`s its durable items back onto the bus. The hub applies each into its own
//!      store. Because the inbox upserts on `(channel, id)`, apply is idempotent (§6.8).
//!   3. Assert: the hub's history now equals the edge's, in order; and replaying AGAIN does not
//!      duplicate (the merge is conflict-free for append-style items).
//!
//! Each test uses a UNIQUE workspace id (in-process peers share a workspace's keyspace) and the
//! multi-thread flavor (boots a Zenoh peer).

use std::sync::Arc;
use std::time::Duration;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_bus::Bus;
use lb_host::{
    history, post, replay_history, sync_channel, ChannelSync, Node, Role as NodeRole, SidecarMap,
};
use lb_inbox::Item;
use lb_mcp::Registry;
use lb_runtime::Engine;
use lb_store::Store;

/// Build a node on an explicit `bus` + `role`. Same wiring as `Node::boot_as`, but we own the bus
/// so edge and hub can be **point-to-point linked** over loopback TCP (see `linked_edge_and_hub`).
/// Mirrors the direct-construction pattern in `cross_node_routing_test.rs` / `ext_publish_test.rs`
/// — nothing mocked, just a real Zenoh peer whose endpoints we chose.
async fn node_on_bus(bus: Bus, role: NodeRole) -> Node {
    Node {
        store: Store::memory().await.expect("in-mem store"),
        bus,
        engine: Engine::new().expect("runtime engine"),
        registry: Arc::new(Registry::new()),
        sidecars: Arc::new(SidecarMap::new()),
        apikeys: Arc::new(lb_host::ApiKeyCache::new()),
        role,
    }
}

/// Stand up an edge and a hub **explicitly linked over a loopback TCP endpoint** so discovery is
/// deterministic regardless of how many other in-process peers are scouting concurrently. Without
/// this, the pair relied on ambient multicast scouting which under a full parallel
/// `cargo test --workspace` could stall past any timeout — the discovery half of the offline-sync
/// flake (debugging/bus/routed-call-races-mesh-discovery.md is the sibling). The publisher-side
/// subscription-readiness barrier in `replay_history` handles the *second* half (a live link but a
/// not-yet-propagated subscription); both are needed for a non-flaky replay.
async fn linked_edge_and_hub() -> (Node, Node) {
    let port = {
        let probe = std::net::TcpListener::bind("127.0.0.1:0").expect("grab a free loopback port");
        probe.local_addr().expect("probe addr").port()
    };
    let endpoint = format!("tcp/127.0.0.1:{port}");

    let hub_bus = Bus::peer_with(&[endpoint.clone()], &[])
        .await
        .expect("hub bus listens on the chosen endpoint");
    let hub = node_on_bus(hub_bus, NodeRole::Hub).await;

    let edge_bus = Bus::peer_with(&[], &[endpoint])
        .await
        .expect("edge bus connects to hub");
    let edge = node_on_bus(edge_bus, NodeRole::Edge).await;

    (edge, hub)
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

/// Drain the hub's sync, applying up to `n` items (each with a short timeout). Returns how many
/// were applied — lets a test assert "all offline writes caught up" without waiting forever.
async fn drain(sync: &ChannelSync, n: usize) -> usize {
    let mut applied = 0;
    for _ in 0..n {
        match tokio::time::timeout(Duration::from_secs(2), sync.apply_next()).await {
            Ok(Some(_)) => applied += 1,
            _ => break,
        }
    }
    applied
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn offline_edge_writes_apply_idempotently_on_reconnect() {
    let ws = "sync-offline-reconnect";
    let (edge, hub) = linked_edge_and_hub().await;
    let p = principal(ws, &["bus:chan/general:pub", "bus:chan/general:sub"]);

    // 1. OFFLINE: the edge posts three messages while the hub is NOT syncing yet. They persist
    //    to the edge's store; the hub never sees the live push.
    for (i, body) in ["alpha", "beta", "gamma"].iter().enumerate() {
        post(
            &edge,
            &p,
            ws,
            "general",
            Item::new(format!("m{i}"), "general", "user:edge", *body, i as u64),
        )
        .await
        .expect("edge posts offline");
    }

    // The hub starts empty — it genuinely missed them (no sync was running).
    let hub_reader = principal(ws, &["bus:chan/general:sub"]);
    let before = history(&hub.store, &hub_reader, ws, "general")
        .await
        .expect("hub reads its own (empty) channel");
    assert!(
        before.is_empty(),
        "hub must NOT have the offline writes before reconnect"
    );

    // 2. RECONNECT: hub starts syncing; edge replays its durable history onto the bus.
    let sync = sync_channel(&hub.bus, &hub.store, ws, "general")
        .await
        .expect("hub starts channel sync");
    let replayed = replay_history(&edge.bus, &edge.store, ws, "general")
        .await
        .expect("edge replays offline writes");
    assert_eq!(replayed, 3, "edge replays all three durable items");

    let applied = drain(&sync, 3).await;
    assert_eq!(
        applied, 3,
        "hub applies all three offline writes on reconnect"
    );

    // 3. The hub's durable history now equals the edge's, in order.
    let after = history(&hub.store, &hub_reader, ws, "general")
        .await
        .expect("hub reads after sync");
    let bodies: Vec<&str> = after.iter().map(|i| i.body.as_str()).collect();
    assert_eq!(
        bodies,
        ["alpha", "beta", "gamma"],
        "synced, ordered oldest→newest"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn replaying_the_same_items_again_does_not_duplicate() {
    // §6.8 idempotent apply: re-delivering items the hub already holds is a no-op. This is the
    // property that makes at-least-once delivery safe — a retried/duplicated replay can never
    // corrupt the merged history.
    let ws = "sync-idempotent-replay";
    let (edge, hub) = linked_edge_and_hub().await;
    let p = principal(ws, &["bus:chan/general:pub", "bus:chan/general:sub"]);

    post(
        &edge,
        &p,
        ws,
        "general",
        Item::new("only", "general", "user:edge", "once", 1),
    )
    .await
    .expect("edge posts");

    let sync = sync_channel(&hub.bus, &hub.store, ws, "general")
        .await
        .expect("hub syncs");

    // Replay the SAME item twice; the hub applies both, but the inbox upserts on (channel,id).
    for _ in 0..2 {
        replay_history(&edge.bus, &edge.store, ws, "general")
            .await
            .expect("replay");
    }
    let applied = drain(&sync, 2).await; // two apply events…
    assert_eq!(applied, 2, "the hub receives both replays (at-least-once)");

    // …but the merged history has exactly ONE row (idempotent merge, §6.8).
    let reader = principal(ws, &["bus:chan/general:sub"]);
    let after = history(&hub.store, &reader, ws, "general")
        .await
        .expect("hub reads");
    assert_eq!(
        after.len(),
        1,
        "duplicate replay must NOT duplicate the row"
    );
    assert_eq!(after[0].body, "once");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn sync_never_crosses_the_workspace_wall() {
    // Isolation across nodes via the SYNC path (the other surface the task calls out): an edge
    // in workspace A replaying must NEVER land in a hub's workspace B. The bus keys are
    // workspace-scoped, so the hub's ws_b sync subscription cannot match ws_a's replay keys.
    let ws_a = "sync-iso-a";
    let ws_b = "sync-iso-b";
    let (edge, hub) = linked_edge_and_hub().await;
    let p_a = principal(ws_a, &["bus:chan/general:pub", "bus:chan/general:sub"]);

    // Hub syncs workspace B; edge posts + replays in workspace A.
    let sync_b = sync_channel(&hub.bus, &hub.store, ws_b, "general")
        .await
        .expect("hub syncs ws_b");
    post(
        &edge,
        &p_a,
        ws_a,
        "general",
        Item::new("m1", "general", "user:a", "ws_a only", 1),
    )
    .await
    .expect("edge posts in ws_a");
    replay_history(&edge.bus, &edge.store, ws_a, "general")
        .await
        .expect("edge replays ws_a");

    // The hub's ws_b sync must apply NOTHING — ws_a's items can't cross into ws_b.
    let applied = drain(&sync_b, 1).await;
    assert_eq!(
        applied, 0,
        "SYNC LEAK: ws_a items crossed into the hub's ws_b"
    );

    let reader_b = principal(ws_b, &["bus:chan/general:sub"]);
    let hub_b = history(&hub.store, &reader_b, ws_b, "general")
        .await
        .expect("hub reads ws_b");
    assert!(hub_b.is_empty(), "hub's ws_b must stay empty");
}
