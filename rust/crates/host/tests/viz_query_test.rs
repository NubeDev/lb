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
        constraint: None,
        run_id: None,
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

/// The per-target `status` object on frame `ref_id` (query-diagnostics scope), or `Null` if absent.
fn frame_status(out: &Value, ref_id: &str) -> Value {
    out["frames"]
        .as_array()
        .expect("frames")
        .iter()
        .find(|f| f["refId"] == json!(ref_id))
        .map(|f| f["status"].clone())
        .unwrap_or(Value::Null)
}

// query-diagnostics scope: a valid query with rows → `ok`; a valid query matching 0 rows → `empty`.
// The UI writes its own "0 rows for <range>" off `empty`, and never confuses it with a bug or a deny.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn status_ok_vs_empty() {
    let ws = "viz-status-oe";
    let node = Arc::new(Node::boot().await.unwrap());
    let p = principal("user:ada", ws, &[VIZ, QUERY, WRITE]);
    seed_series(&node, &p, ws, "cpu", &[10.0, 20.0]).await;

    // ≥1 row → ok, no message.
    let out = viz_query(
        &node,
        &p,
        ws,
        sql_panel("SELECT seq, payload FROM series", json!([])),
    )
    .await
    .unwrap();
    assert_eq!(
        frame_status(&out, "A"),
        json!({ "state": "ok" }),
        "rows → ok"
    );

    // Ran, 0 rows → empty, no message.
    let out = viz_query(
        &node,
        &p,
        ws,
        sql_panel(
            "SELECT seq, payload FROM series WHERE payload > 9999",
            json!([]),
        ),
    )
    .await
    .unwrap();
    assert_eq!(
        frame_status(&out, "A"),
        json!({ "state": "empty" }),
        "0 rows → empty"
    );
}

// query-diagnostics scope: a bad query (the caller's OWN SQL) surfaces `error` WITH the downstream
// tool's message — the single papercut this scope fixes — while the frame stays honestly empty (no
// fabricated rows). Today all four outcomes collapsed to the same silent blank.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn status_error_surfaces_message_frame_still_empty() {
    let ws = "viz-status-err";
    let node = Arc::new(Node::boot().await.unwrap());
    let p = principal("user:ada", ws, &[VIZ, QUERY, WRITE]);
    seed_series(&node, &p, ws, "cpu", &[1.0]).await;

    // A syntactically invalid SELECT → the store's parse error → `BadInput` → surfaced as `error`.
    let out = viz_query(&node, &p, ws, sql_panel("SELECT payload FROM", json!([])))
        .await
        .expect("viz.query verb granted; the TARGET errs, not the call");
    let status = frame_status(&out, "A");
    assert_eq!(
        status["state"],
        json!("error"),
        "bad SQL → error, got {status}"
    );
    assert!(
        status["message"].as_str().unwrap_or("").len() > 0,
        "error carries the downstream tool's message, got {status}"
    );
    // No invented rows — a failed target is still an empty frame.
    assert!(
        out["rows"].as_array().unwrap().is_empty(),
        "errored target → empty rows, never fabricated"
    );
}

// query-diagnostics scope (MANDATORY cap-deny + no enumeration oracle): a target the caller lacks the
// cap for → `denied` with NO message, and the status is BYTE-IDENTICAL whether the queried data exists
// or not. A `denied` that varied by existence would be an enumeration oracle; the opacity is the point.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn status_denied_is_opaque_with_no_enumeration_oracle() {
    let node = Arc::new(Node::boot().await.unwrap());

    // ws-full: rows exist; ws-empty: none. A writer seeds ws-full (needs QUERY+WRITE to seed).
    let writer = principal("user:seed", "ws-full", &[QUERY, WRITE]);
    seed_series(&node, &writer, "ws-full", "cpu", &[1.0, 2.0, 3.0]).await;

    // The caller holds viz.query but NOT store.query, in each workspace.
    let caller_full = principal("user:ada", "ws-full", &[VIZ]);
    let caller_empty = principal("user:ada", "ws-empty", &[VIZ]);

    let out_full = viz_query(
        &node,
        &caller_full,
        "ws-full",
        sql_panel("SELECT seq, payload FROM series", json!([])),
    )
    .await
    .expect("viz.query granted");
    let out_empty = viz_query(
        &node,
        &caller_empty,
        "ws-empty",
        sql_panel("SELECT seq, payload FROM series", json!([])),
    )
    .await
    .expect("viz.query granted");

    let s_full = frame_status(&out_full, "A");
    let s_empty = frame_status(&out_empty, "A");
    // denied, message ABSENT (opaque — never reveals a gate or tool existence).
    assert_eq!(
        s_full,
        json!({ "state": "denied" }),
        "denied carries no message"
    );
    // Byte-identical regardless of whether the underlying data exists → no enumeration oracle.
    assert_eq!(
        s_full, s_empty,
        "denied status must not depend on data existence"
    );
    // And still an honest empty frame (no bypass, no fabrication).
    assert!(out_full["rows"].as_array().unwrap().is_empty());
}

