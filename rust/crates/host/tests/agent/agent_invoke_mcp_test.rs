//! `agent.invoke` over the **MCP bridge** (`lb_host::call_tool`) — the transport a non-browser
//! caller (above all a **reminder**, `action_kind: "mcp-tool"`) uses to reach the agent.
//!
//! **THE REGRESSION this file exists for.** `agent.invoke` had a cap, a `tools.catalog` descriptor
//! and a routed edge→hub path, but NO dispatch arm in `call_agent_tool` — so a call over `/mcp/call`
//! fell through to `NotFound` and answered an opaque `403 "no such tool"` to a caller whose token
//! carried `mcp:agent.invoke:call`. It reads as a permissions bug and is a missing `match` arm. The
//! first test below is a direct guard: a GRANTED caller must not get `NotFound`.
//!
//! Every shipped caller reached the agent on some other transport (the browser posts
//! `POST /agent/invoke`; the palette routes to `postAgent`; the channel worker calls the host fn), so
//! nothing exercised this door until a scheduled run needed it. That is precisely why it needs a test.
//!
//! Model provider: the deterministic `MockProvider` (testing §3 — the only external stubbed); store,
//! bus and the whole dispatch chain are real. Multi-thread flavor + a UNIQUE workspace id per test.

use std::sync::Arc;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{call_tool, memory_get, ErasedModel, Node, RuntimeRegistry};
use lb_mcp::ToolError;
use lb_role_ai_gateway::{AiGateway, AiResponse, MockProvider, ToolCall};
use serde_json::{json, Value};

const INVOKE: &str = "mcp:agent.invoke:call";
const CATALOG: &str = "mcp:tools.catalog:call";
const MEM_SET: &str = "mcp:agent.memory.set:call";
const MEM_GET: &str = "mcp:agent.memory.get:call";
const WS_WRITE: &str = "store:agent_memory/workspace:write";

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

/// A model that proposes ONE host-native `agent.memory.set`, then stops — so the assertion is a real
/// side effect (a stored row), not just a returned string.
fn set_memory_then_stop() -> AiGateway<MockProvider> {
    AiGateway::new(MockProvider::new(vec![
        AiResponse::calls(
            "I'll remember that.",
            vec![ToolCall {
                id: "c1".into(),
                name: "agent.memory.set".into(),
                input: r#"{"scope":"workspace","slug":"ahu-2-drifts","description":"AHU-2 drifts","kind":"project","body":"AHU-2 space temp drifts above setpoint.","ts":1}"#.into(),
            }],
            10,
        ),
        AiResponse::stop("noted: AHU-2 drifts above setpoint", 5),
    ]))
}

/// Boot a node with the in-house default wired to `model`.
async fn node_with_model(model: AiGateway<MockProvider>) -> Arc<Node> {
    let node = Arc::new(Node::boot().await.unwrap());
    let erased: Arc<dyn ErasedModel> = Arc::new(model);
    node.install_runtimes(RuntimeRegistry::with_default(erased));
    node
}

async fn invoke_mcp(
    node: &Arc<Node>,
    p: &Principal,
    ws: &str,
    args: Value,
) -> Result<Value, ToolError> {
    let out = call_tool(node, p, ws, "agent.invoke", &args.to_string()).await?;
    Ok(serde_json::from_str(&out).unwrap_or(Value::Null))
}

// ── the headline: the arm exists, and the run really happens ─────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn agent_invoke_is_reachable_over_the_mcp_bridge() {
    let ws = "ws-invoke-mcp-headline";
    let node = node_with_model(set_memory_then_stop()).await;
    let caller = principal(
        "user:test",
        ws,
        &[INVOKE, CATALOG, MEM_SET, MEM_GET, WS_WRITE],
    );

    let out = invoke_mcp(
        &node,
        &caller,
        ws,
        json!({ "job_id": "sess-mcp-1", "goal": "remember that AHU-2 drifts" }),
    )
    .await
    .expect("a granted caller reaches the agent over MCP");

    assert_eq!(out["answer"], "noted: AHU-2 drifts above setpoint");
    assert_eq!(out["jobId"], "sess-mcp-1", "the durable job id round-trips");

    // The side effect proves the LOOP ran through the real wall, not just that dispatch returned.
    assert!(
        memory_get(&node.store, &caller, ws, Some("workspace"), "ahu-2-drifts")
            .await
            .unwrap()
            .is_some(),
        "the loop executed the host-native write it proposed"
    );
}

/// **The exact regression.** Before the dispatch arm this returned `NotFound` ("no such tool") to a
/// caller HOLDING the grant. Asserting "not NotFound" is the guard: any answer is acceptable except
/// the one that means "this verb isn't wired to this transport".
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_granted_caller_never_gets_no_such_tool() {
    let ws = "ws-invoke-mcp-regression";
    let node = node_with_model(set_memory_then_stop()).await;
    let caller = principal(
        "user:test",
        ws,
        &[INVOKE, CATALOG, MEM_SET, MEM_GET, WS_WRITE],
    );

    let result = invoke_mcp(
        &node,
        &caller,
        ws,
        json!({ "job_id": "sess-mcp-2", "goal": "anything at all" }),
    )
    .await;

    assert!(
        !matches!(result, Err(ToolError::NotFound)),
        "REGRESSION: agent.invoke has no MCP dispatch arm — a granted caller got `no such tool`"
    );
}

