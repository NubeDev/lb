//! `list` — the faceted insights read (insights umbrella scope).
//!
//! AND-composed filter axes (any subset): `status`, `severity` (a floor), `origin_ref`, `tags` (a
//! tag-facet subset), `range` (a logical-ts window). Keyset-paged newest-first per the
//! page-cursor contract (`scope/datasources/page-cursor-scope.md`). Authorization is the host's
//! job; the workspace wall is structural (the store scan is ws-scoped, README §7).
//!
//! **STUB**: the filter composition + keyset paging body is deferred — the signature, the filter
//! shape, and the page-cursor contract are stable so the host + UI can wire against them. See the
//! scaffold-session punch-list.

use lb_store::Store;
use std::collections::BTreeMap;

use crate::error::InsightsError;
use crate::insight::Insight;
use crate::severity::Severity;
use crate::status::Status;

/// The AND filter. Every provided field must match; all absent = "all insights in this ws".
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ListFilter {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<Status>,
    /// A severity floor (≥ this severe).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub severity: Option<Severity>,
    /// Filter by producer ref (the rule/flow id).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin_ref: Option<String>,
    /// Tag facets — the insight must carry ALL (`{ k: v, … }` → subset).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub tags: BTreeMap<String, String>,
    /// `[from, to]` logical-ts window (inclusive on both ends).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub range: Option<(u64, u64)>,
}

/// Keyset cursor — the last id+ts the page returned; the next page starts strictly after.
/// Opaque to the caller (a stringified JSON the host round-trips); the verb parses it here.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PageCursor {
    /// The last `ts` of the previous page (newest-first ordering).
    pub ts: u64,
    /// The last `id` of the previous page (the tiebreaker for same-ts rows).
    pub id: String,
}

/// The full list query (filter + paging + limit).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ListQuery {
    #[serde(default, flatten)]
    pub filter: ListFilter,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<PageCursor>,
    #[serde(default = "default_limit")]
    pub limit: usize,
}

fn default_limit() -> usize {
    50
}

/// One newest-first page of insights + the cursor for the next page (`None` ⇒ last page).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ListPage {
    pub items: Vec<Insight>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next: Option<PageCursor>,
}

/// List insights in workspace `ws` matching `query`, newest-first, keyset-paged.
// SCOPE: docs/scope/insights/insights-scope.md §"MCP surface" (insight.list)
// SCOPE: docs/scope/datasources/page-cursor-scope.md (the keyset contract)
pub async fn list(_store: &Store, _ws: &str, _query: ListQuery) -> Result<ListPage, InsightsError> {
    // 1. Scan the `insight` table in ws (filtered by status/severity via the store `list` field
    //    path for the cheap axes, then post-filter the tag subset + range in Rust).
    // 2. Order newest-first by (last_ts, id); keyset-paginate strictly after `query.cursor`.
    // 3. Bound the page at `query.limit`; compute `next` from the last returned row.
    todo!("insights: faceted list + keyset paging — SCOPE: insights-scope.md §MCP surface")
}
