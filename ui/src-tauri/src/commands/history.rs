//! The `channel_history` command — the shell's IPC verb the UI's `channel.api.ts::history`
//! calls. Thin glue over `lb_host::history`, with the same `sub`-grant check the rest of the
//! host enforces. Reads the durable history, so it works across a restart (state, §3.3).

use lb_host::history;
use lb_inbox::Item;

use crate::state::NodeHandle;

/// Read `channel`'s items (oldest→newest) as the session principal.
pub async fn channel_history(handle: &NodeHandle, channel: &str) -> Result<Vec<Item>, String> {
    history(&handle.node.store, &handle.principal, &handle.ws, channel)
        .await
        .map_err(|e| e.to_string())
}
