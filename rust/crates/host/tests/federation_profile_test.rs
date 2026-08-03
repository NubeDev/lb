//! The durable per-source discovery profile (datasource-profile scope) — the Docker-free e2e.
//!
//! Every mandatory category from the scope's testing plan, over the real stack: real embedded
//! SurrealDB, real caps, the REAL supervisor spawning the REAL `federation` sidecar. The external DB
//! is the ONE sanctioned fake-boundary (testing §0): a REAL on-disk SQLite engine with real rows.
//!
//!   - capability deny (mandatory), for BOTH the read cap and the separate refresh cap,
//!   - workspace isolation (mandatory), on the record AND on the reactor,
//!   - the happy path's record shape: kinds, cardinality, top values, min/max, FKs, group ranges,
//!   - bounds: a 30-table source truncates; a >200-distinct column reports a capped count,
//!   - redaction: a denylisted column's VALUES never travel,
//!   - idempotence: re-profiling an unchanged source upserts a byte-identical record,
//!   - `profile_get` is a PURE read (`NotFound` until asked to compute),
//!   - the reactor enqueues a stale profile exactly once and leaves a fresh one alone.
//!
//! Compiled only with the feature that compiles the subject.

#![cfg(feature = "datasource-profile")]

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

fn admin(ws: &str) -> Principal {
    principal(
        ws,
        &[
            "mcp:native.install:call",
            "mcp:native.call:call",
            "mcp:native.status:call",
            "mcp:federation.query:call",
            "mcp:federation.profile_refresh:call",
            "mcp:datasource.add:call",
            "mcp:datasource.list:call",
            "mcp:datasource.test:call",
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

/// A REAL long/EAV-shaped fixture — the shape this whole scope exists to describe.
///
/// `reading` is the fact table: one numeric `value`, a time column, and an FK to `point`. Each
/// point's values sit in a WILDLY different band (kW ~100s, °C ~20s, a 0/1 flag), which is the
/// metric-vs-place signal `group_ranges` must surface. `wide_text` carries >200 distinct values (the
/// cardinality cap) and `login.password` is denylisted (the redaction assertion).
///
/// `who` MUST be unique per test: cargo runs a binary's tests as THREADS OF ONE PROCESS, so a shared
/// path lets one test delete the fixture another is still reading (determinism is a hard rule).
fn seed_db(who: &str) -> String {
    let path = std::env::temp_dir().join(format!("lb-fed-profile-{}-{who}.db", std::process::id()));
    let _ = std::fs::remove_file(&path);
    let conn = rusqlite::Connection::open(&path).expect("open sqlite fixture");
    conn.execute_batch(
        "CREATE TABLE point (id INTEGER PRIMARY KEY, name TEXT NOT NULL, site TEXT);
         CREATE TABLE reading (time TEXT, point_id INTEGER REFERENCES point(id), value REAL);
         CREATE TABLE login (username TEXT, password TEXT);
         CREATE TABLE wide (wide_text TEXT);
         INSERT INTO point VALUES (1,'Demand kW','Westend'),(2,'Zone Temp','Westend'),
                                  (3,'Occupied','Northside');
         INSERT INTO login VALUES ('ada','hunter2'),('grace','trustno1');",
    )
    .expect("seed schema");
    // Bands: point 1 spans ~100..260, point 2 spans ~18..24, point 3 spans 0..1.
    let mut rows = String::new();
    for i in 0..60 {
        rows.push_str(&format!(
            "('2026-01-{:02}T00:00:00+00:00',1,{}),\
             ('2026-01-{:02}T00:00:00+00:00',2,{}),\
             ('2026-01-{:02}T00:00:00+00:00',3,{}),",
            (i % 28) + 1,
            100 + i * 2,
            (i % 28) + 1,
            18 + (i % 7),
            (i % 28) + 1,
            i % 2,
        ));
    }
    rows.pop();
    conn.execute_batch(&format!("INSERT INTO reading VALUES {rows};"))
        .expect("seed readings");
    // 250 distinct values — past MAX_DISTINCT_SCAN (200), so cardinality must report as capped.
    let wide: Vec<String> = (0..250).map(|i| format!("('v{i}')")).collect();
    conn.execute_batch(&format!("INSERT INTO wide VALUES {};", wide.join(",")))
        .expect("seed wide");
    path.to_string_lossy().into_owned()
}

/// A fixture with more tables than one pass may touch (the bounds assertion).
fn seed_many_tables(who: &str) -> String {
    let path =
        std::env::temp_dir().join(format!("lb-fed-profmany-{}-{who}.db", std::process::id()));
    let _ = std::fs::remove_file(&path);
    let conn = rusqlite::Connection::open(&path).expect("open sqlite fixture");
    let mut sql = String::new();
    for i in 0..30 {
        sql.push_str(&format!("CREATE TABLE t{i:02} (a TEXT, b REAL);"));
        sql.push_str(&format!("INSERT INTO t{i:02} VALUES ('x', 1.0);"));
    }
    conn.execute_batch(&sql).expect("seed 30 tables");
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

/// Pull one table's sub-object out of a profile record.
fn table_of<'a>(profile: &'a Value, name: &str) -> &'a Value {
    profile["tables"]
        .as_array()
        .expect("tables array")
        .iter()
        .find(|t| t["name"] == json!(name))
        .unwrap_or_else(|| panic!("table {name} in profile: {profile}"))
}

/// Pull one column's sub-object out of a table sub-object.
fn column_of<'a>(table: &'a Value, name: &str) -> &'a Value {
    table["columns"]
        .as_array()
        .expect("columns array")
        .iter()
        .find(|c| c["name"] == json!(name))
        .unwrap_or_else(|| panic!("column {name} in table: {table}"))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn profile_end_to_end_shape_bounds_and_idempotence() {
    let dir = federation_dir();
    let db = seed_db("e2e");
    let ws = "acme";
    let node = Arc::new(Node::boot().await.unwrap());
    let admin = admin(ws);
    install_federation(&node, &admin, ws, &dir).await;
    add_source(&node, &admin, ws, "demo", &db).await;

    // ── `profile_get` is a PURE READ: never profiled ⇒ NotFound, not a 20 s pass ──────────────
    let miss = call(
        &node,
        &admin,
        ws,
        "federation.profile_get",
        json!({"source":"demo","ts":2}),
    )
    .await
    .expect_err("an unprofiled source is NotFound, not silently computed");
    assert!(
        matches!(miss, lb_mcp::ToolError::BadInput(_)),
        "unprofiled read is a client error, not a stall: {miss:?}"
    );

    // ── the pass ──────────────────────────────────────────────────────────────────────────────
    let profile = call(
        &node,
        &admin,
        ws,
        "federation.profile",
        json!({"source":"demo","ts":3}),
    )
    .await
    .expect("federation.profile computes + stores");

    // NO DSN, anywhere. The record is destined for agent context; it travels further than a query
    // result, so this is the assertion that must never be allowed to regress.
    let as_text = profile.to_string();
    assert!(!as_text.contains(&db), "profile leaked the path DSN");

    // KINDS come from the Arrow type, never the column name (rule 10).
    let reading = table_of(&profile, "reading");
    assert_eq!(column_of(reading, "value")["kind"], json!("number"));
    assert_eq!(column_of(reading, "time")["kind"], json!("text"));

    // NUMERIC min/max + null fraction, computed in one aggregate for the whole table.
    let value = column_of(reading, "value");
    assert_eq!(value["min"].as_f64(), Some(0.0), "min of value: {value}");
    assert_eq!(value["max"].as_f64(), Some(218.0), "max of value: {value}");
    assert_eq!(value["null_frac"].as_f64(), Some(0.0));

    // TEXT cardinality + top values, on the FK parent that names the metric.
    let point = table_of(&profile, "point");
    let name = column_of(point, "name");
    assert_eq!(name["distinct"], json!(3), "point.name cardinality: {name}");
    let values: Vec<&str> = name["values"]
        .as_array()
        .expect("values array")
        .iter()
        .filter_map(|v| v.as_str())
        .collect();
    assert!(
        values.contains(&"Demand kW") && values.contains(&"Zone Temp"),
        "top values carry the metric names: {name}"
    );
    assert!(
        name.get("distinct_capped").is_none(),
        "a 3-value column is not capped: {name}"
    );

    // FOREIGN KEYS — the real catalog read, so a consumer can resolve `point_id` to "Point name".
    let fks = reading["foreign_keys"].as_array().expect("foreign_keys");
    assert!(
        fks.iter()
            .any(|f| f["column"] == json!("point_id") && f["ref_table"] == json!("point")),
        "reading.point_id → point is present: {reading}"
    );

    // GROUP RANGES — the metric-vs-place signal, computed server-side. `reading` has exactly one
    // numeric column, so its text columns get ranged against it.
    let ranges = reading["group_ranges"]["time"]
        .as_array()
        .expect("group_ranges for reading.time");
    assert!(!ranges.is_empty(), "grouped ranges present: {reading}");
    assert!(
        ranges
            .iter()
            .all(|r| r.get("lo").is_some() && r.get("hi").is_some()),
        "every group carries a [lo, hi] span: {reading}"
    );

    // CARDINALITY CAP — 250 distinct values, scanned to the 200 ceiling and reported as a FLOOR.
    let wide = column_of(table_of(&profile, "wide"), "wide_text");
    assert_eq!(
        wide["distinct"],
        json!(200),
        "capped at the scan ceiling: {wide}"
    );
    assert_eq!(wide["distinct_capped"], json!(true));
    assert!(
        wide["values"].as_array().expect("values").len() <= 60,
        "values are bounded to the retention cap: {wide}"
    );
    assert_eq!(
        profile["truncated"],
        json!(true),
        "a capped count makes the record honestly partial"
    );

    // REDACTION — the denylisted column keeps its shape but not its contents.
    let password = column_of(table_of(&profile, "login"), "password");
    assert_eq!(password["distinct"], json!(2), "shape survives: {password}");
    assert!(
        !as_text.contains("hunter2") && !as_text.contains("trustno1"),
        "denylisted values never travel"
    );
    let redacted: Vec<&str> = password["values"]
        .as_array()
        .expect("values")
        .iter()
        .filter_map(|v| v.as_str())
        .collect();
    assert!(
        redacted.iter().all(|v| *v == "«redacted»"),
        "denylisted values are «redacted»: {password}"
    );

    // ── `profile_get` now serves the stored record — one store read, no external touch ─────────
    let read_back = call(
        &node,
        &admin,
        ws,
        "federation.profile_get",
        json!({"source":"demo","ts":4}),
    )
    .await
    .expect("profile_get reads the stored record");
    assert_eq!(read_back["source"], json!("demo"));
    assert_eq!(read_back["profiled_at"], json!(3));

    // ── IDEMPOTENCE: re-profiling an unchanged source produces the same record ─────────────────
    let again = call(
        &node,
        &admin,
        ws,
        "federation.profile",
        json!({"source":"demo","ts":3}),
    )
    .await
    .expect("re-profile");
    assert_eq!(
        again, profile,
        "an unchanged source re-profiles to a byte-identical record (stable ordering)"
    );

    let _ = std::fs::remove_file(&db);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn profile_bounds_truncate_a_wide_source() {
    let dir = federation_dir();
    let db = seed_many_tables("bounds");
    let ws = "acme";
    let node = Arc::new(Node::boot().await.unwrap());
    let admin = admin(ws);
    install_federation(&node, &admin, ws, &dir).await;
    add_source(&node, &admin, ws, "many", &db).await;

    let profile = call(
        &node,
        &admin,
        ws,
        "federation.profile",
        json!({"source":"many","ts":3}),
    )
    .await
    .expect("federation.profile on a 30-table source");

    assert_eq!(
        profile["tables"].as_array().expect("tables").len(),
        25,
        "a 30-table source is cut to the 25-table ceiling"
    );
    assert_eq!(
        profile["truncated"],
        json!(true),
        "and says so, rather than reading as complete"
    );
    // Deterministic truncation: sorted names, so the SAME 25 land every time.
    assert_eq!(profile["tables"][0]["name"], json!("t00"));

    // Record size stays well inside the context-basket body budget (the scope's ~100 KB worst case).
    let bytes = profile.to_string().len();
    assert!(
        bytes < 100_000,
        "worst-case record stays prompt-sized: {bytes} bytes"
    );

    let _ = std::fs::remove_file(&db);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn profile_denies_and_isolates() {
    let dir = federation_dir();
    let db = seed_db("deny");
    let ws = "acme";
    let node = Arc::new(Node::boot().await.unwrap());
    let admin = admin(ws);
    install_federation(&node, &admin, ws, &dir).await;
    add_source(&node, &admin, ws, "demo", &db).await;
    call(
        &node,
        &admin,
        ws,
        "federation.profile",
        json!({"source":"demo","ts":3}),
    )
    .await
    .expect("seed a profile to read");

    // ── CAPABILITY DENY (mandatory): the read cap ─────────────────────────────────────────────
    // Reads ride `mcp:federation.query:call`; without it BOTH the compute and the read are opaque
    // denies — indistinguishable from "no such source", which is the point.
    let no_read = principal(ws, &["mcp:datasource.list:call"]);
    for verb in ["federation.profile", "federation.profile_get"] {
        let denied = call(&node, &no_read, ws, verb, json!({"source":"demo","ts":5}))
            .await
            .unwrap_err();
        assert!(
            matches!(denied, lb_mcp::ToolError::Denied),
            "opaque deny for {verb} without the read cap: {denied:?}"
        );
    }

    // ── CAPABILITY DENY (mandatory): the separate refresh cap ─────────────────────────────────
    // Holding the READ cap is explicitly NOT enough to spend external-DB work on demand.
    let reader = principal(ws, &["mcp:federation.query:call"]);
    let denied = call(
        &node,
        &reader,
        ws,
        "federation.profile_refresh",
        json!({"source":"demo","ts":6}),
    )
    .await
    .unwrap_err();
    assert!(
        matches!(denied, lb_mcp::ToolError::Denied),
        "the read cap does not grant refresh: {denied:?}"
    );
    // And the admin, who holds it, may.
    let queued = call(
        &node,
        &admin,
        ws,
        "federation.profile_refresh",
        json!({"source":"demo","ts":6}),
    )
    .await
    .expect("refresh with the cap");
    assert_eq!(queued["enqueued"], json!(true));
    // Idempotent: a second refresh while the job is queued returns the SAME job, not a second pass.
    let again = call(
        &node,
        &admin,
        ws,
        "federation.profile_refresh",
        json!({"source":"demo","ts":7}),
    )
    .await
    .expect("second refresh");
    assert_eq!(again["job_id"], queued["job_id"]);
    assert_eq!(
        again["enqueued"],
        json!(false),
        "a burst of refreshes collapses onto one durable job"
    );

    // ── WORKSPACE ISOLATION (mandatory) ───────────────────────────────────────────────────────
    let ws_b = "other";
    let admin_b = crate::admin(ws_b);
    install_federation(&node, &admin_b, ws_b, &dir).await;
    let iso = call(
        &node,
        &admin_b,
        ws_b,
        "federation.profile_get",
        json!({"source":"demo","ts":8}),
    )
    .await
    .expect_err("ws-B cannot read ws-A's profile");
    assert!(
        matches!(iso, lb_mcp::ToolError::BadInput(_)),
        "ws-B finds nothing at all: {iso:?}"
    );

    let _ = std::fs::remove_file(&db);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn profile_reactor_enqueues_stale_once_and_isolates_workspaces() {
    let dir = federation_dir();
    let db = seed_db("reactor");
    let ws = "acme";
    let ws_b = "other";
    let node = Arc::new(Node::boot().await.unwrap());
    let admin = admin(ws);
    install_federation(&node, &admin, ws, &dir).await;
    add_source(&node, &admin, ws, "demo", &db).await;
    // A profile stamped at logical time 3.
    call(
        &node,
        &admin,
        ws,
        "federation.profile",
        json!({"source":"demo","ts":3}),
    )
    .await
    .expect("seed a profile");

    let cfg = lb_host::ProfileReactorConfig {
        refresh_after_secs: 100,
        bounds: lb_host::ProfileBounds::default(),
    };

    // FRESH: `now` is inside the refresh window ⇒ the reactor does nothing at all.
    let fresh = lb_host::react_to_profiles(&node, ws, 50, cfg)
        .await
        .expect("a fresh pass");
    assert_eq!(fresh.enqueued, 0, "a fresh profile is not re-enqueued");
    assert_eq!(fresh.skipped, 0);

    // STALE: past the window ⇒ enqueued exactly once, then RUN by the same tick's drain.
    let stale = lb_host::react_to_profiles(&node, ws, 1_000, cfg)
        .await
        .expect("a stale pass");
    assert_eq!(
        stale.enqueued, 1,
        "the stale profile is enqueued: {stale:?}"
    );
    assert_eq!(stale.ran, 1, "and drained in the same tick: {stale:?}");
    assert_eq!(stale.failed, 0);

    // The rebuild landed: `profiled_at` moved forward and the in-flight guard was released.
    let rebuilt = lb_host::resolve_profile(&node.store, ws, "demo")
        .await
        .expect("store read")
        .expect("record present");
    assert_eq!(rebuilt.profiled_at, 1_000);
    assert_eq!(
        rebuilt.profiling_since, None,
        "landing the record releases the in-flight guard"
    );

    // NO DUPLICATE ENQUEUE: an immediate second tick sees a fresh record and does nothing.
    let second = lb_host::react_to_profiles(&node, ws, 1_001, cfg)
        .await
        .expect("second tick");
    assert_eq!(second.enqueued, 0, "no duplicate enqueue: {second:?}");
    assert_eq!(second.ran, 0);

    // WORKSPACE ISOLATION: a ws-B tick over the same store touches none of ws-A's records.
    let b = lb_host::react_to_profiles(&node, ws_b, 10_000, cfg)
        .await
        .expect("ws-B tick");
    assert_eq!(
        b,
        lb_host::ProfilePass::default(),
        "a ws-B tick never touches ws-A's profiles: {b:?}"
    );
    let still = lb_host::resolve_profile(&node.store, ws, "demo")
        .await
        .expect("store read")
        .expect("record present");
    assert_eq!(still.profiled_at, 1_000, "ws-A's record is untouched");

    let _ = std::fs::remove_file(&db);
}
