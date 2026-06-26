//! Read a channel's durable history — the read verb, backed by the inbox (state, §3.3).
//!
//! This is what makes "restart the node and history is intact" true: the messages live in the
//! store, so a fresh subscriber (or a fresh process) reads them back here, independent of any
//! live bus traffic. Authorization is the same gate as listening — a `bus:chan/{cid}:sub`
//! grant — run before the store is touched.

use lb_auth::Principal;
use lb_caps::Action;
use lb_inbox::{list, Item};
use lb_store::Store;

use super::authorize::authorize;
use super::error::ChannelError;

/// Return channel `cid`'s items in workspace `ws`, oldest→newest, for `principal`. Requires a
/// `sub` grant on the channel; denied (or another workspace's) callers get nothing they
/// shouldn't — gate 1 refuses a cross-workspace read before the query runs.
pub async fn history(
    store: &Store,
    principal: &Principal,
    ws: &str,
    cid: &str,
) -> Result<Vec<Item>, ChannelError> {
    authorize(principal, ws, cid, Action::Sub)?;
    Ok(list(store, ws, cid).await?)
}
