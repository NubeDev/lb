//! The **channel agent worker** (channels-agent scope) — the sibling of [`query_worker`](super::query_worker).
//! Runs INLINE inside [`channel::post`](super::post) when the posted item is a `kind:"agent"` request:
//! it drives an agent run toward the goal through the host-owned runtime seam (external-agent #1,
//! `invoke_via_runtime` → the node's [`RuntimeRegistry`](crate::RuntimeRegistry)), then posts the
//! durable `kind:"agent_result"` (or `kind:"agent_error"`) item back into the same channel under a
//! system identity. The whole exchange lives in durable channel history and streams over SSE; while a
//! run executes it publishes `RunEvent`s on the run's bus subject, so a watcher sees it live
//! (agent-run Part 3), identically for an in-house or an external agent.
//!
//! **Runtime is selected, not branched.** `AgentPayload.runtime` flows straight into the registry
//! (absent → the in-house `default`; a profile id → an external `AcpRuntime`, e.g.
//! `open-interpreter-default` → Open Interpreter over Z.AI). Whether an external agent is even
//! *present* is the node's cargo feature + config (it installs the external entries via
//! `Node::install_runtimes`); this worker is identical either way.
//!
//! Security invariants (do not weaken), mirroring the query worker:
//!   - TWO grants, in order: channel `bus:chan/{cid}:pub` (the member already passed it to post the
//!     `agent` item), then `mcp:agent.invoke:call` when the worker drives the run UNDER THE POSTER'S
//!     principal (`invoke_via_runtime`'s gate 1). A member without the invoke grant is denied here and
//!     gets an OPAQUE `agent_error` ("agent not permitted"). Inside the run every tool the agent tries
//!     re-runs `caps::check` under the derived `agent ∩ caller` principal (no widening).
//!   - The deny path is **opaque**: a missing invoke grant AND a named-but-unknown/ungranted runtime
//!     collapse to the SAME "agent not permitted" — so the poster learns nothing about which runtimes
//!     exist. A genuine run fault is an honest, distinct message.
//!   - **Re-entrancy guard:** only `kind:"agent"` triggers work. The worker's own
//!     `agent_result`/`agent_error` items parse to other variants and are ignored — an infinite loop
//!     is one absent guard away (tested).
//!   - The worker holds NO durable state; a failure never fails the originating post (which already
//!     durably landed) — the worst case is a follow-up `agent_error` (or nothing).
//!
//! **v1 is inline (like the query worker); non-blocking background execution is the run-lifecycle #5
//! follow-up.** An `exec --json` run can take many seconds, so awaiting it here blocks the poster's
//! `post` for the run's duration. That is the faithful reuse of the proven worker pattern and works
//! end-to-end today; spawning it as a durable, supervised, resumable job (so `post` returns at once
//! and only the answer streams later) is `run-lifecycle-scope.md`, flagged in the channels-agent scope.

use lb_auth::Principal;
use lb_inbox::Item;

use super::payload::{
    agent_error_body, agent_result_body, parse_payload, AgentPayload, ItemPayload,
};
use crate::agent::{invoke_via_runtime, AgentError, Substrate};
use crate::boot::Node;

/// The system identity the worker posts results/errors under (host answering its own request — no
/// `pub` re-check, like the query worker's `system:query-worker`).
pub(crate) const WORKER_AUTHOR: &str = "system:agent-worker";

/// The opaque deny message. A missing invoke grant AND a named-but-unknown runtime collapse to this,
/// so the poster cannot tell "you lack the grant" from "that runtime doesn't exist".
const OPAQUE_DENY: &str = "agent not permitted";

/// Hard byte cap on the persisted answer (mirrors the query worker's 256 KB posture) so a verbose run
/// can't bloat channel history / the bus frame. The full step-by-step stays in the run stream/job.
const AGENT_MAX_BYTES: usize = 256 * 1024;

