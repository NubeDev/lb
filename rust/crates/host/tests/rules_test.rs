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
        constraint: None,
        run_id: None,
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
    "mcp:rules.help:call",
    "mcp:store.query:call",
    "mcp:series.read:call",
    "mcp:ingest.write:call",
    "mcp:inbox.record:call",
    "mcp:inbox.list:call",
    "mcp:outbox.enqueue:call",
    "mcp:channel.post:call",
    "mcp:channel.history:call",
    "bus:chan/*:pub",
    "bus:chan/*:sub",
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
        ("rules.help", serde_json::json!({})),
    ] {
        let err = call_tool(&node, &p, ws, tool, &input.to_string()).await;
        assert!(err.is_err(), "{tool} must be denied without its cap");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn rules_help_returns_the_catalog() {
    // The introspection surface: `rules.help` returns the lb_rules::CATALOG entries (name, family,
    // signature, description) so an agent/UI can discover the verb surface. Gated like the other
    // verbs; the catalog itself is the source of truth in the rules crate.
    let ws = "rules-help";
    let node = Arc::new(Node::boot().await.unwrap());
    let p = principal(ws, &["mcp:rules.help:call"]);
    let out = call_tool(&node, &p, ws, "rules.help", "{}")
        .await
        .expect("rules.help succeeds with its cap");
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    let fns = v.get("functions").and_then(|f| f.as_array()).unwrap();
    assert!(!fns.is_empty(), "catalog must be non-empty");
    // Spot-check a known entry (the source verb — every cage has it) carries all four fields.
    let source = fns
        .iter()
        .find(|e| e.get("name").and_then(|n| n.as_str()) == Some("source"))
        .expect("catalog contains `source`");
    assert_eq!(source["family"], "data");
    assert!(source["signature"].as_str().unwrap().contains("source"));
    assert!(!source["description"].as_str().unwrap().is_empty());
    // Every entry has all four fields non-empty.
    for e in fns {
        for k in ["name", "family", "signature", "description"] {
            let s = e.get(k).and_then(|v| v.as_str()).unwrap_or("");
            assert!(!s.is_empty(), "entry {:?} has empty {k}", e["name"]);
        }
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
        None,
        true,
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
        None,
        true,
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

// rules-for-widgets-scope slice 2: a read-only run (`route:false`) still RETURNS the alert finding but
// routes NOTHING — zero new inbox items, zero outbox entries. This is what keeps a 30 s dashboard
// auto-refresh from spamming the Inbox/Outbox on every repaint. The default (`route:true`) path is
// pinned by `run_rollup_alert_rule_raises_inbox_item` above.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn route_false_run_returns_findings_but_routes_nothing() {
    let ws = "rules-route-false";
    let node = Arc::new(Node::boot().await.unwrap());
    // FULL + outbox.due so the test can count what (nothing) was enqueued.
    let mut caps: Vec<&str> = FULL.to_vec();
    caps.push("mcp:outbox.due:call");
    let p = principal(ws, &caps);
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
    // route = false (the last arg) — the panel-repaint mode.
    let result = rules_run(
        &node,
        &p,
        ws,
        Some(body.into()),
        None,
        rhai::Map::new(),
        model,
        7,
        None,
        false,
    )
    .await
    .unwrap();

    // The finding is STILL in the result (honest, visible) — route:false suppresses fan-out, not the finding.
    assert_eq!(
        result.findings.len(),
        1,
        "the alert finding is still returned"
    );
    assert!(result.findings[0].is_alert());

    // …but NOTHING was routed: no inbox item, no outbox entry.
    let items = lb_host::list_inbox(&node.store, &p, ws, "rules")
        .await
        .unwrap();
    assert!(items.is_empty(), "route:false raised NO inbox item");
    let due = lb_host::outbox_due(&node.store, &p, ws, None, 7)
        .await
        .unwrap();
    assert!(due.is_empty(), "route:false enqueued NO outbox effect");
}

// ----- the `channel` rhai handle (slice 3), driven through a REAL `rules.run` ------------------

/// Run `body` through the real `rules.run` chokepoint with a scripted model. Returns the result.
async fn run_body(
    node: &Arc<Node>,
    p: &Principal,
    ws: &str,
    body: &str,
) -> Result<lb_host::RunResult, lb_host::RulesError> {
    let model = Arc::new(ScriptedModel {
        completion: "ok".into(),
        tokens: 1,
        proposed_sql: "SELECT 1 AS v".into(),
    });
    rules_run(
        node,
        p,
        ws,
        Some(body.into()),
        None,
        rhai::Map::new(),
        model,
        7,
        None,
        true,
    )
    .await
}

/// Read a channel's history through the real MCP verb (returns the parsed JSON).
async fn channel_history(
    node: &Arc<Node>,
    p: &Principal,
    ws: &str,
    cid: &str,
) -> serde_json::Value {
    let out = call_tool(
        node,
        p,
        ws,
        "channel.history",
        &serde_json::json!({ "cid": cid }).to_string(),
    )
    .await
    .unwrap();
    serde_json::from_str(&out).unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn rule_channel_post_lands_a_real_message() {
    let ws = "rules-channel-post";
    let node = Arc::new(Node::boot().await.unwrap());
    let p = principal(ws, FULL);
    run_body(
        &node,
        &p,
        ws,
        r#"channel.post("ops", #{ id: "m1", body: "hello from a rule" });"#,
    )
    .await
    .unwrap();

    // Read it back through the real MCP channel.history verb (Sub-capable) — the post committed.
    let hist = channel_history(&node, &p, ws, "ops").await;
    let msgs = hist["messages"].as_array().unwrap();
    assert_eq!(
        msgs.len(),
        1,
        "the rule's channel.post landed a real message"
    );
    assert_eq!(msgs[0]["body"], "hello from a rule");
    // Author is FORCED to the caller (never request-supplied).
    assert_eq!(msgs[0]["author"], "user:test");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn rule_channel_post_is_caller_gated_and_opaque() {
    // The caller holds rules.run + the MCP door but NOT `bus:chan/*:pub`. A rule that posts is denied
    // at the channel gate mid-run, opaquely, and NO message lands.
    let ws = "rules-channel-deny";
    let node = Arc::new(Node::boot().await.unwrap());
    let poster = principal(
        ws,
        &[
            "mcp:rules.run:call",
            "mcp:channel.post:call",
            "mcp:channel.history:call",
            "bus:chan/*:sub", // can read, cannot post
        ],
    );
    let err = run_body(
        &node,
        &poster,
        ws,
        r#"channel.post("ops", #{ body: "should not land" });"#,
    )
    .await;
    assert!(
        err.is_err(),
        "a Pub-less rule must be denied at channel.post"
    );

    // NO write landed — a Sub-capable read shows an empty channel.
    let hist = channel_history(&node, &poster, ws, "ops").await;
    assert!(
        hist["messages"].as_array().unwrap().is_empty(),
        "the denied post left no partial write"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn rule_channel_post_worker_kind_is_fenced_at_the_handle() {
    // A rule posting a `kind:"agent"` item is rejected by the rule handle (a rule cannot spawn a run —
    // Resolved decisions), even with FULL caps. No message lands. The generic MCP verb keeps parity;
    // only the rule layer is fenced.
    let ws = "rules-channel-fence";
    let node = Arc::new(Node::boot().await.unwrap());
    let p = principal(ws, FULL);
    let err = run_body(
        &node,
        &p,
        ws,
        r#"channel.post("ops", #{ kind: "agent", goal: "summarize the logs" });"#,
    )
    .await;
    assert!(err.is_err(), "a rule cannot post a worker kind");

    // The fence fired at the handle before any seam call — the channel is empty.
    let hist = channel_history(&node, &p, ws, "ops").await;
    assert!(
        hist["messages"].as_array().unwrap().is_empty(),
        "the fenced worker-kind post spawned no run and left no message"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn ws_b_rule_cannot_post_into_a_ws_a_channel() {
    // Workspace wall: a ws-B rule posting to "ops" writes ws-B's own "ops", never ws-A's. ws-A's
    // channel is untouched by ws-B's run.
    let node = Arc::new(Node::boot().await.unwrap());
    let a = principal("ws-a", FULL);
    let b = principal("ws-b", FULL);

    // ws-A seeds a message in its own ops channel.
    run_body(
        &node,
        &a,
        "ws-a",
        r#"channel.post("ops", #{ id: "a1", body: "ws-a private" });"#,
    )
    .await
    .unwrap();

    // ws-B posts to "ops" — lands in ws-B's namespace only.
    run_body(
        &node,
        &b,
        "ws-b",
        r#"channel.post("ops", #{ id: "b1", body: "ws-b message" });"#,
    )
    .await
    .unwrap();

    // ws-A's channel still holds only ws-A's message (ws-B never reached it).
    let a_hist = channel_history(&node, &a, "ws-a", "ops").await;
    let a_msgs = a_hist["messages"].as_array().unwrap();
    assert_eq!(a_msgs.len(), 1, "ws-A channel untouched by ws-B");
    assert_eq!(a_msgs[0]["body"], "ws-a private");
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
        None,
        true,
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

// A rule that posts to a channel writes a durable record (channel history lives in the store — state,
// not motion, §3 rule 3). A saved rule survives a restart AND the message it posted is still there when
// a fresh Node re-opens the same store: the run's effect is durable, the rule itself is a record.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn channel_posting_rule_and_its_message_survive_a_restart() {
    let ws = "rules-channel-restart";
    let dir = std::env::temp_dir().join(format!("lb-rules-chan-restart-{}", std::process::id()));
    let path = dir.to_string_lossy().to_string();
    let body = r#"channel.post("ops", #{ id: "durable-1", body: "survives restart" });"#;

    {
        // First boot: save a channel-posting rule, then run it — the post lands a durable record.
        let store = lb_store::Store::open(&path).await.unwrap();
        let node = Arc::new(Node::boot_with_store(store).await.unwrap());
        let p = principal(ws, FULL);
        rules_save(&node.store, &p, ws, "poster", "poster", body, vec![])
            .await
            .unwrap();
        rules_run(
            &node,
            &p,
            ws,
            None,
            Some("poster".into()),
            rhai::Map::new(),
            Arc::new(ScriptedModel {
                completion: "ok".into(),
                tokens: 1,
                proposed_sql: "SELECT 1 AS v".into(),
            }),
            7,
            None,
            true,
        )
        .await
        .unwrap();
    }
    {
        // Restart: a fresh Node on the SAME store still holds the saved rule AND the posted message.
        let store = lb_store::Store::open(&path).await.unwrap();
        let node = Arc::new(Node::boot_with_store(store).await.unwrap());
        let p = principal(ws, FULL);

        let rule = rules_get(&node.store, &p, ws, "poster").await.unwrap();
        assert_eq!(rule.body, body, "the rule survived the restart");

        let hist = channel_history(&node, &p, ws, "ops").await;
        let msgs = hist["messages"].as_array().unwrap();
        assert_eq!(msgs.len(), 1, "the posted message survived the restart");
        assert_eq!(msgs[0]["body"], "survives restart");
        assert_eq!(msgs[0]["id"], "durable-1");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

// Regression: a registered federation datasource must appear in a rule run's source allowlist. The
// allowlist builder once read raw `lb_store::scan` rows whose `data` is the Versioned `{rev, data:{…}}`
// envelope, so `row.data.name` always missed — emptying the allowlist and making every federation
// `source(...)`/`query(...)` resolve as `SourceNotAllowed` → opaque `Denied` (a misleading "not
// permitted"). Mirrors the sibling `rules.list`/`flows.list` envelope bug.
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
