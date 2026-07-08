//! The generic bus pub/sub service, headless (widget-config-vars scope, "Platform fix"). Proves the
//! mandatory categories against a real `Bus`: capability-deny per verb, the workspace-wall subject guard
//! (a reserved prefix / cross-ws / escape attempt is refused), and a publish→watch round-trip within one
//! workspace. Real Zenoh `Bus::peer()` — no mock (CLAUDE §9). Single worker for a deterministic mesh.

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_bus::Bus;
use lb_host::{bus_publish, bus_watch, call_bus_tool, wall_subject};
use lb_mcp::ToolError;
use serde_json::json;
use std::time::Duration;

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

const ALL: &[&str] = &["mcp:bus.publish:call", "mcp:bus.watch:call"];

// The subject wall is pure logic — assert it bites BEFORE any bus call (the structural guard, rule 6).
#[test]
fn wall_subject_namespaces_under_ext_and_refuses_reserved_or_escaping_subjects() {
    // A plain subject is namespaced under `ext/` (the `ws/{id}/` wall is added by the bus layer).
    assert_eq!(wall_subject("cooler/alerts").unwrap(), "ext/cooler/alerts");
    // Reserved platform prefixes are refused (a caller can't impersonate series/channel/internal motion).
    for bad in [
        "series/cpu",
        "channels/x",
        "internal/y",
        "ws/other/series/x",
        "presence/z",
    ] {
        assert!(
            wall_subject(bad).is_err(),
            "reserved must be refused: {bad}"
        );
    }
    // Escape attempts are refused.
    for bad in ["", "  ", "/abs", "a/../b"] {
        assert!(
            wall_subject(bad).is_err(),
            "escape must be refused: {bad:?}"
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn publish_without_the_cap_is_denied_opaque() {
    let bus = Bus::peer().await.unwrap();
    let nobody = principal("user:nobody", "ws-bus-deny", &[]);
    // Neither the direct verb nor the MCP bridge leaks anything but an opaque deny.
    assert!(matches!(
        bus_publish(&bus, &nobody, "ws-bus-deny", "x", b"{}").await,
        Err(lb_host::BusError::Denied)
    ));
    let err = call_bus_tool(
        &bus,
        &nobody,
        "ws-bus-deny",
        "bus.publish",
        &json!({ "subject": "x", "payload": {} }),
    )
    .await
    .unwrap_err();
    assert!(matches!(err, ToolError::Denied));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn watch_without_the_cap_is_denied() {
    let bus = Bus::peer().await.unwrap();
    let nobody = principal("user:nobody", "ws-bus-deny2", &[]);
    assert!(matches!(
        bus_watch(&bus, &nobody, "ws-bus-deny2", "x").await,
        Err(lb_host::BusError::Denied)
    ));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_reserved_or_cross_ws_subject_is_refused_even_with_the_cap() {
    let bus = Bus::peer().await.unwrap();
    let ada = principal("user:ada", "ws-bus-wall", ALL);
    // A reserved prefix is refused with the cap held (the wall, not the cap, bites here).
    assert!(matches!(
        bus_publish(&bus, &ada, "ws-bus-wall", "series/cpu", b"{}").await,
        Err(lb_host::BusError::BadSubject(_))
    ));
    // The subject can NEVER name another workspace — it is a suffix walled under the caller's ws; a
    // `ws/...` subject is reserved-refused, so a cross-ws name is structurally impossible.
    assert!(matches!(
        bus_watch(&bus, &ada, "ws-bus-wall", "ws/ws-other/secret").await,
        Err(lb_host::BusError::BadSubject(_))
    ));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn publish_watch_round_trips_within_one_workspace() {
    let bus = Bus::peer().await.unwrap();
    let ada = principal("user:ada", "ws-bus-rt", ALL);

    // Subscribe FIRST (Zenoh pub/sub is not durable — the sub must exist before the publish).
    let sub = bus_watch(&bus, &ada, "ws-bus-rt", "cooler/alerts")
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(200)).await;

    let payload = serde_json::to_vec(&json!({ "msg": "defrost" })).unwrap();
    bus_publish(&bus, &ada, "ws-bus-rt", "cooler/alerts", &payload)
        .await
        .unwrap();

    let got = tokio::time::timeout(Duration::from_secs(5), sub.recv())
        .await
        .expect("a published frame arrives within 5s")
        .expect("a payload, not a closed stream");
    let value: serde_json::Value = serde_json::from_slice(&got).unwrap();
    assert_eq!(value["msg"], "defrost");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn ws_b_does_not_receive_ws_a_publish() {
    let bus = Bus::peer().await.unwrap();
    let a = principal("user:ada", "ws-bus-a", ALL);
    let b = principal("user:ben", "ws-bus-b", ALL);

    // ben (ws-b) watches the SAME relative subject; the `ws/{id}/` wall makes it a different bus key.
    let sub_b = bus_watch(&bus, &b, "ws-bus-b", "cooler/alerts")
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(200)).await;

    bus_publish(&bus, &a, "ws-bus-a", "cooler/alerts", b"{\"x\":1}")
        .await
        .unwrap();

    // ben must NOT receive ada's publish — a short timeout elapses with nothing (the wall holds).
    let crossed = tokio::time::timeout(Duration::from_millis(700), sub_b.recv()).await;
    assert!(crossed.is_err(), "ws-B must not receive ws-A's publish");
}
