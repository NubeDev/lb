//! The `channel_edit` command — the shell's IPC verb the UI's `channel.api.ts::edit` calls.
//! Thin glue over `lb_host::edit`, with the same `pub`-grant + author-ownership check the rest
//! of the host enforces. The shell adds no authority of its own.

use lb_host::edit;
use lb_inbox::Item;

use crate::state::NodeHandle;

/// Edit message `id` in `channel` (set its body to `body`, ordering ts to `ts`) as the session
/// principal. Only the message's author may (the host re-checks against the stored author).
pub async fn channel_edit(
    handle: &NodeHandle,
    channel: &str,
    id: &str,
    body: &str,
    ts: u64,
) -> Result<Item, String> {
    edit(
        &handle.node,
        &handle.principal,
        &handle.ws,
        channel,
        id,
        body,
        ts,
    )
    .await
    .map_err(|e| e.to_string())
}
