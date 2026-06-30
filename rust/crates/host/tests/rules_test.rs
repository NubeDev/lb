//! Host-layer tests for the `rules.*` service (rules-engine-scope Testing plan). Real store, real
//! caps, real MCP host, data seeded as real series records through the real ingest+commit path. The
//! ONLY fake is the model provider behind the AI seam (a true external) — injected as a deterministic
//! `RuleModel` to exercise the budget + the nsql fence without a live model.
//!
//! Mandatory categories: capability-deny (each verb + a mid-run source deny), workspace-isolation
//! (ws-B cannot get/run a ws-A rule; a ws-B run cannot read a ws-A source), and offline/sync (a saved
//! rule survives a node restart — it's a record).

use std::sync::Arc;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{call_tool, ingest_write, rules_get, rules_run, rules_save, Node, RuleModel};
use lb_ingest::{commit_batch, Qos, Sample};

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
    let token = mint(&key, &claims);
    verify(&key, &token, 1).expect("token verifies")
}

/// The full grant for a rule that reads platform series + runs + saves.
const FULL: &[&str] = &[
    "mcp:rules.run:call",
    "mcp:rules.save:call",
    "mcp:rules.get:call",
    "mcp:rules.list:call",
    "mcp:rules.delete:call",
    "mcp:store.query:call",
    "mcp:series.read:call",
    "mcp:ingest.write:call",
    "mcp:inbox.record:call",
    "mcp:inbox.list:call",
    "mcp:outbox.enqueue:call",
    "store:rule:write",
    "store:rule:read",
    "inbox:rules:write",
];

/// A deterministic model — the sanctioned fake-of-a-true-external (testing §0). Records calls.
struct ScriptedModel {
    completion: String,
    tokens: u32,
    proposed_sql: String,
}

impl RuleModel for ScriptedModel {
    fn complete(&self, _prompt: &str) -> Result<(String, u32), String> {
        Ok((self.completion.clone(), self.tokens))
    }
    fn propose_sql(&self, _q: &str, _hint: &str) -> Result<String, String> {
        Ok(self.proposed_sql.clone())
    }
}

/// Seed `n` samples into a series via the real ingest write + commit path, then return.
async fn seed_series(node: &Node, p: &Principal, ws: &str, series: &str, values: &[f64]) {
    let samples: Vec<Sample> = values
        .iter()
        .enumerate()
        .map(|(i, v)| Sample {
            series: series.to_string(),
            producer: "seed".into(),
            ts: i as u64,
            seq: i as u64,
            payload: serde_json::json!(v),
            labels: serde_json::Value::Null,
            qos: Qos::BestEffort,
        })
        .collect();
    ingest_write(&node.store, p, ws, samples).await.unwrap();
    commit_batch(&node.store, ws, 1000).await.unwrap();
}

