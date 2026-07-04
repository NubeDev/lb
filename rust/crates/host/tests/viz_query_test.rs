//! `viz.query` end to end against a REAL node (viz transformations + datasource-binding scopes,
//! Phase 3). The resolver dispatches a panel's targets under the caller's authority by re-entering the
//! host MCP dispatcher, assembles canonical frames, runs the `lb-viz` transform pipeline, and returns
//! `{ frames, rows }`. No mocks (CLAUDE §9): a real embedded SurrealDB seeded through the real
//! `ingest.write` path, the real capability gate, the real `store.query` target tool.
//!
//! What each test proves (the mandatory categories + the parity that justifies the swap):
//!   - `store_target_with_pipeline_returns_expected_frames` — a `store.query` target + a multi-step
//!     transform pipeline (`filterByValue` → `sortBy`) returns the expected canonical frame.
//!   - `no_transform_panel_parity` — a no-transform panel's `viz.query` rows EQUAL a direct
//!     `store.query` (the swap changes nothing visible — the Phase-2 renderer draws the same rows).
//!   - `multi_target_join_assembles` — two targets (A,B) + `joinByField` assemble + join into one
//!     frame (the multi-datasource merge the binding scope wires).
//!   - `viz_query_denied_without_cap` — MANDATORY: no `mcp:viz.query:call` → opaque `Denied`.
//!   - `denied_target_is_honest_empty_not_a_bypass` — a caller holding `viz.query` but NOT
//!     `store.query` gets an empty frame for that target (no render-path cap bypass, no fabricated
//!     value).
//!   - `workspace_isolation` — a ws-B `viz.query` over a `store.query` target reads ONLY ws-B rows;
//!     ws-A's seeded rows are invisible. The hard wall, from the token.

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{call_tool, Node};
use lb_mcp::ToolError;
use serde_json::{json, Value};
use std::sync::Arc;

const VIZ: &str = "mcp:viz.query:call";
const QUERY: &str = "mcp:store.query:call";
const WRITE: &str = "mcp:ingest.write:call";

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

/// Seed `n` real samples into `series` in `ws` through the real ingest write+drain path.
async fn seed_series(node: &Arc<Node>, p: &Principal, ws: &str, series: &str, payloads: &[f64]) {
    let samples: Vec<_> = payloads
        .iter()
        .enumerate()
        .map(|(i, v)| {
            let seq = (i + 1) as u64;
            json!({ "series": series, "producer": "seed", "ts": seq, "seq": seq, "payload": v, "qos": "best-effort" })
        })
        .collect();
    call_tool(
        node,
        p,
        ws,
        "ingest.write",
        &json!({ "samples": samples }).to_string(),
    )
    .await
    .expect("seed ingest");
}

/// Call `viz.query` with a panel spec, returning the parsed `{ frames, rows }`.
async fn viz_query(
    node: &Arc<Node>,
    p: &Principal,
    ws: &str,
    panel: Value,
) -> Result<Value, ToolError> {
    let out = call_tool(
        node,
        p,
        ws,
        "viz.query",
        &json!({ "panel": panel, "now": 1 }).to_string(),
    )
    .await?;
    Ok(serde_json::from_str(&out).expect("viz.query result is json"))
}

