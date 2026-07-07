//! The `channel_post` command — the shell's IPC verb the UI's `channel.api.ts::post` calls.
//! Thin glue: it forwards to `lb_host::post` with the session principal, so the SAME
//! capability check guards the desktop UI as guards every other caller (capability-first,
//! §3.5). The shell adds no authority of its own.

use lb_host::post;
use lb_inbox::Item;

use crate::state::NodeHandle;

/// Post `item` to `channel` as the session principal. Returns the stored item. Errors are
/// stringified for the IPC boundary (the UI shows them; a `Denied` reads as "denied").
pub async fn channel_post(
    handle: &NodeHandle,
    channel: &str,
    item: Item,
) -> Result<Item, String> {
    post(
        &handle.node,
        &handle.principal,
        &handle.ws,
        channel,
        item,
    )
    .await
    .map_err(|e| e.to_string())
}
