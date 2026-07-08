//! rules-ai-wiring-scope — binding the rule engine's `ai.*` to the REAL model seam. These tests prove
//! the production bridge (`call_tool("rules.run")` → the resolved model) reaches the node's
//! `ModelAccess` for a workspace whose `agent.config` selects a model, and keeps the honest
//! "AI not configured" path otherwise. The `DisabledModel` hardcode is gone from the configured path.
//!
//! **Rule 9: everything real** — `mem://` store, caps, the MCP host, the meter, the nsql fence, and a
//! real `AiGateway<MockProvider>` installed as the node's model. The ONLY fake is the model provider
//! HTTP (`MockProvider`, behind the sanctioned `Provider` trait), which scripts the model's turns.
//!
//! Categories (testing-scope + the scope's Testing plan):
//!   - **AI wired (headline):** a rule's `ai.complete` over the real bridge returns the model's real
//!     (mock-deterministic) output for a configured workspace.
//!   - **AI-not-configured:** an unconfigured workspace runs a data-only rule fine; `ai.*` errors clearly.
//!   - **Workspace-isolation:** ws-A (unconfigured) and ws-B (configured) resolve their OWN model —
//!     ws-A's `ai.*` errors while ws-B's answers.
//!   - **Fence holds:** `ai.ask`'s model-proposed SQL is re-validated through `DataSeam::collect` — a
//!     proposed query against an un-granted source is denied at collect (unchanged).
//!   - **Budget meter charges:** a tiny token budget stops a rule whose `ai.complete` overspends.
//!   - **Adapter unit:** `AgentRuleModel` maps `complete`/`propose_sql` onto `ModelAccess::turn` (no
//!     tools) and reads content correctly, over a scripted `ModelAccess`.

use std::future::Future;
use std::sync::Arc;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{
    agent_config_set, call_tool, AgentConfig, AgentRuleModel, AllowedTool, CallOutcome,
    ErasedModel, ModelAccess, ModelEndpointPatch, Node, RuleModel, RuntimeRegistry, Turn,
};
use lb_role_ai_gateway::{AiGateway, AiResponse, MockProvider};

// ── caps ───────────────────────────────────────────────────────────────────────────────────────
const CONFIG_SET: &str = "mcp:agent.config.set:call";

/// A rule that reads platform series + runs + calls ai.*.
const RULE_CAPS: &[&str] = &[
    "mcp:rules.run:call",
    "mcp:store.query:call",
    "mcp:series.read:call",
    "mcp:inbox.record:call",
    "mcp:outbox.enqueue:call",
    "store:rule:read",
    "inbox:rules:write",
];

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
    verify(&key, &mint(&key, &claims), 1).expect("token verifies")
}

/// A model that answers each turn with the given content (a `stop`, no tool calls). Every `ai.*` in a
/// run consumes one scripted turn; past the end the mock returns a terminal stop.
fn scripted_gateway(turns: &[&str]) -> AiGateway<MockProvider> {
    AiGateway::new(MockProvider::new(
        turns.iter().map(|t| AiResponse::stop(*t, 7)).collect(),
    ))
}

/// Install `gw` as the node's in-house model (the `default` runtime). This is the SAME swap the `node`
/// binary does when it wires a real provider — the rules bridge resolves THIS model.
fn install_model(node: &Node, gw: AiGateway<MockProvider>) {
    let model: Arc<dyn ErasedModel> = Arc::new(gw);
    node.install_runtimes(RuntimeRegistry::with_default(model));
}

/// Select a model endpoint for `ws` via the real `agent.config.set` verb (the catalog pick). This is
/// what makes the resolution treat the workspace as "has a model".
async fn select_model(node: &Arc<Node>, ws: &str) {
    let admin = principal("user:admin", ws, &[CONFIG_SET]);
    let patch = AgentConfig {
        active_definition: None,
        active_persona: None,
        enabled_personas: None,
        default_runtime: None,
        model_endpoint: Some(ModelEndpointPatch {
            provider: Some("zaicoding".into()),
            model: Some("glm-4.6".into()),
            api_key_env: Some("ZAI_API_KEY".into()),
            api_key_secret: None,
            base_url: None,
        }),
    };
    agent_config_set(node, &admin, ws, &patch)
        .await
        .expect("admin selects a model");
}

