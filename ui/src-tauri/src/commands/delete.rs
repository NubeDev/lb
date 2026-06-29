//! The `channel_delete` command — the shell's IPC verb the UI's `channel.api.ts::remove` calls.
//! Thin glue over `lb_host::delete`, with the same `pub`-grant + author-ownership check the rest
//! of the host enforces. The shell adds no authority of its own.

use lb_host::delete;

use crate::state::NodeHandle;

/// Delete message `id` from `channel` as the session principal. Only the message's author may.
pub async fn channel_delete(
    handle: &NodeHandle,
    channel: &str,
    id: &str,
) -> Result<(), String> {
    delete(&handle.node, &handle.principal, &handle.ws, channel, id)
        .await
        .map_err(|e| e.to_string())
}