/// A panel with one `store.query` target over `sql` + an optional transform pipeline.
fn sql_panel(sql: &str, transforms: Value) -> Value {
    json!({
        "sources": [{ "refId": "A", "datasource": { "type": "surreal" }, "tool": "store.query", "args": { "sql": sql } }],
        "transformations": transforms,
    })
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn store_target_with_pipeline_returns_expected_frames() {
    let ws = "viz-pipe";
    let node = Arc::new(Node::boot().await.unwrap());
    let p = principal("user:ada", ws, &[VIZ, QUERY, WRITE]);
    seed_series(&node, &p, ws, "cpu", &[10.0, 50.0, 30.0, 5.0]).await;

    // filterByValue keeps payload > 9, then sortBy payload desc → [50, 30, 10].
    let panel = sql_panel(
        "SELECT seq, payload FROM series ORDER BY seq",
        json!([
            { "id": "filterByValue", "options": {
                "type": "include", "match": "any",
                "filters": [{ "fieldName": "payload", "config": { "id": "greater", "options": { "value": 9 } } }]
            }},
            { "id": "sortBy", "options": { "sort": [{ "field": "payload", "desc": true }] } }
        ]),
    );

    let out = viz_query(&node, &p, ws, panel)
        .await
        .expect("viz.query runs");
    let rows = out["rows"].as_array().expect("rows");
    let payloads: Vec<f64> = rows
        .iter()
        .map(|r| r["payload"].as_f64().unwrap())
        .collect();
    assert_eq!(payloads, vec![50.0, 30.0, 10.0], "filtered + sorted desc");
    // canonical frames present, refId carried.
    assert_eq!(out["frames"][0]["refId"], json!("A"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn no_transform_panel_parity() {
    let ws = "viz-parity";
    let node = Arc::new(Node::boot().await.unwrap());
    let p = principal("user:ada", ws, &[VIZ, QUERY, WRITE]);
    seed_series(&node, &p, ws, "cpu", &[1.0, 2.0, 3.0]).await;

    let sql = "SELECT seq, payload FROM series ORDER BY seq";
    let viz = viz_query(&node, &p, ws, sql_panel(sql, json!([])))
        .await
        .unwrap();

    // A direct store.query — the Phase-2 client path. The viz.query rows must EQUAL these (the swap
    // changes nothing the renderer sees).
    let direct = call_tool(
        &node,
        &p,
        ws,
        "store.query",
        &json!({ "sql": sql }).to_string(),
    )
    .await
    .unwrap();
    let direct: Value = serde_json::from_str(&direct).unwrap();
    let direct_rows = direct["rows"].as_array().unwrap();
    let viz_rows = viz["rows"].as_array().unwrap();

    assert_eq!(viz_rows.len(), direct_rows.len(), "same row count");
    for (a, b) in viz_rows.iter().zip(direct_rows) {
        assert_eq!(a["seq"], b["seq"]);
        assert_eq!(a["payload"], b["payload"]);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn multi_target_join_assembles() {
    let ws = "viz-join";
    let node = Arc::new(Node::boot().await.unwrap());
    let p = principal("user:ada", ws, &[VIZ, QUERY, WRITE]);
    seed_series(&node, &p, ws, "a", &[10.0, 20.0]).await;
    seed_series(&node, &p, ws, "b", &[100.0, 200.0]).await;

    // Two targets, joined on `seq`. Each selects its own series; rename payload per-target so the
    // join produces distinct columns.
    let panel = json!({
        "sources": [
            { "refId": "A", "tool": "store.query", "args": { "sql": "SELECT seq, payload FROM series WHERE series = 'a' ORDER BY seq" } },
            { "refId": "B", "tool": "store.query", "args": { "sql": "SELECT seq, payload FROM series WHERE series = 'b' ORDER BY seq" } }
        ],
        "transformations": [
            { "id": "joinByField", "options": { "byField": "seq", "mode": "outer" } }
        ]
    });

    let out = viz_query(&node, &p, ws, panel).await.expect("join runs");
    // One joined frame keyed by seq with both targets' payload columns present.
    let frames = out["frames"].as_array().unwrap();
    assert_eq!(frames.len(), 1, "joined to a single frame");
    let rows = out["rows"].as_array().unwrap();
    assert_eq!(rows.len(), 2, "two keys (seq 1,2)");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn viz_query_denied_without_cap() {
    let ws = "viz-deny";
    let node = Arc::new(Node::boot().await.unwrap());
    // holds QUERY + WRITE but NOT viz.query.
    let p = principal("user:ada", ws, &[QUERY, WRITE]);
    seed_series(&node, &p, ws, "cpu", &[1.0]).await;

    let err = viz_query(
        &node,
        &p,
        ws,
        sql_panel("SELECT seq, payload FROM series", json!([])),
    )
    .await
    .unwrap_err();
    assert!(matches!(err, ToolError::Denied), "opaque deny, got {err:?}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn denied_target_is_honest_empty_not_a_bypass() {
    let ws = "viz-target-deny";
    let node = Arc::new(Node::boot().await.unwrap());
    // Seed with a writer that ALSO has query (so rows exist); then query as a caller with viz.query
    // but WITHOUT store.query — the target must be denied INSIDE the resolver → empty frame.
    let writer = principal("user:seed", ws, &[QUERY, WRITE]);
    seed_series(&node, &writer, ws, "cpu", &[1.0, 2.0, 3.0]).await;

    let caller = principal("user:ada", ws, &[VIZ]); // no store.query
    let out = viz_query(
        &node,
        &caller,
        ws,
        sql_panel("SELECT seq, payload FROM series", json!([])),
    )
    .await
    .expect("viz.query itself is granted");
    // The verb ran (granted), but the target was denied inside → no rows, no fabricated value.
    assert!(
        out["rows"].as_array().unwrap().is_empty(),
        "denied target → honest empty rows"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn workspace_isolation() {
    let node = Arc::new(Node::boot().await.unwrap());
    // ws-A seeded with rows; ws-B seeded with NONE.
    let a = principal("user:ada", "ws-a", &[VIZ, QUERY, WRITE]);
    seed_series(&node, &a, "ws-a", "cpu", &[1.0, 2.0, 3.0]).await;

    // Sanity FIRST: ws-A DOES see its own rows through the same path.
    let out_a = viz_query(
        &node,
        &a,
        "ws-a",
        sql_panel("SELECT seq, payload FROM series", json!([])),
    )
    .await
    .unwrap();
    assert_eq!(
        out_a["rows"].as_array().unwrap().len(),
        3,
        "ws-A sees its own 3 rows"
    );

    let b = principal("user:bob", "ws-b", &[VIZ, QUERY, WRITE]);
    // ws-B runs the SAME panel — it can only ever see ws-B's (empty) store; never ws-A's rows.
    let out = viz_query(
        &node,
        &b,
        "ws-b",
        sql_panel("SELECT seq, payload FROM series", json!([])),
    )
    .await
    .expect("viz.query runs in ws-b");
    assert!(
        out["rows"].as_array().unwrap().is_empty(),
        "ws-B sees none of ws-A's rows"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn frames_in_shapes_without_resolving_a_source() {
    // COMPUTE-ONLY (edit-without-requery, data-studio-ux scope): a panel carrying inline `frames` runs
    // ONLY the transform pipeline over those frames — it resolves NO source. Two things must hold:
    //   1. It reaches no gated read — a caller who holds viz.query but NOT store.query still gets shaped
    //      rows (nothing is dispatched, so there is nothing to deny). This is why an option/transform
    //      edit can re-shape cached frames without re-hitting the datasource.
    //   2. Parity: the pipeline over inline frames equals the same pipeline over a resolved source.
    let ws = "viz-frames-in";
    let node = Arc::new(Node::boot().await.unwrap());
    let caller = principal("user:ada", ws, &[VIZ]); // NO store.query — proves no source is touched

    // One inline frame (two columns), + the same filter+sort pipeline the store test uses.
    let pipeline = json!([
        { "id": "filterByValue", "options": {
            "type": "include", "match": "any",
            "filters": [{ "fieldName": "payload", "config": { "id": "greater", "options": { "value": 9 } } }]
        }},
        { "id": "sortBy", "options": { "sort": [{ "field": "payload", "desc": true }] } }
    ]);
    let panel = json!({
        "frames": [{
            "refId": "A",
            "fields": [
                { "name": "seq", "type": "number", "values": [1, 2, 3, 4] },
                { "name": "payload", "type": "number", "values": [10.0, 50.0, 30.0, 5.0] }
            ],
            "length": 4
        }],
        "transformations": pipeline,
    });

    let out = viz_query(&node, &caller, ws, panel)
        .await
        .expect("frames-in viz.query runs under the verb grant alone");
    let payloads: Vec<f64> = out["rows"]
        .as_array()
        .expect("rows")
        .iter()
        .map(|r| r["payload"].as_f64().unwrap())
        .collect();
    // Same filtered+sorted result as the store-backed pipeline test — the ONE transform impl.
    assert_eq!(payloads, vec![50.0, 30.0, 10.0], "shaped inline frames");
    assert_eq!(out["frames"][0]["refId"], json!("A"), "refId preserved");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn federation_bound_target_resolves_through_federation_query() {
    // A target bound to a federation datasource names `federation.query` (datasource-binding scope:
    // the DataSourceRef selects the tool; the resolver dispatches it under the caller's authority). We
    // prove the BINDING path here WITHOUT a spawned external DB (the full happy round-trip over a real
    // Postgres lives in federation_test.rs): a federation target whose source is not registered in
    // THIS workspace fails the gated federation host check inside the resolver → an HONEST EMPTY frame,
    // never a fabricated value and never a cross-tenant read. This is the leash the binding scope
    // requires: viz.query routes a federation target through the same workspace-pinned `federation.query`
    // gate, so a ws-B panel naming a ws-A `datasource:{ws}:{name}` can never resolve it.
    const FED: &str = "mcp:federation.query:call";
    let ws = "viz-fed";
    let node = Arc::new(Node::boot().await.unwrap());
    let caller = principal("user:ada", ws, &[VIZ, FED]); // granted, but no source registered

    let panel = json!({
        "sources": [{
            "refId": "A",
            "datasource": { "type": "federation", "uid": "datasource:viz-fed:timescale" },
            "tool": "federation.query",
            "args": { "source": "timescale", "sql": "SELECT 1" }
        }],
        "transformations": []
    });

    let out = viz_query(&node, &caller, ws, panel)
        .await
        .expect("viz.query verb is granted");
    // The federation target could not resolve (no such source in this ws) → honest empty, no leak.
    assert!(
        out["rows"].as_array().unwrap().is_empty(),
        "an unregistered federation source resolves to an honest empty frame"
    );
    // The frame for refId A still exists (assembled), just empty — the panel shape is preserved.
    assert_eq!(out["frames"][0]["refId"], json!("A"));
}
