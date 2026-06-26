//! `resolve_inbox` — record a reviewer's approve/reject/defer on an inbox item (collaboration scope,
//! slice 4). This is the real UI action the S6 approval gate now reads: approving a `needs:approval`
//! item here writes the `Resolution` the reactor turns into a started coding job.
//!
//! Gated by `mcp:inbox.resolve:call` (workspace-first §7). The deciding `actor` is forced to the
//! principal's `sub` (set by the host, never caller-supplied) — a caller cannot forge another
//! reviewer's sign-off. Idempotent on the item id (re-resolving upserts; last decision wins).

use lb_auth::Principal;
use lb_inbox::{resolve, Decision, Resolution};
use lb_mcp::authorize_tool;
use lb_store::Store;

use super::error::InboxError;

/// Resolve inbox item `item_id` in workspace `ws` as `principal` with `decision` at logical `ts`.
pub async fn resolve_inbox(
    store: &Store,
    principal: &Principal,
    ws: &str,
    item_id: &str,
    decision: Decision,
    ts: u64,
) -> Result<(), InboxError> {
    authorize_tool(principal, ws, "inbox.resolve").map_err(|_| InboxError::Denied)?;
    let resolution = Resolution::new(item_id, decision, principal.sub(), ts);
    resolve(store, ws, &resolution).await?;
    Ok(())
}
