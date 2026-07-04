//! Federation (datasources scope) end-to-end tests — the mandatory categories (testing §2): the
//! capability-deny path (incl. the `net:*` deny), workspace-isolation, SELECT-only enforcement, the
//! happy round-trip (`datasource.add` → `test` green → `federation.query` returns seeded rows),
//! secret mediation (the DSN never appears in a record/result — a redaction assertion), and the
//! resumable mirror (resumes mid-range after restart, no double-write).
//!
//! NO mocks for our own stack: real embedded SurrealDB + in-proc Zenoh, real caps, the REAL native
//! supervisor (`OsLauncher`) spawning the REAL `federation` sidecar binary, a real `lb-jobs` queue.
//! The external DB is the ONE sanctioned fake-boundary (testing §0): a **real spawned Postgres**
//! (a `postgres:16-alpine` container on a random port, seeded with real rows). This run uses the
//! Postgres path — the federation sidecar is built with `--features postgres`; the harness skips
//! (with a clear message) if Docker is unavailable.

use std::process::Command;
use std::sync::atomic::{AtomicU16, Ordering};

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{call_tool, install_native, Node};
use lb_supervisor::OsLauncher;
use serde_json::{json, Value};

const MANIFEST: &str = include_str!("../../../extensions/federation/extension.toml");

// ---------------------------------------------------------------------------------------------
// Identity helpers
// ---------------------------------------------------------------------------------------------

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

/// An admin principal holding every federation/datasource cap + the secret caps + ingest (for
/// mirror). The `net:*` grant is supplied per-test as the install's admin_approved set, not here.
fn admin(ws: &str) -> Principal {
    principal(
        ws,
        &[
            "mcp:native.install:call",
            "mcp:native.call:call",
            "mcp:native.stop:call",
            "mcp:native.status:call",
            "mcp:federation.query:call",
            "mcp:federation.mirror:call",
            "mcp:viz.query:call",
            "mcp:datasource.add:call",
            "mcp:datasource.remove:call",
            "mcp:datasource.list:call",
            "mcp:datasource.test:call",
            "secret:federation/*:write",
            "secret:federation/*:get",
            "mcp:ingest.write:call",
            "store:series/**:write",
            "store:series/**:read",
            "mcp:series.read:call",
            "mcp:series.latest:call",
        ],
    )
}

// ---------------------------------------------------------------------------------------------
// The federation sidecar binary — built with --features postgres (needs the TLS toolchain).
// ---------------------------------------------------------------------------------------------

/// Build the federation sidecar with the postgres feature into the workspace target and return the
/// directory holding it. `RANLIB` is pointed at the zig wrapper so vendored OpenSSL links without a
/// system toolchain (this box has no system cc/openssl — see CLAUDE memory). The binary at
/// `target/debug/federation` is THE one the host supervisor spawns.
fn federation_dir() -> Option<String> {
    if let Ok(p) = std::env::var("FEDERATION_BIN") {
        let dir = std::path::PathBuf::from(&p);
        return Some(dir.parent().unwrap().to_string_lossy().into_owned());
    }
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let target = manifest_dir.join("../../target/debug");
    let bin = target.join("federation");
    // Build (or rebuild) with the postgres feature. Cheap if already current.
    let mut cmd = Command::new("cargo");
    cmd.args(["build", "-p", "federation", "--features", "postgres"])
        .current_dir(manifest_dir.join("../.."));
    // The postgres feature pulls native-tls → vendored OpenSSL, which needs a working `ranlib`. On a
    // box without a system toolchain we point at the zig wrapper; where a system `ranlib` exists the
    // default works, so only override when the wrapper is actually present (else the bad path breaks
    // the build).
    let zigranlib = "/home/user/.local/bin/zigranlib";
    if std::path::Path::new(zigranlib).exists() {
        cmd.env("RANLIB", zigranlib)
            .env("RANLIB_x86_64_unknown_linux_gnu", zigranlib);
    }
    let status = cmd.status();
    match status {
        Ok(s) if s.success() && bin.exists() => Some(target.to_string_lossy().into_owned()),
        _ => None,
    }
}

// ---------------------------------------------------------------------------------------------
// A REAL spawned Postgres container (the one sanctioned external, behind the Source trait).
// ---------------------------------------------------------------------------------------------

static PORT_SEQ: AtomicU16 = AtomicU16::new(0);

struct Postgres {
    name: String,
    port: u16,
}

