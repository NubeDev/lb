//! The **query happy-path** round-trip over the REAL gateway (channels-query-charts scope): a
//! `kind:"query"` Item posted into a channel → the inline host worker runs it through the REAL
//! federation sidecar against a REAL on-disk SQLite source → a `kind:"query_result"` Item with the
//! expected columns/rows and a non-null chart appears in `history` AND streams over SSE.
//!
//! This complements the deny-path round-trip in `gateway_routes_test.rs` (which needs no datasource):
//! here we exercise the *successful* execution path end to end. Per testing §0 there are NO mocks for
//! our own stack — real embedded SurrealDB, real bus, the real gateway router, the REAL native
//! supervisor (`OsLauncher`) spawning the REAL `federation` binary. The external DB is the ONE
//! sanctioned fake-boundary: a REAL SQLite engine with real rows on disk (the documented Postgres
//! fallback — needs no Docker/TLS toolchain). The harness skips with a clear message if the
//! sqlite-only sidecar binary is not present (mirroring `federation_test.rs`).
//!
//! Mandatory categories covered here: the happy round-trip (history + SSE), and the
//! **workspace-isolation** assertion for the query path (a ws-A source name resolves to nothing from
//! ws-B, so a ws-B poster gets an opaque `query_error`, never ws-A's rows). The capability-deny path
//! is already covered by `gateway_routes_test.rs::posting_a_query_item_without_the_grant_…`.

mod common;

use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use axum::http::StatusCode;
use common::*;
use lb_auth::{Principal, SigningKey};
use lb_host::{call_tool, install_native, Node, Role as NodeRole};
use lb_inbox::Item;
use lb_role_gateway::router;
use lb_supervisor::OsLauncher;
use serde_json::{json, Value};
use tower::ServiceExt;

const MANIFEST: &str = include_str!("../../../extensions/federation/extension.toml");

/// The full cap set an admin needs to install the sidecar + register and query a source. Mirrors
/// `federation_test.rs::admin`, trimmed to what this happy-path needs.
const ADMIN_CAPS: &[&str] = &[
    "mcp:native.install:call",
    "mcp:native.call:call",
    "mcp:native.status:call",
    "mcp:federation.query:call",
    "mcp:datasource.add:call",
    "secret:federation/*:write",
    "secret:federation/*:get",
];

// ---------------------------------------------------------------------------------------------
// The sqlite-only federation sidecar — built sqlite-only (no TLS/C toolchain). SKIP if absent.
// ---------------------------------------------------------------------------------------------

/// Locate the directory holding the `federation` binary the supervisor will spawn. Prefers
/// `FEDERATION_BIN`, else the workspace `target/release` (where `FEDERATION_NO_POSTGRES=1 cargo build
/// --release -p federation` lands it), else `target/debug`. Returns `None` (→ SKIP) if neither has it.
fn federation_dir() -> Option<String> {
    if let Ok(p) = std::env::var("FEDERATION_BIN") {
        return Some(
            PathBuf::from(&p)
                .parent()
                .unwrap()
                .to_string_lossy()
                .into_owned(),
        );
    }
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    for profile in ["release", "debug"] {
        let dir = manifest_dir.join("../../target").join(profile);
        if dir.join("federation").exists() {
            return Some(dir.to_string_lossy().into_owned());
        }
    }
    None
}

// ---------------------------------------------------------------------------------------------
// A REAL on-disk SQLite source, seeded with real rows (behind the federation `Source` trait).
// ---------------------------------------------------------------------------------------------

static DB_SEQ: AtomicU32 = AtomicU32::new(0);

/// Create a fresh SQLite file under the OS temp dir with a temporal/categorical `daily` table and
/// real rows, then return its path (the federation "DSN" for a sqlite source IS the file path).
/// `day` is an ISO date (temporal) and `signups` numeric → the chart picker yields a LINE chart.
fn seed_sqlite() -> String {
    let n = DB_SEQ.fetch_add(1, Ordering::SeqCst);
    let path = std::env::temp_dir().join(format!("lb-gw-query-{}-{n}.db", std::process::id()));
    let _ = std::fs::remove_file(&path);
    let conn = rusqlite::Connection::open(&path).expect("open sqlite file");
    conn.execute_batch(
        "CREATE TABLE daily (day TEXT, signups INTEGER);
         INSERT INTO daily (day, signups) VALUES
           ('2024-01-01', 3), ('2024-01-02', 5), ('2024-01-03', 7), ('2024-01-04', 4);",
    )
    .expect("seed sqlite rows");
    path.to_string_lossy().into_owned()
}