// ----- capability-deny -------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn each_rules_verb_is_denied_without_its_cap() {
    let ws = "rules-deny";
    let node = Arc::new(Node::boot().await.unwrap());
    let p = principal(ws, &[]); // no caps

    for (tool, input) in [
        ("rules.run", serde_json::json!({ "body": "1" })),
        ("rules.save", serde_json::json!({ "id": "r", "body": "1" })),
        ("rules.get", serde_json::json!({ "id": "r" })),
        ("rules.list", serde_json::json!({})),
        ("rules.delete", serde_json::json!({ "id": "r" })),
    ] {
        let err = call_tool(&node, &p, ws, tool, &input.to_string()).await;
        assert!(err.is_err(), "{tool} must be denied without its cap");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn rule_reading_an_ungranted_source_is_denied_mid_run() {
    let ws = "rules-source-deny";
    let node = Arc::new(Node::boot().await.unwrap());
    // has rules.run but NOT store.query — the collect of a platform source is denied inside the run.
    let p = principal(ws, &["mcp:rules.run:call"]);
    let model = Arc::new(ScriptedModel {
        completion: "x".into(),
        tokens: 1,
        proposed_sql: "SELECT 1 AS v".into(),
    });
    let res = rules_run(
        &node,
        &p,
        ws,
        Some(r#"history("series", "t", "24h").records()"#.into()),
        None,
        rhai::Map::new(),
        model,
        1,
    )
    .await;
    assert!(res.is_err(), "collect without store.query must be denied");
}

// ----- happy path: seed real series, run a rollup+alert rule end to end -------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn run_rollup_alert_rule_raises_inbox_item() {
    let ws = "rules-happy";
    let node = Arc::new(Node::boot().await.unwrap());
    let p = principal(ws, FULL);
    seed_series(&node, &p, ws, "cooler.temp", &[3.0, 9.0, 4.0]).await;

    let body = r#"
        let hot = history("series", "cooler.temp", "24h").filter("value > 5.0");
        if hot.size() > 0 {
            alert(#{ level: "critical", series: "cooler.temp", msg: "hot" });
        }
    "#;
    let model = Arc::new(ScriptedModel {
        completion: "ok".into(),
        tokens: 1,
        proposed_sql: "SELECT 1 AS v".into(),
    });
    let result = rules_run(
        &node,
        &p,
        ws,
        Some(body.into()),
        None,
        rhai::Map::new(),
        model,
        7,
    )
    .await
    .unwrap();
    assert_eq!(result.findings.len(), 1, "one alert finding");
    assert!(result.findings[0].is_alert());

    // The alert raised a real inbox item on the `rules` channel.
    let items = lb_host::list_inbox(&node.store, &p, ws, "rules")
        .await
        .unwrap();
    assert_eq!(items.len(), 1, "alert routed to inbox");
}

// ----- workspace isolation ---------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn ws_b_cannot_get_a_ws_a_saved_rule() {
    let node = Arc::new(Node::boot().await.unwrap());
    let a = principal("ws-a", FULL);
    rules_save(&node.store, &a, "ws-a", "shared", "shared", "1", vec![])
        .await
        .unwrap();
    // ws-B with full caps in its OWN workspace cannot see ws-A's rule (namespace wall).
    let b = principal("ws-b", FULL);
    let err = rules_get(&node.store, &b, "ws-b", "shared").await;
    assert!(err.is_err(), "ws-B must not read a ws-A rule");
}

// ----- AI budget + fence (injected scripted model) ---------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn ai_budget_caps_a_loop() {
    let ws = "rules-ai-budget";
    let node = Arc::new(Node::boot().await.unwrap());
    let p = principal(ws, FULL);
    let model = Arc::new(ScriptedModel {
        completion: "x".into(),
        tokens: 1,
        proposed_sql: "SELECT 1 AS v".into(),
    });
    // default AI_MAX_CALLS is 8; a 100-iteration loop must trip the budget.
    let body = r#"for i in 0..100 { ai.complete("hi"); } 1"#;
    let res = rules_run(
        &node,
        &p,
        ws,
        Some(body.into()),
        None,
        rhai::Map::new(),
        model,
        1,
    )
    .await;
    assert!(res.is_err(), "AI budget must abort the loop");
}

// ----- offline/sync: a saved rule survives a restart (it's a record) ---------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn saved_rule_survives_a_restart() {
    let ws = "rules-restart";
    // A persistent store path so a re-open sees the record (mem:// is per-instance; use a temp dir).
    let dir = std::env::temp_dir().join(format!("lb-rules-restart-{}", std::process::id()));
    let path = dir.to_string_lossy().to_string();
    {
        let store = lb_store::Store::open(&path).await.unwrap();
        let p = principal(ws, FULL);
        rules_save(
            &store,
            &p,
            ws,
            "persisted",
            "persisted",
            "let x = 1; x",
            vec![],
        )
        .await
        .unwrap();
    }
    {
        let store = lb_store::Store::open(&path).await.unwrap();
        let p = principal(ws, FULL);
        let rule = rules_get(&store, &p, ws, "persisted").await.unwrap();
        assert_eq!(rule.body, "let x = 1; x");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

// Regression: a registered federation datasource must appear in a rule run's source allowlist. The
// allowlist builder once read raw `lb_store::scan` rows whose `data` is the Versioned `{rev, data:{…}}`
// envelope, so `row.data.name` always missed — emptying the allowlist and making every federation
// `source(...)`/`query(...)` resolve as `SourceNotAllowed` → opaque `Denied` (a misleading "not
// permitted"). Mirrors the sibling `rules.list`/`chains.list` envelope bug.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn registered_datasource_is_in_the_rule_allowlist() {
    let ws = "rules-allowlist";
    let node = Arc::new(Node::boot().await.unwrap());
    let p = principal(
        ws,
        &["mcp:datasource.add:call", "secret:federation/*:write"],
    );
    lb_host::datasource_add(
        &node,
        &p,
        ws,
        "tsdb",
        "postgres",
        "db.host:5432",
        None,
        None,
        1,
    )
    .await
    .unwrap();

    let sources = lb_host::workspace_datasources(&node, ws).await;
    assert!(
        sources.contains("tsdb"),
        "a registered datasource must be in the rule allowlist (got {sources:?})"
    );
}
