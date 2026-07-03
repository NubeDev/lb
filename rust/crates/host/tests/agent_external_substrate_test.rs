//! Regression (active-agent-wiring / genui): an **explicitly-requested** substrate skill is baked
//! into the goal for a NON-DEFAULT (external) runtime, not just the in-house default.
//!
//! The bug: the AI-widget builder invokes the workspace's active agent with `skill:core.genui-widget`
//! so the run authors OpenUI-Lang against the catalog. When the active agent is EXTERNAL, the dispatch
//! seam used to bake the substrate skill body ONLY for the default runtime — the external path got a
//! one-line catalog description and relied on the agent to `load_skill` itself. A general coding agent
//! (Open Interpreter) ignored that and answered with prose → the parsed IR had no components → the host
//! rejected the cell ("IR has no components"), i.e. the "AI widget" was dead for any external pick.
//!
//! rule 9: real Node + real store + real skill grant/read; the ONLY test double is a capturing
//! `AgentRuntime` standing in for the external ACP driver (a genuine external subprocess — the role
//! crate's opt-in smoke — is not run here). It records the goal it is handed so we can assert the
//! skill body arrived. It does NOT re-implement any node behavior; it just captures its input.

use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{
    grant_skill, invoke_via_runtime, put_skill, AgentRuntime, AllowedTool, ErasedModel, Node,
    RunContext, RuntimeRegistry, Substrate,
};
use lb_role_ai_gateway::{AiGateway, AiResponse, MockProvider};

const INVOKE: &str = "mcp:agent.invoke:call";
// The default caps a dev session carries include skill read/write; the substrate read is gated under
// `caller ∩ agent`, so the caller must actually hold the skill-read cap to load the body.
const SKILL_R: &str = "store:skill/*:read";
const SKILL_W: &str = "store:skill/*:write";

fn principal(sub: &str, ws: &str, caps: &[&str]) -> Principal {
    let key = SigningKey::generate();
    let claims = Claims {
        sub: sub.into(),
        ws: ws.into(),
        role: Role::Member,
        caps: caps.iter().map(|s| s.to_string()).collect(),
        iat: 0,
        exp: u64::MAX,
    };
    let token = mint(&key, &claims);
    verify(&key, &token, 1).expect("token verifies")
}

/// A stand-in for an external ACP runtime: it captures the `goal` it is handed (the ONLY channel an
/// external agent gets its instructions on) and returns a fixed answer. Registered under a non-default
/// id so `invoke_via_runtime` takes the external branch.
struct CapturingRuntime {
    id: String,
    captured_goal: Arc<Mutex<Option<String>>>,
}

impl AgentRuntime for CapturingRuntime {
    fn id(&self) -> &str {
        &self.id
    }
    fn run<'a>(
        &'a self,
        _node: &'a Arc<Node>,
        ctx: RunContext<'a>,
    ) -> Pin<Box<dyn Future<Output = Result<String, lb_host::AgentError>> + Send + 'a>> {
        *self.captured_goal.lock().unwrap() = Some(ctx.goal.to_string());
        Box::pin(async { Ok("external ran".to_string()) })
    }
}

fn answer_model() -> Arc<dyn ErasedModel> {
    Arc::new(AiGateway::new(MockProvider::new(vec![AiResponse::stop("x", 1)])))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn an_explicit_skill_body_reaches_an_external_runtime_goal() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let ws = "ext-substrate";
    let caller = principal("user:ada", ws, &[INVOKE, SKILL_R, SKILL_W]);

    // Seed + grant the skill whose BODY must reach the external agent (the genui contract, in miniature).
    let body = "Emit OpenUI-Lang: root = Stat(...). Never prose.";
    put_skill(&node.store, &caller, ws, "genui", "1", "author a widget", body, 1)
        .await
        .expect("put skill");
    grant_skill(&node.store, &caller, ws, "genui")
        .await
        .expect("grant skill");

    // A registry with a NON-default external runtime that captures its goal.
    let captured = Arc::new(Mutex::new(None));
    let mut registry = RuntimeRegistry::with_default(answer_model());
    registry.register(Arc::new(CapturingRuntime {
        id: "open-interpreter-default".into(),
        captured_goal: captured.clone(),
    }));

    // Invoke the EXTERNAL runtime with an explicit substrate skill (exactly the AI-widget builder path,
    // but pinned to the external runtime).
    let answer = invoke_via_runtime(
        &node,
        &registry,
        Some("open-interpreter-default"),
        &caller,
        &caller.caps().to_vec(),
        ws,
        "job-ext",
        "a stat tile of open alerts",
        Substrate { skill: Some("genui"), doc: None },
        &[] as &[AllowedTool],
        1,
    )
    .await
    .expect("external runtime runs");
    assert_eq!(answer, "external ran");

    let goal = captured.lock().unwrap().clone().expect("runtime captured a goal");
    assert!(
        goal.contains("a stat tile of open alerts"),
        "the caller's goal is present:\n{goal}"
    );
    assert!(
        goal.contains(body),
        "the EXPLICIT skill body was baked into the external runtime's goal (regression: it was \
         dropped for non-default runtimes, so the AI widget got prose, not OpenUI-Lang):\n{goal}"
    );
    assert!(
        goal.contains("[skill genui]"),
        "the skill body is framed the same way the in-house loop frames it:\n{goal}"
    );
}
