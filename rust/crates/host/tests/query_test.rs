//! The saved-PRQL-query surface (`query.*`) headless, against the REAL stack (query scope, testing
//! §0/§2): real embedded SurrealDB + in-proc Zenoh, real caps, the REAL `store.query` read-only gate,
//! and (in the Postgres test) the REAL federation sidecar against a REAL spawned Postgres — reusing
//! the datasources test rig. NO mocks for our own stack.
//!
//! Mandatory categories covered:
//!   - **capability-deny per verb** — each `query.*` denied without its cap;
//!   - **the HEADLINE no-widening deny** — `query.run` on a datasource target is denied when the
//!     caller lacks `mcp:federation.query:call` EVEN WITH `mcp:query.run:call`; a platform-target
//!     run requires `mcp:store.query:call`;
//!   - **workspace-isolation** — ws-B cannot get/run/delete a ws-A query;
//!   - **compile correctness** — `query.compile` returns SQL; a malformed PRQL is a typed error; a
//!     platform PRQL's SQL passes the `store.query` gate; a `lang:"raw"` write is rejected (read-first);
//!   - **params binding** (injection-safe) — `$var` binds through `store.query`'s vars path; a
//!     missing/extra param is a typed error;
//!   - **round-trip** — save → get → edit → save → run returns seeded rows from real platform data,
//!     and (Postgres) from a real external DB; a rule reads `source("query:<name>")` end to end.

use std::sync::Arc;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{call_tool, Node};
use lb_mcp::ToolError;
use serde_json::{json, Value};

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
    verify(&key, &mint(&key, &claims), 1).unwrap()
}

const SAVE: &str = "mcp:query.save:call";
const GET: &str = "mcp:query.get:call";
const LIST: &str = "mcp:query.list:call";
const DEL: &str = "mcp:query.delete:call";
const RUN: &str = "mcp:query.run:call";
const COMPILE: &str = "mcp:query.compile:call";
const STORE_Q: &str = "mcp:store.query:call";
const INGEST_W: &str = "mcp:ingest.write:call";
const RULES_RUN: &str = "mcp:rules.run:call";

/// A caller holding every query cap + store.query + ingest (to seed) + rules.run (for the rule test).
fn admin(ws: &str) -> Principal {
    principal(
        "user:ada",
        ws,
        &[
            SAVE, GET, LIST, DEL, RUN, COMPILE, STORE_Q, INGEST_W, RULES_RUN,
        ],
    )
}

async fn call(
    node: &Arc<Node>,
    p: &Principal,
    ws: &str,
    tool: &str,
    input: Value,
) -> Result<Value, ToolError> {
    let out = call_tool(node, p, ws, tool, &input.to_string()).await?;
    Ok(serde_json::from_str(&out).unwrap())
}

/// Seed `n` real samples into `series` in `ws` through the real ingest write path.
async fn seed_series(node: &Arc<Node>, p: &Principal, ws: &str, series: &str, n: u64) {
    let samples: Vec<_> = (1..=n)
        .map(|seq| {
            json!({ "series": series, "producer": "x", "ts": seq, "seq": seq, "payload": json!(seq as f64), "qos": "best-effort" })
        })
        .collect();
    call(node, p, ws, "ingest.write", json!({ "samples": samples }))
        .await
        .unwrap();
}

