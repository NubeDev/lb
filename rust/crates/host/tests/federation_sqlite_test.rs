//! SQLite as a FIRST-CLASS datasource kind (sqlite-datasource-demo scope) — the Docker-free e2e.
//! Same mandatory categories as `federation_test.rs`, over the `kind:"sqlite"` path: probe green
//! against a real seeded `.db` file, `federation.schema` discovery (tables + columns),
//! `federation.query` round-trip, the honest missing-path probe error (the DSN is a NODE-local
//! file path, goal 4), the capability-deny, and workspace-isolation.
//!
//! NO mocks for our own stack: real embedded SurrealDB, real caps, the REAL supervisor spawning the
//! REAL `federation` sidecar. The external DB is the ONE sanctioned fake-boundary (testing §0): a
//! REAL on-disk SQLite engine with real rows — no Docker, no TLS toolchain (the sidecar builds with
//! DEFAULT features; sqlite is not feature-gated).

use std::process::Command;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{call_tool, install_native, Node};
use lb_supervisor::OsLauncher;
use serde_json::{json, Value};

const MANIFEST: &str = include_str!("../../../extensions/federation/extension.toml");

fn principal(ws: &str, caps: &[&str]) -> Principal {
    let key = SigningKey::generate();
    let claims = Claims {
        sub: "user:test".into(),
        ws: ws.into(),
        role: Role::Member,
        caps: caps.iter().map(|s| s.to_string()).collect(),
        iat: 0,
        exp: u64::MAX,
    };
    verify(&key, &mint(&key, &claims), 1).unwrap()
}

fn admin(ws: &str) -> Principal {
    principal(
        ws,
        &[
            "mcp:native.install:call",
            "mcp:native.call:call",
            "mcp:native.status:call",
            "mcp:federation.query:call",
            "mcp:datasource.add:call",
            "mcp:datasource.list:call",
            "mcp:datasource.test:call",
            "secret:federation/*:write",
            "secret:federation/*:get",
        ],
    )
}

/// Build the sidecar with DEFAULT features (sqlite only — no postgres/TLS toolchain) and return the
/// dir holding it. Unlike the postgres harness this needs nothing external, so a failure is a FAIL,
/// not a skip.
fn federation_dir() -> String {
    if let Ok(p) = std::env::var("FEDERATION_BIN") {
        let dir = std::path::PathBuf::from(&p);
        return dir.parent().unwrap().to_string_lossy().into_owned();
    }
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let target = manifest_dir.join("../../target/debug");
    let status = Command::new("cargo")
        .args(["build", "-p", "federation"])
        .current_dir(manifest_dir.join("../.."))
        .status()
        .expect("cargo build -p federation runs");
    assert!(
        status.success() && target.join("federation").exists(),
        "the default-features (sqlite) federation sidecar builds"
    );
    target.to_string_lossy().into_owned()
}

/// Seed a REAL `.db` fixture with the demo-shaped tables (small — the test's rows, not the seeder's).
fn seed_db() -> String {
    let path = std::env::temp_dir().join(format!("lb-fed-sqlite-{}.db", std::process::id()));
    let _ = std::fs::remove_file(&path);
    let conn = rusqlite::Connection::open(&path).expect("open sqlite fixture");
    conn.execute_batch(
        "CREATE TABLE site (id TEXT PRIMARY KEY, name TEXT NOT NULL);
         CREATE TABLE point_reading (time TEXT, point_id TEXT, value REAL);
         INSERT INTO site VALUES ('site-001','Northside Factory'),('site-002','City Tower');
         INSERT INTO point_reading VALUES
           ('2026-01-01T00:00:00+00:00','p1',1.5),
           ('2026-01-01T00:15:00+00:00','p1',2.5),
           ('2026-01-01T00:30:00+00:00','p1',3.5);",
    )
    .expect("seed fixture rows");
    path.to_string_lossy().into_owned()
}

/// Install the sidecar approving the sqlite `127.0.0.1:0` convention endpoint (a file has no
/// network endpoint) + the secret grant — exactly what `make dev` pre-approves.
async fn install_federation(node: &Node, admin: &Principal, ws: &str, dir: &str) {
    let approved = vec![
        "net:tls:127.0.0.1:0:connect".to_string(),
        "secret:federation/*:get".to_string(),
    ];
    install_native(node, &OsLauncher, admin, ws, MANIFEST, dir, &approved, 1)
        .await
        .expect("federation sidecar installs + spawns");
}

async fn call(
    node: &std::sync::Arc<Node>,
    p: &Principal,
    ws: &str,
    tool: &str,
    input: Value,
) -> Result<Value, lb_mcp::ToolError> {
    let out = call_tool(node, p, ws, tool, &input.to_string()).await?;
    Ok(serde_json::from_str(&out).unwrap())
}

