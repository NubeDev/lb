//! `comments` — read one insight's full comment thread (insight-triage-scope.md).
//!
//! **The whole thread, not a window.** The occurrence ring pages because it evicts and can be long;
//! comments neither evict nor exceed [`crate::comment::MAX_COMMENTS_PER_INSIGHT`], so the thread is
//! complete and bounded by construction — paging it would add a cursor round-trip to a drawer that
//! always wants all of it. Newest-first, the order a responder reads (the last thing anyone learned
//! is the first thing they need).
//!
//! Rides `insight.get`'s capability — no new read cap (the thread is part of the finding's detail,
//! and a reader who may `get` the insight may read its notes). **Never joined into `insight.list`**:
//! that is the `evidence`/`analysis` boundary, and comments are the payload most able to make every
//! roster page expensive.

use lb_store::{list as store_list, Store};

use crate::comment::{Comment, TABLE};
use crate::error::InsightsError;

/// Every comment on `insight_id` in workspace `ws`, newest-first by `seq`. Empty when the insight
/// has no thread (or does not exist — the caller establishes existence; this read does not).
// SCOPE: docs/scope/insights/insight-triage-scope.md §"How it fits the core" (Get / list)
pub async fn comments(
    store: &Store,
    ws: &str,
    insight_id: &str,
) -> Result<Vec<Comment>, InsightsError> {
    // `write`-based rows live under a `data` envelope, so the single-field equality filter
    // `store::list` performs (`data.insight_id`) is the right read here — unlike the occurrence
    // ring, whose `capped_insert` rows are flat and need a direct query.
    let rows = store_list(store, ws, TABLE, "insight_id", insight_id).await?;
    let mut items: Vec<Comment> = rows
        .into_iter()
        .filter_map(|v| serde_json::from_value(v).ok())
        .collect();
    // Newest-first: `seq` is monotone per insight and nothing evicts, so it is a total order.
    items.sort_by(|a, b| b.seq.cmp(&a.seq));
    Ok(items)
}