// ---------------------------------------------------------------------------------------------
// Capability deny per verb
// ---------------------------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn each_verb_denied_without_its_cap() {
    let ws = "q-deny";
    let node = Arc::new(Node::boot().await.unwrap());
    // A caller holding ONLY query.run — no save/get/list/delete/compile.
    let p = principal("user:nocap", ws, &[RUN, STORE_Q]);
    let err = call(
        &node,
        &p,
        ws,
        "query.save",
        json!({ "id": "x", "lang": "prql", "text": "from t", "target": "platform" }),
    )
    .await
    .unwrap_err();
    assert!(matches!(err, ToolError::Denied), "save deny, got {err:?}");

    let err = call(&node, &p, ws, "query.get", json!({ "id": "x" }))
        .await
        .unwrap_err();
    assert!(matches!(err, ToolError::Denied), "get deny, got {err:?}");

    let err = call(&node, &p, ws, "query.list", json!({}))
        .await
        .unwrap_err();
    assert!(matches!(err, ToolError::Denied), "list deny, got {err:?}");

    let err = call(&node, &p, ws, "query.delete", json!({ "id": "x" }))
        .await
        .unwrap_err();
    assert!(matches!(err, ToolError::Denied), "delete deny, got {err:?}");

    let err = call(
        &node,
        &p,
        ws,
        "query.compile",
        json!({ "lang": "prql", "text": "from t", "target": "platform" }),
    )
    .await
    .unwrap_err();
    assert!(
        matches!(err, ToolError::Denied),
        "compile deny, got {err:?}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn headline_no_widening_run_requires_target_cap() {
    let ws = "q-nowiden";
    let node = Arc::new(Node::boot().await.unwrap());
    let ada = admin(ws);
    seed_series(&node, &ada, ws, "cpu", 3).await;

    // Save a platform query as admin. (`select { payload }` projects a column that is NOT the table
    // name — PRQL would otherwise emit `SELECT *, …` and pull SurrealDB's record `id`, which doesn't
    // deserialize to JSON. Selecting non-table-name columns is the supported relational subset.)
    call(
        &node,
        &ada,
        ws,
        "query.save",
        json!({ "id": "top", "name": "top", "lang": "prql", "text": "from series | select { payload } | take 100", "target": "platform", "ts": 1 }),
    )
    .await
    .unwrap();

    // A caller with query.run but NOT store.query → platform run DENIED (no widening).
    let runner_no_store = principal("user:r", ws, &[RUN]);
    let err = call(
        &node,
        &runner_no_store,
        ws,
        "query.run",
        json!({ "id": "top" }),
    )
    .await
    .unwrap_err();
    assert!(
        matches!(err, ToolError::Denied),
        "platform run without store.query cap must DENY, got {err:?}"
    );

    // Same caller holds query.run + store.query → run succeeds.
    let runner = principal("user:r", ws, &[RUN, STORE_Q]);
    let out = call(&node, &runner, ws, "query.run", json!({ "id": "top" }))
        .await
        .unwrap();
    assert_eq!(
        out["rows"].as_array().unwrap().len(),
        3,
        "seeded rows: {out}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn headline_no_widening_datasource_run_denied_without_federation_cap() {
    let ws = "q-fed-nowiden";
    let node = Arc::new(Node::boot().await.unwrap());
    let ada = admin(ws);
    // A datasource-target query (no datasource registered; the deny must bite at the CAP, before
    // resolution — proving the headline even when the source is absent). We save it as admin.
    call(
        &node,
        &ada,
        ws,
        "query.save",
        json!({ "id": "wh", "name": "warehouse", "lang": "prql", "text": "from orders | take 1", "target": "datasource:warehouse", "ts": 1 }),
    )
    .await
    .unwrap();

    // Caller holds query.run but NOT federation.query → DENIED, even though query.run is held. The
    // no-widening rule: the target cap is checked BEFORE datasource resolution.
    let runner = principal("user:r", ws, &[RUN]);
    let err = call(&node, &runner, ws, "query.run", json!({ "id": "wh" }))
        .await
        .unwrap_err();
    assert!(
        matches!(err, ToolError::Denied),
        "datasource run without federation.query cap must DENY (no widening), got {err:?}"
    );
}

// ---------------------------------------------------------------------------------------------
// Workspace isolation
// ---------------------------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn workspace_b_cannot_get_run_delete_ws_a_query() {
    let node = Arc::new(Node::boot().await.unwrap());
    let ada = admin("ws-a");
    let ben = principal("user:ben", "ws-b", &[GET, RUN, DEL, STORE_Q, LIST]);

    // Ada saves a query in ws-a.
    call(
        &node,
        &ada,
        "ws-a",
        "query.save",
        json!({ "id": "secret", "name": "secret", "lang": "prql", "text": "from series | take 1", "target": "platform", "ts": 1 }),
    )
    .await
    .unwrap();

    // Ben (ws-b) get by the SAME id → NotFound (resolves to nothing in ws-b). NOT the ws-a record.
    let err = call(&node, &ben, "ws-b", "query.get", json!({ "id": "secret" }))
        .await
        .unwrap_err();
    assert!(
        matches!(err, ToolError::BadInput(_)),
        "ws-b get of ws-a id should be not-found, got {err:?}"
    );

    // Ben run → NotFound (no widening, no cross-tenant reach).
    let err = call(&node, &ben, "ws-b", "query.run", json!({ "id": "secret" }))
        .await
        .unwrap_err();
    assert!(
        matches!(err, ToolError::BadInput(_)),
        "ws-b run of ws-a id should be not-found, got {err:?}"
    );

    // Ben delete is a no-op tombstone on a ws-b record that doesn't exist; ws-a's record survives.
    call(
        &node,
        &ben,
        "ws-b",
        "query.delete",
        json!({ "id": "secret", "ts": 2 }),
    )
    .await
    .unwrap();
    let still = call(&node, &ada, "ws-a", "query.get", json!({ "id": "secret" }))
        .await
        .unwrap();
    assert_eq!(
        still["id"],
        json!("secret"),
        "ws-a record untouched by ws-b"
    );

    // Ben's list sees nothing from ws-a.
    let bens = call(&node, &ben, "ws-b", "query.list", json!({}))
        .await
        .unwrap();
    assert!(
        bens["queries"].as_array().unwrap().is_empty(),
        "ws-b list leaks nothing: {bens}"
    );
}

