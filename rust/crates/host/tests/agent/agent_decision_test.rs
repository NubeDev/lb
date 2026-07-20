//! agent-run scope **Part 2** — the per-tool-call Allow/Deny/Ask gate + the durable first-settle
//! decision. All against the REAL store + bus + wasm; the ONLY stub is the model provider
//! (`MockProvider`, testing §3). Mandatory categories: first-settle (the test that would FAIL against
//! last-writer-wins `lb_inbox::Resolution`), capability-deny (§2.1), workspace-isolation (§2.2),
//! offline/sync (§2.3), and the Ask→suspend→decide→resume integration (Deny + Allow→replay).

use std::sync::Arc;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{
    call_agent_tool, invoke, load_extension, resume, save_policy, serve_ext, settle_decision,
    AllowedTool, Effect, Invocation, Node, Policy, Rule, SettleOutcome, DENIED_BY_POLICY,
};
use lb_jobs::{JobStatus, SuspensionDecision, TranscriptEvent};
use lb_role_ai_gateway::{AiGateway, AiResponse, MockProvider, ToolCall};
use serde_json::json;

const MANIFEST: &str = include_str!("../../../../extensions/hello/extension.toml");

fn hello_wasm() -> Vec<u8> {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../extensions/hello/target/wasm32-wasip2/release/hello_ext.wasm");
    std::fs::read(&path)
        .unwrap_or_else(|e| panic!("missing hello component at {} ({e})", path.display()))
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
const ECHO: &str = "mcp:hello.echo:call";
const DECIDE: &str = "mcp:agent.decide:call";
const POLICY_SET: &str = "mcp:agent.policy.set:call";
const INBOX_REC: &str = "mcp:inbox.record:call";
const INBOX_LIST: &str = "mcp:inbox.list:call";

fn echo_tool() -> Vec<AllowedTool> {
    vec![AllowedTool {
        name: "hello.echo".into(),
        description: "echo a message".into(),
        input_schema: None,
    }]
}

/// A gateway whose model proposes one `hello.echo` call, then stops with a final answer. (Same shape
/// as the S5 exit-gate test — the policy decides whether the call runs, is denied, or asks.)
fn echo_then_stop() -> AiGateway<MockProvider> {
    AiGateway::new(MockProvider::new(vec![
        AiResponse::calls(
            "I'll echo it.",
            vec![ToolCall {
                id: "c1".into(),
                name: "hello.echo".into(),
                input: r#"{"msg":"agent-says-hi"}"#.into(),
            }],
            10,
        ),
        AiResponse::stop("done", 5),
    ]))
}

/// Boot a node with the hello extension served, plus a granted Ask policy on `hello.echo`.
async fn node_with_ask_policy(ws: &str) -> Arc<Node> {
    let node = Arc::new(Node::boot().await.unwrap());
    load_extension(&node, MANIFEST, &hello_wasm(), &[])
        .await
        .unwrap();
    let _server = serve_ext(
        &node.bus,
        node.registry.clone(),
        "hello",
        &node.node_id(),
        &[ws],
    )
    .await
    .unwrap();
    // Keep the server alive for the test's lifetime by leaking it (the node owns the bus).
    std::mem::forget(_server);
    save_policy(
        &node.store,
        ws,
        &Policy {
            rules: vec![Rule {
                tool: "hello.echo".into(),
                arg: None,
                effect: Effect::Ask,
            }],
        },
    )
    .await
    .unwrap();
    node
}

// ---------------------------------------------------------------------------------------------
// First-settle — THE test that would fail against last-writer-wins `lb_inbox::Resolution`.
// ---------------------------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn two_decides_first_binds_second_is_rejected_and_post_run_decide_is_a_noop() {
    let ws = "agent-first-settle";
    let node = node_with_ask_policy(ws).await;
    let caller = principal("user:ada", ws, &[INVOKE, ECHO, INBOX_REC]);

    // Run suspends on the Ask.
    let gw = echo_then_stop();
    let _ = invoke(
        &node,
        &gw,
        &caller,
        &[ECHO.into(), INBOX_REC.into()],
        ws,
        Invocation {
            job_id: "s",
            goal: "echo",
            skill: None,
            doc: None,
            tools: &echo_tool(),
            ts: 1,
        },
    )
    .await
    .unwrap();
    let job = lb_jobs::load(&node.store, ws, "s").await.unwrap().unwrap();
    assert_eq!(job.status, JobStatus::Suspended);

    // FIRST decide binds.
    let first = settle_decision(&node.store, ws, "s", "c1", SuspensionDecision::Allow, 2)
        .await
        .unwrap();
    assert!(
        matches!(first, SettleOutcome::Bound(_)),
        "first decide binds"
    );

    // SECOND decide on the same {job,tool_call} is REJECTED (not an upsert) — first-settle. Against
    // `lb_inbox::Resolution` (last-writer-wins) this would silently flip the decision.
    let second = settle_decision(&node.store, ws, "s", "c1", SuspensionDecision::Deny, 3)
        .await
        .unwrap();
    assert!(
        matches!(
            second,
            SettleOutcome::AlreadySettled(SuspensionDecision::Allow)
        ),
        "second decide is a no-op returning the ALREADY-BOUND outcome, not the new one"
    );

    // A decide arriving after the tool already ran (resume) is also a no-op.
    let _ = resume(
        &node,
        &gw,
        &caller,
        &[ECHO.into()],
        ws,
        "s",
        &echo_tool(),
        4,
    )
    .await
    .unwrap();
    let post = settle_decision(&node.store, ws, "s", "c1", SuspensionDecision::Deny, 5)
        .await
        .unwrap();
    assert!(
        matches!(post, SettleOutcome::AlreadySettled(_)),
        "a decide after the tool ran is a no-op"
    );
}

