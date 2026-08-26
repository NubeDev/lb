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
use serde_json::Value;

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
    record_inbox_with_meta(store, principal, ws, channel, id, body, ts, Value::Null).await
}

/// [`record_inbox`] with a source-specific `meta` payload attached to the item.
///
/// Split as a second entry point rather than a widened signature because `record_inbox` is the verb
/// the MCP bridge and the flow/rule paths call, and threading a `Value::Null` through every one of
/// them to serve one caller is how a signature acquires an argument nobody reads. The gate, the
/// author-forcing, and the idempotency are identical — `meta` is carried, never inspected
/// (`lb_inbox::Item::meta`).
#[allow(clippy::too_many_arguments)]
pub async fn record_inbox_with_meta(
    store: &Store,
    principal: &Principal,
    ws: &str,
    channel: &str,
    id: &str,
    body: &str,
    ts: u64,
    meta: Value,
) -> Result<(), InboxError> {
    authorize_tool(principal, ws, "inbox.record").map_err(|_| InboxError::Denied)?;
    let item = Item::new(id, channel, principal.sub(), body, ts).with_meta(meta);
    record(store, ws, &item).await?;
    Ok(())
}
