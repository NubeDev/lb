//! The stable record id for an insight + the dedup-key lookup helper.
//!
//! Mirrors `lb_inbox::record_id`/`TABLE`: the id is the Surreal record key; the lookup helper is
//! the (ws, dedup_key) → id resolution the raise verb's dedup/re-open branch needs. Kept in its
//! own file (FILE-LAYOUT §3) so a caller searching for "how is the insight id shaped" finds it.

use lb_store::{list as store_list, Store, StoreError};

use crate::insight::Insight;
use crate::insight::OCC_TABLE;

/// The stable record id for an insight. The raise verb writes at this key; the ULID is
/// host-assigned at first raise and stored in the record's `id` field (the two agree).
pub fn record_id(id: &str) -> String {
    id.to_string()
}

/// Resolve the existing insight (if any) for `dedup_key` in workspace `ws`. The raise verb's
/// dedup/re-open decision reads through here: `Some` ⇒ bump `count`/`last_ts` (or re-open if
/// `resolved`); `None` ⇒ create. Single-row by the dedup-key invariant.
pub async fn dedup_lookup(
    store: &Store,
    ws: &str,
    dedup_key: &str,
) -> Result<Option<Insight>, StoreError> {
    let rows = store_list(store, ws, OCC_TABLE, "dedup_key", dedup_key).await?;
    Ok(rows
        .into_iter()
        .next()
        .and_then(|v| serde_json::from_value(v).ok()))
}
