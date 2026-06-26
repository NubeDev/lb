//! MANDATORY workspace-isolation (testing §2.2) for the coding workflow, across **store + MCP**: a
//! ws-B caller can never see ws-A's issues / approvals / job / outbox effects, and a ws-B relay
//! delivers no ws-A effect. Plus the MCP-surface gate (the `workflow.*` bridge) deny + isolation.
//!
//! Node-booting (Zenoh peer) → multi-thread flavor + a UNIQUE workspace id per test.

use std::sync::Arc;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{
    call_workflow_tool, ingest_issue, relay_outbox, request_approval, resolve_approval,
    start_coding_job, CodingJob, Node, PrSpec, Target,
};
use lb_inbox::Decision;
use lb_outbox::Effect;
use serde_json::json;

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
    verify(&key, &mint(&key, &claims), 1).expect("token verifies")
}

const INGEST: &str = "mcp:workflow.ingest_issue:call";
const REQ: &str = "mcp:workflow.request_approval:call";
const RESOLVE: &str = "mcp:workflow.resolve_approval:call";
const START: &str = "mcp:workflow.start_job:call";

/// A target that always succeeds — used only to prove a relay sees no cross-ws effects.
struct OkTarget(std::sync::Mutex<Vec<String>>);
impl Target for OkTarget {
    async fn deliver(&self, effect: &Effect) -> Result<(), String> {
        self.0.lock().unwrap().push(effect.idempotency_key.clone());
        Ok(())
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn workspace_b_never_sees_workspace_a_workflow_state() {
    // Drive a full flow in ws-A; assert ws-B sees none of it — across the inbox, the job, and the
    // outbox (store layer), and the relay never delivers a ws-A effect.
    let node = Arc::new(Node::boot().await.unwrap());
    let caps = [INGEST, REQ, RESOLVE, START, "bus:chan/*:pub"];
    let a = principal("user:ada", "wf-iso-a", &caps);

    ingest_issue(&node.store, &a, "wf-iso-a", "i1", "secret issue", 1)
        .await
        .unwrap();
    let pr = PrSpec::new("acme/api", "fix", "main", "doc1", "");
    request_approval(&node.store, &a, "wf-iso-a", "ap1", "doc1", "rev", &pr, 2)
        .await
        .unwrap();
    resolve_approval(&node.store, &a, "wf-iso-a", "ap1", Decision::Approved, 3)
        .await
        .unwrap();
    start_coding_job(
        &node,
        &a,
        "wf-iso-a",
        CodingJob {
            job_id: "j1",
            approval_id: "ap1",
            scope_doc: "doc1",
            channel: "c",
            pr: &pr,
            pr_key: "pr:a",
            ts: 4,
        },
    )
    .await
    .unwrap();

    // ws-A has the issue, the job, and the pending effect.
    assert_eq!(
        lb_inbox::list(&node.store, "wf-iso-a", "triage")
            .await
            .unwrap()
            .len(),
        1
    );
    assert!(lb_jobs::load(&node.store, "wf-iso-a", "j1")
        .await
        .unwrap()
        .is_some());
    assert_eq!(
        lb_outbox::pending(&node.store, "wf-iso-a")
            .await
            .unwrap()
            .len(),
        1
    );

    // ws-B sees NONE of it (the hard wall, §7).
    assert!(lb_inbox::list(&node.store, "wf-iso-b", "triage")
        .await
        .unwrap()
        .is_empty());
    assert!(lb_jobs::load(&node.store, "wf-iso-b", "j1")
        .await
        .unwrap()
        .is_none());
    assert!(lb_inbox::resolution(&node.store, "wf-iso-b", "ap1")
        .await
        .unwrap()
        .is_none());
    assert!(lb_outbox::pending(&node.store, "wf-iso-b")
        .await
        .unwrap()
        .is_empty());

    // A ws-B relay delivers nothing of ws-A's.
    let target = OkTarget(std::sync::Mutex::new(Vec::new()));
    let pass = relay_outbox(&node.store, "wf-iso-b", &target, 1)
        .await
        .unwrap();
    assert_eq!(pass.delivered, 0, "ws-B relay sees no ws-A effects");
    assert!(target.0.lock().unwrap().is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_mcp_bridge_denies_and_isolates() {
    // The `workflow.*` MCP surface: a caller without the grant is denied HERE (gate 1), and the gate
    // is workspace-first — a ws-B-scoped principal cannot drive a ws-A call.
    let node = Arc::new(Node::boot().await.unwrap());
    let ws = "wf-mcp";

    // No grant → denied at the MCP gate.
    let nobody = principal("user:mallory", ws, &[]);
    let denied = call_workflow_tool(
        &node,
        &nobody,
        ws,
        "workflow.ingest_issue",
        &json!({"issue_id":"i","payload":"p","ts":1}),
    )
    .await;
    assert!(matches!(denied, Err(lb_mcp::ToolError::Denied)));

    // Granted in ws, the bridge works and returns the verb's JSON.
    let user = principal("user:ada", ws, &[INGEST]);
    let ok = call_workflow_tool(
        &node,
        &user,
        ws,
        "workflow.ingest_issue",
        &json!({"issue_id":"i","payload":"p","ts":1}),
    )
    .await
    .unwrap();
    assert_eq!(ok["channel"], "triage");

    // A ws-B principal calling into ws-A is denied (workspace-first, gate 1).
    let cross = principal("user:eve", "wf-mcp-other", &[INGEST]);
    let crossed = call_workflow_tool(
        &node,
        &cross,
        ws, // target ws-A, but the principal is scoped to ws-B
        "workflow.ingest_issue",
        &json!({"issue_id":"i","payload":"p","ts":1}),
    )
    .await;
    assert!(
        matches!(crossed, Err(lb_mcp::ToolError::Denied)),
        "a ws-B principal cannot drive a ws-A workflow call"
    );
}
