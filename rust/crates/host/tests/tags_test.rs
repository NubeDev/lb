//! Tags at the host layer: the MCP-surface round-trip + the mandatory per-verb deny test, plus
//! series.find discovery built on the tag graph (tags + ingest scopes).

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{call_ingest_tool, call_tags_tool, drain_workspace};
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

const ALL: &[&str] = &[
    "mcp:tags.add:call",
    "mcp:tags.remove:call",
    "mcp:tags.of:call",
    "mcp:tags.find:call",
];

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn add_of_find_round_trip_via_mcp() {
    let store = Store::memory().await.unwrap();
    let p = principal("user:ada", "acme", ALL);

    call_tags_tool(
        &store,
        &p,
        "acme",
        "tags.add",
        &json!({ "entity": "series:cpu", "key": "region", "value": "eu", "source": "producer" }),
    )
    .await
    .unwrap();
    call_tags_tool(
        &store,
        &p,
        "acme",
        "tags.add",
        &json!({ "entity": "series:cpu", "key": "kind", "value": "telemetry" }),
    )
    .await
    .unwrap();

    let of = call_tags_tool(
        &store,
        &p,
        "acme",
        "tags.of",
        &json!({ "entity": "series:cpu" }),
    )
    .await
    .unwrap();
    assert_eq!(of["tags"].as_array().unwrap().len(), 2);

    let found = call_tags_tool(&store, &p, "acme", "tags.find",
        &json!({ "facets": [{"key": "region", "value": "eu"}, {"key": "kind", "value": "telemetry"}] }))
        .await.unwrap();
    assert_eq!(found["entities"], json!(["series:cpu"]));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn denies_each_verb_without_its_grant() {
    let store = Store::memory().await.unwrap();
    // Holds only tags.of — every OTHER verb is denied.
    let p = principal("user:ada", "acme", &["mcp:tags.of:call"]);
    for (verb, input) in [
        (
            "tags.add",
            json!({ "entity": "series:x", "key": "k", "value": "v" }),
        ),
        ("tags.remove", json!({ "entity": "series:x", "key": "k" })),
        ("tags.find", json!({ "facets": [{"key": "k"}] })),
    ] {
        let err = call_tags_tool(&store, &p, "acme", verb, &input)
            .await
            .unwrap_err();
        assert!(matches!(err, ToolError::Denied), "{verb} must be denied");
    }
    // The held verb is allowed (reads empty).
    call_tags_tool(
        &store,
        &p,
        "acme",
        "tags.of",
        &json!({ "entity": "series:x" }),
    )
    .await
    .unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn series_find_discovers_by_tags() {
    let store = Store::memory().await.unwrap();
    let p = principal(
        "client:pi",
        "acme",
        &[
            "mcp:ingest.write:call",
            "mcp:series.read:call",
            "mcp:series.find:call",
            "mcp:tags.add:call",
        ],
    );

    // Commit a series, then tag it.
    call_ingest_tool(&store, &p, "acme", "ingest.write",
        &json!({ "samples": [{"series":"node.cpu_temp","producer":"x","ts":1,"seq":1,"payload":61.4,"qos":"best-effort"}] }))
        .await.unwrap();
    drain_workspace(&store, "acme").await.unwrap();
    call_tags_tool(&store, &p, "acme", "tags.add",
        &json!({ "entity": "series:node.cpu_temp", "key": "region", "value": "eu", "source": "producer" }))
        .await.unwrap();

    // series.find by the tag returns the series entity (and only series entities).
    let found = call_ingest_tool(
        &store,
        &p,
        "acme",
        "series.find",
        &json!({ "facets": [{"key": "region", "value": "eu"}] }),
    )
    .await
    .unwrap();
    assert_eq!(found["series"], json!(["series:node.cpu_temp"]));
}