// ---------------------------------------------------------------------------------------------
// Capability-deny (§2.1)
// ---------------------------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn cannot_decide_without_the_decide_cap_nor_set_policy_without_admin_cap() {
    let ws = "agent-decide-deny";
    let node = Arc::new(Node::boot().await.unwrap());

    // Lacking `mcp:agent.decide:call` → agent.decide is denied (opaque).
    let no_decide = principal("user:eve", ws, &[POLICY_SET]);
    let err = call_agent_tool(
        &node,
        &no_decide,
        ws,
        "agent.decide",
        &json!({"job_id":"s","tool_call_id":"c1","decision":"allow"}),
    )
    .await
    .expect_err("decide denied without the cap");
    assert!(matches!(err, lb_mcp::ToolError::Denied));

    // Lacking the admin `mcp:agent.policy.set:call` → agent.policy.set is denied.
    let no_admin = principal("user:eve", ws, &[DECIDE]);
    let err = call_agent_tool(
        &node,
        &no_admin,
        ws,
        "agent.policy.set",
        &json!({"rules":[]}),
    )
    .await
    .expect_err("policy.set denied without the admin cap");
    assert!(matches!(err, lb_mcp::ToolError::Denied));
}

// ---------------------------------------------------------------------------------------------
// Workspace-isolation (§2.2)
// ---------------------------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_ws_b_decide_cannot_settle_a_ws_a_suspension() {
    let ws_a = "agent-iso-a";
    let node = node_with_ask_policy(ws_a).await;
    let caller = principal("user:ada", ws_a, &[INVOKE, ECHO, INBOX_REC]);
    let _ = invoke(
        &node,
        &echo_then_stop(),
        &caller,
        &[ECHO.into(), INBOX_REC.into()],
        ws_a,
        Invocation {
            job_id: "s",
            goal: "echo",
            skill: None,
            doc: None,
            tools: &echo_tool(),
            ts: 1,
        },
    )
    .await
    .unwrap();

    // A decide in ws-B addresses ws-B's namespace — there is no such decision there, so it is a
    // refusal (no pending decision), and ws-A's suspension is untouched.
    let err = settle_decision(
        &node.store,
        "agent-iso-b",
        "s",
        "c1",
        SuspensionDecision::Allow,
        2,
    )
    .await;
    assert!(err.is_err(), "a ws-B settle finds no ws-A decision");
    let job_a = lb_jobs::load(&node.store, ws_a, "s")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(job_a.status, JobStatus::Suspended, "ws-A stays suspended");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_ws_b_policy_does_not_affect_ws_a() {
    // ws-B has an Ask policy on hello.echo; ws-A has none → ws-A's run is NOT gated (default-allow).
    let node = Arc::new(Node::boot().await.unwrap());
    load_extension(&node, MANIFEST, &hello_wasm(), &[])
        .await
        .unwrap();
    // Both workspaces this test exercises — the ws-A run and the ws-B policy.
    let server = serve_ext(
        &node.bus,
        node.registry.clone(),
        "hello",
        &node.node_id(),
        &["agent-pol-a", "agent-pol-b"],
    )
    .await
    .unwrap();
    std::mem::forget(server);
    save_policy(
        &node.store,
        "agent-pol-b",
        &Policy {
            rules: vec![Rule {
                tool: "hello.echo".into(),
                arg: None,
                effect: Effect::Ask,
            }],
        },
    )
    .await
    .unwrap();

    let ws_a = "agent-pol-a";
    let caller = principal("user:ada", ws_a, &[INVOKE, ECHO]);
    let answer = invoke(
        &node,
        &echo_then_stop(),
        &caller,
        &[ECHO.into()],
        ws_a,
        Invocation {
            job_id: "s",
            goal: "echo",
            skill: None,
            doc: None,
            tools: &echo_tool(),
            ts: 1,
        },
    )
    .await
    .unwrap();
    assert_eq!(
        answer, "done",
        "ws-A ran to completion — ws-B's Ask policy did not leak"
    );
    let job = lb_jobs::load(&node.store, ws_a, "s")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(job.status, JobStatus::Done);
}