// ── the mandatory capability-deny category (testing §2.1) ────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn without_the_grant_the_call_is_denied_opaquely() {
    let ws = "ws-invoke-mcp-deny";
    let node = node_with_model(set_memory_then_stop()).await;
    // Everything EXCEPT the invoke cap.
    let capless = principal("user:nogrant", ws, &[CATALOG, MEM_SET, MEM_GET, WS_WRITE]);

    let err = invoke_mcp(
        &node,
        &capless,
        ws,
        json!({ "job_id": "sess-mcp-deny", "goal": "run without the grant" }),
    )
    .await
    .expect_err("a caller without mcp:agent.invoke:call is refused");

    assert!(
        matches!(err, ToolError::Denied),
        "the refusal must be the OPAQUE deny (no capability/existence leak), got {err:?}"
    );

    // And nothing ran.
    assert!(
        memory_get(&node.store, &capless, ws, Some("workspace"), "ahu-2-drifts")
            .await
            .unwrap()
            .is_none(),
        "a denied invoke executes no tool"
    );
}

// ── the arg contract ─────────────────────────────────────────────────────────────────────────────

/// `job_id` is REQUIRED on this transport. The HTTP route may derive a stable id from the goal, but a
/// reminder fires the same payload on every tick — a derived id would be identical each time and the
/// loop, being idempotent on `job_id`, would REPLAY the first answer forever without calling the
/// model. A named error beats a silently frozen agent.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_missing_job_id_is_a_named_bad_input() {
    let ws = "ws-invoke-mcp-jobid";
    let node = node_with_model(set_memory_then_stop()).await;
    let caller = principal(
        "user:test",
        ws,
        &[INVOKE, CATALOG, MEM_SET, MEM_GET, WS_WRITE],
    );

    let err = invoke_mcp(&node, &caller, ws, json!({ "goal": "no job id here" }))
        .await
        .expect_err("job_id is required");

    match err {
        ToolError::BadInput(m) => assert!(
            m.contains("job_id"),
            "the error must NAME the missing arg, got: {m}"
        ),
        other => panic!("expected BadInput naming job_id, got {other:?}"),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_missing_goal_is_a_named_bad_input() {
    let ws = "ws-invoke-mcp-goal";
    let node = node_with_model(set_memory_then_stop()).await;
    let caller = principal(
        "user:test",
        ws,
        &[INVOKE, CATALOG, MEM_SET, MEM_GET, WS_WRITE],
    );

    let err = invoke_mcp(&node, &caller, ws, json!({ "job_id": "sess-mcp-3" }))
        .await
        .expect_err("goal is required");

    match err {
        ToolError::BadInput(m) => assert!(
            m.contains("goal"),
            "the error must NAME the missing arg, got: {m}"
        ),
        other => panic!("expected BadInput naming goal, got {other:?}"),
    }
}

// ── the recursion guard ──────────────────────────────────────────────────────────────────────────

/// The menu handed to the model must NOT contain `agent.invoke`. The dispatcher applies no depth
/// ceiling to host-native verbs, so an agent that could propose `agent.invoke` would spawn a nested
/// run carrying its own full `MAX_STEPS` budget — unbounded fan-out from one scheduled trigger.
///
/// **Honest scope of this guard:** it is the MENU, not the wall. Execution is gated by capabilities,
/// and a caller who holds `mcp:agent.invoke:call` grants the run that cap too (`agent ∩ caller`), so
/// a model that invents the call anyway would still execute it. Filtering the menu removes the
/// *suggestion*, which is what a small local model actually follows; a hard nesting ceiling would be
/// a separate change in the dispatcher. Deliberate nested invokes over a separate MCP call keep
/// working, which is why the cap is not touched.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_model_is_never_offered_agent_invoke_itself() {
    let ws = "ws-invoke-mcp-recursion";
    let node = node_with_model(set_memory_then_stop()).await;
    let caller = principal(
        "user:test",
        ws,
        &[INVOKE, CATALOG, MEM_SET, MEM_GET, WS_WRITE],
    );

    // Precondition: the raw catalog DOES offer `agent.invoke` (it is a real descriptor), so the
    // filter in the invoke tool is doing real work. If the descriptor is ever dropped this fails
    // loudly rather than leaving a vacuous assertion below.
    let menu = lb_host::reachable_tools(&node, &caller, ws).await;
    assert!(
        menu.iter().any(|t| t.name == "agent.invoke"),
        "precondition: the catalog offers agent.invoke, so the filter is doing real work"
    );

    // What the run is handed is that menu MINUS the verb itself — the filter the invoke tool applies.
    let offered: Vec<&str> = menu
        .iter()
        .map(|t| t.name.as_str())
        .filter(|n| *n != "agent.invoke")
        .collect();
    assert!(
        !offered.contains(&"agent.invoke"),
        "the run must never be offered the verb that starts a run"
    );
    assert!(
        offered.contains(&"agent.memory.set"),
        "the filter removes ONLY agent.invoke, not the rest of the menu"
    );
}