// ---------------------------------------------------------------------------------------------
// Compile correctness + read-only gate
// ---------------------------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn compile_returns_sql_and_malformed_is_typed_error() {
    let ws = "q-compile";
    let node = Arc::new(Node::boot().await.unwrap());
    let p = principal("user:c", ws, &[COMPILE]);

    let out = call(
        &node,
        &p,
        ws,
        "query.compile",
        json!({ "lang": "prql", "text": "from orders | take 5", "target": "platform" }),
    )
    .await
    .unwrap();
    let sql = out["sql"].as_str().unwrap();
    assert!(
        sql.to_string().split_whitespace().any(|t| t == "SELECT"),
        "sql: {sql}"
    );
    assert!(
        sql.contains("LIMIT 5") || sql.contains("LIMIT\n  5") || sql.contains("LIMIT"),
        "sql: {sql}"
    );

    let err = call(
        &node,
        &p,
        ws,
        "query.compile",
        json!({ "lang": "prql", "text": "from | select {", "target": "platform" }),
    )
    .await
    .unwrap_err();
    assert!(
        matches!(err, ToolError::BadInput(ref m) if m.contains("compile")),
        "malformed prql is typed BadInput, got {err:?}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn raw_platform_write_rejected_by_read_only_gate() {
    let ws = "q-readonly";
    let node = Arc::new(Node::boot().await.unwrap());
    let ada = admin(ws);
    seed_series(&node, &ada, ws, "cpu", 1).await;

    // A `lang:"raw"` DELETE against platform — the store.query parse-allowlist rejects it (read-first).
    let err = call(
        &node,
        &ada,
        ws,
        "query.run",
        json!({ "lang": "raw", "text": "DELETE series", "target": "platform" }),
    )
    .await
    .unwrap_err();
    assert!(
        matches!(err, ToolError::BadInput(_)),
        "raw write rejected by gate, got {err:?}"
    );
}

// ---------------------------------------------------------------------------------------------
// Params binding (injection-safe) — platform path through store.query vars
// ---------------------------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn params_bind_safely_through_store_query_vars() {
    let ws = "q-params";
    let node = Arc::new(Node::boot().await.unwrap());
    let ada = admin(ws);
    seed_series(&node, &ada, ws, "cpu", 3).await;
    seed_series(&node, &ada, ws, "mem", 2).await;

    // A raw SurrealQL query with a `$name` placeholder, declared param "name". The value binds via
    // store.query's vars path — never string interpolation (injection-safe).
    call(
        &node,
        &ada,
        ws,
        "query.save",
        json!({
            "id": "byname", "name": "byname",
            "lang": "raw",
            "text": "SELECT series, payload FROM series WHERE series = $name",
            "target": "platform",
            "params": ["name"],
            "ts": 1
        }),
    )
    .await
    .unwrap();

    let out = call(
        &node,
        &ada,
        ws,
        "query.run",
        json!({ "id": "byname", "vars": { "name": "cpu" } }),
    )
    .await
    .unwrap();
    let rows = out["rows"].as_array().unwrap();
    assert_eq!(rows.len(), 3, "bound to cpu: {out}");
    assert!(rows.iter().all(|r| r["series"] == json!("cpu")));

    // Missing param → typed error.
    let err = call(&node, &ada, ws, "query.run", json!({ "id": "byname" }))
        .await
        .unwrap_err();
    assert!(
        matches!(err, ToolError::BadInput(ref m) if m.contains("missing param")),
        "missing param typed error, got {err:?}"
    );

    // Extra (undeclared) param → typed error.
    let err = call(
        &node,
        &ada,
        ws,
        "query.run",
        json!({ "id": "byname", "vars": { "name": "cpu", "rogue": 1 } }),
    )
    .await
    .unwrap_err();
    assert!(
        matches!(err, ToolError::BadInput(ref m) if m.contains("undeclared param")),
        "undeclared param typed error, got {err:?}"
    );
}