/// Install the federation sidecar in `ws` with a wildcard `net:*` grant (a sqlite source has no real
/// network endpoint, so we register it at a dummy `127.0.0.1:0` and approve `net:tls:*:*:connect`).
async fn install_federation(node: &Node, admin: &Principal, ws: &str, dir: &str) {
    let approved = vec![
        "net:tls:*:*:connect".to_string(),
        "secret:federation/*:get".to_string(),
    ];
    install_native(node, &OsLauncher, admin, ws, MANIFEST, dir, &approved, 1)
        .await
        .expect("federation sidecar installs + spawns");
}

/// Register the seeded sqlite file as source `name` in `ws` (DSN = the file path, kind `sqlite`,
/// endpoint a dummy host:port the wildcard net grant covers).
async fn add_source(node: &Arc<Node>, admin: &Principal, ws: &str, name: &str, db_path: &str) {
    let out = call_tool(
        node,
        admin,
        ws,
        "datasource.add",
        &json!({
            "name": name, "kind": "sqlite",
            "endpoint": "127.0.0.1:0", "dsn": db_path, "ts": 1
        })
        .to_string(),
    )
    .await;
    out.expect("datasource.add sqlite source");
}

fn admin_token(key: &SigningKey, ws: &str, cid: &str) -> String {
    let mut caps: Vec<&str> = ADMIN_CAPS.to_vec();
    let pub_cap = format!("bus:chan/{cid}:pub");
    let sub_cap = format!("bus:chan/{cid}:sub");
    caps.push(&pub_cap);
    caps.push(&sub_cap);
    token(key, "user:ada", ws, &caps)
}

// ---------------------------------------------------------------------------------------------
// THE HAPPY PATH — post a query item, get a query_result with columns/rows/chart in history + SSE.
// ---------------------------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn posting_a_query_item_round_trips_a_result_with_columns_rows_and_chart() {
    let Some(dir) = federation_dir() else {
        eprintln!(
            "SKIP query happy-path: no `federation` binary (build with \
             `FEDERATION_NO_POSTGRES=1 cargo build --release -p federation`)"
        );
        return;
    };
    let db = seed_sqlite();

    let node = Arc::new(Node::boot_as(NodeRole::Hub).await.expect("node boots"));
    let key = SigningKey::generate();
    let ws = "acme";
    let cid = "analytics";

    let admin = lb_auth::verify(&key, &admin_token(&key, ws, cid), NOW).expect("admin verifies");
    install_federation(&node, &admin, ws, &dir).await;
    add_source(&node, &admin, ws, "warehouse", &db).await;

    let tok = admin_token(&key, ws, cid);

    // --- post the kind:"query" item over the real gateway ---
    let body = json!({
        "kind": "query", "source": "warehouse",
        "sql": "SELECT day, signups FROM daily ORDER BY day"
    })
    .to_string();
    let item = Item::new("q1", cid, "user:ada", body, 1);
    let resp = router(gateway_on(node.clone(), &key))
        .oneshot(bearer(post_req(cid, &item), &tok))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK, "the query item posts");

    // --- the worker's query_result is now in history with the expected columns/rows + a chart ---
    let resp = router(gateway_on(node.clone(), &key))
        .oneshot(bearer(get_req(&format!("/channels/{cid}/messages")), &tok))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let items: Vec<Item> = json_body(resp).await;
    let result = items
        .iter()
        .find(|i| i.author == "system:query-worker")
        .expect("history shows the worker's answer");
    let payload: Value = serde_json::from_str(&result.body).unwrap();
    assert_eq!(payload["kind"], "query_result", "got: {payload}");
    assert_eq!(
        payload["columns"],
        json!(["day", "signups"]),
        "the seeded columns: {payload}"
    );
    assert_eq!(
        payload["rows"].as_array().unwrap().len(),
        4,
        "the four seeded rows: {payload}"
    );
    // A temporal x (`day`) + numeric series (`signups`) → a non-null LINE chart (chart picker).
    assert_eq!(
        payload["chart"]["type"], "line",
        "auto-plotted line: {payload}"
    );
    assert_eq!(payload["chart"]["x"], "day");
    assert_eq!(payload["chart"]["series"][0]["field"], "signups");

    // --- the result also arrives LIVE over SSE (mirror the SSE test in gateway_routes_test.rs) ---
    assert_streams_result_over_sse(node, &key, ws, cid, &db).await;
}

