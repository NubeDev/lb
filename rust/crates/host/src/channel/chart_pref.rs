//! Per-viewer chart preference for a channel query-result item (channel query charts, "best long
//! term" persistence). A `query_result` item is authored by the query worker and is IMMUTABLE — the
//! canonical result never changes. How a VIEWER chooses to plot it is separate, per-user state: a
//! small record keyed by `(channel, item, user)`, read back and merged over the host's default at
//! render. Two viewers can plot the same result differently, and the author-ownership + state/motion
//! invariants stay intact (§3).
//!
//! Authorization reuses the channel gate (one owner of "may this principal reach this channel?"):
//! you may read/save a plot pref for a channel you may READ (`bus:chan/{cid}:sub`), workspace-first.
//! The spec is stored opaquely — the UI owns its shape (`PlotSpec`); the host only scopes + persists.
//!
//! One responsibility: the `(cid, item, user)` chart-pref record's read/write + its two gated verbs.

use lb_auth::Principal;
use lb_caps::Action;
use lb_store::{read, write, Store};
use serde_json::Value;

use super::authorize::authorize as authorize_channel;
use super::error::ChannelError;

/// The table holding per-user chart preferences. The workspace is the namespace (the hard wall, §6),
/// so the same `(cid, item, user)` in workspace B is a different record — never reachable from A.
const TABLE: &str = "channel_chart_pref";

/// The record id for `(cid, item, user)` — user-scoped so two viewers of the same result don't
/// collide. `__` joins the parts (mirrors the inbox `channel__id` convention); `user` is the
/// principal's `sub`, taken from the token, never an argument.
fn pref_id(cid: &str, item: &str, user: &str) -> String {
    format!("{cid}__{item}__{user}")
}

/// Read the caller's saved plot spec for `(cid, item)`, or `None` when they never saved one (the
/// surface then shows the host's default). Same gate as reading the channel's messages.
pub async fn chart_pref_get(
    store: &Store,
    principal: &Principal,
    ws: &str,
    cid: &str,
    item: &str,
) -> Result<Option<Value>, ChannelError> {
    authorize_channel(principal, ws, cid, Action::Sub)?;
    Ok(read(store, ws, TABLE, &pref_id(cid, item, principal.sub())).await?)
}

/// Upsert the caller's plot spec for `(cid, item)` (idempotent on the id). The spec rides opaquely.
pub async fn chart_pref_set(
    store: &Store,
    principal: &Principal,
    ws: &str,
    cid: &str,
    item: &str,
    spec: &Value,
) -> Result<(), ChannelError> {
    authorize_channel(principal, ws, cid, Action::Sub)?;
    write(store, ws, TABLE, &pref_id(cid, item, principal.sub()), spec).await?;
    Ok(())
}
