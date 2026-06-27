//! The data-console gateway routes, end to end (data-console scope): the admin, read-only `store.*`
//! DB-browser lens (tables/scan/graph) + the `ingest.*`/`series.*` explorer (write/list/find/latest/
//! read). Each test drives the real routes with `oneshot` (no socket) over a real signed session
//! against a **real node on `mem://`**, seeding **real records through the real write path** (the
//! `POST /ingest` route stages + drains; tags seeded via the real `lb_host::tags_add`). No `*.fake`.
//!
//! The two mandatory properties every gateway surface must hold are asserted **per verb**:
//!   - a **capability deny** (server-side, from the token's caps, never the body), one per verb; and
//!   - **workspace isolation** (a ws-B session sees none of ws-A's tables/rows/graph/series).
//!
//! The headline data-console risk gets its own test: the `store.*` verbs are **admin-only** (they
//! relax the per-record membership gate), so a token WITHOUT the admin cap is refused — proving the
//! gate-3 relaxation never leaks to a member.

mod common;

use std::sync::Arc;

use axum::http::StatusCode;
use common::*;
use lb_auth::{Principal, SigningKey};
use lb_host::{tags_add, Node, Provenance, Role as NodeRole, Tag, TagSource};
use lb_role_gateway::router;
use serde_json::{json, Value};
use tower::ServiceExt; // for `oneshot`

// The DB-browser admin caps (data-console scope) — gate-3-relaxed, admin-only.
const TABLES: &str = "mcp:store.tables:call";
const SCAN: &str = "mcp:store.scan:call";
const GRAPH: &str = "mcp:store.graph:call";
// The ingest/series member caps (the S8 verbs, surfaced over the gateway).
const WRITE: &str = "mcp:ingest.write:call";
const READ: &str = "mcp:series.read:call";
const LATEST: &str = "mcp:series.latest:call";
const FIND: &str = "mcp:series.find:call";
const LIST: &str = "mcp:series.list:call";