/// If `item` is a `kind:"agent"` request, drive the run and post the result/error item. Otherwise
/// (chat, a `query*`/`agent_result`/`agent_error` payload, …) do nothing — the re-entrancy guard.
/// Never errors: a worker failure becomes an `agent_error` item (or is swallowed if even that cannot
/// land); the originating post has already succeeded.
pub async fn run_if_agent(node: &Node, poster: &Principal, ws: &str, cid: &str, item: &Item) {
    // RE-ENTRANCY GUARD: only a `kind:"agent"` item triggers work. A result/error item (or plain chat,
    // or a query payload) parses to another variant / None and returns here — never feeds on its output.
    let Some(ItemPayload::Agent(AgentPayload { goal, runtime, job })) = parse_payload(&item.body)
    else {
        return;
    };

    let ts = item.ts.saturating_add(1);
    // The runtime label the result echoes: the requested id, or the default's id when absent.
    let runtime_label = runtime.clone().unwrap_or_else(|| "default".to_string());

    match drive_run(node, poster, ws, &goal, runtime.as_deref(), &job, ts).await {
        Ok(answer) => {
            let (answer, truncated) = cap_answer(answer);
            let body = agent_result_body(&goal, &runtime_label, &job, &answer, truncated);
            let _ = post_worker_item(node, ws, cid, &job, body, ts).await;
        }
        Err(msg) => {
            let body = agent_error_body(&goal, &msg);
            let _ = post_worker_item(node, ws, cid, &job, body, ts).await;
        }
    }
}

/// Drive the run under the poster's authority via the runtime seam, returning the final answer or an
/// already-shaped error message for an `agent_error` item. The opaque/honest split happens here:
/// a named-but-unknown runtime and a capability deny both collapse to [`OPAQUE_DENY`]; a genuine run
/// fault is honest.
async fn drive_run(
    node: &Node,
    poster: &Principal,
    ws: &str,
    goal: &str,
    runtime: Option<&str>,
    job: &str,
    ts: u64,
) -> Result<String, String> {
    let registry = node.runtimes();

    // OPAQUE unknown-runtime: a named runtime that isn't registered collapses to the same deny as a
    // missing grant — no runtime-existence leak. (Absent runtime → default, always present.)
    if let Some(id) = runtime {
        if registry.resolve(Some(id)).is_err() {
            return Err(OPAQUE_DENY.to_string());
        }
    }

    // The agent acts with the ASKER'S authority, bounded by the asker's grants: effective principal is
    // `agent_caps ∩ caller`, and we pass the poster's own caps as the agent's — so the run can do
    // exactly what the poster is granted, nothing more. (The invoke gate `mcp:agent.invoke:call` fires
    // inside `invoke_via_runtime` under the poster.)
    let agent_caps = poster.caps().to_vec();

    invoke_via_runtime(
        node,
        &registry,
        runtime,
        poster,
        &agent_caps,
        ws,
        job,
        goal,
        Substrate::default(),
        &[], // no tool list surfaced this slice (the #3 MCP bridge is not built; in-house default too)
        ts,
    )
    .await
    .map_err(|e| match e {
        // Deny is opaque — same message as an unknown runtime (no capability/existence leak).
        AgentError::Denied => OPAQUE_DENY.to_string(),
        // A genuine run fault is honest and distinct (e.g. the external subprocess failed/timed out).
        other => format!("agent run failed: {other}"),
    })
}

/// Enforce the byte cap on the answer. Returns `(answer, truncated)`; trims to a char boundary at or
/// below [`AGENT_MAX_BYTES`] so the persisted item stays bounded. Pure.
fn cap_answer(answer: String) -> (String, bool) {
    if answer.len() <= AGENT_MAX_BYTES {
        return (answer, false);
    }
    let mut end = AGENT_MAX_BYTES;
    while end > 0 && !answer.is_char_boundary(end) {
        end -= 1;
    }
    (answer[..end].to_string(), true)
}

/// Post a worker result/error item under the system identity via the shared channel `deliver`
/// (STATE-first, then MOTION) — no `pub` gate (the host is posting its own answer). The id ties the
/// answer to the run (`a:<job>`) so a client can correlate them; `ts` orders it after the request.
async fn post_worker_item(
    node: &Node,
    ws: &str,
    cid: &str,
    job: &str,
    body: String,
    ts: u64,
) -> Result<(), super::error::ChannelError> {
    let item = Item::new(format!("a:{job}"), cid, WORKER_AUTHOR, body, ts);
    super::post::deliver(&node.store, &node.bus, ws, cid, item)
        .await
        .map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cap_answer_passes_small_answers_through() {
        let (a, t) = cap_answer("hello".to_string());
        assert_eq!(a, "hello");
        assert!(!t);
    }

    #[test]
    fn cap_answer_trims_oversized_at_char_boundary() {
        let big = "é".repeat(AGENT_MAX_BYTES); // 2 bytes each → well over the cap
        let (a, t) = cap_answer(big);
        assert!(t);
        assert!(a.len() <= AGENT_MAX_BYTES);
        // Trimmed at a char boundary — the string is still valid UTF-8 (no panic on slicing).
        assert!(a.chars().all(|c| c == 'é'));
    }
}
