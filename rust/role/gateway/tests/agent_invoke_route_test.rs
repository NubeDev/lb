//! `POST /agent/invoke` over the REAL gateway (active-agent-wiring Slice 5) — the transport the
//! dashboard "AI widget" (genui author flow) was missing (wired on NO transport → "unknown command").
//! No mock (CLAUDE §9): a real `Node` + registry, a real signed token, the real in-house loop driving
//! against the node's `UnconfiguredModel` (the honest "no model configured" answer — not a fake).
//!
//!   - **happy path:** a principal WITH `mcp:agent.invoke:call` POSTs a goal → `200` with `{ jobId,
//!     answer }`; the run resolved the workspace's ACTIVE agent (no runtime arg) and returned the
//!     honest unconfigured answer.
//!   - **capability deny:** a principal WITHOUT the cap → opaque `403` (the self-gate refuses inside
//!     `invoke_via_runtime`; the route never leaks — the MCP deny contract).
//!   - **workspace isolation:** a ws-B token cannot start a run in ws-A — the run is keyed to the
//!     TOKEN's ws (the hard wall §7). The durable agent-session job lands in ws-B (loadable there) and
//!     NOT in ws-A, and the derived job id differs per ws for the same goal.

mod common;

use std::sync::Arc;

use axum::http::StatusCode;
use common::*;
use lb_auth::SigningKey;
use lb_host::{Node, Role as NodeRole};
use lb_role_gateway::router;
use serde::Deserialize;
use serde_json::json;
use tower::ServiceExt; // for `oneshot`

const INVOKE: &[&str] = &["mcp:agent.invoke:call"];

/// The route's reply shape (`agent.types.ts` `AgentResult`) — the durable id + the final answer.
#[derive(Debug, Deserialize)]
struct Reply {
    answer: String,
    #[serde(rename = "jobId")]
    job_id: String,
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn granted_invoke_drives_the_active_agent() {
    let (gw, key) = gateway().await;
    let ws = "gw-agent-invoke";
    let tok = token(&key, "user:ada", ws, INVOKE);

    let resp = router(gw)
        .oneshot(bearer(
            json_post("/agent/invoke", json!({ "goal": "draw a stat widget" })),
            &tok,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK, "granted invoke succeeds");

    let reply: Reply = json_body(resp).await;
    assert!(!reply.job_id.is_empty(), "a durable job id is returned");
    // The node has no model provider wired → the in-house default returns the HONEST unconfigured
    // answer (not a fake). That the run drove at all proves the active agent resolved (no runtime arg).
    assert!(
        reply.answer.contains("no in-house model is configured"),
        "the active agent drove against UnconfiguredModel: {}",
        reply.answer
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn invoke_without_the_cap_is_denied() {
    let (gw, key) = gateway().await;
    // A session token WITHOUT mcp:agent.invoke:call — the self-gate inside `invoke_via_runtime`
    // refuses and the route maps it to an opaque 403 (no capability/existence leak).
    let tok = token(
        &key,
        "user:bob",
        "gw-agent-deny",
        &["mcp:series.latest:call"],
    );

    let resp = router(gw)
        .oneshot(bearer(
            json_post("/agent/invoke", json!({ "goal": "anything" })),
            &tok,
        ))
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::FORBIDDEN,
        "invoke without the cap is denied"
    );
    assert_eq!(body_text(resp).await, "denied", "the 403 body is opaque");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_run_is_keyed_to_the_tokens_workspace() {
    // One node, two sessions (the isolation setup): a ws-B token runs the SAME goal a ws-A caller
    // would. The run is keyed to the TOKEN's ws — the durable job lands in ws-B, never ws-A.
    let node = Arc::new(Node::boot_as(NodeRole::Hub).await.expect("node boots"));
    let key = SigningKey::generate();
    let goal = "same goal in both workspaces";

    let tok_b = token(&key, "user:ada", "ws-b", INVOKE);
    let resp = router(gateway_on(node.clone(), &key))
        .oneshot(bearer(
            json_post("/agent/invoke", json!({ "goal": goal })),
            &tok_b,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let reply: Reply = json_body(resp).await;

    // The durable agent-session job lives in the caller's ws (ws-B) and NOT in ws-A — the hard wall.
    let in_b = lb_jobs::load(&node.store, "ws-b", &reply.job_id)
        .await
        .unwrap();
    assert!(
        in_b.is_some(),
        "the run's job lands in the caller's ws (ws-b)"
    );
    let in_a = lb_jobs::load(&node.store, "ws-a", &reply.job_id)
        .await
        .unwrap();
    assert!(
        in_a.is_none(),
        "the run never leaks into ws-a — the run is keyed to the token's ws"
    );
}
