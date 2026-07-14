//! Series lifecycle (data-console scope) through the MCP bridge: `series.delete` removes a whole
//! series' footprint (samples + tag edges, so `series.find` no longer returns it), and
//! `series.rename` carries samples AND tag edges to a new name while refusing a merge into an
//! occupied name. Plus the MANDATORY capability-deny and workspace-isolation tests (testing §2.2):
//! a caller without the destructive cap is refused, and a ws-B token cannot delete/rename ws-A's
//! series — gate 1 (workspace) fires before the capability (§3.6). Real store, no mocks (rule 9).

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

/// Every ingest cap a lifecycle test needs: write + read/find/latest + delete + rename.
const ALL: &[&str] = &[
    "mcp:ingest.write:call",
    "mcp:series.read:call",
    "mcp:series.latest:call",
    "mcp:series.find:call",
    "mcp:series.list:call",
    "mcp:series.delete:call",
    "mcp:series.rename:call",
];

/// A sample carrying a `host` label — so commit lays a tag edge on the `series:<name>` entity and
/// `series.find` can prove the edge is present (then gone after delete / moved after rename).
fn labeled(series: &str, seq: u64, payload: serde_json::Value, host: &str) -> serde_json::Value {
    json!({
        "series": series, "producer": "x", "ts": seq, "seq": seq,
        "payload": payload, "qos": "must-deliver", "labels": { "host": host },
    })
}

async fn write(store: &Store, p: &Principal, ws: &str, sample: serde_json::Value) {
    call_ingest_tool(
        store,
        p,
        ws,
        "ingest.write",
        &json!({ "samples": [sample] }),
    )
    .await
    .unwrap();
    drain_workspace(store, ws).await.unwrap();
}

