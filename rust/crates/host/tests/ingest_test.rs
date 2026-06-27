//! Ingest at the host layer: the MCP-surface round-trip + the mandatory deny test. The producer is
//! stamped from the authenticated principal (un-spoofable), and the durable exactly-once round-trip
//! is proven end to end through `call_ingest_tool`. The `ingest.write` MCP verb drains staging →
//! `series` synchronously (there is no background drain worker; the gateway's `POST /ingest` route
//! drains for the same reason), so a write is visible to the very next read over the same bridge — the
//! round-trip the proof-panel page proves. A subsequent explicit `drain_workspace` is then a no-op
//! (exactly-once per `(series, producer, seq)`), which this test asserts.

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
    };
    let token = mint(&key, &claims);
    verify(&key, &token, 1).expect("token verifies")
}

fn sample(series: &str, seq: u64, payload: serde_json::Value) -> serde_json::Value {
    // producer is overwritten by the host with the authenticated principal — value here is ignored.
    json!({ "series": series, "producer": "ignored", "ts": seq, "seq": seq, "payload": payload, "qos": "best-effort" })
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn write_drain_read_round_trip_via_mcp() {
    let store = Store::memory().await.unwrap();
    let p = principal(
        "client:pi-7",
        "acme",
        &[
            "mcp:ingest.write:call",
            "mcp:series.read:call",
            "mcp:series.latest:call",
        ],
    );

    let out = call_ingest_tool(
        &store,
        &p,
        "acme",
        "ingest.write",
        &json!({ "samples": [sample("cpu", 1, json!(61.4)), sample("cpu", 2, json!(62.0))] }),
    )
    .await
    .unwrap();
    assert_eq!(out["accepted"], 2);

    // `ingest.write` already drained staging → series synchronously, so a SECOND explicit drain finds
    // nothing left to commit — exactly-once, never a double-commit.
    let pass = drain_workspace(&store, "acme").await.unwrap();
    assert_eq!(
        pass.committed, 0,
        "the write already committed; the drain is a no-op"
    );

    let read = call_ingest_tool(
        &store,
        &p,
        "acme",
        "series.read",
        &json!({ "series": "cpu" }),
    )
    .await
    .unwrap();
    let samples = read["samples"].as_array().unwrap();
    assert_eq!(samples.len(), 2);
    // Producer stamped from the authenticated principal, not the wire value.
    assert_eq!(samples[0]["producer"], "client:pi-7");

    let latest = call_ingest_tool(
        &store,
        &p,
        "acme",
        "series.latest",
        &json!({ "series": "cpu" }),
    )
    .await
    .unwrap();
    assert_eq!(latest["sample"]["seq"], 2);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn denies_write_without_capability() {
    let store = Store::memory().await.unwrap();
    // Holds series.read but NOT ingest.write.
    let p = principal("client:pi-7", "acme", &["mcp:series.read:call"]);
    let err = call_ingest_tool(
        &store,
        &p,
        "acme",
        "ingest.write",
        &json!({ "samples": [sample("cpu", 1, json!(1))] }),
    )
    .await
    .unwrap_err();
    assert!(matches!(err, ToolError::Denied), "no grant → Denied");

    // And nothing landed (the deny is before any write).
    let reader = principal("admin", "acme", &["mcp:series.read:call"]);
    let read = call_ingest_tool(
        &store,
        &reader,
        "acme",
        "series.read",
        &json!({ "series": "cpu" }),
    )
    .await
    .unwrap();
    assert!(read["samples"].as_array().unwrap().is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn denies_read_without_capability() {
    let store = Store::memory().await.unwrap();
    let p = principal("client:pi-7", "acme", &["mcp:ingest.write:call"]);
    let err = call_ingest_tool(
        &store,
        &p,
        "acme",
        "series.read",
        &json!({ "series": "cpu" }),
    )
    .await
    .unwrap_err();
    assert!(matches!(err, ToolError::Denied));
}
