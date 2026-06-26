//! `channel_list` — every registered channel in a workspace, for the UI's channel switcher.
//!
//! Gated by the channel `sub` capability (`bus:chan/*:sub`): listing channels is "may I read the
//! channel surface" — the same gate `history` passes. Resource `*` (the list is over all channels,
//! not one). Workspace-first (§7): the namespace is selected from `ws`, so a ws-B list can
//! physically only return ws-B channels — the wall holds.

use lb_auth::Principal;
use lb_caps::{check, Action, Decision, Request, Surface};
use lb_store::{list as store_list, Store};

use crate::channel::ChannelError;

use super::model::{ChannelRecord, KIND, TABLE};

/// Return every registered channel in workspace `ws` for `principal`, oldest→newest by `ts`.
pub async fn channel_list(
    store: &Store,
    principal: &Principal,
    ws: &str,
) -> Result<Vec<ChannelRecord>, ChannelError> {
    // "List channels" = read the channel surface. Reuse the bus `sub` grant over the `*` resource.
    let req = Request::new(ws, Surface::Bus, "chan/*", Action::Sub);
    if !matches!(check(principal, &req), Decision::Allowed) {
        return Err(ChannelError::Denied);
    }
    // Select every channel row by the constant `kind` discriminant, then sort by the logical `ts`
    // (the store `list` is a pure equality filter — it does not order).
    let rows = store_list(store, ws, TABLE, "kind", KIND).await?;
    let mut records: Vec<ChannelRecord> = rows
        .into_iter()
        .map(|v| {
            serde_json::from_value(v)
                .map_err(|e| lb_store::StoreError::Decode(e.to_string()).into())
        })
        .collect::<Result<_, ChannelError>>()?;
    records.sort_by_key(|r| r.ts);
    Ok(records)
}
