//! `occurrences` — read a per-insight occurrence ring newest-first, keyset-paged
//! (insight-occurrences-scope.md).
//!
//! Own verb, own cap `mcp:insight.occurrences:call` — the list/get read cap does NOT imply
//! evidence access (evidence may be more sensitive than the headline). The page is newest-first
//! (the analyst wants the last N transactions); the parent's `count` (which may exceed the stored
//! rows) is on the parent Insight, not here.
//!
//! **STUB**: the keyset-paged ring scan body is deferred — see the punch-list.

use serde_json::Value;

use lb_store::Store;

use crate::error::InsightsError;
use crate::occurrence::{Occurrence, TABLE};

/// The keyset cursor for occurrence paging. Newest-first: the next page starts strictly BEFORE
/// (older than) the cursor's `seq`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct OccCursor {
    /// The oldest `seq` of the previous page (the next page is `seq < this`).
    pub seq: u64,
}

/// One newest-first page of the occurrence ring.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct OccurrencePage {
    pub items: Vec<Occurrence>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next: Option<OccCursor>,
}

/// Read the occurrence ring for insight `insight_id` in workspace `ws`, newest-first.
// SCOPE: docs/scope/insights/insight-occurrences-scope.md §"Verb surface"
// SCOPE: docs/scope/datasources/page-cursor-scope.md (the keyset contract)
pub async fn occurrences(
    store: &Store,
    ws: &str,
    insight_id: &str,
    cursor: Option<OccCursor>,
    limit: usize,
) -> Result<OccurrencePage, InsightsError> {
    let limit = limit.clamp(1, 500);
    // Occurrence rows are stored FLAT by `capped_insert` (no `data` envelope), so read them with a
    // direct flat query (the telemetry precedent), NOT `store::list` (which assumes the wrapper).
    // Filter to this insight; the ring is bounded by the policy cap, so ordering in Rust is cheap.
    let sql = "SELECT * OMIT id, in, out FROM type::table($tb) WHERE insight_id = $iid";
    let mut resp = store
        .query_ws(
            ws,
            sql,
            vec![
                ("tb".into(), Value::String(TABLE.to_string())),
                ("iid".into(), Value::String(insight_id.to_string())),
            ],
        )
        .await?;
    let rows: Vec<Value> = resp
        .take(0)
        .map_err(|e| lb_store::StoreError::Decode(e.to_string()))?;
    let mut items: Vec<Occurrence> = rows
        .into_iter()
        .filter_map(|v| serde_json::from_value(v).ok())
        .collect();
    // Newest-first by the monotone per-insight sequence.
    items.sort_by(|a, b| b.seq.cmp(&a.seq));
    // Keyset: strictly before (older than) the cursor's seq.
    if let Some(cur) = cursor {
        items.retain(|o| o.seq < cur.seq);
    }
    // The `next` cursor is the oldest row of a FULL page (a short page is the end).
    let has_more = items.len() > limit;
    items.truncate(limit);
    let next = if has_more {
        items.last().map(|o| OccCursor { seq: o.seq })
    } else {
        None
    };
    Ok(OccurrencePage { items, next })
}
