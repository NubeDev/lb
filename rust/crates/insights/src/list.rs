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
use std::collections::{BTreeMap, HashSet};

use crate::error::InsightsError;
use crate::insight::{Insight, OCC_TABLE};
use crate::severity::Severity;
use crate::status::Status;
use crate::table_scan::scan_all;

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
///
/// Not `Eq` — [`Insight`] isn't, since `evidence.threshold` is an `f64`.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ListPage {
    pub items: Vec<Insight>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next: Option<PageCursor>,
}

/// List insights in workspace `ws` matching `query`, newest-first, keyset-paged.
///
/// `tag_allow` is the tag-facet gate: when `query.filter.tags` is non-empty, the host pre-resolves
/// the matching entity ids through the tag graph (`tags.find`) and passes the id set here (the
/// crate is tag-graph-agnostic — README §7, the wall is the host's). `None` ⇒ no tag facet (or the
/// host resolved "everything"); `Some(set)` ⇒ keep only insights whose id is in the set.
// SCOPE: docs/scope/insights/insights-scope.md §"MCP surface" (insight.list)
// SCOPE: docs/scope/datasources/page-cursor-scope.md (the keyset contract)
pub async fn list(
    store: &Store,
    ws: &str,
    query: ListQuery,
    tag_allow: Option<&HashSet<String>>,
) -> Result<ListPage, InsightsError> {
    let f = &query.filter;
    let rows = scan_all(store, ws, OCC_TABLE).await?;
    let mut items: Vec<Insight> = rows
        .into_iter()
        .filter_map(|v| serde_json::from_value::<Insight>(v).ok())
        .filter(|i| f.status.map(|s| i.status == s).unwrap_or(true))
        .filter(|i| f.severity.map(|s| i.severity.at_least(s)).unwrap_or(true))
        .filter(|i| {
            f.origin_ref
                .as_ref()
                .map(|r| &i.origin.reference == r)
                .unwrap_or(true)
        })
        .filter(|i| {
            f.range
                .map(|(from, to)| i.last_ts >= from && i.last_ts <= to)
                .unwrap_or(true)
        })
        .filter(|i| tag_allow.map(|set| set.contains(&i.id)).unwrap_or(true))
        .collect();

    // Newest-first by (last_ts, id) — id is the ULID tiebreaker for same-ts rows.
    items.sort_by(|a, b| b.last_ts.cmp(&a.last_ts).then_with(|| b.id.cmp(&a.id)));

    // Keyset: strictly after the cursor in the (last_ts DESC, id DESC) order.
    if let Some(cur) = &query.cursor {
        items.retain(|i| (i.last_ts, i.id.as_str()) < (cur.ts, cur.id.as_str()));
    }

    let limit = query.limit.clamp(1, 500);
    let has_more = items.len() > limit;
    items.truncate(limit);
    let next = if has_more {
        items.last().map(|i| PageCursor {
            ts: i.last_ts,
            id: i.id.clone(),
        })
    } else {
        None
    };

    // Strip `evidence` from every listed record — it is echoed by `insight.get` only. Two reasons:
    // a roster page is many-record and the descriptor would bloat every one of them for a field
    // only the detail view reads; and the SQL it carries is schema disclosure, which the narrower
    // per-finding read already implies but a broad list does not. Stripped AFTER truncation so the
    // cost is bounded by the page, not the scan.
    // SCOPE: docs/scope/insights/insight-evidence-scope.md §"How it fits" (Capabilities)
    for i in &mut items {
        i.evidence = None;
    }

    Ok(ListPage { items, next })
}
