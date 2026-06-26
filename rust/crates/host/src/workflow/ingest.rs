//! `ingest_issue` — the inbound `github-bridge` path (vision §3 step 1, coding-workflow scope).
//!
//! A GitHub webhook normalizes into ONE inbox item in the `triage` channel, tagged `source:github
//! needs:triage` (the only contract between the bridge and whatever consumes it). The item id is the
//! issue id, so a retried webhook upserts one item — replay-safe ingress (idempotent on `(channel,
//! id)`, inbox scope). State only: the item is durable; no motion here (triage picks it up).
//!
//! Authorization: `mcp:workflow.ingest_issue:call`, workspace-first. A bridge without the grant
//! cannot deposit an issue (the deny path). Raw inbox `record` runs after the gate.

use lb_auth::Principal;
use lb_inbox::{record, Item};

use super::authorize::authorize_workflow;
use super::error::WorkflowError;

/// The channel inbound issues land in; consumers watch it for `needs:triage`.
pub const TRIAGE_CHANNEL: &str = "triage";

/// Ingest GitHub issue `issue_id` (with its normalized `payload`) into workspace `ws`'s triage
/// inbox as `principal` (the bridge). Idempotent on `issue_id`. Returns the stored item.
pub async fn ingest_issue(
    store: &lb_store::Store,
    principal: &Principal,
    ws: &str,
    issue_id: &str,
    payload: &str,
    ts: u64,
) -> Result<Item, WorkflowError> {
    authorize_workflow(principal, ws, "ingest_issue")?;
    // The `source:github needs:triage` tags ride in the body alongside the payload — the inbox
    // `Item` shape is intentionally stable (inbox scope non-goal), so the triage contract lives in
    // the channel name + this tagged body, not a new column.
    let body = format!("source:github needs:triage\n{payload}");
    let item = Item::new(issue_id, TRIAGE_CHANNEL, "ext:github-bridge", body, ts);
    record(store, ws, &item).await?;
    Ok(item)
}
