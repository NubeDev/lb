//! Regression (agent run loop): the run's ANSWER must survive an empty final turn, and a run that
//! hits the MAX_STEPS ceiling must say so instead of settling silently with whatever a mid-work
//! turn happened to carry (often nothing) — the dock rendered an EMPTY answer for a widget-building
//! ask that ran out of turns. See
//! debugging/agent/run-answer-empty-last-turn-content-overwrites.md.

use std::sync::Arc;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{run_session, AllowedTool, Node, MAX_STEPS};
use lb_role_ai_gateway::{AiGateway, AiResponse, MockProvider, ToolCall};

const INVOKE: &str = "mcp:agent.invoke:call";

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

fn a_call(id: &str) -> ToolCall {
    ToolCall {
        id: id.into(),
        // A tool that will simply fail dispatch (no extension loaded) — the loop feeds the error
        // back to the model; this test only cares about the ANSWER text, not the tool outcome.
        // The name varies per call id so the ceiling run is genuine WORK, not the identical-call
        // spiral the loop detector (hardening slice B) now deliberately ends before the ceiling.
        name: format!("nosuch-{id}.tool"),
        input: "{}".into(),
    }
}

fn no_tools() -> Vec<AllowedTool> {
    vec![AllowedTool {
        name: "nosuch.tool".into(),
        description: "a tool".into(),
        input_schema: None,
    }]
}

/// A tool-call turn carried real text; the model's final `done` turn carried NONE (the GLM
/// think-strip shape). The answer must be the last non-empty content, not "".
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn an_empty_final_turn_does_not_wipe_the_answer() {
    let ws = "agent-empty-final";
    let node = Arc::new(Node::boot().await.unwrap());
    let caller = principal("user:ada", ws, &[INVOKE]);
    let gw = AiGateway::new(MockProvider::new(vec![
        AiResponse::calls("I created the widget for you.", vec![a_call("c1")], 5),
        // Two empty stops: the loop nudges once after the first, then must settle on the fallback.
        AiResponse::stop("", 0),
        AiResponse::stop("", 0),
    ]));

    let answer = run_session(
        &node,
        &gw,
        &caller,
        &[],
        ws,
        "job-empty-final",
        "add a widget",
        &no_tools(),
        None,
        None,
        1,
    )
    .await
    .unwrap();
    assert_eq!(answer, "I created the widget for you.");
}

/// A run that is STILL proposing calls when MAX_STEPS runs out must answer with the honest ceiling
/// note (appended to any real text it produced), never settle silently mid-work.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_ceiling_exit_answers_with_the_honest_note() {
    let ws = "agent-ceiling";
    let node = Arc::new(Node::boot().await.unwrap());
    let caller = principal("user:ada", ws, &[INVOKE]);
    // Every turn proposes another call and carries no text — the run works right into the ceiling.
    let script: Vec<AiResponse> = (0..MAX_STEPS)
        .map(|i| AiResponse::calls("", vec![a_call(&format!("c{i}"))], 1))
        .collect();
    let gw = AiGateway::new(MockProvider::new(script));

    let answer = run_session(
        &node,
        &gw,
        &caller,
        &[],
        ws,
        "job-ceiling",
        "add a widget",
        &no_tools(),
        None,
        None,
        1,
    )
    .await
    .unwrap();
    assert!(
        answer.contains("turn ceiling"),
        "ceiling exit must be honest, got: {answer:?}"
    );
}

/// A tool-heavy run whose model ends with a BARE stop (empty content, no calls) must be nudged
/// once for its final answer instead of settling on the turn-1 preamble — the live "5 timeseries
/// queries" run answered with only "I'll help you…" because the loop took the empty `done` at face
/// value. See debugging/agent/run-finished-empty-after-tool-work-answers-with-preamble.md.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_bare_stop_after_tool_work_is_nudged_for_the_real_answer() {
    let ws = "agent-bare-stop";
    let node = Arc::new(Node::boot().await.unwrap());
    let caller = principal("user:ada", ws, &[INVOKE]);
    let gw = AiGateway::new(MockProvider::new(vec![
        AiResponse::calls(
            "I'll help you with that. Let me explore first.",
            vec![a_call("c1")],
            5,
        ),
        AiResponse::stop("", 0),
        AiResponse::stop("Here are your 5 queries: …", 3),
    ]));

    let answer = run_session(
        &node,
        &gw,
        &caller,
        &[],
        ws,
        "job-bare-stop",
        "give me 5 queries",
        &no_tools(),
        None,
        None,
        1,
    )
    .await
    .unwrap();
    assert_eq!(answer, "Here are your 5 queries: …");
}