async fn add_source(
    node: &std::sync::Arc<Node>,
    admin: &Principal,
    ws: &str,
    name: &str,
    dsn: &str,
) {
    call(
        node,
        admin,
        ws,
        "datasource.add",
        json!({"name": name, "kind":"sqlite", "endpoint":"127.0.0.1:0", "dsn": dsn, "ts": 1}),
    )
    .await
    .expect("datasource.add sqlite");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn federation_end_to_end_sqlite() {
    let dir = federation_dir();
    let db = seed_db();
    let ws = "acme";
    let node = std::sync::Arc::new(Node::boot().await.unwrap());
    let admin = admin(ws);
    install_federation(&node, &admin, ws, &dir).await;
    add_source(&node, &admin, ws, "demo", &db).await;

    // --- SECRET MEDIATION: the path DSN gets no special case — only the ref is listed ---
    let listed = call(&node, &admin, ws, "datasource.list", json!({}))
        .await
        .unwrap()
        .to_string();
    assert!(!listed.contains(&db), "datasource.list leaked the path DSN");
    assert!(listed.contains("\"secret_ref\""));

    // --- PROBE green against the real file ---
    let probe = call(
        &node,
        &admin,
        ws,
        "datasource.test",
        json!({"source":"demo","ts":2}),
    )
    .await
    .expect("datasource.test");
    assert_eq!(probe["ok"], true, "probe green: {probe}");

    // --- DISCOVERY: tables + columns (the Data Studio picker path) ---
    let tables = call(
        &node,
        &admin,
        ws,
        "federation.schema",
        json!({"source":"demo","ts":3}),
    )
    .await
    .expect("federation.schema lists tables");
    let names: Vec<&str> = tables["tables"]
        .as_array()
        .expect("tables array")
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    assert!(
        names.contains(&"site") && names.contains(&"point_reading"),
        "discovery lists the seeded tables: {tables}"
    );
    let cols = call(
        &node,
        &admin,
        ws,
        "federation.schema",
        json!({"source":"demo","table":"point_reading","ts":3}),
    )
    .await
    .expect("federation.schema describes columns");
    let col_names: Vec<&str> = cols["columns"]
        .as_array()
        .expect("columns array")
        .iter()
        .map(|c| c["name"].as_str().unwrap())
        .collect();
    assert!(
        col_names.contains(&"time") && col_names.contains(&"value"),
        "describe returns real columns: {cols}"
    );

    // --- QUERY round-trip ---
    let q = call(
        &node,
        &admin,
        ws,
        "federation.query",
        json!({"source":"demo","sql":"SELECT point_id, value FROM point_reading ORDER BY time","ts":4}),
    )
    .await
    .expect("federation.query");
    assert_eq!(
        q["rows"].as_array().unwrap().len(),
        3,
        "three seeded rows: {q}"
    );
    assert!(
        !q.to_string().contains(&db),
        "query result leaked the path DSN"
    );

    // --- MISSING PATH (goal 4): an honest node-local-path error, never a silent empty db ---
    add_source(&node, &admin, ws, "laptop", "/home/someone/on-my-laptop.db").await;
    let bad = call(
        &node,
        &admin,
        ws,
        "datasource.test",
        json!({"source":"laptop","ts":5}),
    )
    .await
    .expect_err("a missing node-local path probes RED (a real error, never a silent empty db)");
    let msg = format!("{bad:?}");
    assert!(
        msg.contains("node running the federation sidecar"),
        "the error names the node-local path semantics: {msg}"
    );
    assert!(
        !std::path::Path::new("/home/someone/on-my-laptop.db").exists(),
        "the probe did NOT create an empty db at the bad path"
    );

    // --- CAPABILITY-DENY: query without the federation cap ---
    let no_cap = principal(ws, &["mcp:datasource.list:call"]);
    let denied = call(
        &node,
        &no_cap,
        ws,
        "federation.query",
        json!({"source":"demo","sql":"SELECT 1","ts":6}),
    )
    .await
    .expect_err("query without mcp:federation.query:call is denied");
    assert!(
        matches!(denied, lb_mcp::ToolError::Denied),
        "opaque deny: {denied:?}"
    );

    // --- WORKSPACE ISOLATION: the ws-A sqlite source resolves to nothing from ws-B ---
    let ws_b = "other";
    let admin_b = self::admin(ws_b);
    install_federation(&node, &admin_b, ws_b, &dir).await;
    let b_caller = principal(ws_b, &["mcp:federation.query:call"]);
    let iso = call(
        &node,
        &b_caller,
        ws_b,
        "federation.query",
        json!({"source":"demo","sql":"SELECT 1","ts":7}),
    )
    .await
    .expect_err("ws-B cannot resolve ws-A's sqlite source");
    assert!(
        matches!(iso, lb_mcp::ToolError::BadInput(_)),
        "not found in ws-B: {iso:?}"
    );

    let _ = std::fs::remove_file(&db);
}
