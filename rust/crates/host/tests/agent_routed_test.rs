//! S5 EXIT-GATE (the routing part): an **edge user invokes the central agent over the routed MCP
//! namespace** — `caps::check` on the EDGE (workspace-first), the invocation routes over a Zenoh
//! queryable to the HUB, the hub runs the agent loop (calling the gateway + a granted tool hosted
//! ON the hub), and replies. Plus the across-nodes mandatory categories:
//!   - capability-deny: an ungranted invoke is refused on the edge, never routes;
//!   - workspace-isolation: an edge principal in ws-B can never route into ws-A.
//!
//! Two in-process nodes (separate Zenoh sessions auto-discovering into one network), the mock
//! provider on the hub, and `hello` hosted + served on the hub. Multi-thread flavor + unique ids.

use std::sync::Arc;
use std::time::Duration;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{
    invoke_remote, load_extension, serve_agent, serve_ext, AgentError, AllowedTool, Node,
    Role as NodeRole,
};
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
    }]
}

/// Boot a hub that hosts + serves `hello` AND serves the central agent (mock model), and an edge.
/// The agent's own caps include echo, so the routed loop can call the hub-hosted tool. The agent
/// server + tool server are leaked (kept alive for the test).
async fn hub_and_edge(agent_caps: Vec<String>) -> (Node, Arc<Node>) {
    let hub = Arc::new(Node::boot_as(NodeRole::Hub).await.expect("hub boots"));
    load_extension(&hub, MANIFEST, &hello_wasm(), &[])
        .await
        .expect("hub loads hello");
    std::mem::forget(
        serve_ext(&hub.bus, hub.registry.clone(), "hello")
            .await
            .unwrap(),
    );

    let model = Arc::new(AiGateway::new(MockProvider::new(vec![
        AiResponse::calls(
            "echoing",
            vec![ToolCall {
                id: "c1".into(),
                name: "hello.echo".into(),
                input: r#"{"msg":"routed-hi"}"#.into(),
            }],
            10,
        ),
        AiResponse::stop("routed: done", 5),
    ])));
    std::mem::forget(serve_agent(hub.clone(), model, agent_caps).await.unwrap());

    let edge = Node::boot_as(NodeRole::Edge).await.expect("edge boots");
    // The hub is returned as an Arc the test holds, keeping its Zenoh peer (and the served agent +
    // tool) alive for the call's duration.
    (edge, hub)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn an_edge_invokes_the_hub_agent_over_the_routed_namespace() {
    let ws = "agent-routed";
    let (edge, _hub) = hub_and_edge(vec![ECHO.into()]).await;
    let caller = principal(ws, &[INVOKE, ECHO]);

    let answer = tokio::time::timeout(
        Duration::from_secs(10),
        invoke_remote(
            &edge.bus,
            &caller,
            ws,
            "routed-sess",
            "echo something",
            None,
            None,
            &echo_tool(),
            1,
        ),
    )
    .await
    .expect("the routed invocation returns in time")
    .expect("the hub agent answered the edge");

    assert_eq!(
        answer, "routed: done",
        "the hub's agent ran the loop and replied"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_routed_invoke_is_denied_without_the_cap_and_never_leaves_the_edge() {
    // MANDATORY capability-deny across nodes: authorize runs on the EDGE first. Without
    // mcp:agent.invoke:call the invocation is refused there — it never routes to the hub.
    let ws = "agent-routed-deny";
    let (edge, _hub) = hub_and_edge(vec![ECHO.into()]).await;
    let caller = principal(ws, &[ECHO]); // can echo, but cannot invoke the agent

    let err = invoke_remote(
        &edge.bus,
        &caller,
        ws,
        "s",
        "x",
        None,
        None,
        &echo_tool(),
        1,
    )
    .await
    .expect_err("ungranted routed invoke is refused on the edge");
    assert!(
        matches!(err, AgentError::Denied),
        "denied on the edge, before routing"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn an_edge_principal_in_ws_b_cannot_route_into_ws_a() {
    // MANDATORY workspace-isolation across nodes via the agent routing seam. The principal is scoped
    // to ws-B but TRIES to invoke targeting ws-A → gate 1 fires on the edge, before any bus hop.
    let ws_a = "agent-routed-iso-a";
    let ws_b = "agent-routed-iso-b";
    let (edge, _hub) = hub_and_edge(vec![ECHO.into()]).await;
    let intruder = principal(ws_b, &[INVOKE, ECHO]);

    let err = invoke_remote(
        &edge.bus,
        &intruder,
        ws_a, // targeting workspace A while scoped to B
        "s",
        "x",
        None,
        None,
        &echo_tool(),
        1,
    )
    .await
    .expect_err("cross-workspace routed invoke is refused");
    assert!(
        matches!(err, AgentError::Denied),
        "isolation gate fires on the edge"
    );
}
