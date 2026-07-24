//! Interval-macro substitution end to end through `viz.query` over a REAL SQLite datasource (viz
//! panel-resolution scope, issue #101, slice 2). A `federation.query` chart target whose SQL carries
//! `$__interval_ms` / `$__timeFrom` / `$__timeTo` is substituted by the resolver BEFORE dispatch, so the
//! warehouse aggregates into buckets over the visible window — the same negotiation the platform series
//! path gets, by pushdown. And the load-bearing invariant: an UN-macro'd SQL is left byte-identical, so
//! a hand-SQL tile runs verbatim (asserted by parity against a direct `federation.query`).
//!
//! NO mocks for our own stack (CLAUDE §9): real embedded SurrealDB, real caps, the REAL supervisor
//! spawning the REAL `federation` sidecar. The external DB is the one sanctioned fake-boundary
//! (testing §0): a REAL on-disk SQLite engine with real rows — no Docker (default sqlite features).

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

/// Seed a REAL `.db` with a numeric-ms time series `readings(t INTEGER, v REAL)` — 100 rows at
/// t = 1000..100000 (step 1000ms), value 20.0 everywhere with ONE 200.0 spike at t=50000.
fn seed_db(tag: &str) -> String {
    let path = std::env::temp_dir().join(format!("lb-viz-macros-{}-{tag}.db", std::process::id()));
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

/// A one-target federation panel with the given SQL (macro'd or not) + a numeric window.
fn fed_panel(sql: &str, from: u64, to: u64) -> Value {
    json!({
        "sources": [{
            "refId": "A",
            "datasource": { "type": "federation", "uid": "datasource:acme:demo" },
            "tool": "federation.query",
            "args": { "source": "demo", "sql": sql, "from": from, "to": to }
        }],
        "transformations": []
    })
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
async fn macros_substitute_and_bucket_over_real_sqlite() {
    let ws = "acme";
    let dir = federation_dir();
    let db = seed_db(ws);
    let node = Arc::new(Node::boot().await.unwrap());
    let admin = admin(ws);
    install_federation(&node, &admin, ws, &dir).await;
    add_source(&node, &admin, ws, "demo", &db).await;

    // A macro'd bucket aggregate: integer-ms bucketing keyed on the derived width, windowed by the
    // derived from/to. `$__interval_ms` and `$__timeFrom`/`$__timeTo` are substituted by the resolver.
    let macrod = "SELECT (t / $__interval_ms) * $__interval_ms AS bucket, \
                  avg(v) AS avg, max(v) AS max, min(v) AS min \
                  FROM readings WHERE t BETWEEN $__timeFrom AND $__timeTo \
                  GROUP BY bucket ORDER BY bucket";
    // Window [0, 101000] over 100 rows; default budget 1000 → width 1s (1000ms) → one bucket per row.
    let rows = viz_rows(&node, &admin, ws, fed_panel(macrod, 0, 101_000)).await;
    assert!(
        !rows.is_empty(),
        "the macro'd aggregate returned bucket rows"
    );
    // The aggregate shape the author asked for survived (a `bucket` + `max` column) — proof the macros
    // substituted (an un-substituted `$__interval_ms` would be a SQL error, not rows).
    assert!(
        rows[0].get("bucket").is_some() && rows[0].get("max").is_some(),
        "bucketed aggregate columns present: {}",
        rows[0]
    );
    // The spike survives in the author's max() (the federation macro path yields whatever the SELECT
    // computes — here an explicit max(), so the 200 peak is on screen).
    assert!(
        rows.iter().any(|r| r["max"].as_f64() == Some(200.0)),
        "the seeded 200 spike survives in a bucket max"
    );

    // BYTE-IDENTICAL un-macro'd SQL: a macro-free federation target run through viz.query returns the
    // SAME rows as a DIRECT federation.query with that SQL — the resolver never rewrote it.
    let plain = "SELECT t, v FROM readings ORDER BY t";
    let via_viz = viz_rows(&node, &admin, ws, fed_panel(plain, 0, 101_000)).await;
    let direct = call(
        &node,
        &admin,
        ws,
        "federation.query",
        json!({ "source": "demo", "sql": plain, "ts": 2 }),
    )
    .await;
    let direct_rows: Vec<Value> = {
        // federation.query returns columnar {columns, rows}; zip to compare row values.
        let cols: Vec<&str> = direct["columns"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c.as_str().unwrap())
            .collect();
        direct["rows"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| {
                let cells = r.as_array().unwrap();
                let obj: serde_json::Map<String, Value> = cols
                    .iter()
                    .zip(cells)
                    .map(|(c, v)| (c.to_string(), v.clone()))
                    .collect();
                Value::Object(obj)
            })
            .collect()
    };
    assert_eq!(
        via_viz.len(),
        direct_rows.len(),
        "un-macro'd SQL through viz.query returns the same row count as a direct query"
    );
    assert_eq!(
        via_viz, direct_rows,
        "un-macro'd SQL is byte-identical: viz.query did not rewrite a macro-free SELECT"
    );

    let _ = std::fs::remove_file(&db);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn macro_target_denied_without_federation_cap_is_honest_empty() {
    let ws = "acme-deny";
    let dir = federation_dir();
    let db = seed_db(ws);
    let node = Arc::new(Node::boot().await.unwrap());
    let admin = admin(ws);
    install_federation(&node, &admin, ws, &dir).await;
    add_source(&node, &admin, ws, "demo", &db).await;

    // A viewer holding viz.query but NOT federation.query → the macro target is denied INSIDE the
    // resolver → an honest empty frame (no bypass, no leak — MANDATORY deny, re-asserted through the
    // macro-substituted args).
    let viewer = principal(ws, &[VIZ]);
    let macrod =
        "SELECT max(v) AS max FROM readings WHERE t BETWEEN $__timeFrom AND $__timeTo GROUP BY (t/$__interval_ms)";
    let rows = viz_rows(&node, &viewer, ws, fed_panel(macrod, 0, 101_000)).await;
    assert!(
        rows.is_empty(),
        "denied federation macro target → honest empty ({} rows)",
        rows.len()
    );

    let _ = std::fs::remove_file(&db);
}
