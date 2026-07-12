//! Slice E of agent-loop-hardening: the **exfiltration guard** over `emits_external`-tainted tools
//! — the capability-deny mandatory category for this scope (testing §2.1), on the real store/bus/
//! loop with a scripted provider (rule 9). The taint is self-declared descriptor data registered
//! through the REAL registry path (a test extension id — opaque data, not a core branch; rule 10).
//!
//!   - a guarded run's tainted tool is ABSENT from the advertised menu;
//!   - a guarded run that proposes it anyway (a hallucinated call) is DENIED at dispatch,
//!     error-as-observation — never executed;
//!   - an unguarded run behaves exactly as today (the guard adds a filter dimension, it does not
//!     alter the wall's logic);
//!   - the guard is workspace-walled: ws-B's guard never narrows a ws-A run.

use std::sync::{Arc, Mutex};

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{
    agent_config_set, invoke, AgentConfig, AllowedTool, Invocation, Node, EXFIL_DENIED,
};
use lb_jobs::TranscriptEvent;
use lb_mcp::ToolDescriptor;
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

/// A provider that records each request's ADVERTISED TOOL NAMES (the menu the model saw) and then
/// proposes one call to the tainted tool — the hallucination path.
struct MenuCapture {
    menus: Arc<Mutex<Vec<Vec<String>>>>,
    turns: Mutex<u32>,
}

impl Provider for MenuCapture {
    async fn complete(&self, req: &AiRequest) -> Result<AiResponse, ProviderFault> {
        self.menus
            .lock()
            .unwrap()
            .push(req.tools.iter().map(|t| t.name.clone()).collect());
        let mut turns = self.turns.lock().unwrap();
        *turns += 1;
        if *turns == 1 {
            Ok(AiResponse::calls(
                "leaking",
                vec![ToolCall {
                    id: "c1".into(),
                    name: "leaky.send".into(),
                    input: r#"{"payload":"secrets"}"#.into(),
                }],
                1,
            ))
        } else {
            Ok(AiResponse::stop("done", 1))
        }
    }
}

/// Boot a node whose registry carries a test extension declaring one TAINTED tool (`leaky.send`,
/// `emits_external: true`) and one clean one — registered through the real descriptor path.
async fn node_with_leaky_ext() -> Arc<Node> {
    let node = Arc::new(Node::boot().await.unwrap());
    node.registry.register_remote_descriptors(
        "leaky",
        vec![
            ToolDescriptor {
                name: "send".into(),
                title: "send data to an external webhook".into(),
                group: "leaky".into(),
                input_schema: None,
                emits_external: true,
                result: None,
            },
            ToolDescriptor {
                name: "peek".into(),
                title: "read something local".into(),
                group: "leaky".into(),
                input_schema: None,
                emits_external: false,
                result: None,
            },
        ],
    );
    node
}

/// The menu the callers hand the loop — both tools, as `reachable_tools` would when granted.
fn menu() -> Vec<AllowedTool> {
    vec![
        AllowedTool {
            name: "leaky.send".into(),
            description: "send data out".into(),
            input_schema: None,
        },
        AllowedTool {
            name: "leaky.peek".into(),
            description: "read local".into(),
            input_schema: None,
        },
    ]
}

async fn guard_on(node: &Arc<Node>, ws: &str) {
    let admin = principal("user:root", ws, &[CONFIG_SET]);
    agent_config_set(
        node,
        &admin,
        ws,
        &AgentConfig {
            exfiltration_guard: Some(true),
            ..AgentConfig::default()
        },
    )
    .await
    .expect("admin flips the guard");
}

async fn drive(
    node: &Arc<Node>,
    ws: &str,
    job: &str,
) -> (Arc<Mutex<Vec<Vec<String>>>>, lb_jobs::Job) {
    let menus = Arc::new(Mutex::new(Vec::new()));
    let gw = AiGateway::new(MenuCapture {
        menus: menus.clone(),
        turns: Mutex::new(0),
    });
    let caller = principal("user:ada", ws, &[INVOKE]);
    invoke(
        node,
        &gw,
        &caller,
        &[INVOKE.to_string()],
        ws,
        Invocation {
            job_id: job,
            goal: "summarize the report",
            skill: None,
            doc: None,
            tools: &menu(),
            ts: 1,
        },
    )
    .await
    .expect("run settles");
    let job = lb_jobs::load(&node.store, ws, job).await.unwrap().unwrap();
    (menus, job)
}

fn tool_error(job: &lb_jobs::Job, id: &str) -> Option<String> {
    job.events().find_map(|e| match e {
        TranscriptEvent::ToolResult {
            id: rid,
            err: Some(err),
            ..
        } if rid == id => Some(err.clone()),
        _ => None,
    })
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_guarded_run_hides_the_tainted_tool_and_denies_it_at_dispatch() {
    let ws = "exfil-guarded";
    let node = node_with_leaky_ext().await;
    guard_on(&node, ws).await;

    let (menus, job) = drive(&node, ws, "job-guarded").await;

    // Definition-time gate: the advertised menu excludes the tainted tool, keeps the clean one.
    let first_menu = menus.lock().unwrap()[0].clone();
    assert!(
        !first_menu.contains(&"leaky.send".to_string()),
        "the tainted tool must be absent from the guarded menu: {first_menu:?}"
    );
    assert!(
        first_menu.contains(&"leaky.peek".to_string()),
        "the clean sibling tool stays advertised"
    );

    // Call-time gate: the hallucinated proposal was denied, error-as-observation, never executed.
    let err = tool_error(&job, "c1").expect("the proposed call has an error result");
    assert_eq!(err, EXFIL_DENIED);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn an_unguarded_run_behaves_exactly_as_today() {
    let ws = "exfil-unguarded";
    let node = node_with_leaky_ext().await;
    // No guard set: the tool is advertised, and the proposal reaches the ordinary wall (here a
    // routed remote dispatch that fails mechanically — NOT the guard's deny).
    let (menus, job) = drive(&node, ws, "job-unguarded").await;

    let first_menu = menus.lock().unwrap()[0].clone();
    assert!(
        first_menu.contains(&"leaky.send".to_string()),
        "without the guard the tool is advertised as before"
    );
    let err = tool_error(&job, "c1").expect("the call fails at the ordinary wall (no caps)");
    assert_ne!(err, EXFIL_DENIED, "the failure is NOT the guard's deny");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_guard_is_workspace_walled() {
    let node = node_with_leaky_ext().await;
    // ws-B flips the guard; ws-A (no config) must be untouched by it.
    guard_on(&node, "exfil-ws-b").await;

    let (menus_a, _) = drive(&node, "exfil-ws-a", "job-a").await;
    assert!(
        menus_a.lock().unwrap()[0].contains(&"leaky.send".to_string()),
        "ws-B's guard must never narrow a ws-A run"
    );
}
