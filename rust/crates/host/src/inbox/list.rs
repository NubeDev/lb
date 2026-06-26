//! `list_inbox` — a channel's durable inbox items, for the real inbox view.
//!
//! Gated by `mcp:inbox.list:call` (workspace-first §7). Reads `lb_inbox::list` — the durable items,
//! so the view survives a restart and shows the real `needs:triage`/`needs:approval` items (not the
//! workflow fake's simulated ones). Workspace-scoped: the namespace is selected from `ws`, so a ws-B
//! list can physically only return ws-B items.

use lb_auth::Principal;
use lb_inbox::{list, Item};
use lb_mcp::authorize_tool;
use lb_store::Store;

use super::error::InboxError;

/// Return the items of inbox `channel` in workspace `ws` for `principal`, oldest→newest.
pub async fn list_inbox(
    store: &Store,
    principal: &Principal,
    ws: &str,
    channel: &str,
) -> Result<Vec<Item>, InboxError> {
    authorize_tool(principal, ws, "inbox.list").map_err(|_| InboxError::Denied)?;
    let items = list(store, ws, channel).await?;
    Ok(items)
}
