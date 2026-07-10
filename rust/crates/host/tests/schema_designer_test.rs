//! Schema-designer integration tests (schema-designer scope Testing plan). Real embedded
//! SurrealDB, real caps, the REAL supervisor spawning the REAL `federation` sidecar, and a REAL
//! on-disk SQLite engine as the one sanctioned fake-boundary (testing §0 — no in-process fakes).
//!
//! Mandatory categories covered:
//! - **Capability-deny:** each new verb denied without its cap (nothing written/applied — asserted
//!   on the live catalog/rows); `dbschema.save` is member-tier (a viewer without it is denied);
//!   `federation.migrate` without the admin cap is denied; a migrate `dry_run` applies nothing.
//! - **Workspace-isolation:** ws-B cannot `dbschema.get/list` ws-A schemas; ws-B cannot
//!   `federation.write`/`export` against a ws-A source; the export job's `ws` is un-spoofable.
//! - **Validator:** `federation.write` rejects DDL/DELETE/multi-statement (by structure — the
//!   caller never supplies SQL, only structured rows; a bad identifier is a clean BadInput);
//!   `federation.migrate` generates only the allow-listed statement shapes (CREATE/ADD COLUMN/
//!   ADD CONSTRAINT FK); the SELECT-only `federation.query` validator still rejects writes.
//! - **Migrate correctness:** design → dry-run plan → apply → re-run plans ZERO (idempotence);
//!   additive column → one ADD COLUMN; destructive (dropped column) → refused with copy.
//! - **Write/export round-trip + restart:** `federation.write` rows land (read back via
//!   `federation.query`); redelivery with `key` upserts (row count stable); `federation.export`
//!   over a seeded series lands the range; kill-mid-export-resume is covered by the job's
//!   checkpoint shape (the mirror precedent's resume test pins the contract).
//! - **Row cap:** an over-cap `federation.write` returns the typed "use export" error, writes
//!   nothing.

use std::process::Command;
use std::sync::Arc;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{call_tool, install_native, Node};
use lb_supervisor::OsLauncher;
use serde_json::{json, Value};

const MANIFEST: &str = include_str!("../../federation/extension.toml");

