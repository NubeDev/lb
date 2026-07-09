//! Mandatory workspace-isolation for ingest (testing §2.2): a ws-B producer cannot write or read a
//! ws-A series, even with a matching capability whose token is scoped to B. Gate 1 (workspace) fires
//! before the capability is consulted (§3.6). Plus the two-producer collision: producer-A and
//! producer-B both writing seq=5 to ONE series — BOTH survive (the (series, producer, seq) key).

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{call_ingest_tool, drain_workspace};
use lb_mcp::ToolError;
use lb_store::Store;
use serde_json::json;

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

const ALL: &[&str] = &[
    "mcp:ingest.write:call",
    "mcp:series.read:call",
    "mcp:series.latest:call",
];

fn sample(series: &str, seq: u64, payload: serde_json::Value) -> serde_json::Value {
    json!({ "series": series, "producer": "x", "ts": seq, "seq": seq, "payload": payload, "qos": "must-deliver" })
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn ws_b_cannot_read_ws_a_series() {
    let store = Store::memory().await.unwrap();
    // ws-A producer writes and the series commits in A.
    let a = principal("client:a", "ws-a", ALL);
    call_ingest_tool(
        &store,
        &a,
        "ws-a",
        "ingest.write",
        &json!({ "samples": [sample("secret", 1, json!(42))] }),
    )
    .await
    .unwrap();
    drain_workspace(&store, "ws-a").await.unwrap();

    // A ws-B token (same identity-shaped sub) reading B's own "secret" series sees nothing of A's.
    let b = principal("client:a", "ws-b", ALL);
    let read = call_ingest_tool(
        &store,
        &b,
        "ws-b",
        "series.read",
        &json!({ "series": "secret" }),
    )
    .await
    .unwrap();
    assert!(
        read["samples"].as_array().unwrap().is_empty(),
        "ws-B must not see ws-A samples"
    );

    // And a ws-B token asking for ws-A's namespace is refused at gate 1 (workspace), opaque Denied.
    let cross = call_ingest_tool(
        &store,
        &b,
        "ws-a",
        "series.read",
        &json!({ "series": "secret" }),
    )
    .await
    .unwrap_err();
    assert!(matches!(cross, ToolError::Denied));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn ws_b_cannot_write_ws_a_series() {
    let store = Store::memory().await.unwrap();
    // ws-B token attempting to write into ws-A is denied at gate 1.
    let b = principal("client:b", "ws-b", ALL);
    let err = call_ingest_tool(
        &store,
        &b,
        "ws-a",
        "ingest.write",
        &json!({ "samples": [sample("m", 1, json!(1))] }),
    )
    .await
    .unwrap_err();
    assert!(matches!(err, ToolError::Denied));

    // Nothing landed in ws-A: an admin in A reads an empty series.
    let admin_a = principal("admin", "ws-a", ALL);
    drain_workspace(&store, "ws-a").await.unwrap();
    let read = call_ingest_tool(
        &store,
        &admin_a,
        "ws-a",
        "series.read",
        &json!({ "series": "m" }),
    )
    .await
    .unwrap();
    assert!(read["samples"].as_array().unwrap().is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn two_producers_same_seq_both_survive_via_host() {
    // The dedup identity is (series, producer, seq). Two principals write seq=5 to one series; the
    // producer is stamped from each principal, so BOTH rows survive.
    let store = Store::memory().await.unwrap();
    let pa = principal("prod-a", "acme", ALL);
    let pb = principal("prod-b", "acme", ALL);
    call_ingest_tool(
        &store,
        &pa,
        "acme",
        "ingest.write",
        &json!({ "samples": [sample("shared", 5, json!("a"))] }),
    )
    .await
    .unwrap();
    call_ingest_tool(
        &store,
        &pb,
        "acme",
        "ingest.write",
        &json!({ "samples": [sample("shared", 5, json!("b"))] }),
    )
    .await
    .unwrap();
    drain_workspace(&store, "acme").await.unwrap();

    let read = call_ingest_tool(
        &store,
        &pa,
        "acme",
        "series.read",
        &json!({ "series": "shared" }),
    )
    .await
    .unwrap();
    let samples = read["samples"].as_array().unwrap();
    assert_eq!(samples.len(), 2, "both producers' seq=5 survive");
}
