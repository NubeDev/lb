//! S5 EXIT-GATE (local part): an edge user invokes the central agent; the agent calls the gateway
//! for a model turn and runs a **granted MCP tool** inside its loop, over a substrate of a granted
//! **skill** and a shared **doc** — all under the DERIVED (intersected) principal. Plus the
//! mandatory **capability-deny** category (testing §2.1): the invoke gate, and a tool the
//! intersection forbids inside the loop.
//!
//! The model provider is the deterministic `MockProvider` (testing §3 — the only external stubbed);
//! the store + bus + wasm are real. Multi-thread flavor + a UNIQUE workspace id per test (a node
//! boots a Zenoh peer; in-process peers share a workspace's keyspace — carry-forward from S3).

use std::sync::Arc;

use lb_assets::ContentType;
use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{
    grant_skill, invoke, load_extension, put_doc, put_skill, serve_ext, AllowedTool, Invocation,
    Node,
};
use lb_role_ai_gateway::{AiGateway, AiResponse, MockProvider, ToolCall};

const MANIFEST: &str = include_str!("../../../../extensions/hello/extension.toml");

fn hello_wasm() -> Vec<u8> {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../extensions/hello/target/wasm32-wasip2/release/hello_ext.wasm");
    std::fs::read(&path).unwrap_or_else(|e| {
        panic!(
            "missing hello component at {} ({e}).\nBuild it first:\n  \
             (cd rust/extensions/hello && cargo build --target wasm32-wasip2 --release)",
            path.display()
        )
    })
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
const SKILL_R: &str = "store:skill/*:read";
const SKILL_W: &str = "store:skill/*:write";
const DOC_R: &str = "store:doc/*:read";
const DOC_W: &str = "store:doc/*:write";

/// The tools the model may propose. One: the granted echo tool on the hub.
fn echo_tool() -> Vec<AllowedTool> {
    vec![AllowedTool {
        name: "hello.echo".into(),
        description: "echo a message".into(),
        input_schema: None,
    }]
}

/// A gateway whose model proposes one `hello.echo` call, then stops with a final answer.
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
        AiResponse::stop("done: echoed agent-says-hi", 5),
    ]))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn an_edge_user_invokes_the_agent_which_calls_the_gateway_and_a_granted_tool() {
    // THE S5 EXIT GATE (local): invoke → gateway turn → granted tool call → final answer, with a
    // granted skill + shared doc as substrate, all under the derived principal.
    let ws = "agent-exit-gate";
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

    // The caller can invoke the agent, call echo, and read/write the substrate.
    let caller = principal(
        "user:ada",
        ws,
        &[INVOKE, ECHO, SKILL_R, SKILL_W, DOC_R, DOC_W],
    );

    // Seed the substrate: a granted skill and a shared doc (owner-readable).
    put_skill(
        &node.store,
        &caller,
        ws,
        "summarize",
        "1",
        "summarize text",
        "Be concise.",
        1,
    )
    .await
    .unwrap();
    grant_skill(&node.store, &caller, ws, "summarize")
        .await
        .unwrap();
    put_doc(
        &node.store,
        &caller,
        ws,
        "spec",
        "Spec",
        "the design",
        ContentType::Text,
        &[],
        1,
    )
    .await
    .unwrap();

    let gw = echo_then_stop();
    // The agent's own caps include echo + the substrate reads; effective grant is caps ∩ caller.
    let agent_caps: Vec<String> = vec![ECHO.into(), SKILL_R.into(), DOC_R.into()];

    let answer = invoke(
        &node,
        &gw,
        &caller,
        &agent_caps,
        ws,
        Invocation {
            job_id: "sess-1",
            goal: "summarize the spec",
            skill: Some("summarize"),
            doc: Some("spec"),
            tools: &echo_tool(),
            ts: 1,
        },
    )
    .await
    .expect("agent runs to completion");

    assert_eq!(answer, "done: echoed agent-says-hi");
    // The session is durable: the job persisted, with the tool step recorded.
    let job = lb_jobs::load(&node.store, ws, "sess-1")
        .await
        .unwrap()
        .expect("job persisted");
    assert_eq!(job.status, lb_jobs::JobStatus::Done);
    // The typed transcript (Part 0) recorded the proposed call WITH its args and an OK result.
    let ok = job
        .events()
        .any(|e| matches!(e, lb_jobs::TranscriptEvent::ToolResult { ok: Some(_), .. }));
    assert!(ok, "the granted echo tool succeeded inside the loop");
    let proposed = job.events().any(|e| {
        matches!(
            e,
            lb_jobs::TranscriptEvent::ToolCallProposed { name, .. } if name == "hello.echo"
        )
    });
    assert!(proposed, "the proposed call + args were recorded");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn invoking_the_agent_is_denied_without_the_invoke_cap() {
    // MANDATORY capability-deny (§2.1): the invoke gate. No mcp:agent.invoke:call → refused before
    // the loop runs, the same opaque Denied as any tool.
    let ws = "agent-invoke-deny";
    let node = Arc::new(Node::boot().await.unwrap());
    let caller = principal("user:ada", ws, &[ECHO]); // can echo, but cannot invoke the agent
    let gw = echo_then_stop();

    let err = invoke(
        &node,
        &gw,
        &caller,
        &[ECHO.into()],
        ws,
        Invocation {
            job_id: "s",
            goal: "x",
            skill: None,
            doc: None,
            tools: &echo_tool(),
            ts: 1,
        },
    )
    .await
    .expect_err("invoke without the cap is denied");
    assert!(matches!(err, lb_host::AgentError::Denied));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_tool_the_caller_cannot_use_is_denied_inside_the_loop_even_if_the_agent_holds_it() {
    // MANDATORY capability-deny (§2.1), the intersection: the AGENT lists the echo cap, but the
    // CALLER does NOT. The derived principal is agent ∩ caller, so the echo call inside the loop is
    // denied — invoking the agent never escalates the caller's access (agent scope no-widening).
    // The denial is fed back to the model (not a crash); the loop still completes.
    let ws = "agent-loop-deny";
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

    // Caller can invoke the agent but CANNOT call echo.
    let caller = principal("user:ada", ws, &[INVOKE]);
    let gw = echo_then_stop();
    // The agent's own caps DO include echo — but the intersection with the caller's removes it.
    let agent_caps: Vec<String> = vec![ECHO.into()];

    let answer = invoke(
        &node,
        &gw,
        &caller,
        &agent_caps,
        ws,
        Invocation {
            job_id: "s",
            goal: "echo something",
            skill: None,
            doc: None,
            tools: &echo_tool(),
            ts: 1,
        },
    )
    .await
    .expect("the loop completes even though the tool was denied");

    assert_eq!(answer, "done: echoed agent-says-hi");
    let job = lb_jobs::load(&node.store, ws, "s").await.unwrap().unwrap();
    let denied = job
        .events()
        .any(|e| matches!(e, lb_jobs::TranscriptEvent::ToolResult { err: Some(_), .. }));
    assert!(
        denied,
        "the echo call was DENIED inside the loop (intersection), fed back as an error"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_agent_cannot_load_a_skill_the_workspace_did_not_grant() {
    // MANDATORY capability-deny (§2.1), the substrate: a skill that exists but was NOT granted to
    // the workspace is invisible to the agent — the S4 grant gate fires under the derived principal.
    let ws = "agent-skill-deny";
    let node = Arc::new(Node::boot().await.unwrap());
    let caller = principal("user:ada", ws, &[INVOKE, SKILL_R, SKILL_W]);

    // Put the skill but do NOT grant it.
    put_skill(&node.store, &caller, ws, "secret", "1", "d", "body", 1)
        .await
        .unwrap();

    let gw = echo_then_stop();
    let err = invoke(
        &node,
        &gw,
        &caller,
        &[SKILL_R.into()],
        ws,
        Invocation {
            job_id: "s",
            goal: "use the secret skill",
            skill: Some("secret"),
            doc: None,
            tools: &[],
            ts: 1,
        },
    )
    .await
    .expect_err("an ungranted skill is denied to the agent");
    assert!(matches!(err, lb_host::AgentError::Denied));
}