// query-diagnostics scope (legacy/compat): the frames-in (compute-only) path resolves NO target, so
// its frames carry NO `status` — a client that ignores status renders identically to before, and a
// `viz.query` responder that omits status still validates (`#[serde(default)]`).
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn status_absent_on_frames_in_path() {
    let ws = "viz-status-fi";
    let node = Arc::new(Node::boot().await.unwrap());
    let caller = principal("user:ada", ws, &[VIZ]);

    let panel = json!({
        "frames": [{
            "refId": "A",
            "fields": [{ "name": "v", "type": "number", "values": [1, 2, 3] }],
            "length": 3
        }],
        "transformations": [],
    });
    let out = viz_query(&node, &caller, ws, panel).await.unwrap();
    assert_eq!(
        frame_status(&out, "A"),
        Value::Null,
        "frames-in resolves no target → no status field (legacy shape)"
    );
}

const RUN: &str = "mcp:rules.run:call";
const SAVE: &str = "mcp:rules.save:call";
const GET: &str = "mcp:rules.get:call";
const RULE_READ: &str = "store:rule:read";
const RULE_WRITE: &str = "store:rule:write";

/// Save a rule by id/body through the real `rules.save` verb.
async fn save_rule(node: &Arc<Node>, p: &Principal, ws: &str, id: &str, body: &str) {
    call_tool(
        node,
        p,
        ws,
        "rules.save",
        &json!({ "id": id, "name": id, "body": body }).to_string(),
    )
    .await
    .expect("rules.save");
}

/// A panel with one `rules.run` target over a saved rule id.
fn rules_panel(rule_id: &str) -> Value {
    json!({
        "sources": [{ "refId": "A", "tool": "rules.run", "args": { "rule_id": rule_id } }],
        "transformations": [],
    })
}

// Slice 1 diagnostic (rules-for-widgets-scope): a panel bound to `{tool:"rules.run"}` must render the
// rule's rows through the recursive dispatch. A rule whose last expression is an array of row maps
// (`RuleOutput::Scalar(value:[…])`) is the scalar-array kind.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn rules_target_scalar_array_renders_rows() {
    let ws = "viz-rules-scalar";
    let node = Arc::new(Node::boot().await.unwrap());
    let caps = &[VIZ, RUN, SAVE, GET, RULE_READ, RULE_WRITE];
    let p = principal("user:ada", ws, caps);
    save_rule(
        &node,
        &p,
        ws,
        "hourly",
        "let rows = [#{ h: 0, v: 10 }, #{ h: 1, v: 20 }, #{ h: 2, v: 30 }]; rows",
    )
    .await;

    let out = viz_query(&node, &p, ws, rules_panel("hourly"))
        .await
        .expect("viz.query runs over a rules.run target");
    let rows = out["rows"].as_array().expect("rows");
    assert_eq!(rows.len(), 3, "the rule's 3 rows render through viz.query");
    assert_eq!(rows[0]["v"], json!(10));
    assert_eq!(rows[2]["v"], json!(30));
}

// MANDATORY (rules-for-widgets-scope Testing plan): a viewer holding `viz.query` but NOT `rules.run`
// gets an honest EMPTY frame for a rules target — no bypass, no 500, no other source's data.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn rules_target_denied_without_run_cap_is_honest_empty() {
    let ws = "viz-rules-deny";
    let node = Arc::new(Node::boot().await.unwrap());
    // A writer with the full grant saves the rule so it exists in the store.
    let writer = principal(
        "user:seed",
        ws,
        &[VIZ, RUN, SAVE, GET, RULE_READ, RULE_WRITE],
    );
    save_rule(&node, &writer, ws, "hourly", "[#{ v: 1 }, #{ v: 2 }]").await;

    // The caller holds viz.query but NOT rules.run → the recursive dispatch denies inside → empty.
    let caller = principal("user:ada", ws, &[VIZ]);
    let out = viz_query(&node, &caller, ws, rules_panel("hourly"))
        .await
        .expect("viz.query itself is granted");
    assert!(
        out["rows"].as_array().unwrap().is_empty(),
        "a denied rules target → honest empty rows"
    );
}