async fn find_by_host(store: &Store, p: &Principal, ws: &str, host: &str) -> Vec<String> {
    let out = call_ingest_tool(
        store,
        p,
        ws,
        "series.find",
        &json!({ "facets": [{ "key": "host", "value": host }] }),
    )
    .await
    .unwrap();
    serde_json::from_value(out["series"].clone()).unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn delete_removes_samples_and_tag_edges() {
    let store = Store::memory().await.unwrap();
    let p = principal("prod", "acme", ALL);
    write(&store, &p, "acme", labeled("temp", 1, json!(21), "pi-7")).await;

    // Present before: sample readable, series listed, and discoverable by its tag.
    let read = call_ingest_tool(
        &store,
        &p,
        "acme",
        "series.read",
        &json!({ "series": "temp" }),
    )
    .await
    .unwrap();
    assert_eq!(read["samples"].as_array().unwrap().len(), 1);
    assert_eq!(
        find_by_host(&store, &p, "acme", "pi-7").await,
        vec!["series:temp"]
    );

    // Delete the whole series.
    let out = call_ingest_tool(
        &store,
        &p,
        "acme",
        "series.delete",
        &json!({ "series": "temp" }),
    )
    .await
    .unwrap();
    assert_eq!(out["ok"], json!(true));

    // Gone: no samples, not listed, and the tag edge is cleared (find returns nothing).
    let read = call_ingest_tool(
        &store,
        &p,
        "acme",
        "series.read",
        &json!({ "series": "temp" }),
    )
    .await
    .unwrap();
    assert!(
        read["samples"].as_array().unwrap().is_empty(),
        "samples gone"
    );
    let list: Vec<String> = serde_json::from_value(
        call_ingest_tool(&store, &p, "acme", "series.list", &json!({}))
            .await
            .unwrap()["series"]
            .clone(),
    )
    .unwrap();
    assert!(!list.contains(&"temp".to_string()), "series delisted");
    assert!(
        find_by_host(&store, &p, "acme", "pi-7").await.is_empty(),
        "tag edge cleared — series.find no longer returns it"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn delete_unknown_series_is_ok() {
    let store = Store::memory().await.unwrap();
    let p = principal("prod", "acme", ALL);
    // Idempotent: deleting a series that never existed succeeds (no-op), not an error.
    let out = call_ingest_tool(
        &store,
        &p,
        "acme",
        "series.delete",
        &json!({ "series": "ghost" }),
    )
    .await
    .unwrap();
    assert_eq!(out["ok"], json!(true));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn rename_carries_samples_and_tags() {
    let store = Store::memory().await.unwrap();
    let p = principal("prod", "acme", ALL);
    write(
        &store,
        &p,
        "acme",
        labeled("old.name", 1, json!(21), "pi-7"),
    )
    .await;
    write(
        &store,
        &p,
        "acme",
        labeled("old.name", 2, json!(22), "pi-7"),
    )
    .await;

    let out = call_ingest_tool(
        &store,
        &p,
        "acme",
        "series.rename",
        &json!({ "from": "old.name", "to": "new.name" }),
    )
    .await
    .unwrap();
    assert_eq!(out["ok"], json!(true));

    // The old name is empty; the new name holds both samples and the moved tag edge.
    let old = call_ingest_tool(
        &store,
        &p,
        "acme",
        "series.read",
        &json!({ "series": "old.name" }),
    )
    .await
    .unwrap();
    assert!(
        old["samples"].as_array().unwrap().is_empty(),
        "old name emptied"
    );
    let new = call_ingest_tool(
        &store,
        &p,
        "acme",
        "series.read",
        &json!({ "series": "new.name" }),
    )
    .await
    .unwrap();
    assert_eq!(
        new["samples"].as_array().unwrap().len(),
        2,
        "both samples carried"
    );
    assert_eq!(
        find_by_host(&store, &p, "acme", "pi-7").await,
        vec!["series:new.name"],
        "tag edge re-pointed to the new entity"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn rename_into_occupied_name_is_refused() {
    let store = Store::memory().await.unwrap();
    let p = principal("prod", "acme", ALL);
    write(&store, &p, "acme", labeled("a", 1, json!(1), "h")).await;
    write(&store, &p, "acme", labeled("b", 1, json!(2), "h")).await;

    // `b` already exists → refused (no silent merge). BadInput, not Denied (a client error).
    let err = call_ingest_tool(
        &store,
        &p,
        "acme",
        "series.rename",
        &json!({ "from": "a", "to": "b" }),
    )
    .await
    .unwrap_err();
    assert!(
        matches!(err, ToolError::BadInput(_)),
        "occupied target refused"
    );

    // Both series are intact — the refusal touched nothing.
    for s in ["a", "b"] {
        let read = call_ingest_tool(&store, &p, "acme", "series.read", &json!({ "series": s }))
            .await
            .unwrap();
        assert_eq!(read["samples"].as_array().unwrap().len(), 1, "{s} intact");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn delete_and_rename_denied_without_cap() {
    let store = Store::memory().await.unwrap();
    // A writer WITHOUT the destructive caps: can write, cannot delete or rename.
    let writer = principal(
        "prod",
        "acme",
        &["mcp:ingest.write:call", "mcp:series.read:call"],
    );
    write(&store, &writer, "acme", labeled("temp", 1, json!(1), "h")).await;

    let del = call_ingest_tool(
        &store,
        &writer,
        "acme",
        "series.delete",
        &json!({ "series": "temp" }),
    )
    .await
    .unwrap_err();
    assert!(matches!(del, ToolError::Denied), "delete needs its own cap");

    let ren = call_ingest_tool(
        &store,
        &writer,
        "acme",
        "series.rename",
        &json!({ "from": "temp", "to": "temp2" }),
    )
    .await
    .unwrap_err();
    assert!(matches!(ren, ToolError::Denied), "rename needs its own cap");

    // The series survived both denied attempts.
    let read = call_ingest_tool(
        &store,
        &writer,
        "acme",
        "series.read",
        &json!({ "series": "temp" }),
    )
    .await
    .unwrap();
    assert_eq!(
        read["samples"].as_array().unwrap().len(),
        1,
        "series untouched"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn ws_b_cannot_delete_or_rename_ws_a_series() {
    let store = Store::memory().await.unwrap();
    let a = principal("prod", "ws-a", ALL);
    write(&store, &a, "ws-a", labeled("secret", 1, json!(42), "h")).await;

    // A ws-B token (fully capped in B) aimed at ws-A is refused at gate 1 (workspace), opaque.
    let b = principal("prod", "ws-b", ALL);
    let del = call_ingest_tool(
        &store,
        &b,
        "ws-a",
        "series.delete",
        &json!({ "series": "secret" }),
    )
    .await
    .unwrap_err();
    assert!(matches!(del, ToolError::Denied));
    let ren = call_ingest_tool(
        &store,
        &b,
        "ws-a",
        "series.rename",
        &json!({ "from": "secret", "to": "leaked" }),
    )
    .await
    .unwrap_err();
    assert!(matches!(ren, ToolError::Denied));

    // ws-A's series is untouched by the cross-workspace attempts.
    let read = call_ingest_tool(
        &store,
        &a,
        "ws-a",
        "series.read",
        &json!({ "series": "secret" }),
    )
    .await
    .unwrap();
    assert_eq!(
        read["samples"].as_array().unwrap().len(),
        1,
        "ws-A series intact"
    );
}