// ---------------------------------------------------------------------------------------------
// Round-trip: save → get → edit → save → run (platform, real SurrealDB rows)
// ---------------------------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn round_trip_save_get_edit_save_run_platform() {
    let ws = "q-rt";
    let node = Arc::new(Node::boot().await.unwrap());
    let ada = admin(ws);
    seed_series(&node, &ada, ws, "cpu", 10).await;

    // save
    call(
        &node,
        &ada,
        ws,
        "query.save",
        json!({ "id": "recent", "name": "Recent", "description": "v1", "lang": "prql", "text": "from series | select { payload } | take 5", "target": "platform", "ts": 1 }),
    )
    .await
    .unwrap();

    // get (re-open)
    let got = call(&node, &ada, ws, "query.get", json!({ "id": "recent" }))
        .await
        .unwrap();
    assert_eq!(
        got["text"],
        json!("from series | select { payload } | take 5")
    );
    assert_eq!(got["name"], json!("Recent"));

    // run v1 → 5 rows
    let out = call(&node, &ada, ws, "query.run", json!({ "id": "recent" }))
        .await
        .unwrap();
    assert_eq!(out["rows"].as_array().unwrap().len(), 5);

    // edit (save same id with new text) — overwrite in place, no revision history (v1 decision)
    call(
        &node,
        &ada,
        ws,
        "query.save",
        json!({ "id": "recent", "name": "Recent", "lang": "prql", "text": "from series | select { payload } | take 2", "target": "platform", "ts": 2 }),
    )
    .await
    .unwrap();
    let got2 = call(&node, &ada, ws, "query.get", json!({ "id": "recent" }))
        .await
        .unwrap();
    assert_eq!(
        got2["text"],
        json!("from series | select { payload } | take 2"),
        "edited in place"
    );

    // run v2 → 2 rows
    let out2 = call(&node, &ada, ws, "query.run", json!({ "id": "recent" }))
        .await
        .unwrap();
    assert_eq!(out2["rows"].as_array().unwrap().len(), 2);

    // list shows it (flat roster, no text/result data)
    let listed = call(&node, &ada, ws, "query.list", json!({}))
        .await
        .unwrap();
    let arr = listed["queries"].as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["id"], json!("recent"));
    assert!(
        arr[0].get("text").is_none(),
        "list must not dump query text"
    );

    // delete (soft) → get now NotFound
    call(
        &node,
        &ada,
        ws,
        "query.delete",
        json!({ "id": "recent", "ts": 3 }),
    )
    .await
    .unwrap();
    let err = call(&node, &ada, ws, "query.get", json!({ "id": "recent" }))
        .await
        .unwrap_err();
    assert!(matches!(err, ToolError::BadInput(_)), "deleted → not found");
}

// ---------------------------------------------------------------------------------------------
// A rule reads source("query:<name>") end to end
// ---------------------------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn rule_reads_saved_query_by_name() {
    let ws = "q-rule";
    let node = Arc::new(Node::boot().await.unwrap());
    let ada = admin(ws);
    seed_series(&node, &ada, ws, "cpu", 4).await;

    // Save a platform query the rule will reuse by name.
    call(
        &node,
        &ada,
        ws,
        "query.save",
        json!({ "id": "cpucount", "name": "cpucount", "lang": "prql", "text": "from series | select { payload } | take 100", "target": "platform", "ts": 1 }),
    )
    .await
    .unwrap();

    // A rule that collects source("query:cpucount") and returns the row count. Runs under the rule
    // principal's caller ∩ grant: it holds rules.run + query.run + store.query. `.records()`
    // materializes the grid through the seam (which routes `query:<id>` → query.run).
    let body = r#"source("query:cpucount").records().len()"#;
    let out = call(
        &node,
        &ada,
        ws,
        "rules.run",
        json!({ "body": body, "ts": 1 }),
    )
    .await
    .unwrap();
    // The rule's output is the count of rows the saved query returned — the 4 seeded rows.
    assert_eq!(
        out["output"]["value"].as_i64(),
        Some(4),
        "rule read saved query rows: {out}"
    );
}

// ---------------------------------------------------------------------------------------------
// Round-trip against a REAL Postgres (datasource target) — reuses the federation test rig. Skips
// cleanly if Docker / the sidecar build is unavailable.
// ---------------------------------------------------------------------------------------------

mod postgres_rig {
    use std::process::Command;
    use std::sync::atomic::{AtomicU16, Ordering};

