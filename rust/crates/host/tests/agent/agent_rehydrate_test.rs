//! agent-run scope **Part 0** — the prerequisite, tested FIRST (the gate the whole scope rests on).
//! A run that performs N turns and activates a skill, then is **reloaded from the store**, must
//! rehydrate the *identical* loop state (messages, prior tool results, active skills) — i.e. a
//! resumed run **continues the conversation**, it does NOT re-ask from the goal (the old `run.rs`
//! behavior this fixes). Plus the cancel hook: cancel mid-run leaves a terminal, restorable state.
//!
//! Two layers, both real (no mocks): the pure `rehydrate` fold (unit — the projection from the
//! durable transcript), and the full loop over a real node + store + the mock *provider only*.

use std::sync::Arc;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{cancel_run, load_extension, rehydrate, resume, serve_ext, AllowedTool, Node};
use lb_jobs::{append_event, create, JobStatus, TranscriptEvent};
use lb_role_ai_gateway::{AiGateway, AiResponse, MockProvider, ToolCall};

const MANIFEST: &str = include_str!("../../../../extensions/hello/extension.toml");

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
    verify(&key, &mint(&key, &claims), 1).unwrap()
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

#[tokio::test]
async fn rehydrate_reconstructs_messages_prior_and_active_skills() {
    // UNIT: the fold from the durable transcript is the same view the live loop held — the
    // event-sourced projection (live and a reload yield the same state). N turns + a skill.
    let events = vec![
        TranscriptEvent::AssistantTurn {
            content: "thinking".into(),
        },
        TranscriptEvent::SkillActivated {
            id: "repo-conventions".into(),
        },
        TranscriptEvent::ToolCallProposed {
            id: "c0".into(),
            name: "hello.echo".into(),
            args: r#"{"msg":"a"}"#.into(),
        },
        TranscriptEvent::ToolResult {
            id: "c0".into(),
            ok: Some("a".into()),
            err: None,
        },
        TranscriptEvent::AssistantTurn {
            content: "more".into(),
        },
    ];
    let refs: Vec<&TranscriptEvent> = events.iter().collect();
    let state = rehydrate("SYS", "the goal", &refs);

    // The conversation is rebuilt in order: system, goal, assistant, tool-result, assistant.
    assert_eq!(state.messages[0], ("system".into(), "SYS".into()));
    assert_eq!(state.messages[1], ("user".into(), "the goal".into()));
    assert_eq!(state.messages[2], ("assistant".into(), "thinking".into()));
    assert_eq!(state.messages[3].0, "tool");
    assert!(state.messages[3].1.contains("c0=ok:a"));
    assert_eq!(state.messages[4], ("assistant".into(), "more".into()));
    // The activated skill survives — Part 5 depends on this.
    assert_eq!(state.active_skills, vec!["repo-conventions".to_string()]);
    assert_eq!(state.last_content, "more");
    // `prior` carries the last settled turn's outcomes.
    assert_eq!(state.prior.len(), 1);
    assert_eq!(state.prior[0].ok.as_deref(), Some("a"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_reloaded_run_continues_the_conversation_instead_of_re_asking() {
    // INTEGRATION: the headline Part-0 gate. Seed a durable transcript of one completed turn, then
    // `resume`. The resumed model turn must SEE the prior tool result in its messages (it continues)
    // — not start from the goal. We prove continuation by the post-resume answer + a fresh result.
    let ws = "agent-rehydrate";
    let node = Arc::new(Node::boot().await.unwrap());
    load_extension(&node, MANIFEST, &hello_wasm(), &[])
        .await
        .unwrap();
    std::mem::forget(
        serve_ext(
            &node.bus,
            node.registry.clone(),
            "hello",
            &node.node_id(),
            &[ws],
        )
        .await
        .unwrap(),
    );
    let caller = principal(ws, &[INVOKE, ECHO]);

    create(
        &node.store,
        ws,
        &lb_jobs::Job::new("sess", "agent-session", "summarize the findings", 1),
    )
    .await
    .unwrap();
    for (i, ev) in [
        TranscriptEvent::AssistantTurn {
            content: "I gathered the data.".into(),
        },
        TranscriptEvent::ToolCallProposed {
            id: "c0".into(),
            name: "hello.echo".into(),
            args: r#"{"msg":"data"}"#.into(),
        },
        TranscriptEvent::ToolResult {
            id: "c0".into(),
            ok: Some("data".into()),
            err: None,
        },
    ]
    .into_iter()
    .enumerate()
    {
        append_event(&node.store, ws, "sess", i as u32, ev)
            .await
            .unwrap();
    }

    // The resume model script: continue (one more echo), then stop. A fresh gateway models a new
    // process picking up the durable job.
    let gw = AiGateway::new(MockProvider::new(vec![
        AiResponse::calls("continuing", vec![echo_call("c1", "more")], 5),
        AiResponse::stop("final summary", 5),
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
    .expect("resume continues");
    assert_eq!(answer, "final summary");

    let job = lb_jobs::load(&node.store, ws, "sess")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(job.status, JobStatus::Done);
    // The pre-resume turn survived (one "data" result) AND a new turn ran ("more") — continuation,
    // not a restart-from-goal.
    let results: Vec<_> = job
        .events()
        .filter_map(|e| match e {
            TranscriptEvent::ToolResult { ok: Some(v), .. } => Some(v.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(
        results.iter().filter(|v| *v == "data").count(),
        1,
        "prior turn not re-run"
    );
    assert!(
        results.iter().any(|v| v.contains("more")),
        "new turn ran on top: {results:?}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn cancel_leaves_a_terminal_restorable_state() {
    // The cancel hook (Part 0): a cancelled run is terminal + non-resumable, but its transcript is
    // kept (restorable for audit/replay), and re-invoking does NOT restart it.
    let ws = "agent-cancel";
    let node = Arc::new(Node::boot().await.unwrap());
    let caller = principal(ws, &[INVOKE, ECHO]);
    create(
        &node.store,
        ws,
        &lb_jobs::Job::new("sess", "agent-session", "do work", 1),
    )
    .await
    .unwrap();
    append_event(
        &node.store,
        ws,
        "sess",
        0,
        TranscriptEvent::AssistantTurn {
            content: "started".into(),
        },
    )
    .await
    .unwrap();

    cancel_run(&node, ws, "sess").await.unwrap();
    let job = lb_jobs::load(&node.store, ws, "sess")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(job.status, JobStatus::Cancelled);
    assert!(!job.status.is_resumable());

    // Re-invoking a cancelled run does not re-run the loop (no provider call) — it returns the
    // answer-so-far from the durable transcript.
    let gw = AiGateway::new(MockProvider::new(vec![AiResponse::stop(
        "should not run",
        0,
    )]));
    let answer = resume(
        &node,
        &gw,
        &caller,
        &[ECHO.into()],
        ws,
        "sess",
        &echo_tool(),
        3,
    )
    .await
    .expect("re-invoke of a cancelled run is a no-op read");
    assert_eq!(
        answer, "started",
        "returns the answer so far, did not re-run"
    );
    assert_eq!(gw.provider_calls(), 0, "a cancelled run does not re-spend");
}