// ---------------------------------------------------------------------------------------------
// Offline/sync (§2.3): suspend → reload the job from store → decide resumes exactly once.
// ---------------------------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn suspend_reload_then_decide_resumes_exactly_once() {
    let ws = "agent-offline";
    let node = node_with_ask_policy(ws).await;
    let caller = principal("user:ada", ws, &[INVOKE, ECHO, INBOX_REC]);
    // One persistent gateway across invoke+resume — models a real node whose model-turn cache + script
    // position survive the suspension (a fresh provider would re-answer turn 0 and re-propose the call).
    let gw = echo_then_stop();
    let _ = invoke(
        &node,
        &gw,
        &caller,
        &[ECHO.into(), INBOX_REC.into()],
        ws,
        Invocation {
            job_id: "s",
            goal: "echo",
            skill: None,
            doc: None,
            tools: &echo_tool(),
            ts: 1,
        },
    )
    .await
    .unwrap();

    // Model a dropped connection / restart by reloading the job from the store (the suspension lives
    // in the agent_decision record + transcript, not the connection).
    let reloaded = lb_jobs::load(&node.store, ws, "s").await.unwrap().unwrap();
    assert_eq!(reloaded.status, JobStatus::Suspended);

    // A later agent.decide (Allow→replay) settles and the run resumes.
    let bound = settle_decision(&node.store, ws, "s", "c1", SuspensionDecision::Allow, 2)
        .await
        .unwrap();
    assert!(matches!(bound, SettleOutcome::Bound(_)));
    let answer = resume(
        &node,
        &gw,
        &caller,
        &[ECHO.into()],
        ws,
        "s",
        &echo_tool(),
        3,
    )
    .await
    .unwrap();
    assert_eq!(answer, "done");

    // Exactly once: the transcript has ONE OK result for c1 (the replayed echo), not two. A duplicate
    // decide + a second resume must not double-apply.
    let _ = settle_decision(&node.store, ws, "s", "c1", SuspensionDecision::Allow, 4)
        .await
        .unwrap();
    let _ = resume(
        &node,
        &gw,
        &caller,
        &[ECHO.into()],
        ws,
        "s",
        &echo_tool(),
        5,
    )
    .await
    .unwrap();
    let job = lb_jobs::load(&node.store, ws, "s").await.unwrap().unwrap();
    let ok_results = job
        .events()
        .filter(|e| matches!(e, TranscriptEvent::ToolResult { id, ok: Some(_), .. } if id == "c1"))
        .count();
    assert_eq!(ok_results, 1, "the replayed echo ran exactly once");
}

