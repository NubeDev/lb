//! `record_inbox` — create a durable inbox item over the capability gate (proof-workflow-sim scope).
//!
//! The host-callback's first inbox WRITE that PRODUCES workflow motion: a guest (or any bridged caller)
//! creates an item that then shows up in `list_inbox`. Gated by `mcp:inbox.record:call` (workspace-first
//! §7). The deciding `author` is **forced** to the principal's `sub` (set by the host, never
//! caller-supplied) — exactly like `resolve_inbox`'s actor, so a caller can't forge another source's
//! authorship. Idempotent on `(channel, id)` (re-recording the same id upserts; inbox-outbox scope).
//!
//! The raw item persistence stays in `lb_inbox::record`; this layer is authorization + author-forcing
//! only (one verb per file, FILE-LAYOUT §3).

use lb_auth::Principal;
use lb_inbox::{record, Item};
use lb_mcp::authorize_tool;
use lb_store::Store;

use super::error::InboxError;

/// Record an inbox item on `channel` in workspace `ws` as `principal`. `id` is the stable item id
/// (idempotent on `(channel, id)`); `body`/`ts` are the item's content + logical ordering ts. The
/// author is forced to `principal.sub()` — never caller-supplied.
pub async fn record_inbox(
    store: &Store,
    principal: &Principal,
    ws: &str,
    channel: &str,
    id: &str,
    body: &str,
    ts: u64,
) -> Result<(), InboxError> {
    authorize_tool(principal, ws, "inbox.record").map_err(|_| InboxError::Denied)?;
    let item = Item::new(id, channel, principal.sub(), body, ts);
    record(store, ws, &item).await?;
    Ok(())
}