// MANDATORY: a rule saved in ws-A yields NO rows for a ws-B viewer's identical panel (the hard wall,
// re-proven at the render layer — the store read + caps check both refuse before the cage runs).
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn rules_target_workspace_isolation() {
    let node = Arc::new(Node::boot().await.unwrap());
    let caps = &[VIZ, RUN, SAVE, GET, RULE_READ, RULE_WRITE];
    let a = principal("user:ada", "ws-a", caps);
    save_rule(
        &node,
        &a,
        "ws-a",
        "hourly",
        "[#{ v: 1 }, #{ v: 2 }, #{ v: 3 }]",
    )
    .await;

    // Sanity: ws-A sees its own 3 rows through this path.
    let out_a = viz_query(&node, &a, "ws-a", rules_panel("hourly"))
        .await
        .unwrap();
    assert_eq!(
        out_a["rows"].as_array().unwrap().len(),
        3,
        "ws-A sees its rule's rows"
    );

    // ws-B runs the SAME panel (same rule id) — the rule does not exist in ws-B → empty.
    let b = principal("user:bob", "ws-b", caps);
    let out_b = viz_query(&node, &b, "ws-b", rules_panel("hourly"))
        .await
        .expect("viz.query runs in ws-b");
    assert!(
        out_b["rows"].as_array().unwrap().is_empty(),
        "ws-B sees none of ws-A's rule rows"
    );
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

/// Panel time override end to end (grafana-parity-backend P1): `queryOptions.timeFrom` REPLACES the
/// target's range with `[now - timeFrom, now]`, and `timeShift` moves a caller-supplied range
/// earlier — proven against the REAL `series.read` dispatch over really-seeded samples (ts 1..4),
/// not a unit stub. The semantics pin lives in `viz/time_override.rs` + the P1 session doc.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn panel_time_override_applies_to_target_dispatch() {
    const READ: &str = "mcp:series.read:call";
    let ws = "viz-timeover";
    let node = Arc::new(Node::boot().await.unwrap());
    let p = principal("user:ada", ws, &[VIZ, READ, WRITE]);
    seed_series(&node, &p, ws, "cpu", &[10.0, 20.0, 30.0, 40.0]).await; // ts = 1..4

    let series_panel = |query_options: Value, args: Value| {
        json!({
            "sources": [{ "refId": "A", "tool": "series.read", "args": args }],
            "queryOptions": query_options,
        })
    };
    let run = |panel: Value| {
        let node = node.clone();
        let p = p.clone();
        async move {
            let out = call_tool(
                &node,
                &p,
                ws,
                "viz.query",
                &json!({ "panel": panel, "now": 100 }).to_string(),
            )
            .await
            .expect("viz.query runs");
            let out: Value = serde_json::from_str(&out).expect("json");
            out["rows"].as_array().expect("rows").len()
        }
    };

    // NOTE (issue #101): these targets pin `mode:"rows"` so the test stays focused on time-override
    // math — an explicit mode always wins over the panel-resolution bucket injection (a mode-less
    // series.read with a numeric window would now be upgraded to buckets; that path is covered in
    // viz_resolution_test.rs). Override semantics are identical in either mode.

    // Baseline: no override → all 4 seeded rows.
    let n = run(series_panel(
        json!({}),
        json!({ "series": "cpu", "mode": "rows" }),
    ))
    .await;
    assert_eq!(n, 4, "baseline reads every seeded row");

    // timeFrom "50s" at now=100 → range [50, 100]; the seeded ts 1..4 fall OUTSIDE → 0 rows. The
    // override REPLACES even a caller-supplied range (that is what a Grafana panel override does).
    let n = run(series_panel(
        json!({ "timeFrom": "50s" }),
        json!({ "series": "cpu", "mode": "rows", "from": 0, "to": 10 }),
    ))
    .await;
    assert_eq!(n, 0, "timeFrom replaced the range with [50,100]");

    // timeShift "1m" over a caller range [61, 100] → [1, 40]; the seeded rows come back into view.
    let n = run(series_panel(
        json!({ "timeShift": "1m" }),
        json!({ "series": "cpu", "mode": "rows", "from": 61, "to": 100 }),
    ))
    .await;
    assert_eq!(n, 4, "timeShift moved the window back onto the seeded rows");
}

/// Tranche 2a end to end (grafana-parity-backend P2): a `renameByRegex` + `p90` reduce pipeline —
/// the scope's example flow — runs through the REAL `viz.query` over a real `store.query` target on
/// really-seeded rows. And the tranche bound stays honest: an unknown transform id is carried
/// (skip-with-notice), never an error or a mangled frame.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn tranche_2a_pipeline_runs_end_to_end() {
    let ws = "viz-t2a";
    let node = Arc::new(Node::boot().await.unwrap());
    let p = principal("user:ada", ws, &[VIZ, QUERY, WRITE]);
    seed_series(
        &node,
        &p,
        ws,
        "cpu",
        &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0],
    )
    .await;

    let panel = sql_panel(
        "SELECT payload FROM series ORDER BY payload",
        json!([
            { "id": "renameByRegex", "options": { "regex": "payload", "renamePattern": "cpu_load" } },
            { "id": "reduce", "options": { "reducers": ["p90"], "mode": "reduceFields" } }
        ]),
    );
    let out = viz_query(&node, &p, ws, panel).await.expect("runs");
    let rows = out["rows"].as_array().expect("rows");
    assert_eq!(rows.len(), 1, "reduceFields collapses to one row");
    // p90 over 1..=10 = sorted[floor(0.9·9)] = sorted[8] = 9 — computed, not degraded (the pNN pin),
    // over the REGEX-RENAMED field name.
    assert_eq!(rows[0]["cpu_load"], json!(9.0));

    // The tranche bound: an unknown id in the same pipeline position is carried — frames unchanged.
    let panel = sql_panel(
        "SELECT payload FROM series ORDER BY payload",
        json!([{ "id": "groupingToMatrix", "options": {} }]),
    );
    let out = viz_query(&node, &p, ws, panel).await.expect("runs");
    assert_eq!(
        out["rows"].as_array().expect("rows").len(),
        10,
        "unknown transform id skipped-with-notice, rows intact"
    );
}