// ---------------------------------------------------------------------------------------------
// Integration: Ask → suspend → decide Deny → resume → model gets denied result → finishes.
// ---------------------------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn ask_suspends_then_deny_resumes_with_a_denied_result() {
    let ws = "agent-ask-deny";
    let node = node_with_ask_policy(ws).await;
    let caller = principal("user:ada", ws, &[INVOKE, ECHO, INBOX_REC, INBOX_LIST]);
    let gw = echo_then_stop();

    let _ = invoke(
        &node,
        &gw,
        &caller,
        &[ECHO.into(), INBOX_REC.into()],
        ws,
        Invocation {
            job_id: "s",
            goal: "echo",
            skill: None,
            doc: None,
            tools: &echo_tool(),
            ts: 1,
        },
    )
    .await
    .unwrap();

    // Suspended, with SuspensionOpened in the transcript and a needs:approval inbox item.
    let job = lb_jobs::load(&node.store, ws, "s").await.unwrap().unwrap();
    assert_eq!(job.status, JobStatus::Suspended);
    assert!(
        job.events().any(|e| matches!(e, TranscriptEvent::SuspensionOpened { tool_call_id, .. } if tool_call_id == "c1")),
        "SuspensionOpened recorded"
    );
    let inbox = lb_host::list_inbox(&node.store, &caller, ws, lb_host::DECISION_APPROVAL_CHANNEL)
        .await
        .unwrap();
    assert!(
        !inbox.is_empty(),
        "a needs:approval inbox item was surfaced"
    );

    // Deny it, then resume — the model gets a denied result and finishes.
    settle_decision(&node.store, ws, "s", "c1", SuspensionDecision::Deny, 2)
        .await
        .unwrap();
    let answer = resume(
        &node,
        &gw,
        &caller,
        &[ECHO.into()],
        ws,
        "s",
        &echo_tool(),
        3,
    )
    .await
    .unwrap();
    assert_eq!(answer, "done");
    let job = lb_jobs::load(&node.store, ws, "s").await.unwrap().unwrap();
    assert_eq!(job.status, JobStatus::Done);
    assert!(
        job.events().any(|e| matches!(e, TranscriptEvent::ToolResult { id, err: Some(m), .. } if id == "c1" && m == DENIED_BY_POLICY)),
        "the denied-by-policy result was fed to the model"
    );
    assert!(
        job.events().any(|e| matches!(
            e,
            TranscriptEvent::SuspensionSettled {
                decision: SuspensionDecision::Deny,
                ..
            }
        )),
        "the settle was recorded on resume"
    );
}

// ---------------------------------------------------------------------------------------------
// Integration: Allow→replay — the originally-proposed call runs from the persisted args.
// ---------------------------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn ask_suspends_then_allow_replays_the_original_call() {
    let ws = "agent-ask-allow";
    let node = node_with_ask_policy(ws).await;
    let caller = principal("user:ada", ws, &[INVOKE, ECHO, INBOX_REC]);
    let gw = echo_then_stop();

    let _ = invoke(
        &node,
        &gw,
        &caller,
        &[ECHO.into(), INBOX_REC.into()],
        ws,
        Invocation {
            job_id: "s",
            goal: "echo",
            skill: None,
            doc: None,
            tools: &echo_tool(),
            ts: 1,
        },
    )
    .await
    .unwrap();

    settle_decision(&node.store, ws, "s", "c1", SuspensionDecision::Allow, 2)
        .await
        .unwrap();
    let answer = resume(
        &node,
        &gw,
        &caller,
        &[ECHO.into()],
        ws,
        "s",
        &echo_tool(),
        3,
    )
    .await
    .unwrap();
    assert_eq!(answer, "done");
    let job = lb_jobs::load(&node.store, ws, "s").await.unwrap().unwrap();
    assert_eq!(job.status, JobStatus::Done);
    assert!(
        job.events().any(
            |e| matches!(e, TranscriptEvent::ToolResult { id, ok: Some(_), .. } if id == "c1")
        ),
        "Allow→replay ran the original echo and recorded an OK result"
    );
}
