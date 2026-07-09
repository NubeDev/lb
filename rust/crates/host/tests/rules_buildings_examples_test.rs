//! Regression test for the click-to-load BUILDINGS DEMO rule examples (rules-editor-ux / the beginner
//! lesson, step 2). The four examples a newcomer clicks in the Rules editor
//! (`buildings-intensity-query` / `-strict` / `-alert` / `-chart`) are HOST-OWNED strings in
//! `crates/host/src/rules/buildings_examples.json` — the SAME file the UI imports
//! (`ui/src/features/rules/examples/examples.ts`). This test `include_str!`s that JSON and runs each
//! example body through the REAL path — no re-implementation, no drift: an edit to the shared JSON that
//! breaks the query fails CI here, and the UI ships the exact strings this test proved green.
//!
//! Rule 9 (no mocks): real embedded SurrealDB, real caps, the REAL supervisor spawning the REAL
//! `federation` sidecar, and the REAL seeded `.lazybones/data/demo/buildings.db` (testing §0's one
//! sanctioned external — a real on-disk SQLite engine, no Docker). The only fake is the AI model seam
//! (a true external), and the buildings bodies never touch `ai.*` so it is never even called.
//!
//! Mandatory categories: the happy path (query/strict → 8 rows, 0 findings; alert → 8 rows, 1 finding
//! on Riverside; chart → 8 rows trimmed to label+value), capability-deny (a caller missing
//! `mcp:federation.query:call` is denied mid-run), and workspace-isolation (ws-B, with the cap but no
//! registered source, cannot read ws-A's `demo-buildings`).

use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{call_tool, install_native, rules_run, Node, RuleModel};
use lb_rules::RuleOutput;
use lb_supervisor::OsLauncher;
use serde::Deserialize;
use serde_json::{json, Value};

const MANIFEST: &str = include_str!("../../../extensions/federation/extension.toml");

/// The HOST-OWNED example catalog — the single source of truth shared with the UI. Parsed here so the
/// bodies this test runs ARE the bodies the editor ships.
const EXAMPLES_JSON: &str = include_str!("../src/rules/buildings_examples.json");

#[derive(Deserialize)]
struct Catalog {
    examples: Vec<ExampleEntry>,
}

#[derive(Deserialize)]
struct ExampleEntry {
    id: String,
    /// Stored as a line array (readable JSON diffs); joined on `\n` to reconstruct the runnable body —
    /// exactly what `examples.ts` does with `.body.join("\n")`.
    body: Vec<String>,
}

impl Catalog {
    fn body(&self, id: &str) -> String {
        self.examples
            .iter()
            .find(|e| e.id == id)
            .unwrap_or_else(|| panic!("example `{id}` present in the shared catalog"))
            .body
            .join("\n")
    }
}

fn catalog() -> Catalog {
    serde_json::from_str(EXAMPLES_JSON).expect("buildings_examples.json is valid JSON")
}

/// A never-called model — the buildings bodies never touch the `ai.*` seam, but `rules_run` requires a
/// `RuleModel`. Panicking makes an accidental AI call loud rather than silently mocked.
struct NoModel;
impl RuleModel for NoModel {
    fn complete(&self, _prompt: &str) -> Result<(String, u32), String> {
        panic!("the buildings examples must not call the AI seam");
    }
    fn propose_sql(&self, _q: &str, _hint: &str) -> Result<String, String> {
        panic!("the buildings examples must not call the AI seam");
    }
}

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

/// Admin holds what `make dev` pre-approves plus the caps a rule needs to query + raise an alert. The
/// alert body routes to inbox + outbox, so those two caps are required (handover fact 7).
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
            "mcp:rules.run:call",
            "mcp:inbox.record:call",
            "mcp:inbox.list:call",
            "mcp:outbox.enqueue:call",
            "inbox:rules:write",
            "secret:federation/*:write",
            "secret:federation/*:get",
        ],
    )
}