/// Run a rule body over the REAL bridge (`call_tool("rules.run")`) and return the parsed result JSON.
async fn run_rule(
    node: &Arc<Node>,
    p: &Principal,
    ws: &str,
    body: &str,
) -> Result<serde_json::Value, lb_mcp::ToolError> {
    let input = serde_json::json!({ "body": body, "ts": 1 }).to_string();
    let out = call_tool(node, p, ws, "rules.run", &input).await?;
    Ok(serde_json::from_str(&out).expect("rules.run returns JSON"))
}

// ── AI wired (the headline) ──────────────────────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ai_complete_reaches_the_real_model_for_a_configured_workspace() {
    let ws = "rules-ai-wired";
    let node = Arc::new(Node::boot().await.unwrap());
    install_model(&node, scripted_gateway(&["boilers ran hot at 14:00"]));
    select_model(&node, ws).await;

    let p = principal("user:ada", ws, RULE_CAPS);
    let body = r#"let answer = ai.complete("summarize"); emit(#{ summary: answer });"#;
    let res = run_rule(&node, &p, ws, body).await.expect("rule runs");

    // The rule's ai.complete returned the model's REAL (mock-deterministic) output — proving the
    // DisabledModel hardcode is gone from the configured path. The emitted value lands in `findings`.
    let out = serde_json::to_string(&res["findings"]).unwrap();
    assert!(
        out.contains("boilers ran hot at 14:00"),
        "ai.complete must return the model output, got: {out}"
    );
    // The meter recorded one AI call.
    assert_eq!(res["ai"]["calls"].as_u64(), Some(1), "one ai call charged");
}

// ── AI not configured ────────────────────────────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unconfigured_workspace_runs_data_only_but_errors_on_ai() {
    let ws = "rules-ai-unconfigured";
    // A REAL model is installed on the node, but the workspace never SELECTS one (no agent.config).
    let node = Arc::new(Node::boot().await.unwrap());
    install_model(&node, scripted_gateway(&["unused"]));
    let p = principal("user:ada", ws, RULE_CAPS);

    // Data-only rule runs fine.
    let ok = run_rule(&node, &p, ws, "emit(#{ n: 1 });").await;
    assert!(ok.is_ok(), "a data-only rule runs without a selected model");

    // A rule calling ai.* gets the honest error (surfaced as author feedback / BadInput).
    let err = run_rule(&node, &p, ws, r#"emit(#{ a: ai.complete("x") });"#).await;
    assert!(err.is_err(), "ai.* without a configured model must error");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn selected_but_node_has_no_provider_errors_on_ai() {
    // The workspace selects a model, but the node has only the UnconfiguredModel placeholder (the
    // boot default). Resolution must still yield the honest error — not pretend to answer.
    let ws = "rules-ai-no-provider";
    let node = Arc::new(Node::boot().await.unwrap());
    select_model(&node, ws).await; // selected, but no real model installed
    let p = principal("user:ada", ws, RULE_CAPS);

    let err = run_rule(&node, &p, ws, r#"emit(#{ a: ai.complete("x") });"#).await;
    assert!(
        err.is_err(),
        "a selected model on a provider-less node must still error, never fabricate"
    );
}

// ── workspace isolation ──────────────────────────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn each_workspace_resolves_its_own_model_selection() {
    // One node, one installed model. ws-B selects a model; ws-A does not. A rule in ws-A must error on
    // ai.* (unconfigured) while the SAME rule in ws-B answers — the selection is per-workspace.
    let node = Arc::new(Node::boot().await.unwrap());
    install_model(&node, scripted_gateway(&["ws-b answer", "ws-b answer 2"]));
    select_model(&node, "ws-b").await;

    let pa = principal("user:a", "ws-a", RULE_CAPS);
    let pb = principal("user:b", "ws-b", RULE_CAPS);
    let body = r#"emit(#{ a: ai.complete("q") });"#;

    let a = run_rule(&node, &pa, "ws-a", body).await;
    assert!(a.is_err(), "ws-a did not select a model → ai.* errors");

    let b = run_rule(&node, &pb, "ws-b", body).await.expect("ws-b runs");
    let out = serde_json::to_string(&b["findings"]).unwrap();
    assert!(out.contains("ws-b answer"), "ws-b resolves its own model");
}

// ── the fence holds (regression) ─────────────────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ai_ask_proposed_sql_is_refenced_and_denied_without_the_source_cap() {
    // ws selects a model; the model proposes SQL. The caller has rules.run but NOT store.query, so the
    // proposed SQL's collect is denied at the fence — a model-proposed query cannot skip caps::check.
    let ws = "rules-ai-fence";
    let node = Arc::new(Node::boot().await.unwrap());
    install_model(&node, scripted_gateway(&["SELECT value FROM readings"]));
    select_model(&node, ws).await;

    // rules.run but no store.query → the collect of the platform source is denied inside the run.
    let p = principal("user:ada", ws, &["mcp:rules.run:call"]);
    let err = run_rule(
        &node,
        &p,
        ws,
        r#"emit(ai.ask("which ran hot?").records());"#,
    )
    .await;
    assert!(
        err.is_err(),
        "model-proposed SQL must be re-fenced through collect + caps — denied without store.query"
    );
}

