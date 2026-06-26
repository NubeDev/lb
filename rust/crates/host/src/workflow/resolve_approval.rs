//! `resolve_approval` — a reviewer's decision on a `needs:approval` inbox item (vision §3 step 5,
//! coding-workflow scope). Writes the [`Resolution`](lb_inbox::Resolution) facet (approve/reject/
//! defer + actor + ts) the job-start gate reads.
//!
//! The deciding `actor` is forced to the principal's `sub` (set by the host, never caller-supplied)
//! so audit shows who actually approved — a caller cannot forge another reviewer's sign-off.
//! Authorization: `mcp:workflow.resolve_approval:call`, workspace-first (only a grantee — i.e. a
//! reviewer — may resolve). State only; the resolution is the durable record the gate consults.

use lb_auth::Principal;
use lb_inbox::{resolve, Decision, Resolution};

use super::authorize::authorize_workflow;
use super::error::WorkflowError;

/// Resolve approval item `approval_id` in workspace `ws` as `principal` with `decision`. Idempotent
/// (re-resolving upserts; last decision wins — a deferred item can later be approved).
pub async fn resolve_approval(
    store: &lb_store::Store,
    principal: &Principal,
    ws: &str,
    approval_id: &str,
    decision: Decision,
    ts: u64,
) -> Result<(), WorkflowError> {
    authorize_workflow(principal, ws, "resolve_approval")?;
    let resolution = Resolution::new(approval_id, decision, principal.sub(), ts);
    resolve(store, ws, &resolution).await?;
    Ok(())
}
