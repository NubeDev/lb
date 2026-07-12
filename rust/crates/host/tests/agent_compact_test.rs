//! Slice A of agent-loop-hardening: transcript-structural **context compaction**.
//!
//! Unit half (pure, property-style over generated conversations): whole turn groups only (an
//! assistant message and its tool summary are atomic), system messages + the goal + the latest
//! group always survive, exactly one cumulative breadcrumb, retained messages are a subsequence.
//!
//! Integration half (real store/bus/loop; scripted provider per rule 9): a provider **overflow
//! fault mid-run is recovered by compacting and continuing the SAME run**, and the workspace's
//! `agent.config.compact_budget` triggers preflight compaction — workspace-walled (a ws-B budget
//! never compacts a ws-A run).

use std::sync::{Arc, Mutex};

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{
    agent_config_set, compact_to_budget, estimate_message_tokens, invoke, AgentConfig, Invocation,
    Node, BREADCRUMB_PREFIX,
};
use lb_jobs::JobStatus;
use lb_role_ai_gateway::{AiGateway, AiRequest, AiResponse, Provider, ProviderFault, ToolCall};

// ── unit: the compaction invariants ──────────────────────────────────────────────────────────────

type Msg = (String, String);

fn msg(role: &str, content: &str) -> Msg {
    (role.into(), content.into())
}

/// A conversation with `n` turn groups after the seed, a system line injected mid-history, and a
/// content size that makes each group ~25 estimated tokens.
fn conversation(n: usize) -> Vec<Msg> {
    let mut m = vec![
        msg("system", "You are a workspace agent."),
        msg("user", "reconcile the dataset"),
    ];
    for i in 0..n {
        if i == n / 2 {
            m.push(msg("system", "[skill mid-run] injected body"));
        }
        m.push(msg("assistant", &format!("assistant-turn-{i:02} {}", "x".repeat(40))));
        m.push(msg("tool", &format!("tool-summary-{i:02} {}", "y".repeat(40))));
    }
    m
}

#[test]
fn groups_drop_whole_never_split_and_protected_content_survives() {
    let original = conversation(10);
    let mut m = original.clone();
    let dropped = compact_to_budget(&mut m, 60, 0, 0);
    assert!(dropped > 0, "an over-budget conversation must compact");

    // Protected: every system message, the goal, the latest group.
    assert!(m.contains(&original[0]), "the system seed survives");
    assert!(m.contains(&original[1]), "the goal survives");
    assert!(
        m.iter().any(|(r, c)| r == "system" && c.starts_with("[skill mid-run]")),
        "an injected system line survives compaction"
    );
    let last_assistant = original[original.len() - 2].clone();
    let last_tool = original[original.len() - 1].clone();
    assert!(m.contains(&last_assistant) && m.contains(&last_tool), "the latest group survives");

    // Atomicity: for every turn i, the assistant message and its tool summary live or die together.
    for i in 0..10 {
        let a = m.iter().any(|(_, c)| c.starts_with(&format!("assistant-turn-{i:02}")));
        let t = m.iter().any(|(_, c)| c.starts_with(&format!("tool-summary-{i:02}")));
        assert_eq!(a, t, "group {i} was split (assistant retained={a}, tool retained={t})");
    }

    // Exactly one breadcrumb, carrying the drop count.
    let crumbs: Vec<&Msg> = m.iter().filter(|(_, c)| c.starts_with(BREADCRUMB_PREFIX)).collect();
    assert_eq!(crumbs.len(), 1, "exactly one breadcrumb");
    assert!(crumbs[0].1.contains(&format!("{dropped} turns")), "breadcrumb counts the drops");

    // Subsequence: the retained non-breadcrumb messages appear in the original, in order.
    let mut it = original.iter();
    for kept in m.iter().filter(|(_, c)| !c.starts_with(BREADCRUMB_PREFIX)) {
        assert!(
            it.any(|o| o == kept),
            "retained message not an in-order subsequence: {kept:?}"
        );
    }
}

#[test]
fn under_budget_is_untouched_and_recompaction_keeps_one_cumulative_breadcrumb() {
    let mut m = conversation(3);
    let before = m.clone();
    assert_eq!(compact_to_budget(&mut m, 1_000_000, 0, 0), 0);
    assert_eq!(m, before, "an under-budget conversation is untouched");

    // First compaction drops some, second (with more history) updates the SAME breadcrumb.
    let mut m = conversation(10);
    let d1 = compact_to_budget(&mut m, 200, 0, 0);
    assert!(d1 > 0);
    // Grow the tail again, compact harder.
    for i in 10..16 {
        m.push(msg("assistant", &format!("assistant-turn-{i:02} {}", "x".repeat(40))));
        m.push(msg("tool", &format!("tool-summary-{i:02} {}", "y".repeat(40))));
    }
    let d2 = compact_to_budget(&mut m, 100, 0, d1);
    assert!(d2 > 0);
    let crumbs: Vec<&Msg> = m.iter().filter(|(_, c)| c.starts_with(BREADCRUMB_PREFIX)).collect();
    assert_eq!(crumbs.len(), 1, "re-compaction replaces, never stacks, the breadcrumb");
    assert!(
        crumbs[0].1.contains(&format!("{} turns", d1 + d2)),
        "the breadcrumb is cumulative: {}",
        crumbs[0].1
    );
}

#[test]
fn nothing_droppable_returns_zero_not_a_mangled_seed() {
    // Only the seed + one group: the latest group is protected, so nothing can drop.
    let mut m = conversation(1);
    let before = m.clone();
    assert_eq!(compact_to_budget(&mut m, 1, 0, 0), 0);
    assert_eq!(m, before, "protected-only content is never mangled");
    assert!(estimate_message_tokens(&m) > 1, "and it genuinely was over budget");
}