/// Build the default-features (sqlite) federation sidecar; a failure is a FAIL, not a skip (nothing
/// external needed). Mirrors `federation_sqlite_test.rs`.
fn federation_dir() -> String {
    if let Ok(p) = std::env::var("FEDERATION_BIN") {
        return PathBuf::from(&p)
            .parent()
            .unwrap()
            .to_string_lossy()
            .into_owned();
    }
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
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

/// The absolute path to the REAL seeded demo dataset committed under `.lazybones/data/demo/`. Skipping
/// is NOT allowed — the file is committed, so its absence is a real failure.
fn buildings_db() -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../.lazybones/data/demo/buildings.db")
        .canonicalize()
        .expect("the seeded .lazybones/data/demo/buildings.db exists (a committed fixture)");
    path.to_string_lossy().into_owned()
}

async fn install_federation(node: &Arc<Node>, admin: &Principal, ws: &str, dir: &str) {
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
        .unwrap_or_else(|e| panic!("{tool} call: {e:?}"));
    serde_json::from_str(&out).unwrap()
}

/// Register the REAL buildings.db as the `demo-buildings` sqlite datasource — exactly what
/// `make seed-demo-sqlite` does, but in-process against the same node.
async fn register_buildings(node: &Arc<Node>, admin: &Principal, ws: &str, db: &str) {
    call(
        node,
        admin,
        ws,
        "datasource.add",
        json!({"name":"demo-buildings","kind":"sqlite","endpoint":"127.0.0.1:0","dsn":db,"ts":1}),
    )
    .await;
}

/// Assert the run produced a scalar (`.records()` → an array of row MAPS) of exactly `n` rows. The
/// cage's `records()` honors its `Array<Map>` catalog contract on both seam shapes — federation's
/// column-aligned positional rows are zipped into named maps at `grid.rs::row_to_map`, so a row is
/// `{"building": "...", "kwh_per_m2": ...}` keyed by the SELECT aliases (not a positional array).
fn assert_rows(output: &RuleOutput, n: usize) -> Vec<Value> {
    match output {
        RuleOutput::Scalar(v) => {
            let rows = v
                .as_array()
                .expect("the result is the ranked table (an array)");
            assert_eq!(
                rows.len(),
                n,
                "expected {n} building rows, got {}",
                rows.len()
            );
            rows.clone()
        }
        other => panic!("expected a scalar array result, got {other:?}"),
    }
}

