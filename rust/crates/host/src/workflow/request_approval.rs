//! `request_approval` — write the `needs:approval` inbox item that gates the coding job (vision §3
//! step 5, coding-workflow scope). Reasoning is cheap; acting on a repo is not, so the workflow
//! inserts a human gate before any job starts.
//!
//! An approval is just an inbox item tagged `needs:approval` routed to a team (a tag, not a policy
//! engine) — same normalized shape as any item (inbox scope). The item id is the gate key the
//! job-start verb later reads a [`Resolution`](lb_inbox::Resolution) for. State only; the reviewer's
//! UI resolves it. Authorization: `mcp:workflow.request_approval:call`, workspace-first.

use lb_auth::Principal;
use lb_inbox::{record, Item};

use super::authorize::authorize_workflow;
use super::error::WorkflowError;

/// The channel approval items land in; members of the routed team watch it.
pub const APPROVAL_CHANNEL: &str = "approvals";

/// Request approval `approval_id` for the proposed coding job on `scope_doc`, routed to `team`, in
/// workspace `ws` as `principal`. Idempotent on `approval_id`. Returns the stored item; its id is
/// the key `resolve_approval` and the job-start gate use.
pub async fn request_approval(
    store: &lb_store::Store,
    principal: &Principal,
    ws: &str,
    approval_id: &str,
    scope_doc: &str,
    team: &str,
    ts: u64,
) -> Result<Item, WorkflowError> {
    authorize_workflow(principal, ws, "request_approval")?;
    let body = format!("needs:approval route:team:{team} scope_doc:{scope_doc}");
    let item = Item::new(
        approval_id,
        APPROVAL_CHANNEL,
        "ext:coding-workflow",
        body,
        ts,
    );
    record(store, ws, &item).await?;
    Ok(item)
}