/// Open an SSE stream, post a SECOND query, and assert the `query_result` event streams to the
/// browser with the columns embedded — the live-motion half of the round-trip.
async fn assert_streams_result_over_sse(
    node: Arc<Node>,
    key: &SigningKey,
    ws: &str,
    cid: &str,
    _db: &str,
) {
    use std::time::Duration;

    let tok = admin_token(key, ws, cid);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let app = router(gateway_on(node.clone(), key));
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let client = reqwest::Client::new();
    let mut resp = client
        .get(format!("http://{addr}/channels/{cid}/stream?token={tok}"))
        .send()
        .await
        .expect("sse stream opens");
    assert_eq!(resp.status(), 200);

    // Post a second query through the host on the shared node; the worker answers and publishes.
    let poster = lb_auth::verify(key, &tok, NOW).expect("poster verifies");
    let body = json!({
        "kind": "query", "source": "warehouse",
        "sql": "SELECT day, signups FROM daily ORDER BY day"
    })
    .to_string();
    lb_host::post(
        node.as_ref(),
        &poster,
        ws,
        cid,
        Item::new("q2", cid, "user:ada", body, 2),
    )
    .await
    .expect("second query posts");

    let stream = tokio::time::timeout(Duration::from_secs(10), async {
        let mut acc = String::new();
        while let Some(chunk) = resp.chunk().await.expect("read chunk") {
            acc.push_str(&String::from_utf8_lossy(&chunk));
            if acc.contains("query_result") && acc.contains("signups") {
                return acc;
            }
        }
        acc
    })
    .await
    .expect("the query_result arrives over SSE in time");

    assert!(
        stream.contains("event: message"),
        "framed as a message event: {stream:?}"
    );
    assert!(
        stream.contains("query_result") && stream.contains("\\\"signups\\\""),
        "the result payload (with its columns) streamed to the browser: {stream:?}"
    );
}

// ---------------------------------------------------------------------------------------------
// WORKSPACE ISOLATION — a ws-A source name resolves to nothing from ws-B (opaque query_error).
// ---------------------------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ws_b_cannot_query_a_ws_a_source_name() {
    let Some(dir) = federation_dir() else {
        eprintln!("SKIP ws isolation query: no `federation` binary");
        return;
    };
    let db = seed_sqlite();

    let node = Arc::new(Node::boot_as(NodeRole::Hub).await.expect("node boots"));
    let key = SigningKey::generate();
    let cid = "analytics";

    // ws-A registers `warehouse`; ws-B does not.
    let admin_a = lb_auth::verify(&key, &admin_token(&key, "acme", cid), NOW).unwrap();
    install_federation(&node, &admin_a, "acme", &dir).await;
    add_source(&node, &admin_a, "acme", "warehouse", &db).await;

    // ws-B has the sidecar installed (so the deny is RESOLUTION, not binary-absence) but no source.
    let admin_b = lb_auth::verify(&key, &admin_token(&key, "other", cid), NOW).unwrap();
    install_federation(&node, &admin_b, "other", &dir).await;

    // A ws-B poster names the ws-A source `warehouse`. It resolves to nothing in ws-B → the worker
    // posts an OPAQUE query_error; ws-B never sees ws-A's rows (the hard wall).
    let tok_b = admin_token(&key, "other", cid);
    let body = json!({ "kind": "query", "source": "warehouse", "sql": "SELECT 1" }).to_string();
    let item = Item::new("qb", cid, "user:ada", body, 1);
    let resp = router(gateway_on(node.clone(), &key))
        .oneshot(bearer(post_req(cid, &item), &tok_b))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let resp = router(gateway_on(node, &key))
        .oneshot(bearer(
            get_req(&format!("/channels/{cid}/messages")),
            &tok_b,
        ))
        .await
        .unwrap();
    let items: Vec<Item> = json_body(resp).await;
    let answer = items
        .iter()
        .find(|i| i.author == "system:query-worker")
        .expect("worker answered in ws-B");
    let payload: Value = serde_json::from_str(&answer.body).unwrap();
    assert_eq!(
        payload["kind"], "query_error",
        "ws-B gets an error, not ws-A's data: {payload}"
    );
    assert_eq!(
        payload["error"], "query not permitted",
        "opaque — ws-B cannot tell the ws-A source exists: {payload}"
    );
    // The hard guarantee: no ws-A row value ever appears in ws-B's history.
    let history = items.iter().map(|i| i.body.as_str()).collect::<String>();
    assert!(
        !history.contains("signups"),
        "ws-A column/data must never surface in ws-B history"
    );

    let _ = std::fs::remove_file(&db);
}