async fn run(
    node: &Arc<Node>,
    p: &Principal,
    ws: &str,
    body: &str,
    now: u64,
) -> lb_host::RunResult {
    rules_run(
        node,
        p,
        ws,
        Some(body.to_string()),
        None,
        rhai::Map::new(),
        Arc::new(NoModel),
        now,
        None,
        true, // route: the alert body must fan out to inbox/outbox for the finding assertion
    )
    .await
    .unwrap_or_else(|e| panic!("rules.run: {e:?}"))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn buildings_examples_run_end_to_end() {
    let cat = catalog();
    let dir = federation_dir();
    let db = buildings_db();
    let ws = "acme";
    let node = Arc::new(Node::boot().await.unwrap());
    let admin = admin(ws);
    install_federation(&node, &admin, ws, &dir).await;
    register_buildings(&node, &admin, ws, &db).await;

    // --- QUERY-ONLY body: 8 buildings, Riverside Data Center on top (4.68 kWh/m²), NO findings ---
    let query = run(
        &node,
        &admin,
        ws,
        &cat.body("buildings-intensity-query"),
        10,
    )
    .await;
    let rows = assert_rows(&query.output, 8);
    assert_eq!(
        rows[0].get("building").and_then(|v| v.as_str()),
        Some("Riverside Data Center"),
        "Riverside is the most energy-intense building (r.building): {:?}",
        rows[0]
    );
    assert!(
        (rows[0].get("kwh_per_m2").and_then(|v| v.as_f64()).unwrap() - 4.68).abs() < 0.01,
        "Riverside is 4.68 kWh/m² (r.kwh_per_m2): {:?}",
        rows[0]
    );
    assert!(
        query.findings.is_empty(),
        "the query-only body raises NO finding (the emit block is commented out): {:?}",
        query.findings
    );

    // --- STRICT body: same 8 rows, still no live finding (its emit block is commented out too) ---
    let strict = run(
        &node,
        &admin,
        ws,
        &cat.body("buildings-intensity-strict"),
        20,
    )
    .await;
    assert_rows(&strict.output, 8);
    assert!(
        strict.findings.is_empty(),
        "the strict body's finding block is commented out: {:?}",
        strict.findings
    );

    // --- ALERT body: same 8 rows, exactly ONE alert finding (only Riverside > 1.0 kWh/m²) ---
    let alerted = run(
        &node,
        &admin,
        ws,
        &cat.body("buildings-intensity-alert"),
        30,
    )
    .await;
    assert_rows(&alerted.output, 8);
    assert_eq!(
        alerted.findings.len(),
        1,
        "only Riverside exceeds the 1.0 kWh/m² budget: {:?}",
        alerted.findings
    );
    let finding = &alerted.findings[0];
    assert!(
        finding.is_alert(),
        "it is an alert (routes to inbox/outbox)"
    );
    assert_eq!(
        finding.data.get("building").and_then(|v| v.as_str()),
        Some("Riverside Data Center"),
        "the alert names Riverside: {finding:?}"
    );
    // The alert fanned out to a REAL inbox item on the `rules` channel.
    let items = lb_host::list_inbox(&node.store, &admin, ws, "rules")
        .await
        .unwrap();
    assert_eq!(items.len(), 1, "the alert routed one inbox item");

    // --- CHART body: `category(...)` trims each row to one label + one numeric column (the bar/pie
    // shape a panel draws). 8 rows still (one per building), each exactly 2 fields. This is the
    // slice-3 (rules-for-widgets) promise pinned on the REAL federation path: a rule whose last
    // line is `category(query(...).records(), ...)` is a complete chart-ready rule. ---
    let chart = run(
        &node,
        &admin,
        ws,
        &cat.body("buildings-intensity-chart"),
        35,
    )
    .await;
    let chart_rows = assert_rows(&chart.output, 8);
    // `category` trims to ONLY the label + value columns — nothing else leaks through.
    assert_eq!(
        chart_rows[0].as_object().unwrap().len(),
        2,
        "category trimmed to label + value: {:?}",
        chart_rows[0]
    );
    assert_eq!(
        chart_rows[0].get("building").and_then(|v| v.as_str()),
        Some("Riverside Data Center"),
        "the chart's first bar is Riverside (label column intact): {:?}",
        chart_rows[0]
    );
    assert!(
        (chart_rows[0]
            .get("kwh_per_m2")
            .and_then(|v| v.as_f64())
            .unwrap()
            - 4.68)
            .abs()
            < 0.01,
        "the chart's first bar value is 4.68 kWh/m²: {:?}",
        chart_rows[0]
    );
    assert!(
        chart.findings.is_empty(),
        "the chart body raises NO finding (pure shape trim): {:?}",
        chart.findings
    );

    // --- CAPABILITY-DENY: the same body, minus `mcp:federation.query:call`, is denied mid-run ---
    let no_fed = principal(ws, &["mcp:rules.run:call"]);
    let denied = rules_run(
        &node,
        &no_fed,
        ws,
        Some(cat.body("buildings-intensity-query")),
        None,
        rhai::Map::new(),
        Arc::new(NoModel),
        40,
        None,
        true,
    )
    .await;
    assert!(
        denied.is_err(),
        "querying demo-buildings without mcp:federation.query:call must be denied mid-run"
    );

    // --- WORKSPACE ISOLATION: ws-B holds the cap but never registered `demo-buildings` — the source
    // does not resolve across the workspace wall, so the run fails (never reads ws-A's data) ---
    let ws_b = "other";
    let admin_b = self::admin(ws_b);
    install_federation(&node, &admin_b, ws_b, &dir).await;
    let iso = rules_run(
        &node,
        &admin_b,
        ws_b,
        Some(cat.body("buildings-intensity-query")),
        None,
        rhai::Map::new(),
        Arc::new(NoModel),
        50,
        None,
        true,
    )
    .await;
    assert!(
        iso.is_err(),
        "ws-B cannot resolve ws-A's demo-buildings source (workspace is the hard wall)"
    );
}
