//! S5 EXIT-GATE (the offline/sync part) + the MANDATORY offline/sync category (testing §2.3): a
//! workflow **job survives the edge disconnecting and resumes idempotently**. Two honest models of
//! "interrupted then resumed", with the durable job record as the only source of truth:
//!
//!   1. **Mid-loop interruption.** The session persists step 0, then the edge disconnects BEFORE the
//!      loop finished (status still `Running`, cursor at 1). A `resume` re-reads the durable record
//!      and continues from the cursor — it does NOT re-run step 0, does NOT duplicate it, and the
//!      gateway's idempotency cache means the resumed turn is not re-spent.
//!   2. **Duplicate delivery.** The edge missed the completion and retries the whole invocation with
//!      the SAME gateway. The append-addressed transcript + the idempotency cache make the retry a
//!      no-op: the same steps, the same provider-call count — at-least-once is safe.
//!
//! Mock provider (the only external stubbed). Multi-thread flavor + unique workspace ids.

use std::sync::Arc;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{invoke, load_extension, resume, serve_ext, AllowedTool, Invocation, Node};
use lb_jobs::{append_event, create, Job, JobStatus, TranscriptEvent};
use lb_role_ai_gateway::{AiGateway, AiResponse, MockProvider, ToolCall};

const MANIFEST: &str = include_str!("../../../extensions/hello/extension.toml");

fn hello_wasm() -> Vec<u8> {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../extensions/hello/target/wasm32-wasip2/release/hello_ext.wasm");
    std::fs::read(&path).expect("hello wasm built")
}

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
    let token = mint(&key, &claims);
    verify(&key, &token, 1).expect("token verifies")
}

const INVOKE: &str = "mcp:agent.invoke:call";
const ECHO: &str = "mcp:hello.echo:call";

fn echo_tool() -> Vec<AllowedTool> {
    vec![AllowedTool {
        name: "hello.echo".into(),
        description: "echo".into(),
        input_schema: None,
    }]
}