    use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
    use lb_host::{call_tool, install_native, Node};
    use lb_supervisor::OsLauncher;
    use serde_json::Value;

    pub const MANIFEST: &str = include_str!("../../federation/extension.toml");

    pub fn principal(ws: &str, caps: &[&str]) -> Principal {
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

    pub fn federation_dir() -> Option<String> {
        if let Ok(p) = std::env::var("FEDERATION_BIN") {
            return Some(
                std::path::PathBuf::from(&p)
                    .parent()
                    .unwrap()
                    .to_string_lossy()
                    .into_owned(),
            );
        }
        let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let target = manifest_dir.join("../../target/debug");
        let bin = target.join("federation");
        let mut cmd = Command::new("cargo");
        cmd.args(["build", "-p", "federation", "--features", "postgres"])
            .current_dir(manifest_dir.join("../.."));
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

    static PORT_SEQ: AtomicU16 = AtomicU16::new(0);

    pub struct Postgres {
        name: String,
        port: u16,
    }

    impl Postgres {
        pub fn spawn() -> Option<Self> {
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
            let name = format!("lb-q-test-{}-{}", std::process::id(), port);
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

        pub fn dsn(&self) -> String {
            format!(
                "host=127.0.0.1 port={} user=lb password=pw dbname=fed sslmode=disable",
                self.port
            )
        }

        pub fn endpoint(&self) -> String {
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

    pub async fn call(
        node: &std::sync::Arc<Node>,
        p: &Principal,
        ws: &str,
        tool: &str,
        input: Value,
    ) -> Result<Value, lb_mcp::ToolError> {
        let out = call_tool(node, p, ws, tool, &input.to_string()).await?;
        Ok(serde_json::from_str(&out).unwrap())
    }

    pub async fn install(node: &Node, admin: &Principal, ws: &str, dir: &str, endpoint: &str) {
        let (host, port) = endpoint.rsplit_once(':').unwrap();
        let approved = vec![
            format!("net:tls:{host}:{port}:connect"),
            "secret:federation/*:get".to_string(),
        ];
        install_native(node, &OsLauncher, admin, ws, MANIFEST, dir, &approved, 1)
            .await
            .unwrap();
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn round_trip_datasource_real_postgres() {
    let Some(dir) = postgres_rig::federation_dir() else {
        eprintln!("SKIP round_trip_datasource_real_postgres: could not build federation --features postgres");
        return;
    };
    let Some(pg) = postgres_rig::Postgres::spawn() else {
        eprintln!("SKIP round_trip_datasource_real_postgres: Docker/Postgres unavailable");
        return;
    };
    eprintln!(
        "query datasource test DB path: REAL Postgres on {}",
        pg.endpoint()
    );

    let ws = "q-pg";
    let node = Arc::new(Node::boot().await.unwrap());
    let admin = postgres_rig::principal(
        ws,
        &[
            "mcp:native.install:call",
            "mcp:native.call:call",
            "mcp:datasource.add:call",
            "mcp:datasource.list:call",
            "mcp:federation.query:call",
            "secret:federation/*:write",
            "secret:federation/*:get",
            SAVE,
            GET,
            RUN,
            COMPILE,
        ],
    );
    postgres_rig::install(&node, &admin, ws, &dir, &pg.endpoint()).await;

    // register the datasource (DSN into lb-secrets, only the ref in the record)
    postgres_rig::call(
        &node,
        &admin,
        ws,
        "datasource.add",
        json!({ "name": "wh", "kind": "postgres", "endpoint": pg.endpoint(), "dsn": pg.dsn(), "ts": 1 }),
    )
    .await
    .unwrap();

    // save a PRQL query targeting the datasource
    postgres_rig::call(
        &node,
        &admin,
        ws,
        "query.save",
        json!({ "id": "temps", "name": "temps", "lang": "prql", "text": "from readings | take 100", "target": "datasource:wh", "ts": 2 }),
    )
    .await
    .unwrap();

    // compile-preview → postgres SQL
    let compiled = postgres_rig::call(
        &node,
        &admin,
        ws,
        "query.compile",
        json!({ "lang": "prql", "text": "from readings | take 100", "target": "datasource:wh" }),
    )
    .await
    .unwrap();
    assert!(compiled["sql"].as_str().unwrap().contains("readings"));

    // run → the 5 seeded Postgres rows
    let out = postgres_rig::call(&node, &admin, ws, "query.run", json!({ "id": "temps" }))
        .await
        .unwrap();
    assert_eq!(
        out["rows"].as_array().unwrap().len(),
        5,
        "real pg rows: {out}"
    );
}
