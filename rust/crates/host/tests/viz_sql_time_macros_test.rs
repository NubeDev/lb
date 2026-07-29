//! The Grafana FUNCTION macros end to end (viz sql-time-macros scope): `$__timeGroup(col,'$__interval')`
//! / `$__timeFilter(col)` in a `federation.query` target are value-substituted + handed the derived
//! window by `viz.query` (additive `resolution`), then expanded PER ENGINE in the real federation
//! sidecar at query time — so one engine-agnostic SQL buckets correctly and re-coarsens with the
//! panel budget. Also the honesty contract: a direct call with a time macro but no `resolution`
//! fails NAMING the missing field; an unsupported macro fails naming the token; capability-deny and
//! workspace-isolation hold through macro'd targets (mandatory cases).
//!
//! NO mocks for our own stack (CLAUDE §9): real embedded SurrealDB, real caps, the REAL supervisor
//! spawning the REAL `federation` sidecar; the external DB is a REAL on-disk SQLite file.

use std::process::Command;
use std::sync::Arc;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{call_tool, install_native, Node};
use lb_supervisor::OsLauncher;
use serde_json::{json, Value};

const MANIFEST: &str = include_str!("../../federation/extension.toml");
const VIZ: &str = "mcp:viz.query:call";
const FED: &str = "mcp:federation.query:call";

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

fn admin(ws: &str) -> Principal {
    principal(
        ws,
        &[
            "mcp:native.install:call",
            "mcp:native.call:call",
            "mcp:native.status:call",
            VIZ,
            FED,
            "mcp:datasource.add:call",
            "mcp:datasource.list:call",
            "secret:federation/*:write",
            "secret:federation/*:get",
        ],
    )
}

