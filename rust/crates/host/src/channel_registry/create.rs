//! `channel_create` — explicitly register a channel so it is listable before anything is posted.
//!
//! Gated by the channel `pub` capability (`bus:chan/{cid}:pub`): creating a channel is exactly "may
//! I post here", so it reuses the channel gate verbatim — no new capability (collaboration scope).
//! Workspace-first (§7). Idempotent on the channel id: re-creating upserts the same row (the last
//! `created_by`/`ts` win), so `create` and create-on-post never conflict.

use lb_auth::Principal;
use lb_caps::Action;
use lb_store::{write, Store};

use crate::channel::{authorize_channel, ChannelError};

use super::model::{ChannelRecord, TABLE};

/// Register channel `cid` in workspace `ws` as `principal`. Returns the stored record.
pub async fn channel_create(
    store: &Store,
    principal: &Principal,
    ws: &str,
    cid: &str,
    ts: u64,
) -> Result<ChannelRecord, ChannelError> {
    authorize_channel(principal, ws, cid, Action::Pub)?;
    let record = ChannelRecord::new(cid, principal.sub(), ts);
    let value = serde_json::to_value(&record)
        .map_err(|e| ChannelError::Store(lb_store::StoreError::Decode(e.to_string())))?;
    write(store, ws, TABLE, cid, &value).await?;
    Ok(record)
}
