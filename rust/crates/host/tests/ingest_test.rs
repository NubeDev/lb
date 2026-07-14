//! Ingest at the host layer: the MCP-surface round-trip + the mandatory deny test. The producer is
//! ROOTED at the authenticated principal — `{principal}` when the caller declares no sub-namespace,
//! `{principal}/{declared}` when it does — so the root is un-spoofable while one principal can still
//! run many independent `seq` spaces (a caller can only ever carve up its own namespace). The
//! durable exactly-once round-trip is proven end to end through `call_ingest_tool`. The
//! `ingest.write` MCP verb drains staging → `series` synchronously (the gateway's `POST /ingest`
//! route drains for the same reason), so a write is visible to the very next read over the same
//! bridge — the round-trip the proof-panel page proves. A subsequent explicit `drain_workspace` is
//! then a no-op (exactly-once per `(series, producer, seq)`), which this test asserts.
//!
//! That synchronous drain is now **bounded to the caller's own batch** and a background reactor
//! (`spawn_ingest_reactors`) owns the backlog — so the round-trip below holds without a caller ever
//! being billed for another producer's staged rows (drain-backpressure scope). The samples here fit
//! one batch, so the write commits them inline exactly as before.

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

/// A sample declaring NO producer sub-namespace — the back-compatible default, so the host stamps
/// the bare authenticated principal.
///
/// The producer is ROOTED at the principal, not simply discarded: `ingest.write` stamps
/// `{principal}` when the caller declares nothing and `{principal}/{declared}` when it does (so one
/// principal can run many independent seq spaces — see `sample_ns` below). An empty declaration is
/// what selects the bare-principal form.
fn sample(series: &str, seq: u64, payload: serde_json::Value) -> serde_json::Value {
    json!({ "series": series, "producer": "", "ts": seq, "seq": seq, "payload": payload, "qos": "best-effort" })
}

/// A sample declaring producer sub-namespace `ns`, which the host roots under the principal.
fn sample_ns(series: &str, ns: &str, seq: u64, payload: serde_json::Value) -> serde_json::Value {
    json!({ "series": series, "producer": ns, "ts": seq, "seq": seq, "payload": payload, "qos": "best-effort" })
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
    // Producer is ROOTED at the authenticated principal. These samples declare no sub-namespace, so
    // the stamp is the bare principal — never the wire value.
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

/// A caller-declared producer is ROOTED under the authenticated principal — never taken verbatim,
/// and never able to name another principal's namespace.
///
/// This is the half the old fixture could not express: it hardcoded `producer: "ignored"` with the
/// comment "value here is ignored", which was true only while the stamp discarded the wire value
/// outright. Rooting it means the declared leaf now REACHES the stored id, so the forgery question
/// ("can a caller shape that leaf to impersonate someone else?") became a real one and needs a real
/// test.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_declared_producer_is_rooted_under_the_principal_and_cannot_forge_another() {
    let store = Store::memory().await.unwrap();
    let p = principal(
        "client:pi-7",
        "acme",
        &["mcp:ingest.write:call", "mcp:series.read:call"],
    );

    let out = call_ingest_tool(
        &store,
        &p,
        "acme",
        "ingest.write",
        &json!({ "samples": [
            // A plain sub-namespace: rides beneath this principal.
            sample_ns("cpu", "epoch-2", 1, json!(1)),
            // A hostile one: tries to break out and pose as another principal's stream. The
            // separator is the only character the caller must not control.
            sample_ns("cpu", "../client:other/epoch-9", 2, json!(2)),
        ] }),
    )
    .await
    .unwrap();
    assert_eq!(out["accepted"], 2);

    let read = call_ingest_tool(
        &store,
        &p,
        "acme",
        "series.read",
        &json!({ "series": "cpu" }),
    )
    .await
    .unwrap();
    let producers: Vec<&str> = read["samples"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["producer"].as_str().unwrap())
        .collect();

    // EVERY stored producer begins with this principal's root — that is the isolation property.
    for got in &producers {
        assert!(
            got.starts_with("client:pi-7"),
            "every producer must be rooted at the authenticated principal, got {got:?}"
        );
    }
    assert!(
        producers.contains(&"client:pi-7/epoch-2"),
        "a declared sub-namespace rides beneath the principal, got {producers:?}"
    );
    // The hostile leaf keeps its text but loses every separator, so it can only ever be ONE level
    // beneath this principal — it cannot become `client:other/...`.
    assert!(
        !producers.iter().any(|g| g.contains("client:other/")),
        "a declared producer must never forge another principal's namespace, got {producers:?}"
    );
    assert!(
        producers
            .iter()
            .all(|g| g.matches('/').count() <= 1 && g.starts_with("client:pi-7")),
        "a declared producer must stay exactly one level beneath its own root, got {producers:?}"
    );
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