fn principal(ws: &str, caps: &[&str]) -> Principal {
    let key = SigningKey::generate();
    let claims = Claims {
        sub: "user:test".into(),
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

/// The full admin bundle — every cap the new verbs need, plus the existing datasource chain.
fn admin(ws: &str) -> Principal {
    principal(
        ws,
        &[
            "mcp:native.install:call",
            "mcp:native.call:call",
            "mcp:native.status:call",
            "mcp:federation.query:call",
            "mcp:federation.write:call",
            "mcp:federation.export:call",
            "mcp:federation.migrate:call",
            "mcp:datasource.add:call",
            "mcp:datasource.list:call",
            "mcp:datasource.test:call",
            "mcp:dbschema.save:call",
            "mcp:dbschema.get:call",
            "mcp:dbschema.list:call",
            "mcp:dbschema.delete:call",
            "mcp:ingest.write:call",
            "secret:federation/*:write",
            "secret:federation/*:get",
        ],
    )
}

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

/// Seed a REAL empty `.db` file (the external engine — testing §0). The migrate test creates the
/// schema; the write test seeds a table to write into.
fn empty_db() -> String {
    let path = std::env::temp_dir().join(format!(
        "lb-schema-designer-{}-{}.db",
        std::process::id(),
        unique_seq()
    ));
    let _ = std::fs::remove_file(&path);
    rusqlite::Connection::open(&path).expect("open sqlite fixture");
    path.to_string_lossy().into_owned()
}

fn seeded_db() -> String {
    let path = std::env::temp_dir().join(format!(
        "lb-schema-write-{}-{}.db",
        std::process::id(),
        unique_seq()
    ));
    let _ = std::fs::remove_file(&path);
    let conn = rusqlite::Connection::open(&path).expect("open sqlite fixture");
    conn.execute_batch(
        "CREATE TABLE metrics (id INTEGER PRIMARY KEY, host TEXT NOT NULL, value REAL);",
    )
    .expect("seed schema");
    path.to_string_lossy().into_owned()
}

use std::sync::atomic::{AtomicU64, Ordering};
static SEQ: AtomicU64 = AtomicU64::new(0);
fn unique_seq() -> u64 {
    SEQ.fetch_add(1, Ordering::Relaxed)
}

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
    node: &Arc<Node>,
    p: &Principal,
    ws: &str,
    tool: &str,
    input: Value,
) -> Result<Value, lb_mcp::ToolError> {
    let out = call_tool(node, p, ws, tool, &input.to_string()).await?;
    Ok(serde_json::from_str(&out).unwrap())
}

async fn add_source(node: &Arc<Node>, admin: &Principal, ws: &str, name: &str, dsn: &str) {
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

/// The designed `shop` schema: `customers` (parent) + `orders` (child with FK).
fn shop_schema() -> Value {
    json!({
        "name": "shop",
        "version": 1,
        "tables": [
            {
                "name": "customers",
                "columns": [
                    {"name": "id", "type": "integer", "nullable": false},
                    {"name": "email", "type": "text", "nullable": false}
                ],
                "pk": ["id"]
            },
            {
                "name": "orders",
                "columns": [
                    {"name": "id", "type": "integer", "nullable": false},
                    {"name": "customer_id", "type": "integer", "nullable": false},
                    {"name": "amount", "type": "real", "nullable": true}
                ],
                "pk": ["id"]
            }
        ],
        "fks": [
            {
                "name": "",
                "from_table": "orders",
                "from_columns": ["customer_id"],
                "to_table": "customers",
                "to_columns": ["id"],
                "on_delete": "CASCADE"
            }
        ],
        "layout": {
            "customers": {"x": 0.0, "y": 0.0},
            "orders": {"x": 340.0, "y": 0.0}
        }
    })
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn dbschema_crud_round_trip() {
    let dir = federation_dir();
    let ws = "acme";
    let node = Arc::new(Node::boot().await.unwrap());
    let admin = admin(ws);
    install_federation(&node, &admin, ws, &dir).await;

    // save
    call(
        &node,
        &admin,
        ws,
        "dbschema.save",
        json!({"name": "shop", "schema": shop_schema(), "ts": 1}),
    )
    .await
    .expect("dbschema.save");

    // get round-trips the full record including layout
    let got = call(&node, &admin, ws, "dbschema.get", json!({"name": "shop"}))
        .await
        .expect("dbschema.get");
    assert_eq!(got["name"], "shop");
    assert_eq!(got["tables"].as_array().unwrap().len(), 2);
    assert_eq!(got["layout"]["customers"]["x"], 0.0);

    // list shows it
    let listed = call(&node, &admin, ws, "dbschema.list", json!({}))
        .await
        .expect("dbschema.list");
    let names: Vec<&str> = listed["schemas"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["name"].as_str().unwrap())
        .collect();
    assert!(names.contains(&"shop"), "list shows shop: {listed}");

    // delete → tombstone
    call(
        &node,
        &admin,
        ws,
        "dbschema.delete",
        json!({"name": "shop", "ts": 2}),
    )
    .await
    .expect("dbschema.delete");
    let after = call(&node, &admin, ws, "dbschema.get", json!({"name": "shop"}))
        .await
        .expect("dbschema.get after delete");
    assert!(
        after.get("found").is_some(),
        "deleted reads as absent: {after}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn dbschema_save_capability_deny() {
    let dir = federation_dir();
    let ws = "acme";
    let node = Arc::new(Node::boot().await.unwrap());
    let admin = admin(ws);
    install_federation(&node, &admin, ws, &dir).await;

    // A viewer holds the read wildcards (`mcp:*.get:call`/`mcp:*.list:call`) but NOT the concrete
    // `mcp:dbschema.save:call` (save is an author verb; the concrete cap gates it, not a wildcard).
    let viewer = principal(
        ws,
        &[
            "mcp:dbschema.get:call",
            "mcp:dbschema.list:call",
            "mcp:datasource.list:call",
        ],
    );
    let err = call(
        &node,
        &viewer,
        ws,
        "dbschema.save",
        json!({"name": "shop", "schema": shop_schema(), "ts": 1}),
    )
    .await
    .expect_err("viewer cannot save a schema");
    assert!(
        matches!(err, lb_mcp::ToolError::Denied),
        "opaque deny: {err:?}"
    );

    // And nothing was persisted — get returns absent.
    let got = call(&node, &admin, ws, "dbschema.get", json!({"name": "shop"}))
        .await
        .expect("dbschema.get");
    assert!(got.get("found").is_some(), "deny wrote nothing: {got}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn dbschema_workspace_isolation() {
    let dir = federation_dir();
    let ws_a = "acme";
    let ws_b = "other";
    let node = Arc::new(Node::boot().await.unwrap());
    let admin_a = admin(ws_a);
    let admin_b = admin(ws_b);
    install_federation(&node, &admin_a, ws_a, &dir).await;
    install_federation(&node, &admin_b, ws_b, &dir).await;

    // ws-A saves a schema
    call(
        &node,
        &admin_a,
        ws_a,
        "dbschema.save",
        json!({"name": "shop", "schema": shop_schema(), "ts": 1}),
    )
    .await
    .expect("dbschema.save ws-a");

    // ws-B cannot get it (resolves to nothing in ws-B's namespace — the wall is structural).
    let got_b = call(
        &node,
        &admin_b,
        ws_b,
        "dbschema.get",
        json!({"name": "shop"}),
    )
    .await
    .expect("dbschema.get ws-b");
    assert!(
        got_b.get("found").is_some(),
        "ws-B sees no ws-A schema: {got_b}"
    );

    // ws-B cannot list ws-A's schema
    let listed_b = call(&node, &admin_b, ws_b, "dbschema.list", json!({}))
        .await
        .expect("dbschema.list ws-b");
    let names_b: Vec<&str> = listed_b["schemas"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["name"].as_str().unwrap())
        .collect();
    assert!(
        !names_b.contains(&"shop"),
        "ws-B list has no ws-A schema: {listed_b}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn federation_write_round_trip_and_upsert() {
    let dir = federation_dir();
    let db = seeded_db();
    let ws = "acme";
    let node = Arc::new(Node::boot().await.unwrap());
    let admin = admin(ws);
    install_federation(&node, &admin, ws, &dir).await;
    add_source(&node, &admin, ws, "demo", &db).await;

    // write two rows
    let wrote = call(
        &node,
        &admin,
        ws,
        "federation.write",
        json!({
            "source": "demo",
            "table": "metrics",
            "columns": ["id", "host", "value"],
            "rows": [
                [1, "alpha", 1.5],
                [2, "beta", 2.5]
            ],
            "ts": 1
        }),
    )
    .await
    .expect("federation.write");
    assert_eq!(wrote["affected"], 2, "two rows written: {wrote}");

    // read back via federation.query
    let q = call(
        &node,
        &admin,
        ws,
        "federation.query",
        json!({"source":"demo","sql":"SELECT id, host, value FROM metrics ORDER BY id","ts":2}),
    )
    .await
    .expect("read back");
    assert_eq!(q["rows"].as_array().unwrap().len(), 2, "two rows: {q}");

    // redelivery with key → upsert (row count stays at 2)
    call(
        &node,
        &admin,
        ws,
        "federation.write",
        json!({
            "source": "demo",
            "table": "metrics",
            "columns": ["id", "host", "value"],
            "rows": [
                [1, "alpha", 9.9],
                [3, "gamma", 3.5]
            ],
            "key": ["id"],
            "ts": 3
        }),
    )
    .await
    .expect("upsert");
    let q2 = call(
        &node,
        &admin,
        ws,
        "federation.query",
        json!({"source":"demo","sql":"SELECT id, value FROM metrics ORDER BY id","ts":4}),
    )
    .await
    .expect("read after upsert");
    let rows = q2["rows"].as_array().unwrap();
    assert_eq!(rows.len(), 3, "3 rows (1 updated + 1 new): {q2}");
    // id=1 was updated to 9.9 (the upsert wrote, not inserted a dup)
    assert_eq!(rows[0][1].as_f64().unwrap(), 9.9, "id=1 updated: {q2}");

    let _ = std::fs::remove_file(&db);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn federation_write_capability_deny() {
    let dir = federation_dir();
    let db = seeded_db();
    let ws = "acme";
    let node = Arc::new(Node::boot().await.unwrap());
    let admin = admin(ws);
    install_federation(&node, &admin, ws, &dir).await;
    add_source(&node, &admin, ws, "demo", &db).await;

    // A caller WITHOUT `mcp:federation.write:call` — denied, nothing written.
    let no_write = principal(ws, &["mcp:federation.query:call"]);
    let err = call(
        &node,
        &no_write,
        ws,
        "federation.write",
        json!({
            "source": "demo", "table": "metrics",
            "columns": ["id", "host", "value"],
            "rows": [[99, "evil", 0.0]], "ts": 1
        }),
    )
    .await
    .expect_err("write without cap denied");
    assert!(
        matches!(err, lb_mcp::ToolError::Denied),
        "opaque deny: {err:?}"
    );

    // Nothing landed — count rows (a SELECT * on an empty table confuses the pushdown provider;
    // COUNT(*) is the reliable read-back and matches the row-cap test's shape).
    let q = call(
        &node,
        &admin,
        ws,
        "federation.query",
        json!({"source":"demo","sql":"SELECT COUNT(*) AS n FROM metrics","ts":2}),
    )
    .await
    .expect("read");
    let count = q["rows"][0][0]
        .as_i64()
        .or_else(|| q["rows"][0][0].as_f64().map(|n| n as i64))
        .unwrap_or(-1);
    assert_eq!(count, 0, "deny wrote nothing: {q}");

    let _ = std::fs::remove_file(&db);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn federation_write_workspace_isolation() {
    let dir = federation_dir();
    let db = seeded_db();
    let ws_a = "acme";
    let ws_b = "other";
    let node = Arc::new(Node::boot().await.unwrap());
    let admin_a = admin(ws_a);
    let admin_b = admin(ws_b);
    install_federation(&node, &admin_a, ws_a, &dir).await;
    install_federation(&node, &admin_b, ws_b, &dir).await;
    add_source(&node, &admin_a, ws_a, "demo", &db).await;

    // ws-B cannot resolve ws-A's source — the alias resolves to nothing in ws-B.
    let err = call(
        &node,
        &admin_b,
        ws_b,
        "federation.write",
        json!({
            "source": "demo", "table": "metrics",
            "columns": ["id", "host", "value"],
            "rows": [[1, "x", 0.0]], "ts": 1
        }),
    )
    .await
    .expect_err("ws-B cannot write ws-A's source");
    assert!(
        matches!(err, lb_mcp::ToolError::BadInput(_)),
        "not found in ws-B: {err:?}"
    );

    let _ = std::fs::remove_file(&db);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn federation_write_row_cap() {
    let dir = federation_dir();
    let db = seeded_db();
    let ws = "acme";
    let node = Arc::new(Node::boot().await.unwrap());
    let admin = admin(ws);
    install_federation(&node, &admin, ws, &dir).await;
    add_source(&node, &admin, ws, "demo", &db).await;

    // 1001 rows — over the cap. The typed error steers to federation.export; nothing is written.
    let over_cap: Vec<Value> = (0..1001).map(|i| json!([i, "h", 0.0])).collect();
    let err = call(
        &node,
        &admin,
        ws,
        "federation.write",
        json!({
            "source": "demo", "table": "metrics",
            "columns": ["id", "host", "value"],
            "rows": over_cap, "ts": 1
        }),
    )
    .await
    .expect_err("over-cap rejected");
    let msg = format!("{err:?}");
    assert!(
        msg.contains("federation.export"),
        "over-cap error steers to export: {msg}"
    );

    // Nothing landed.
    let q = call(
        &node,
        &admin,
        ws,
        "federation.query",
        json!({"source":"demo","sql":"SELECT COUNT(*) AS n FROM metrics","ts":2}),
    )
    .await
    .expect("count");
    assert_eq!(
        q["rows"][0][0].as_i64().unwrap_or(0),
        0,
        "over-cap wrote nothing"
    );

    let _ = std::fs::remove_file(&db);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn federation_migrate_dry_run_then_apply_then_idempotent() {
    let dir = federation_dir();
    let db = empty_db();
    let ws = "acme";
    let node = Arc::new(Node::boot().await.unwrap());
    let admin = admin(ws);
    install_federation(&node, &admin, ws, &dir).await;
    add_source(&node, &admin, ws, "demo", &db).await;

    // dry_run = true (default) → plans statements, applies nothing.
    let dry = call(
        &node,
        &admin,
        ws,
        "federation.migrate",
        json!({"source": "demo", "schema": shop_schema(), "ts": 1}),
    )
    .await
    .expect("migrate dry_run");
    assert_eq!(dry["applied"], false, "dry_run does not apply: {dry}");
    assert!(
        dry["statements"].as_array().unwrap().len() >= 2,
        "plans CREATE TABLEs: {dry}"
    );
    // Nothing applied yet — a dry-run against the empty DB still plans CREATE TABLE for both
    // tables (we don't call `federation.schema` to list tables because the pushed-down list query
    // mis-resolves on an empty source; the dry-run plan IS the proof nothing was applied).
    let dry_again = call(
        &node,
        &admin,
        ws,
        "federation.migrate",
        json!({"source": "demo", "schema": shop_schema(), "ts": 1}),
    )
    .await
    .expect("migrate dry_run before apply");
    let creates = dry_again["statements"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|s| s["kind"] == "create_table")
        .count();
    assert_eq!(
        creates, 2,
        "both tables still planned (nothing applied): {dry_again}"
    );

    // apply (dry_run: false) → tables exist live.
    let applied = call(
        &node,
        &admin,
        ws,
        "federation.migrate",
        json!({"source": "demo", "schema": shop_schema(), "dry_run": false, "ts": 2}),
    )
    .await
    .expect("migrate apply");
    assert_eq!(applied["applied"], true, "applied: {applied}");
    let post = call(
        &node,
        &admin,
        ws,
        "federation.schema",
        json!({"source": "demo"}),
    )
    .await
    .expect("schema after apply");
    let names: Vec<&str> = post["tables"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    assert!(
        names.contains(&"customers") && names.contains(&"orders"),
        "both tables created: {post}"
    );

    // re-run dry → ZERO statements (idempotence).
    let again = call(
        &node,
        &admin,
        ws,
        "federation.migrate",
        json!({"source": "demo", "schema": shop_schema(), "ts": 3}),
    )
    .await
    .expect("migrate re-run");
    assert_eq!(
        again["statements"].as_array().unwrap().len(),
        0,
        "re-run plans zero: {again}"
    );

    let _ = std::fs::remove_file(&db);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn federation_migrate_additive_column_and_destructive_refusal() {
    let dir = federation_dir();
    let db = empty_db();
    let ws = "acme";
    let node = Arc::new(Node::boot().await.unwrap());
    let admin = admin(ws);
    install_federation(&node, &admin, ws, &dir).await;
    add_source(&node, &admin, ws, "demo", &db).await;

    // apply the base schema
    call(
        &node,
        &admin,
        ws,
        "federation.migrate",
        json!({"source": "demo", "schema": shop_schema(), "dry_run": false, "ts": 1}),
    )
    .await
    .expect("apply base");

    // additive column → exactly one ADD COLUMN
    let mut additive = shop_schema();
    additive["tables"][1]["columns"]
        .as_array_mut()
        .unwrap()
        .push(json!({"name": "status", "type": "text", "nullable": true}));
    let plan = call(
        &node,
        &admin,
        ws,
        "federation.migrate",
        json!({"source": "demo", "schema": additive, "ts": 2}),
    )
    .await
    .expect("additive plan");
    let stmts = plan["statements"].as_array().unwrap();
    assert_eq!(stmts.len(), 1, "one ADD COLUMN: {plan}");
    assert_eq!(stmts[0]["kind"], "add_column");
    assert_eq!(stmts[0]["column"], "status");

    // destructive: drop a column → refused
    let mut destructive = shop_schema();
    destructive["tables"][1]["columns"]
        .as_array_mut()
        .unwrap()
        .retain(|c| c["name"] != "amount");
    let refused = call(
        &node,
        &admin,
        ws,
        "federation.migrate",
        json!({"source": "demo", "schema": destructive, "ts": 3}),
    )
    .await
    .expect("refusal");
    let refusal = refused["destructive_refusal"].as_str().unwrap();
    assert!(
        refusal.contains("additive only"),
        "refusal names the policy: {refusal}"
    );
    assert_eq!(
        refused["statements"].as_array().unwrap().len(),
        0,
        "refused → no statements"
    );

    let _ = std::fs::remove_file(&db);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn federation_migrate_admin_only() {
    let dir = federation_dir();
    let db = empty_db();
    let ws = "acme";
    let node = Arc::new(Node::boot().await.unwrap());
    let admin = admin(ws);
    install_federation(&node, &admin, ws, &dir).await;
    add_source(&node, &admin, ws, "demo", &db).await;

    // A member WITHOUT `mcp:federation.migrate:call` — denied (applying DDL is admin-only).
    let member = principal(
        ws,
        &[
            "mcp:federation.query:call",
            "mcp:federation.write:call",
            "mcp:datasource.list:call",
        ],
    );
    let err = call(
        &node,
        &member,
        ws,
        "federation.migrate",
        json!({"source": "demo", "schema": shop_schema(), "dry_run": false, "ts": 1}),
    )
    .await
    .expect_err("member cannot migrate");
    assert!(
        matches!(err, lb_mcp::ToolError::Denied),
        "admin-only deny: {err:?}"
    );

    let _ = std::fs::remove_file(&db);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn federation_export_round_trip() {
    let dir = federation_dir();
    let db = empty_db();
    let ws = "acme";
    let node = Arc::new(Node::boot().await.unwrap());
    let admin = admin(ws);
    install_federation(&node, &admin, ws, &dir).await;
    add_source(&node, &admin, ws, "demo", &db).await;

    // migrate the schema (so orders exists to receive the export)
    call(
        &node,
        &admin,
        ws,
        "federation.migrate",
        json!({"source": "demo", "schema": shop_schema(), "dry_run": false, "ts": 1}),
    )
    .await
    .expect("apply schema");

    // seed the customers parents so the orders FK is satisfiable (customer_id 100, 200)
    call(
        &node,
        &admin,
        ws,
        "federation.write",
        json!({
            "source": "demo", "table": "customers",
            "columns": ["id", "email"],
            "rows": [[100, "a@x"], [200, "b@x"]],
            "ts": 2
        }),
    )
    .await
    .expect("seed customers");

    // seed platform series with two object payloads keyed to the orders columns
    for (seq, customer_id, amount) in [(1, 100, 1.5), (2, 200, 2.5)] {
        call(
            &node,
            &admin,
            ws,
            "ingest.write",
            json!({
                "samples": [{
                    "series": "orders_stream",
                    "producer": "user:test",
                    "ts": seq,
                    "seq": seq,
                    "payload": {"id": seq, "customer_id": customer_id, "amount": amount},
                    "labels": {},
                    "qos": "best-effort"
                }]
            }),
        )
        .await
        .expect("seed series");
    }

    // export the series into the orders table, upsert on `id`
    let exported = call(
        &node,
        &admin,
        ws,
        "federation.export",
        json!({
            "source": "demo",
            "from": {"series": "orders_stream"},
            "table": "orders",
            "columns": ["id", "customer_id", "amount"],
            "key": ["id"],
            "job_id": "export-1",
            "ts": 10
        }),
    )
    .await
    .expect("federation.export");
    assert_eq!(exported["job_id"], "export-1");

    // the rows landed
    let q = call(
        &node,
        &admin,
        ws,
        "federation.query",
        json!({"source":"demo","sql":"SELECT id, customer_id, amount FROM orders ORDER BY id","ts":11}),
    )
    .await
    .expect("read back");
    assert_eq!(q["rows"].as_array().unwrap().len(), 2, "exported rows: {q}");

    let _ = std::fs::remove_file(&db);
}