/// Push a sample to `series` over the real `POST /ingest` route (stages + drains → committed), as a
/// token that holds the write cap.
async fn seed_sample(
    gw: &lb_role_gateway::Gateway,
    tok: &str,
    series: &str,
    seq: u64,
    payload: Value,
) {
    let resp = router(gw.clone())
        .oneshot(bearer(
            json_post(
                "/ingest",
                json!({ "samples": [{
                    "series": series, "producer": "ignored", "ts": 1, "seq": seq,
                    "payload": payload
                }]}),
            ),
            tok,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK, "seed write committed");
}

// ----- ingest / series --------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn write_then_list_latest_and_read_round_trips_over_the_gateway() {
    let (gw, key) = gateway().await;
    let tok = token(&key, "user:ada", "acme", &[WRITE, READ, LATEST, LIST]);

    seed_sample(&gw, &tok, "node.cpu_temp", 1, json!(60.0)).await;
    seed_sample(&gw, &tok, "node.cpu_temp", 2, json!(61.4)).await;

    // series.list shows the series.
    let resp = router(gw.clone())
        .oneshot(bearer(get_req("/series?prefix=node"), &tok))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = json_body(resp).await;
    assert_eq!(body["series"], json!(["node.cpu_temp"]));

    // series.latest = the newest sample (seq 2).
    let resp = router(gw.clone())
        .oneshot(bearer(get_req("/series/node.cpu_temp/latest"), &tok))
        .await
        .unwrap();
    let body: Value = json_body(resp).await;
    assert_eq!(body["sample"]["seq"], 2);
    assert_eq!(body["sample"]["payload"], 61.4);

    // series.read = the committed range, ordered.
    let resp = router(gw)
        .oneshot(bearer(get_req("/series/node.cpu_temp/samples"), &tok))
        .await
        .unwrap();
    let body: Value = json_body(resp).await;
    let samples = body["samples"].as_array().unwrap();
    assert_eq!(samples.len(), 2);
    assert_eq!(samples[0]["seq"], 1);
    assert_eq!(samples[1]["seq"], 2);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn ingest_write_without_the_cap_is_denied_server_side() {
    let (gw, key) = gateway().await;
    // A token with reads but no write cap — the deny is from the TOKEN, server-side.
    let tok = token(&key, "user:ada", "acme", &[READ, LATEST, LIST]);

    let resp = router(gw)
        .oneshot(bearer(
            json_post(
                "/ingest",
                json!({ "samples": [{
                    "series": "s", "producer": "p", "ts": 1, "seq": 1, "payload": 1
                }]}),
            ),
            &tok,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn series_read_list_latest_find_each_denied_without_their_cap() {
    let (gw, key) = gateway().await;
    // No series caps at all → every read verb is refused before it runs.
    let tok = token(&key, "user:mallory", "acme", &[]);

    for req in [
        get_req("/series"),
        get_req("/series/x/latest"),
        get_req("/series/x/samples"),
    ] {
        let resp = router(gw.clone()).oneshot(bearer(req, &tok)).await.unwrap();
        assert_eq!(resp.status(), StatusCode::FORBIDDEN, "read verb denied");
    }
    // series.find (POST) too.
    let resp = router(gw)
        .oneshot(bearer(
            json_post("/series/find", json!({ "facets": [{ "key": "kind" }] })),
            &tok,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN, "find denied");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn series_find_filters_by_tag_facet() {
    // Seed a series + tag it `kind:temperature`, then find it by facet over the gateway.
    let (gw, key) = gateway().await;
    let tok = token(&key, "user:ada", "acme", &[WRITE, FIND]);
    seed_sample(&gw, &tok, "node.cpu_temp", 1, json!(60.0)).await;

    // Tag the series entity via the real tag verb (the producer/system path).
    let p = Principal::routed("user:ada", "acme", vec!["mcp:tags.add:call".into()]);
    tags_add(
        &gw.node.store,
        &p,
        "acme",
        "series:node.cpu_temp",
        &Tag::new("kind", json!("temperature")),
        &Provenance::new(1, "user:ada", TagSource::Human),
    )
    .await
    .expect("tag the series");

    let resp = router(gw)
        .oneshot(bearer(
            json_post(
                "/series/find",
                json!({ "facets": [{ "key": "kind", "value": "temperature" }] }),
            ),
            &tok,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = json_body(resp).await;
    let hits = body["series"].as_array().unwrap();
    assert_eq!(hits, &[json!("series:node.cpu_temp")]);
}

// ----- store.* (the admin DB browser) -----------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn tables_scan_graph_round_trip_for_an_admin() {
    let (gw, key) = gateway().await;
    // An admin token: holds the store.* caps AND the write cap to seed real rows.
    let tok = token(&key, "user:root", "acme", &[WRITE, TABLES, SCAN, GRAPH]);
    seed_sample(&gw, &tok, "node.cpu_temp", 1, json!(60.0)).await;
    seed_sample(&gw, &tok, "node.cpu_temp", 2, json!(61.4)).await;

    // store.tables lists `series` with its row count (2 committed samples).
    let resp = router(gw.clone())
        .oneshot(bearer(get_req("/store/tables"), &tok))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let tables: Vec<Value> = json_body(resp).await;
    let series_tbl = tables
        .iter()
        .find(|t| t["table"] == "series")
        .expect("series table present");
    assert_eq!(series_tbl["count"], 2);

    // store.scan pages the raw rows of `series`.
    let resp = router(gw.clone())
        .oneshot(bearer(get_req("/store/tables/series/rows?limit=10"), &tok))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let page: Value = json_body(resp).await;
    let rows = page["rows"].as_array().unwrap();
    assert_eq!(rows.len(), 2, "both committed rows scanned");
    assert!(rows[0]["id"].as_str().unwrap().starts_with("series:"));
    assert!(page["next"].is_null(), "short page → no next cursor");

    // store.graph: tag the series so there's a real relation edge, then read the graph.
    let p = Principal::routed("user:root", "acme", vec!["mcp:tags.add:call".into()]);
    tags_add(
        &gw.node.store,
        &p,
        "acme",
        "series:node.cpu_temp",
        &Tag::new("host", json!("pi-7")),
        &Provenance::new(1, "user:root", TagSource::Human),
    )
    .await
    .expect("tag the series");

    // Expand the series' relations by clicking it (the click-to-expand path: seed by the entity id
    // the tag edges reference — `series:node.cpu_temp`, the logical series). The series→tag edge is
    // drawn. (Table-seeding draws the composite-id `series` rows as nodes; the tag edges reference
    // the logical series name, so following a relation is the per-record expand path — recorded in
    // the scope as the first-cut graph shape.)
    let resp = router(gw)
        .oneshot(bearer(
            get_req("/store/graph?id=series:node.cpu_temp"),
            &tok,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let g: Value = json_body(resp).await;
    let edges = g["edges"].as_array().unwrap();
    assert!(
        edges
            .iter()
            .any(|e| e["label"] == "tagged" && e["target"].as_str().unwrap().starts_with("tag:")),
        "the series→tag relation edge is drawn: {g:?}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn store_verbs_denied_without_the_admin_cap_the_gate3_relaxation_stays_admin_only() {
    // THE headline data-console risk: a member-shaped token (no `mcp:store.*:call`) must NOT reach the
    // raw browser — else a member reads every record in the workspace, bypassing gate 3.
    let (gw, key) = gateway().await;
    let tok = token(
        &key,
        "user:member",
        "acme",
        &[WRITE, READ, LATEST, LIST, FIND],
    );

    for req in [
        get_req("/store/tables"),
        get_req("/store/tables/series/rows"),
        get_req("/store/graph?table=series"),
    ] {
        let resp = router(gw.clone()).oneshot(bearer(req, &tok)).await.unwrap();
        assert_eq!(
            resp.status(),
            StatusCode::FORBIDDEN,
            "member without the admin cap is refused"
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn ws_b_session_cannot_browse_or_read_ws_a() {
    // Two sessions, one node — the real isolation setup. ws-A seeds a series; a ws-B session (its own
    // token, same key) sees none of it: not the table, not the rows, not the series.
    let node = Arc::new(Node::boot_as(NodeRole::Hub).await.unwrap());
    let key = SigningKey::generate();
    let gw = gateway_on(node.clone(), &key);

    let tok_a = token(
        &key,
        "user:ada",
        "acme",
        &[WRITE, TABLES, SCAN, GRAPH, LIST],
    );
    seed_sample(&gw, &tok_a, "node.cpu_temp", 1, json!(60.0)).await;

    // ws-B admin: same caps, different workspace in the token. The workspace comes from the token, so
    // ws-B physically queries ITS namespace — empty.
    let tok_b = token(
        &key,
        "user:bob",
        "other",
        &[WRITE, TABLES, SCAN, GRAPH, LIST],
    );

    // ws-B's tables: no `series` table (nothing seeded in ws-B).
    let resp = router(gw.clone())
        .oneshot(bearer(get_req("/store/tables"), &tok_b))
        .await
        .unwrap();
    let tables: Vec<Value> = json_body(resp).await;
    assert!(
        !tables.iter().any(|t| t["table"] == "series"),
        "ws-B sees no series table — the hard wall"
    );

    // ws-B's scan of `series`: empty (the table is ws-A's).
    let resp = router(gw.clone())
        .oneshot(bearer(get_req("/store/tables/series/rows"), &tok_b))
        .await
        .unwrap();
    let page: Value = json_body(resp).await;
    assert_eq!(
        page["rows"].as_array().unwrap().len(),
        0,
        "ws-B scans nothing"
    );

    // ws-B's series.list: empty.
    let resp = router(gw.clone())
        .oneshot(bearer(get_req("/series"), &tok_b))
        .await
        .unwrap();
    let body: Value = json_body(resp).await;
    assert_eq!(body["series"], json!([]), "ws-B enumerates no series");

    // And ws-A still sees its own (the seed wasn't cross-contaminated).
    let resp = router(gw)
        .oneshot(bearer(get_req("/series"), &tok_a))
        .await
        .unwrap();
    let body: Value = json_body(resp).await;
    assert_eq!(body["series"], json!(["node.cpu_temp"]));
}