// ── integration: overflow → compact → continue the SAME run ─────────────────────────────────────

/// A scripted provider that also captures every request's messages — so the test can SEE the
/// compacted context + breadcrumb the model was sent. Rule 9: this stands in only for the provider
/// HTTP, like `MockProvider`, with a capture hook the assertion needs.
struct CapturingScript {
    script: Mutex<Vec<Result<AiResponse, ProviderFault>>>,
    seen: Arc<Mutex<Vec<Vec<(String, String)>>>>,
}

impl Provider for CapturingScript {
    async fn complete(&self, req: &AiRequest) -> Result<AiResponse, ProviderFault> {
        self.seen.lock().unwrap().push(
            req.messages
                .iter()
                .map(|m| (m.role.clone(), m.content.clone()))
                .collect(),
        );
        let mut script = self.script.lock().unwrap();
        if script.is_empty() {
            return Ok(AiResponse::stop("(script exhausted)", 0));
        }
        script.remove(0)
    }
}

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
    let token = mint(&key, &claims);
    verify(&key, &token, 1).expect("token verifies")
}

const INVOKE: &str = "mcp:agent.invoke:call";
const CONFIG_SET: &str = "mcp:agent.config.set:call";

/// A turn proposing one call to a tool that doesn't exist — the error outcome keeps the loop
/// rolling and builds droppable history (padded so groups carry real weight).
fn probe_turn(i: usize) -> AiResponse {
    AiResponse::calls(
        format!("probing {i} {}", "p".repeat(200)),
        vec![ToolCall {
            id: format!("c{i}"),
            name: "no.such_tool".into(),
            input: "{}".into(),
        }],
        5,
    )
}

async fn drive(node: &Arc<Node>, ws: &str, job: &str, gw: &AiGateway<CapturingScript>) -> String {
    let caller = principal("user:ada", ws, &[INVOKE]);
    invoke(
        node,
        gw,
        &caller,
        &[INVOKE.to_string()],
        ws,
        Invocation {
            job_id: job,
            goal: "do the long thing",
            skill: None,
            doc: None,
            tools: &[],
            ts: 1,
        },
    )
    .await
    .expect("run settles")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_provider_overflow_is_recovered_by_compaction_and_the_run_continues() {
    let ws = "compact-overflow";
    let node = Arc::new(Node::boot().await.unwrap());
    let seen = Arc::new(Mutex::new(Vec::new()));
    let gw = AiGateway::new(CapturingScript {
        script: Mutex::new(vec![
            Ok(probe_turn(0)),
            Ok(probe_turn(1)),
            Ok(probe_turn(2)),
            Err(ProviderFault::overflow(400, "context too long")),
            Ok(AiResponse::stop("recovered after compaction", 5)),
        ]),
        seen: seen.clone(),
    });

    let answer = drive(&node, ws, "job-overflow", &gw).await;
    assert_eq!(answer, "recovered after compaction", "the SAME run continued");
    assert_eq!(
        lb_jobs::load(&node.store, ws, "job-overflow").await.unwrap().unwrap().status,
        JobStatus::Done
    );

    let seen = seen.lock().unwrap();
    // 5 provider calls: 3 probes + the overflow attempt + the compacted retry.
    assert_eq!(seen.len(), 5, "the overflow retry re-called the provider");
    let retry = &seen[4];
    assert!(
        retry.iter().any(|(_, c)| c.starts_with(BREADCRUMB_PREFIX)),
        "the retried request carries the compaction breadcrumb: {retry:?}"
    );
    let overflowed = &seen[3];
    assert!(
        retry.len() < overflowed.len(),
        "the retried request is genuinely smaller ({} vs {})",
        retry.len(),
        overflowed.len()
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_workspace_compact_budget_preflights_and_is_workspace_walled() {
    let ws_a = "compact-ws-a";
    let ws_b = "compact-ws-b";
    let node = Arc::new(Node::boot().await.unwrap());

    // ws-B (and ONLY ws-B) sets a tiny budget through the real admin verb.
    let admin_b = principal("user:root", ws_b, &[CONFIG_SET]);
    agent_config_set(
        &node,
        &admin_b,
        ws_b,
        &AgentConfig {
            compact_budget: Some(100),
            ..AgentConfig::default()
        },
    )
    .await
    .expect("admin sets ws-B budget");

    let script = || {
        vec![
            Ok(probe_turn(0)),
            Ok(probe_turn(1)),
            Ok(probe_turn(2)),
            Ok(AiResponse::stop("done", 5)),
        ]
    };

    // ws-B: the tiny budget preflights — a later request carries the breadcrumb.
    let seen_b = Arc::new(Mutex::new(Vec::new()));
    let gw_b = AiGateway::new(CapturingScript {
        script: Mutex::new(script()),
        seen: seen_b.clone(),
    });
    drive(&node, ws_b, "job-b", &gw_b).await;
    assert!(
        seen_b
            .lock()
            .unwrap()
            .iter()
            .any(|req| req.iter().any(|(_, c)| c.starts_with(BREADCRUMB_PREFIX))),
        "ws-B's tiny budget must trigger preflight compaction"
    );

    // ws-A: same run shape, NO configured budget — the ws-B setting must not leak across the wall.
    let seen_a = Arc::new(Mutex::new(Vec::new()));
    let gw_a = AiGateway::new(CapturingScript {
        script: Mutex::new(script()),
        seen: seen_a.clone(),
    });
    drive(&node, ws_a, "job-a", &gw_a).await;
    assert!(
        !seen_a
            .lock()
            .unwrap()
            .iter()
            .any(|req| req.iter().any(|(_, c)| c.starts_with(BREADCRUMB_PREFIX))),
        "ws-A must not be compacted by ws-B's budget (workspace isolation)"
    );
}