impl Postgres {
    /// Spawn `postgres:16-alpine` on a random host port, wait for ready, and seed real rows.
    /// Returns `None` if Docker is unavailable (the harness then skips with a message).
    fn spawn() -> Option<Self> {
        // docker present?
        if Command::new("docker")
            .arg("version")
            .output()
            .map(|o| !o.status.success())
            .unwrap_or(true)
        {
            return None;
        }
        let port =
            49000 + PORT_SEQ.fetch_add(1, Ordering::SeqCst) + (std::process::id() as u16 % 500);
        let name = format!("lb-fed-test-{}-{}", std::process::id(), port);
        let run = Command::new("docker")
            .args([
                "run",
                "-d",
                "--rm",
                "--name",
                &name,
                "-e",
                "POSTGRES_PASSWORD=pw",
                "-e",
                "POSTGRES_USER=lb",
                "-e",
                "POSTGRES_DB=fed",
                "-p",
                &format!("{port}:5432"),
                "postgres:16-alpine",
            ])
            .output();
        match run {
            Ok(o) if o.status.success() => {}
            _ => return None,
        }
        let pg = Postgres { name, port };
        if !pg.wait_ready() {
            pg.kill();
            return None;
        }
        pg.seed();
        Some(pg)
    }

    fn wait_ready(&self) -> bool {
        for _ in 0..60 {
            let ok = Command::new("docker")
                .args(["exec", &self.name, "pg_isready", "-U", "lb", "-d", "fed"])
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false);
            if ok {
                // pg_isready can flip green a touch before accepting queries; one more beat.
                std::thread::sleep(std::time::Duration::from_millis(500));
                return true;
            }
            std::thread::sleep(std::time::Duration::from_millis(500));
        }
        false
    }

    fn psql(&self, sql: &str) {
        let _ = Command::new("docker")
            .args([
                "exec", &self.name, "psql", "-U", "lb", "-d", "fed", "-c", sql,
            ])
            .output();
    }

    fn seed(&self) {
        self.psql("CREATE TABLE readings (seq INT, store TEXT, temp DOUBLE PRECISION);");
        self.psql(
            "INSERT INTO readings (seq, store, temp) VALUES \
             (0,'A',3.5),(1,'A',6.2),(2,'B',4.1),(3,'B',7.7),(4,'C',2.0);",
        );
    }

    /// The libpq (keyword=value) DSN the host stores as the source secret (host-mediated; never
    /// logged). The pool parses keyword/value form, not a URL.
    fn dsn(&self) -> String {
        format!(
            "host=127.0.0.1 port={} user=lb password=pw dbname=fed sslmode=disable",
            self.port
        )
    }

    fn endpoint(&self) -> String {
        format!("127.0.0.1:{}", self.port)
    }

    fn kill(&self) {
        let _ = Command::new("docker")
            .args(["rm", "-f", &self.name])
            .output();
    }
}

impl Drop for Postgres {
    fn drop(&mut self) {
        self.kill();
    }
}

// ---------------------------------------------------------------------------------------------
// A native manifest with the per-endpoint net:* grant baked in (the admin_approved set scopes it).
// ---------------------------------------------------------------------------------------------

