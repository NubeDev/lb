//! `occurrences` — read a per-insight occurrence ring newest-first, keyset-paged
//! (insight-occurrences-scope.md).
//!
//! Own verb, own cap `mcp:insight.occurrences:call` — the list/get read cap does NOT imply
//! evidence access (evidence may be more sensitive than the headline). The page is newest-first
//! (the analyst wants the last N transactions); the parent's `count` (which may exceed the stored
//! rows) is on the parent Insight, not here.
//!
//! **STUB**: the keyset-paged ring scan body is deferred — see the punch-list.

use lb_store::Store;

use crate::error::InsightsError;
use crate::occurrence::Occurrence;

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
    _store: &Store,
    _ws: &str,
    _insight_id: &str,
    _cursor: Option<OccCursor>,
    _limit: usize,
) -> Result<OccurrencePage, InsightsError> {
    // 1. Scan the `insight_occ` table filtered by `insight_id` (the store `list` field path).
    // 2. Order newest-first by `seq` (desc); keyset-paginate strictly-before `cursor.seq`.
    // 3. Bound the page at `limit`; compute `next` from the oldest returned row.
    todo!("insights: occurrence ring read (newest-first keyset) — SCOPE: occurrences-scope.md §Verb surface")
}
