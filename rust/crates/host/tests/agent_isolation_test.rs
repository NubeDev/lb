//! MANDATORY workspace-isolation (testing §2.2) for the agent — across **store + MCP**: an agent
//! invoked in workspace B can never see workspace A's docs / skills / jobs. The hard wall holds at
//! every surface the agent touches (§3.6, §7).
//!
//! Mock provider (the only external stubbed). Multi-thread flavor + unique workspace ids per test.

use std::sync::Arc;

use lb_assets::ContentType;
use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{invoke, put_doc, AllowedTool, Invocation, Node};
use lb_role_ai_gateway::{AiGateway, AiResponse, MockProvider};

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

const INVOKE: &str = "mcp:agent.invoke:call";
const DOC_R: &str = "store:doc/*:read";
const DOC_W: &str = "store:doc/*:write";

/// A model that just stops (no tool calls) — we only exercise the substrate/isolation path here.
fn just_stop() -> AiGateway<MockProvider> {
    AiGateway::new(MockProvider::new(vec![AiResponse::stop("ok", 1)]))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn an_agent_in_ws_b_cannot_read_ws_a_substrate_doc() {
    // Store-surface isolation: a doc lives in ws-A; an agent invoked in ws-B asks for it as
    // substrate → gate 1 (workspace) refuses. The namespace wall makes ws-A's doc invisible to ws-B.
    let node = Arc::new(Node::boot().await.unwrap());

    // ws-A: owner writes a secret doc.
    let ada_a = principal("user:ada", "agent-iso-a", &[INVOKE, DOC_R, DOC_W]);
    put_doc(
        &node.store,
        &ada_a,
        "agent-iso-a",
        "secret",
        "Secret",
        "ws-a only",
        ContentType::Text,
        &[],
        1,
    )
    .await
    .unwrap();

    // ws-B: a principal (even same sub) invokes the agent in ws-B, asking for ws-A's doc id.
    let ada_b = principal("user:ada", "agent-iso-b", &[INVOKE, DOC_R]);
    let err = invoke(
        &node,
        &just_stop(),
        &ada_b,
        &[DOC_R.into()],
        "agent-iso-b",
        Invocation {
            job_id: "s",
            goal: "read secret",
            skill: None,
            doc: Some("secret"), // a doc that only exists in ws-A
            tools: &[],
            ts: 1,
        },
    )
    .await
    .expect_err("ws-B agent cannot read ws-A's doc");
    assert!(matches!(
        err,
        lb_host::AgentError::Denied | lb_host::AgentError::NotFound
    ));

    // And ws-A's doc is intact + unreadable from ws-B at the store verb directly (belt + suspenders).
    let from_b = lb_host::get_doc(&node.store, &ada_b, "agent-iso-b", "secret").await;
    assert!(
        from_b.is_err(),
        "ws-B must not read ws-A's doc via the store verb either"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn an_agents_job_is_invisible_across_the_workspace_wall() {
    // The session record itself is ws-scoped: a job created by an agent in ws-A is invisible to
    // ws-B (a ws-B resume can never read a ws-A session). The hard wall on the durable session.
    let node = Arc::new(Node::boot().await.unwrap());
    let ada_a = principal("user:ada", "agent-jobiso-a", &[INVOKE]);

    invoke(
        &node,
        &just_stop(),
        &ada_a,
        &[],
        "agent-jobiso-a",
        Invocation {
            job_id: "sess",
            goal: "do a thing",
            skill: None,
            doc: None,
            tools: &[] as &[AllowedTool],
            ts: 1,
        },
    )
    .await
    .unwrap();

    // ws-A has the job; ws-B does not — even with the same job id.
    assert!(lb_jobs::load(&node.store, "agent-jobiso-a", "sess")
        .await
        .unwrap()
        .is_some());
    assert!(
        lb_jobs::load(&node.store, "agent-jobiso-b", "sess")
            .await
            .unwrap()
            .is_none(),
        "ws-B must not see ws-A's agent session"
    );
}
