//! agent-run scope Part 4 (in-process, against a REAL node) — the driver paths that need a real
//! store/bus/policy and are awkward to script over a pipe: trusted-session auth deny, the cancel
//! hook, and the **disconnect-mid-permission** contract end to end (suspend on an Ask → prompt ends
//! with the "suspended" stop reason → settle out-of-band → `session/resume` continues to a finish).
//!
//! The only stub is the model provider (MockProvider). Everything else — node, store, bus, the wasm
//! `hello.echo` tool, the policy + decision records — is real (rule 9).

use std::sync::Arc;

use lb_auth::{mint, Claims, Role, SigningKey};
use lb_host::{call_tool, load_extension, save_policy, serve_ext, AllowedTool, Node, Policy, Rule};
use lb_role_acp::AcpSession;
use lb_role_ai_gateway::{AiGateway, AiResponse, MockProvider, ToolCall};
use serde_json::json;

const MANIFEST: &str = include_str!("../../../extensions/hello/extension.toml");

fn hello_wasm() -> Vec<u8> {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../extensions/hello/target/wasm32-wasip2/release/hello_ext.wasm");
    std::fs::read(&path).expect("hello wasm built")
}

const INVOKE: &str = "mcp:agent.invoke:call";
const WATCH: &str = "mcp:agent.watch:call";
const ECHO: &str = "mcp:hello.echo:call";
const DECIDE: &str = "mcp:agent.decide:call";

fn token(key: &SigningKey, ws: &str, caps: &[&str]) -> String {
    mint(
        key,
        &Claims {
            sub: "user:dev".into(),
            ws: ws.into(),
            role: Role::Member,
            caps: caps.iter().map(|s| s.to_string()).collect(),
            iat: 0,
            exp: u64::MAX,
            constraint: None,
            run_id: None,
        },
    )
}