/// Build the sidecar with DEFAULT features (sqlite only — no external toolchain).
fn federation_dir() -> String {
    if let Ok(p) = std::env::var("FEDERATION_BIN") {
        return std::path::PathBuf::from(&p)
            .parent()
            .unwrap()
            .to_string_lossy()
            .into_owned();
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

/// A REAL `.db` with an epoch-ms series `readings(t INTEGER, v REAL)` — 100 rows at t = 1000..100000
/// (step 1000 ms), value 20.0 with ONE 200.0 spike at t = 50000. Epoch-ms INTEGER is the documented
/// v1 sqlite timestamp-column assumption of the expansion table.
fn seed_db(tag: &str) -> String {
    let path = std::env::temp_dir().join(format!(
        "lb-sql-time-macros-{}-{tag}.db",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&path);
    let conn = rusqlite::Connection::open(&path).expect("open sqlite fixture");
    conn.execute_batch("CREATE TABLE readings (t INTEGER, v REAL);")
        .expect("create table");
    let tx = conn.unchecked_transaction().unwrap();
    for i in 1..=100u64 {
        let t = i * 1000;
        let v = if i == 50 { 200.0 } else { 20.0 };
        tx.execute("INSERT INTO readings (t, v) VALUES (?1, ?2)", (t, v))
            .expect("insert row");
    }
    tx.commit().unwrap();
    path.to_string_lossy().into_owned()
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

async fn call(node: &Arc<Node>, p: &Principal, ws: &str, tool: &str, input: Value) -> Value {
    let out = call_tool(node, p, ws, tool, &input.to_string())
        .await
        .expect("tool call");
    serde_json::from_str(&out).unwrap()
}

async fn add_source(node: &Arc<Node>, admin: &Principal, ws: &str, name: &str, dsn: &str) {
    call(
        node,
        admin,
        ws,
        "datasource.add",
        json!({"name": name, "kind":"sqlite", "endpoint":"127.0.0.1:0", "dsn": dsn, "ts": 1}),
    )
    .await;
}

/// One engine-agnostic macro'd SQL — the exact emission the Quick Chart builder produces.
const MACROD: &str = "SELECT $__timeGroup(t, '$__interval') AS bucket, \
                      avg(v) AS avg, max(v) AS max FROM readings \
                      WHERE $__timeFilter(t) \
                      GROUP BY $__timeGroup(t, '$__interval') ORDER BY bucket";

/// A one-target federation panel with the given SQL + numeric window (+ optional point budget).
fn fed_panel(sql: &str, from: u64, to: u64, max_points: Option<u64>) -> Value {
    let mut panel = json!({
        "sources": [{
            "refId": "A",
            "datasource": { "type": "federation", "uid": "datasource:acme:demo" },
            "tool": "federation.query",
            "args": { "source": "demo", "sql": sql, "from": from, "to": to }
        }],
        "transformations": []
    });
    if let Some(n) = max_points {
        panel["queryOptions"] = json!({ "maxDataPoints": n });
    }
    panel
}

async fn viz_rows(node: &Arc<Node>, p: &Principal, ws: &str, panel: Value) -> Vec<Value> {
    let out = call(
        node,
        p,
        ws,
        "viz.query",
        json!({ "panel": panel, "now": 1 }),
    )
    .await;
    out["rows"].as_array().cloned().unwrap_or_default()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn function_macros_expand_per_engine_and_recoarsen_with_the_budget() {
    let ws = "acme";
    let dir = federation_dir();
    let db = seed_db(ws);
    let node = Arc::new(Node::boot().await.unwrap());
    let admin = admin(ws);
    install_federation(&node, &admin, ws, &dir).await;
    add_source(&node, &admin, ws, "demo", &db).await;

    // Default budget (1000) over [0, 101000] derives 1s buckets → one bucket per seeded row. The
    // macros expanded in the CHILD (sqlite integer-floor form) — an unexpanded `$__timeGroup` would
    // be a SQL error, not rows.
    let rows = viz_rows(&node, &admin, ws, fed_panel(MACROD, 0, 101_000, None)).await;
    assert_eq!(rows.len(), 100, "1s buckets → one per row: {}", rows.len());
    assert!(
        rows[0].get("bucket").is_some() && rows[0].get("avg").is_some(),
        "the author's aggregate shape survives: {}",
        rows[0]
    );
    // The seeded spike survives in the author's max() — buckets are real, not resampled.
    assert!(
        rows.iter().any(|r| r["max"].as_f64() == Some(200.0)),
        "the 200 spike survives in a bucket max"
    );

    // Same SQL, budget 10 → the derivation coarsens to 30s buckets and the CHILD re-expands: ≤ 10
    // buckets (the panel-resolution invariant re-asserted through the FUNCTION macros — zoom/budget
    // changes flow through with no client change).
    let coarse = viz_rows(&node, &admin, ws, fed_panel(MACROD, 0, 101_000, Some(10))).await;
    assert!(
        coarse.len() <= 10 && !coarse.is_empty(),
        "budget 10 → ≤10 coarser buckets, got {}",
        coarse.len()
    );
    assert!(
        coarse.iter().any(|r| r["max"].as_f64() == Some(200.0)),
        "the spike still survives coarser buckets"
    );

    // The window actually filters: a half-open [0, 50000) window excludes the t=50000 spike row.
    let windowed = viz_rows(&node, &admin, ws, fed_panel(MACROD, 0, 50_000, None)).await;
    assert!(
        !windowed.is_empty() && windowed.iter().all(|r| r["max"].as_f64() != Some(200.0)),
        "$__timeFilter is a half-open range — t=50000 is outside [0, 50000)"
    );

    let _ = std::fs::remove_file(&db);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn named_errors_missing_resolution_and_unsupported_macro() {
    let ws = "acme-err";
    let dir = federation_dir();
    let db = seed_db(ws);
    let node = Arc::new(Node::boot().await.unwrap());
    let admin = admin(ws);
    install_federation(&node, &admin, ws, &dir).await;
    add_source(&node, &admin, ws, "demo", &db).await;

    // A DIRECT federation.query with a time macro and no `resolution` → the error NAMES the missing
    // field and the fix (never a guess, never a cryptic engine error).
    let err = call_tool(
        &node,
        &admin,
        ws,
        "federation.query",
        &json!({"source": "demo", "sql": "SELECT max(v) FROM readings WHERE $__timeFilter(t)", "ts": 1})
            .to_string(),
    )
    .await
    .expect_err("time macro with no resolution must fail");
    let msg = err.to_string();
    assert!(
        msg.contains("resolution") && msg.contains("viz.query"),
        "names the field and the fix: {msg}"
    );

    // The same call WITH an explicit resolution succeeds — the additive field is caller-usable.
    let ok = call(
        &node,
        &admin,
        ws,
        "federation.query",
        json!({"source": "demo", "sql": "SELECT max(v) AS m FROM readings WHERE $__timeFilter(t)",
               "resolution": {"from_ms": 0, "to_ms": 101_000, "width_ms": 1_000}, "ts": 2}),
    )
    .await;
    assert_eq!(
        ok["rows"][0][0].as_f64(),
        Some(200.0),
        "explicit resolution executes: {ok}"
    );

    // An unsupported macro → named token (the honesty contract; imported exotic panels say WHY).
    let err = call_tool(
        &node,
        &admin,
        ws,
        "federation.query",
        &json!({"source": "demo", "sql": "SELECT $__unixEpochFilter(t) FROM readings", "ts": 3})
            .to_string(),
    )
    .await
    .expect_err("unsupported macro must fail");
    assert!(
        err.to_string()
            .contains("unsupported macro $__unixEpochFilter"),
        "names the token: {err}"
    );

    let _ = std::fs::remove_file(&db);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn macro_target_deny_and_workspace_isolation_hold() {
    let ws = "acme-deny";
    let dir = federation_dir();
    let db = seed_db(ws);
    let node = Arc::new(Node::boot().await.unwrap());
    let owner = admin(ws);
    install_federation(&node, &owner, ws, &dir).await;
    add_source(&node, &owner, ws, "demo", &db).await;

    // Capability-deny (MANDATORY): viz.query but NOT federation.query → honest empty, no bypass
    // through the macro path.
    let viewer = principal(ws, &[VIZ]);
    let rows = viz_rows(&node, &viewer, ws, fed_panel(MACROD, 0, 101_000, None)).await;
    assert!(rows.is_empty(), "denied macro'd target → honest empty");

    // Workspace-isolation (MANDATORY): ws-B, full caps, naming ws-A's source through a macro'd
    // target resolves NOTHING (the wall is structural at the namespace — same as un-macro'd).
    let intruder = admin("other-ws");
    let rows = viz_rows(
        &node,
        &intruder,
        "other-ws",
        fed_panel(MACROD, 0, 101_000, None),
    )
    .await;
    assert!(rows.is_empty(), "cross-workspace macro'd target → empty");

    let _ = std::fs::remove_file(&db);
}
