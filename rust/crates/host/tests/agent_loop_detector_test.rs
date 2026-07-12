//! Slice B of agent-loop-hardening, at the LOOP level (real store/bus/loop; scripted provider per
//! rule 9): a model that repeats the identical failing call climbs the full ladder — **warn** (a
//! corrective message it can see) → **block** (the call is refused, error-as-observation) →
//! **break** (an honest `Failed` terminal, never a silent stop or an infinite spiral). Also: the
//! graceful **ceiling exit** makes one final tools-free summary call, and `agent.config.loop_window
//! = 0` disables the detector for the workspace (and only that workspace).

use std::sync::{Arc, Mutex};

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{
    agent_config_set, invoke, AgentConfig, Invocation, Node, LOOP_BLOCKED, LOOP_WARNING, MAX_STEPS,
};
use lb_jobs::{JobStatus, TranscriptEvent};
use lb_role_ai_gateway::{AiGateway, AiRequest, AiResponse, Provider, ProviderFault, ToolCall};

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

/// A scripted provider that also captures each request's messages (to assert the warn injection)
/// and counts calls. Rule 9: stands in only for the provider HTTP.
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

/// The identical failing call, every turn — the spiral the detector must end.
fn same_call(i: usize) -> AiResponse {
    AiResponse::calls(
        "",
        vec![ToolCall {
            id: format!("c{i}"),
            name: "spin.tool".into(),
            input: r#"{"x":1}"#.into(),
        }],
        1,
    )
}

async fn drive(
    node: &Arc<Node>,
    ws: &str,
    job: &str,
    gw: &AiGateway<CapturingScript>,
) -> (String, JobStatus) {
    let caller = principal("user:ada", ws, &[INVOKE]);
    let answer = invoke(
        node,
        gw,
        &caller,
        &[INVOKE.to_string()],
        ws,
        Invocation {
            job_id: job,
            goal: "do the task",
            skill: None,
            doc: None,
            tools: &[],
            ts: 1,
        },
    )
    .await
    .expect("run settles");
    let status = lb_jobs::load(&node.store, ws, job)
        .await
        .unwrap()
        .unwrap()
        .status;
    (answer, status)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn an_identical_call_spiral_climbs_warn_block_break() {
    let ws = "detector-ladder";
    let node = Arc::new(Node::boot().await.unwrap());
    let seen = Arc::new(Mutex::new(Vec::new()));
    // More turns scripted than the ladder allows — break must end the run before the script does.
    let gw = AiGateway::new(CapturingScript {
        script: Mutex::new((0..10).map(|i| Ok(same_call(i))).collect()),
        seen: seen.clone(),
    });

    let (answer, status) = drive(&node, ws, "job-spiral", &gw).await;

    // Break: an honest Failed terminal naming the detector — not a silent stop, not 10 turns.
    assert_eq!(status, JobStatus::Failed);
    assert!(
        answer.contains("loop detector"),
        "the terminal answer names the detector, got: {answer}"
    );

    // The ladder's shape: turn 3 fired Warn → turn 4's request carries the corrective message.
    let seen = seen.lock().unwrap();
    assert_eq!(
        seen.len(),
        5,
        "warn at 3, block at 4, break at 5 — five provider calls"
    );
    assert!(
        seen[3].iter().any(|(_, c)| c == LOOP_WARNING),
        "the model was warned before being blocked"
    );

    // Block: turn 5's proposal was refused pre-dispatch, error-as-observation in the transcript.
    let job = lb_jobs::load(&node.store, ws, "job-spiral")
        .await
        .unwrap()
        .unwrap();
    assert!(
        job.events().any(|e| matches!(
            e,
            TranscriptEvent::ToolResult { err: Some(err), .. } if err == LOOP_BLOCKED
        )),
        "the blocked call's refusal is a recorded observation"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn loop_window_zero_disables_the_detector_for_that_workspace_only() {
    let ws_off = "detector-off";
    let node = Arc::new(Node::boot().await.unwrap());

    // ws-off disables the detector through the real admin verb.
    let admin = principal("user:root", ws_off, &[CONFIG_SET]);
    agent_config_set(
        &node,
        &admin,
        ws_off,
        &AgentConfig {
            loop_window: Some(0),
            ..AgentConfig::default()
        },
    )
    .await
    .expect("admin disables the detector");

    // The same spiral now runs all the way to the step ceiling (MAX_STEPS turns + the summary
    // call) — the detector never intervenes.
    let seen = Arc::new(Mutex::new(Vec::new()));
    let gw = AiGateway::new(CapturingScript {
        script: Mutex::new(
            (0..MAX_STEPS as usize + 1)
                .map(|i| Ok(same_call(i)))
                .collect(),
        ),
        seen: seen.clone(),
    });
    let (answer, status) = drive(&node, ws_off, "job-undetected", &gw).await;
    assert_eq!(
        status,
        JobStatus::Done,
        "no detector break in the opted-out ws"
    );
    assert!(
        answer.contains("turn ceiling"),
        "the run ran to the honest ceiling instead: {answer}"
    );
    assert!(
        seen.lock().unwrap().len() as u32 > MAX_STEPS,
        "all ceiling turns + the summary call ran"
    );

    // A sibling workspace with NO config still gets the default detector (isolation: ws-off's
    // opt-out never leaks).
    let seen2 = Arc::new(Mutex::new(Vec::new()));
    let gw2 = AiGateway::new(CapturingScript {
        script: Mutex::new((0..10).map(|i| Ok(same_call(i))).collect()),
        seen: seen2.clone(),
    });
    let (_, status2) = drive(&node, "detector-default-ws", "job-detected", &gw2).await;
    assert_eq!(
        status2,
        JobStatus::Failed,
        "the default ws still breaks the spiral"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_ceiling_exit_makes_one_tools_free_summary_call() {
    let ws = "ceiling-summary";
    let node = Arc::new(Node::boot().await.unwrap());
    let seen = Arc::new(Mutex::new(Vec::new()));

    // MAX_STEPS turns of genuine (distinct) work, then the summary completion.
    let mut script: Vec<Result<AiResponse, ProviderFault>> = (0..MAX_STEPS as usize)
        .map(|i| {
            Ok(AiResponse::calls(
                "",
                vec![ToolCall {
                    id: format!("c{i}"),
                    name: format!("distinct{i}.tool"),
                    input: "{}".into(),
                }],
                1,
            ))
        })
        .collect();
    script.push(Ok(AiResponse::stop(
        "I mapped the schema and drafted 3 of 5 queries; the joins remain.",
        3,
    )));
    let gw = AiGateway::new(CapturingScript {
        script: Mutex::new(script),
        seen: seen.clone(),
    });

    let (answer, status) = drive(&node, ws, "job-ceiling", &gw).await;
    assert_eq!(status, JobStatus::Done);
    assert!(
        answer.contains("I mapped the schema") && answer.contains("turn ceiling"),
        "the wrap-up summary leads and the honest note follows, got: {answer}"
    );

    // The summary request was TOOLS-FREE: exactly one more provider call than the ceiling, and the
    // summary turn is persisted as a normal assistant turn (watchers/transcript carry it).
    assert_eq!(seen.lock().unwrap().len() as u32, MAX_STEPS + 1);
    let job = lb_jobs::load(&node.store, ws, "job-ceiling")
        .await
        .unwrap()
        .unwrap();
    assert!(job.events().any(|e| matches!(
        e,
        TranscriptEvent::AssistantTurn { content } if content.contains("I mapped the schema")
    )));
}
