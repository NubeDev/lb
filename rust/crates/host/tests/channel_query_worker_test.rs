//! Channel query-worker re-entrancy + opaque-deny tests (channels-query-charts scope), over a REAL
//! `Node` and the REAL `post` path (no mocks; testing §0). Two mandatory invariants:
//!
//!   - RE-ENTRANCY GUARD: only a `kind:"query"` item triggers the worker. Posting a `query_result`
//!     (the worker's own output shape) must NOT spawn another worker item — an infinite loop is one
//!     absent guard away. Asserted at the post level: history holds exactly the posted item.
//!   - OPAQUE DENY: a poster WITHOUT `mcp:federation.query:call` who posts a `kind:"query"` gets a
//!     `query_error` whose message is EXACTLY "query not permitted" — the same string a missing
//!     source yields, so the poster learns nothing about whether the source exists (no existence
//!     leak). This deny is decided host-side BEFORE any sidecar, so it needs no federation binary.

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{history, post, Node};
use lb_inbox::Item;

fn principal(ws: &str, caps: &[&str]) -> Principal {
    let key = SigningKey::generate();
    let claims = Claims {
        sub: "user:ada".into(),
        ws: ws.into(),
        role: Role::Member,
        caps: caps.iter().map(|s| s.to_string()).collect(),
        iat: 0,
        exp: u64::MAX,
        constraint: None,
        run_id: None,
    };
    verify(&key, &mint(&key, &claims), 1).unwrap()
}

/// The `query_result` body shape the worker itself posts — used to prove that re-posting it does NOT
/// re-trigger the worker (the re-entrancy guard parses it to a non-`Query` payload and returns).
fn query_result_body() -> String {
    serde_json::json!({
        "kind": "query_result",
        "source": "pg",
        "sql": "SELECT 1",
        "columns": ["v"],
        "rows": [{ "v": 1 }],
    })
    .to_string()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn posting_a_query_result_item_does_not_re_trigger_the_worker() {
    let node = Node::boot().await.expect("node boots");
    let ws = "acme";
    let cid = "analytics";
    // A member who can pub + sub the channel (no federation grant needed — this body is not a query).
    let p = principal(
        ws,
        &[
            &format!("bus:chan/{cid}:pub"),
            &format!("bus:chan/{cid}:sub"),
        ],
    );

    let item = Item::new("r1", cid, "user:ada", query_result_body(), 1);
    post(&node, &p, ws, cid, item)
        .await
        .expect("post the result item");

    let items = history(&node.store, &p, ws, cid).await.expect("history");
    assert_eq!(
        items.len(),
        1,
        "a query_result item must NOT spawn a worker item — history holds only the posted one: {:?}",
        items.iter().map(|i| (&i.id, &i.author)).collect::<Vec<_>>()
    );
    assert_eq!(items[0].id, "r1");
    assert!(
        !items.iter().any(|i| i.author == "system:query-worker"),
        "the worker never ran for a non-query body"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_query_without_the_datasource_grant_yields_opaque_query_not_permitted() {
    let node = Node::boot().await.expect("node boots");
    let ws = "acme";
    let cid = "analytics";
    // The poster can pub/sub the channel (so the post itself is authorized) but lacks
    // `mcp:federation.query:call` — the worker's federation run is denied host-side, BEFORE any
    // sidecar, and collapses to the opaque "query not permitted".
    let p = principal(
        ws,
        &[
            &format!("bus:chan/{cid}:pub"),
            &format!("bus:chan/{cid}:sub"),
        ],
    );

    let body = serde_json::json!({
        "kind": "query",
        "source": "pg",
        "sql": "SELECT 1",
    })
    .to_string();
    let item = Item::new("q1", cid, "user:ada", body, 1);
    post(&node, &p, ws, cid, item)
        .await
        .expect("the query item posts");

    let items = history(&node.store, &p, ws, cid).await.expect("history");
    // The original query item + the worker's query_error answer.
    let err_item = items
        .iter()
        .find(|i| i.author == "system:query-worker")
        .expect("the worker posted a query_error answer");
    let body: serde_json::Value =
        serde_json::from_str(&err_item.body).expect("the worker body is JSON");
    assert_eq!(body["kind"], "query_error");
    assert_eq!(
        body["error"], "query not permitted",
        "the deny is opaque (same string a missing source yields — no existence leak): {body}"
    );
}