fn echo_call(id: &str, msg: &str) -> ToolCall {
    ToolCall {
        id: id.into(),
        name: "hello.echo".into(),
        input: format!(r#"{{"msg":"{msg}"}}"#),
    }
}

async fn hub_with_hello(ws_caps: &[&str], ws: &str) -> (Arc<Node>, Principal) {
    let node = Arc::new(Node::boot().await.unwrap());
    load_extension(&node, MANIFEST, &hello_wasm(), &[])
        .await
        .unwrap();
    // Leak the server: kept alive for the test by std::mem::forget would be cleaner, but returning
    // it complicates the signature; instead the caller holds the node and we serve here, dropping
    // the handle is fine because the queryable task holds its own Arc<Registry>.
    std::mem::forget(
        serve_ext(&node.bus, node.registry.clone(), "hello", &node.node_id(), &[ws])
            .await
            .unwrap(),
    );
    (node.clone(), principal(ws, ws_caps))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_session_interrupted_mid_loop_resumes_from_its_cursor() {
    let ws = "agent-offline-resume";
    let (node, caller) = hub_with_hello(&[INVOKE, ECHO], ws).await;

    // 1. The edge ran turn 0 (an assistant turn + one tool call + its result) and persisted those
    //    typed events, THEN disconnected — the loop never reached `done` (status still `Running`).
    //    We model that durable partial state directly as a real typed transcript (Part 0): three
    //    events for one completed turn, so the rehydrating resume continues the conversation.
    let job = Job::new("sess", "agent-session", "echo twice", 1);
    create(&node.store, ws, &job).await.unwrap();
    append_event(
        &node.store,
        ws,
        "sess",
        0,
        TranscriptEvent::AssistantTurn {
            content: "I'll echo first.".into(),
        },
    )
    .await
    .unwrap();
    append_event(
        &node.store,
        ws,
        "sess",
        1,
        TranscriptEvent::ToolCallProposed {
            id: "c0".into(),
            name: "hello.echo".into(),
            args: r#"{"msg":"before-disconnect"}"#.into(),
        },
    )
    .await
    .unwrap();
    append_event(
        &node.store,
        ws,
        "sess",
        2,
        TranscriptEvent::ToolResult {
            id: "c0".into(),
            ok: Some("before-disconnect".into()),
            err: None,
        },
    )
    .await
    .unwrap();
    let job = lb_jobs::load(&node.store, ws, "sess")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        job.cursor, 3,
        "one full turn's events landed before the disconnect"
    );
    assert_eq!(job.status, JobStatus::Running, "the session did not finish");

    // 2. RECONNECT + RESUME. The model's script begins at the RESUME turn: one more tool call, then
    //    stop. The resume continues from cursor 1 — it must NOT re-run step 0.
    let gw = AiGateway::new(MockProvider::new(vec![
        AiResponse::calls("resuming", vec![echo_call("c1", "after-resume")], 5),
        AiResponse::stop("resumed and finished", 5),
    ]));

    let answer = resume(
        &node,
        &gw,
        &caller,
        &[ECHO.into()],
        ws,
        "sess",
        &echo_tool(),
        2,
    )
    .await
    .expect("resume continues the session");

    assert_eq!(answer, "resumed and finished");

    let done = lb_jobs::load(&node.store, ws, "sess")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(done.status, JobStatus::Done, "resumed to completion");

    // The pre-disconnect events survived untouched (NOT re-run, NOT duplicated): the first three
    // slots are exactly the seeded turn-0 events.
    assert_eq!(
        done.steps[0].event,
        TranscriptEvent::AssistantTurn {
            content: "I'll echo first.".into()
        },
        "the pre-disconnect assistant turn survived untouched"
    );
    assert_eq!(
        done.steps[2].event,
        TranscriptEvent::ToolResult {
            id: "c0".into(),
            ok: Some("before-disconnect".into()),
            err: None,
        },
        "the pre-disconnect tool result survived untouched"
    );

    // The post-resume echo ran exactly once — there is a NEW `after-resume` tool result that is not
    // the seeded one (proving the resume continued the conversation, did not re-ask from the goal).
    let post_results: Vec<_> = done
        .events()
        .filter_map(|e| match e {
            TranscriptEvent::ToolResult { ok: Some(v), .. } => Some(v.clone()),
            _ => None,
        })
        .collect();
    assert!(
        post_results.iter().any(|v| v.contains("after-resume")),
        "the post-resume echo ran: {post_results:?}"
    );
    assert_eq!(
        post_results
            .iter()
            .filter(|v| *v == "before-disconnect")
            .count(),
        1,
        "the pre-disconnect step was NOT re-run (no double-apply)"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_duplicated_invocation_does_not_double_apply_or_re_spend() {
    // §2.3 idempotent apply: the edge missed the completion and retries the WHOLE invocation with
    // the same gateway. The append-addressed transcript + idempotency cache make it a no-op.
    let ws = "agent-offline-dup";
    let (node, caller) = hub_with_hello(&[INVOKE, ECHO], ws).await;

    // One tool step then stop — two provider turns total.
    let gw = AiGateway::new(MockProvider::new(vec![
        AiResponse::calls("go", vec![echo_call("c0", "once")], 5),
        AiResponse::stop("finished", 5),
    ]));

    let tools = echo_tool();
    let mk = || Invocation {
        job_id: "sess",
        goal: "echo once",
        skill: None,
        doc: None,
        tools: &tools,
        ts: 1,
    };

    let first = invoke(&node, &gw, &caller, &[ECHO.into()], ws, mk())
        .await
        .unwrap();
    let calls_after_first = gw.provider_calls();

    // The edge retries (didn't see the completion). Same gateway → same idempotency keys.
    let second = invoke(&node, &gw, &caller, &[ECHO.into()], ws, mk())
        .await
        .unwrap();

    assert_eq!(first, second, "the retry returns the same answer");
    assert_eq!(
        gw.provider_calls(),
        calls_after_first,
        "the retry re-spent NOTHING — every turn hit the idempotency cache"
    );

    let job = lb_jobs::load(&node.store, ws, "sess")
        .await
        .unwrap()
        .unwrap();
    // Exactly one tool result recorded — the retry did NOT duplicate the turn's events.
    let tool_results = job
        .events()
        .filter(|e| matches!(e, TranscriptEvent::ToolResult { .. }))
        .count();
    assert_eq!(tool_results, 1, "the retry did NOT duplicate the step");
    assert_eq!(job.status, JobStatus::Done);
}