// ── the budget meter still charges ───────────────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn call_budget_bites_with_a_real_model_behind() {
    // The AI budget is unchanged by this scope; prove it STILL bites with a real model behind. The
    // default call budget is 8 (`LB_RULES_AI_MAX_CALLS`); a rule making 9 `ai.complete` calls exceeds
    // it — asserted WITHOUT mutating process-global env (which would race parallel tests).
    let ws = "rules-ai-budget";
    let node = Arc::new(Node::boot().await.unwrap());
    // Script 9 turns so the mock isn't the limiter — the meter is.
    let turns: Vec<&str> = vec!["ok"; 9];
    install_model(&node, scripted_gateway(&turns));
    select_model(&node, ws).await;
    let p = principal("user:ada", ws, RULE_CAPS);

    // 9 ai.complete calls > the default 8-call budget → the meter stops the run.
    let body = r#"for i in 0..9 { ai.complete("x"); } emit("done");"#;
    let err = run_rule(&node, &p, ws, body).await;
    assert!(
        err.is_err(),
        "the AI call budget must bite with a real model behind"
    );
}

// ── adapter unit ─────────────────────────────────────────────────────────────────────────────────

/// A tiny scripted `ModelAccess` — answers every turn with a fixed content string, no tools.
struct AnswerModel(String);
impl ModelAccess for AnswerModel {
    fn turn(
        &self,
        _ws: &str,
        _messages: &[(String, String)],
        _tools: &[AllowedTool],
        _prior: &[CallOutcome],
        _key: &str,
    ) -> impl Future<Output = Turn> + Send {
        let c = self.0.clone();
        async move {
            Turn {
                content: c,
                calls: vec![],
                done: true,
            }
        }
    }
}

/// A model that only proposes a tool call with empty content — the single-turn adapter must surface a
/// clear error (a rule has no loop to run tools), never hang.
struct ToolOnlyModel;
impl ModelAccess for ToolOnlyModel {
    fn turn(
        &self,
        _ws: &str,
        _messages: &[(String, String)],
        _tools: &[AllowedTool],
        _prior: &[CallOutcome],
        _key: &str,
    ) -> impl Future<Output = Turn> + Send {
        async {
            Turn {
                content: String::new(),
                calls: vec![lb_host::ProposedCall {
                    id: "c1".into(),
                    name: "some.tool".into(),
                    input: "{}".into(),
                }],
                done: false,
            }
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn adapter_maps_complete_and_propose_sql_onto_turn() {
    let model: Arc<dyn ErasedModel> = Arc::new(AnswerModel("SELECT 1 AS v".into()));
    // The adapter captures `Handle::current()` at construction (on the async thread) and `block_on`s
    // in its (synchronous) seam methods — so it is exercised on a BLOCKING thread, exactly as the rule
    // engine drives it under `spawn_blocking`. Calling `complete` on the async worker would panic
    // ("cannot block_on from within a runtime"), which is the correct constraint.
    let adapter = AgentRuleModel::new(model, "ws-x", "idem-test");
    let (text, tokens, sql) = tokio::task::spawn_blocking(move || {
        let (text, tokens) = adapter.complete("hello").expect("complete");
        let sql = adapter
            .propose_sql("which ran hot?", "readings(value:number)")
            .expect("propose_sql");
        (text, tokens, sql)
    })
    .await
    .unwrap();

    assert_eq!(text, "SELECT 1 AS v");
    assert!(tokens >= 1, "tokens estimated (non-zero) from content");
    assert_eq!(
        sql, "SELECT 1 AS v",
        "propose_sql returns the model's SQL, trimmed"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn adapter_errors_when_model_returns_only_tool_calls() {
    let model: Arc<dyn ErasedModel> = Arc::new(ToolOnlyModel);
    let adapter = AgentRuleModel::new(model, "ws-x", "idem-test");
    let err = tokio::task::spawn_blocking(move || adapter.complete("hello"))
        .await
        .unwrap();
    assert!(
        err.is_err(),
        "a tool-only turn must error (single-turn, no loop), never hang"
    );
}