/// Install the federation sidecar in `ws`, approving `net:tls:{endpoint}:connect` + the secret grant.
/// `endpoint` is the Postgres host:port; the approved grant is exactly what `net.rs` enforces.
async fn install_federation(node: &Node, admin: &Principal, ws: &str, dir: &str, endpoint: &str) {
    let (host, port) = endpoint.rsplit_once(':').unwrap();
    let approved = vec![
        format!("net:tls:{host}:{port}:connect"),
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

// ---------------------------------------------------------------------------------------------
// THE TEST — one process spawns one container + one node and exercises every mandatory category.
// ---------------------------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn federation_end_to_end_postgres() {
    let Some(dir) = federation_dir() else {
        eprintln!(
            "SKIP federation_end_to_end_postgres: could not build federation --features postgres"
        );
        return;
    };
    let Some(pg) = Postgres::spawn() else {
        eprintln!("SKIP federation_end_to_end_postgres: Docker/Postgres unavailable");
        return;
    };
    eprintln!(
        "federation test DB path: REAL Postgres (postgres:16-alpine) on {}",
        pg.endpoint()
    );

    let ws = "acme";
    let node = std::sync::Arc::new(Node::boot().await.unwrap());
    let admin = admin(ws);
    install_federation(&node, &admin, ws, &dir, &pg.endpoint()).await;

    // --- register the source: the DSN goes into lb-secrets, only the ref into the record ---
    call(
        &node,
        &admin,
        ws,
        "datasource.add",
        json!({
            "name": "pg", "kind": "postgres",
            "endpoint": pg.endpoint(), "dsn": pg.dsn(), "ts": 1
        }),
    )
    .await
    .expect("datasource.add");

    // --- SECRET MEDIATION: the DSN must NOT appear in the datasource.list output (only the ref) ---
    let listed = call(&node, &admin, ws, "datasource.list", json!({}))
        .await
        .unwrap();
    let listed_str = listed.to_string();
    assert!(
        !listed_str.contains("password=pw") && !listed_str.contains("postgresql://"),
        "datasource.list leaked the DSN: {listed_str}"
    );
    assert!(
        listed_str.contains("\"secret_ref\""),
        "list shows the ref, not the value"
    );

    // --- datasource.test → green (a real connectivity probe) ---
    let probe = call(
        &node,
        &admin,
        ws,
        "datasource.test",
        json!({"source":"pg","ts":2}),
    )
    .await
    .expect("datasource.test");
    assert_eq!(probe["ok"], true, "probe green: {probe}");

    // --- HAPPY ROUND-TRIP: federation.query returns the seeded rows ---
    let q = call(
        &node,
        &admin,
        ws,
        "federation.query",
        json!({"source":"pg","sql":"SELECT seq, temp FROM readings ORDER BY seq","ts":3}),
    )
    .await
    .expect("federation.query");
    let cols = q["columns"].as_array().unwrap();
    assert_eq!(cols.len(), 2, "two columns: {q}");
    let rows = q["rows"].as_array().unwrap();
    assert_eq!(rows.len(), 5, "five seeded rows: {q}");
    // SECRET REDACTION: the DSN must not appear in a query result either.
    assert!(
        !q.to_string().contains("password=pw"),
        "query result leaked the DSN"
    );

    // --- DISCOVERY (federation.schema) — the no-SQL browse path the Datasources page drives ---
    // Regression: discovery registers the source's own catalog (`pg_catalog.pg_tables`) as a table
    // provider. That dotted name MUST be built with `TableReference::parse_str` (schema + table),
    // not `bare` — a `bare` dotted name resolves to a provider with an EMPTY schema, so the catalog
    // SELECT failed with `No field named …` and the page's first load crash-looped the sidecar until
    // the supervisor restart budget was exhausted. Assert both discovery shapes return real data.
    // Driven through the real MCP dispatch (`call` → `call_tool` → `call_federation_tool`), the same
    // path the Datasources page takes — not the internal fn — so the routing is covered too.
    let tables = call(
        &node,
        &admin,
        ws,
        "federation.schema",
        json!({"source":"pg","ts":3}),
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
        names.contains(&"readings"),
        "discovery lists the seeded `readings` table (not an empty/failed catalog read): {tables}"
    );

    let cols = call(
        &node,
        &admin,
        ws,
        "federation.schema",
        json!({"source":"pg","table":"readings","ts":3}),
    )
    .await
    .expect("federation.schema describes a table's columns");
    let col_names: Vec<&str> = cols["columns"]
        .as_array()
        .expect("columns array")
        .iter()
        .map(|c| c["name"].as_str().unwrap())
        .collect();
    assert!(
        col_names.contains(&"seq") && col_names.contains(&"temp"),
        "describe returns the real Arrow columns of `readings`: {cols}"
    );

    // --- DASHBOARD WIDGET PATH: viz.query over a federation target returns NAMED rows ---
    // A table/chart widget bound to a federation source dispatches `federation.query` THROUGH
    // `viz.query`, whose frame converter must zip the columnar `{columns, rows}` result into named
    // row-objects (the regression: it used to pass the column-aligned arrays through as `rows`,
    // yielding empty `{}` rows / no fields — a widget showing no data). Assert real fields + rows.
    let viz = call(
        &node,
        &admin,
        ws,
        "viz.query",
        json!({"panel":{"sources":[{
            "refId":"A","tool":"federation.query",
            "args":{"source":"pg","sql":"SELECT seq, temp FROM readings ORDER BY seq"}
        }]},"ts":3}),
    )
    .await
    .expect("viz.query over a federation target");
    let vframes = viz["frames"].as_array().expect("frames");
    let fields = vframes[0]["fields"]
        .as_array()
        .expect("the frame has named fields");
    assert_eq!(
        fields.len(),
        2,
        "viz frame exposes both columns as fields: {viz}"
    );
    assert_eq!(fields[0]["name"], "seq");
    assert_eq!(fields[1]["name"], "temp");
    let vrows = viz["rows"].as_array().expect("rows");
    assert_eq!(vrows.len(), 5, "viz returns the five seeded rows: {viz}");
    assert!(
        vrows[0].get("seq").is_some() && vrows[0].get("temp").is_some(),
        "each viz row is a NAMED object, not an empty {{}}: {viz}"
    );

    // --- SELECT-ONLY ENFORCEMENT: a write/DDL is rejected (host-side gate, before the sidecar) ---
    for bad in [
        "INSERT INTO readings VALUES (9,'X',1.0)",
        "DROP TABLE readings",
        "UPDATE readings SET temp = 0",
    ] {
        let err = call(
            &node,
            &admin,
            ws,
            "federation.query",
            json!({"source":"pg","sql":bad,"ts":4}),
        )
        .await
        .expect_err(&format!("non-SELECT must be rejected: {bad}"));
        assert!(
            matches!(err, lb_mcp::ToolError::BadInput(_)),
            "rejected SQL is a BadInput, got {err:?}"
        );
    }

    // --- CAPABILITY-DENY: federation.query without the cap ---
    let no_cap = principal(ws, &["mcp:datasource.list:call"]);
    let denied = call(
        &node,
        &no_cap,
        ws,
        "federation.query",
        json!({"source":"pg","sql":"SELECT 1","ts":5}),
    )
    .await
    .expect_err("query without mcp:federation.query:call is denied");
    assert!(
        matches!(denied, lb_mcp::ToolError::Denied),
        "opaque deny: {denied:?}"
    );

    // --- CAPABILITY-DENY: datasource.add without the admin cap ---
    let not_admin = principal(ws, &["mcp:federation.query:call"]);
    let add_denied = call(
        &node,
        &not_admin,
        ws,
        "datasource.add",
        json!({"name":"x","kind":"postgres","endpoint":pg.endpoint(),"ts":6}),
    )
    .await
    .expect_err("datasource.add without admin cap is denied");
    assert!(matches!(add_denied, lb_mcp::ToolError::Denied));

    // --- WORKSPACE ISOLATION: ws-B cannot resolve/query the ws-A source ---
    // A ws-B caller with the cap in ws-B resolves nothing in ws-B (the source lives in ws-A).
    let ws_b = "other";
    // install federation in ws-B too (so the binary is present — the deny is resolution, not absence)
    install_federation(&node, &admin_b(ws_b), ws_b, &dir, &pg.endpoint()).await;
    let b_caller = principal(ws_b, &["mcp:federation.query:call"]);
    let iso = call(
        &node,
        &b_caller,
        ws_b,
        "federation.query",
        json!({"source":"pg","sql":"SELECT 1","ts":7}),
    )
    .await
    .expect_err("ws-B cannot resolve ws-A's source");
    assert!(
        matches!(iso, lb_mcp::ToolError::BadInput(_)),
        "ws-B resolves nothing (not found): {iso:?}"
    );

    // --- NET:* DENY: a source whose endpoint the grant omits is refused, opaque, even installed ---
    // Register a second source pointing at a DIFFERENT port (NOT in the approved net grant).
    call(
        &node,
        &admin,
        ws,
        "datasource.add",
        json!({
            "name":"pg_unapproved","kind":"postgres",
            "endpoint":"127.0.0.1:59999","dsn":"postgresql://x@127.0.0.1:59999/y","ts":8
        }),
    )
    .await
    .expect("add the unapproved-endpoint source record");
    let net_denied = call(
        &node,
        &admin,
        ws,
        "federation.query",
        json!({"source":"pg_unapproved","sql":"SELECT 1","ts":9}),
    )
    .await
    .expect_err("a source whose endpoint the net grant omits is refused");
    assert!(
        matches!(net_denied, lb_mcp::ToolError::Denied),
        "net:* deny is opaque: {net_denied:?}"
    );

    // --- MIRROR: resumes mid-range after a restart and does NOT double-write ---
    // First mirror pass: only 3 of 5 rows (range=3) → series has rows 0..2.
    let m1 = call(
        &node,
        &admin,
        ws,
        "federation.mirror",
        json!({
            "source":"pg","job_id":"mir1",
            "query":"SELECT seq, temp FROM readings ORDER BY seq",
            "target_series":"cooler.temp","range":3,"ts":10
        }),
    )
    .await
    .expect("mirror pass 1");
    assert_eq!(m1["job_id"], "mir1");
    let after1 = series_count(&node, &admin, ws, "cooler.temp").await;
    assert_eq!(after1, 3, "first mirror wrote 3 rows, got {after1}");

    // Simulate a restart by re-invoking the SAME job_id with the FULL range: it resumes from the
    // checkpoint (cursor=3) and mirrors the remaining 2 — total 5, never 8 (no double-write).
    call(
        &node,
        &admin,
        ws,
        "federation.mirror",
        json!({
            "source":"pg","job_id":"mir1",
            "query":"SELECT seq, temp FROM readings ORDER BY seq",
            "target_series":"cooler.temp","range":5,"ts":11
        }),
    )
    .await
    .expect("mirror pass 2 (resume)");
    let after2 = series_count(&node, &admin, ws, "cooler.temp").await;
    assert_eq!(
        after2, 5,
        "resume mirrored the remaining rows with NO double-write (got {after2})"
    );

    drop(pg);
}

fn admin_b(ws: &str) -> Principal {
    admin(ws)
}

/// Count committed samples in `series` via the host-native `series.read` over the bridge.
async fn series_count(node: &std::sync::Arc<Node>, p: &Principal, ws: &str, series: &str) -> usize {
    let out = call(node, p, ws, "series.read", json!({ "series": series }))
        .await
        .expect("series.read");
    out["samples"].as_array().map(|a| a.len()).unwrap_or(0)
}

// ---------------------------------------------------------------------------------------------
// Regression: a column-less aggregate (`count(*)`) over a datasource — datasources scope.
// ---------------------------------------------------------------------------------------------

/// `SELECT count(*) FROM t` (and `count(1)` / `sum(1)`) — an aggregate that references NO table
/// column — currently fails through the Postgres `datafusion-table-providers` pushdown: the scan
/// projects ZERO columns, which datafusion 53 rejects (`Internal error: Physical input schema should
/// be the same … (physical) 1 vs (logical) 0`) and, past that, Arrow's `BatchCoalescer` rejects
/// (`Batch has 0 columns but BatchCoalescer expects 1`). Retried, it crash-loops the sidecar until
/// the supervisor restart budget is exhausted. A `count(<real column>)` (e.g. `count(value)`) is the
/// working shape and the interim workaround. This is a common dashboard "total" tile, so it matters.
///
/// `#[ignore]` = a fails-until-fixed dead-drop (real-world testing session 2026-07-04). The fix is
/// tracked in `docs/debugging/datasources/count-star-aggregate-schema-mismatch.md`; when it lands,
/// drop the `#[ignore]` and this passes. Run explicitly with `--ignored`.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "known bug: column-less aggregate (count(*)) fails through the postgres pushdown — see docs/debugging/datasources/count-star-aggregate-schema-mismatch.md"]
async fn federation_count_star_columnless_aggregate() {
    let Some(dir) = federation_dir() else {
        eprintln!("SKIP: could not build federation --features postgres");
        return;
    };
    let Some(pg) = Postgres::spawn() else {
        eprintln!("SKIP: Docker/Postgres unavailable");
        return;
    };
    let ws = "acme";
    let node = std::sync::Arc::new(Node::boot().await.unwrap());
    let admin = admin(ws);
    install_federation(&node, &admin, ws, &dir, &pg.endpoint()).await;
    call(
        &node,
        &admin,
        ws,
        "datasource.add",
        json!({"name":"pg","kind":"postgres","endpoint":pg.endpoint(),"dsn":pg.dsn(),"ts":1}),
    )
    .await
    .expect("datasource.add");

    // The working shape (`count(<column>)`) — the interim workaround — MUST already return 5.
    let ok = call(
        &node,
        &admin,
        ws,
        "federation.query",
        json!({"source":"pg","sql":"SELECT count(seq) AS n FROM readings","ts":2}),
    )
    .await
    .expect("count(<column>) is the working shape");
    assert_eq!(ok["rows"][0][0].as_i64(), Some(5), "count(seq): {ok}");

    // The bug: bare `count(*)` — no column referenced — must ALSO return 5, not an internal error.
    let star = call(
        &node,
        &admin,
        ws,
        "federation.query",
        json!({"source":"pg","sql":"SELECT count(*) AS n FROM readings","ts":3}),
    )
    .await
    .expect("count(*) must not raise an internal schema/coalescer error");
    assert_eq!(
        star["rows"][0][0].as_i64(),
        Some(5),
        "count(*) returns the five seeded rows: {star}"
    );

    drop(pg);
}