fn echo_tool() -> Vec<AllowedTool> {
    vec![AllowedTool {
        name: "hello.echo".into(),
        description: "echo".into(),
        input_schema: None,
    }]
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_forged_or_unsigned_token_is_rejected() {
    // The trusted-session wall (Part 4 risk): a token signed by a DIFFERENT key does not verify — the
    // adapter is as denied as any forged call. No bypass for a local stdio process.
    let node = Arc::new(Node::boot().await.unwrap());
    let model = Arc::new(AiGateway::new(MockProvider::new(vec![])));
    let real_key = SigningKey::generate();
    let attacker_key = SigningKey::generate();
    let forged = token(&attacker_key, "acp-ws", &[INVOKE, WATCH]);

    match AcpSession::authenticate(node, model, &real_key, &forged, 1, vec![], echo_tool()) {
        Err(e) => assert_eq!(e.code, lb_role_acp::codes::UNAUTHENTICATED),
        Ok(_) => panic!("a token signed by another key must not authenticate"),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn disconnect_mid_permission_suspends_durably_and_resumes_out_of_band() {
    // THE Part-4 contract: the run hits an Ask, the prompt turn ends "suspended" (the editor could
    // disconnect here and nothing is lost — the pause is durable), the decision is settled OUT OF
    // BAND, and session/resume picks the run back up to a finish.
    let ws = "acp-suspend";
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

    // Policy: ASK on hello.echo — so the proposed call suspends the run (Part 2).
    save_policy(
        &node.store,
        ws,
        &Policy {
            rules: vec![Rule {
                tool: "hello.echo".into(),
                arg: None,
                effect: lb_host::Effect::Ask,
            }],
        },
    )
    .await
    .unwrap();

    // The model proposes the echo (which the policy will Ask on), then — after the denial result on
    // resume — stops.
    let model = Arc::new(AiGateway::new(MockProvider::new(vec![
        AiResponse::calls(
            "I'll echo.",
            vec![ToolCall {
                id: "c1".into(),
                name: "hello.echo".into(),
                input: r#"{"msg":"hi"}"#.into(),
            }],
            5,
        ),
        AiResponse::stop("done after decision", 5),
    ])));

    let key = SigningKey::generate();
    let tok = token(&key, ws, &[INVOKE, WATCH, ECHO, DECIDE]);
    let mut session = AcpSession::authenticate(
        node.clone(),
        model.clone(),
        &key,
        &tok,
        1,
        vec![ECHO.into()],
        echo_tool(),
    )
    .unwrap();

    // session/new + session/prompt → the run SUSPENDS on the Ask; the turn ends "refusal" (our
    // mapped suspended stop reason).
    session
        .handle("session/new", &json!({"sessionId": "run1"}))
        .await
        .unwrap();
    let prompt = session
        .handle(
            "session/prompt",
            &json!({"sessionId": "run1", "prompt": "echo hi"}),
        )
        .await
        .unwrap();
    assert_eq!(
        prompt.result["stopReason"], "refusal",
        "a suspended turn ends with the suspended/awaiting-permission stop reason: {}",
        prompt.result
    );

    // The run is durably Suspended — independent of any connection (the editor could be gone).
    let job = lb_jobs::load(&node.store, ws, "run1")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(job.status, lb_jobs::JobStatus::Suspended);

    // Settle the decision OUT OF BAND (a reviewer, via the MCP verb) — Deny the call.
    let decider = lb_auth::verify(&key, &token(&key, ws, &[DECIDE]), 1).unwrap();
    call_tool(
        &node,
        &decider,
        ws,
        "agent.decide",
        &json!({"job_id": "run1", "tool_call_id": "c1", "decision": "deny"}).to_string(),
    )
    .await
    .expect("the reviewer settles the decision");

    // The editor reconnects and resumes — the run rehydrates, applies the Deny, and finishes.
    let resumed = session
        .handle("session/resume", &json!({"sessionId": "run1"}))
        .await
        .unwrap();
    assert_eq!(
        resumed.result["stopReason"], "end_turn",
        "after the out-of-band decision, resume continues to a normal finish: {}",
        resumed.result
    );
    let job = lb_jobs::load(&node.store, ws, "run1")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        job.status,
        lb_jobs::JobStatus::Done,
        "the run completed after resume"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn session_cancel_stops_the_run_durably() {
    // The Part-0 cancel hook reached via ACP session/cancel: a run paused on an Ask (still
    // resumable) is cancelled, leaving a terminal, restorable transcript. (Cancelling an
    // already-FINISHED run is correctly refused by Part 0 — that is tested in jobs; here we cancel a
    // live, resumable run, which is the case a UI stop button / session/cancel actually hits.)
    let ws = "acp-cancel";
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
    // Ask on hello.echo → the run suspends (stays resumable) rather than finishing.
    save_policy(
        &node.store,
        ws,
        &Policy {
            rules: vec![Rule {
                tool: "hello.echo".into(),
                arg: None,
                effect: lb_host::Effect::Ask,
            }],
        },
    )
    .await
    .unwrap();
    let model = Arc::new(AiGateway::new(MockProvider::new(vec![AiResponse::calls(
        "echo",
        vec![ToolCall {
            id: "c1".into(),
            name: "hello.echo".into(),
            input: r#"{"msg":"hi"}"#.into(),
        }],
        1,
    )])));
    let key = SigningKey::generate();
    let tok = token(&key, ws, &[INVOKE, WATCH, ECHO]);
    let mut session = AcpSession::authenticate(
        node.clone(),
        model,
        &key,
        &tok,
        1,
        vec![ECHO.into()],
        echo_tool(),
    )
    .unwrap();

    session
        .handle("session/new", &json!({"sessionId": "run1"}))
        .await
        .unwrap();
    // Drive one prompt → the run suspends on the Ask (resumable).
    session
        .handle(
            "session/prompt",
            &json!({"sessionId": "run1", "prompt": "hi"}),
        )
        .await
        .unwrap();
    let job = lb_jobs::load(&node.store, ws, "run1")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        job.status,
        lb_jobs::JobStatus::Suspended,
        "paused, still resumable"
    );
    // Cancel it (Suspended → Cancelled is allowed).
    session
        .handle("session/cancel", &json!({"sessionId": "run1"}))
        .await
        .unwrap();
    let job = lb_jobs::load(&node.store, ws, "run1")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(job.status, lb_jobs::JobStatus::Cancelled);
}
