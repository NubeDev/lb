//! `list` — the faceted insights read (insights umbrella scope).
//!
//! AND-composed filter axes (any subset): `status`, `severity` (a floor), `origin_ref`, `tags` (a
//! tag-facet subset), `range` (a logical-ts window). Keyset-paged newest-first per the
//! page-cursor contract (`scope/datasources/page-cursor-scope.md`). Authorization is the host's
//! job; the workspace wall is structural (the store scan is ws-scoped, README §7).
//!
//! Every path here costs exactly ONE database query. `limit == 0` asks the database to count and
//! never reads a row; a page without counts pushes the whole filter into SQL; a page WITH counts
//! reads the matching set once and tallies it in memory, because the rows and the tally answer the
//! same question and asking twice is the N+1 in disguise.

use lb_store::Store;
use std::collections::{BTreeMap, HashSet};

use crate::count::StatusCounts;
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
    /// The entity limit the HOST sets for a restricted caller (entity-scoped data): the tag key and
    /// the values it may read. Never read from the wire (`serde(skip)`): a client cannot send it.
    /// `Some` narrows to insights whose `tags[key]` is one of the values — an insight without the
    /// tag matches nothing.
    #[serde(skip)]
    pub entity: Option<(String, Vec<String>)>,
    /// Free-text over the insight's NAME (`title`) — the roster's search box, resolved by the
    /// `insight_name` BM25 index rather than by filtering rows in the browser.
    ///
    /// NAME ONLY, by decision. The box used to match the tag-derived columns (site, asset, data
    /// type) as well, but it did so over the rows the browser happened to hold, so it searched a
    /// window and reported its size as a total. Searching the name server-side searches every row;
    /// widening it to the tag columns would mean indexing several more fields for a box people type
    /// a fault name into.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub search: Option<String>,
    /// Filter by OWNER — the triage roster's primary axis (insight-triage-scope.md). The raw wire
    /// value, one of:
    ///   - a subject (`user:priya` / `team:mechanical`) — only that subject's insights;
    ///   - `"me"` — the calling principal **and every team they belong to** (a team-assigned insight
    ///     must appear in the member's "mine" view; a naive sub-equality check silently drops it);
    ///   - `"none"` — the unassigned (the triage queue's primary view).
    ///
    /// `"me"` cannot be resolved here — team membership lives in the planes this crate is
    /// deliberately agnostic of — so the SERVICE layer resolves this string into an
    /// [`AssigneeFilter`] and passes it to [`list`] alongside, exactly as it does for tag facets.
    /// The field stays on the filter so the wire shape is one flat object.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assigned_to: Option<String>,
}

/// The wire literal meaning "unassigned" in [`ListFilter::assigned_to`].
pub const ASSIGNEE_NONE: &str = "none";
/// The wire literal meaning "the calling principal and their teams" in [`ListFilter::assigned_to`].
pub const ASSIGNEE_ME: &str = "me";

/// The host-resolved owner filter — what [`ListFilter::assigned_to`] means after the service layer
/// has expanded `"me"` against the principal and their teams.
///
/// A resolved enum rather than a bare string set because "unassigned" is not a subject and folding
/// it in as one (a sentinel `""` in the set) is how an empty-string assignee silently becomes a
/// legal owner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssigneeFilter {
    /// `"none"` — only insights with no `assigned_to`.
    Unassigned,
    /// One or more subjects; an insight matches if its `assigned_to` is any of them. `"me"` resolves
    /// to the principal's own sub plus each `team:` they are a member of.
    AnyOf(HashSet<String>),
}

impl AssigneeFilter {
    /// Does `assigned_to` satisfy this filter?
    fn matches(&self, assigned_to: Option<&String>) -> bool {
        match self {
            AssigneeFilter::Unassigned => assigned_to.is_none(),
            AssigneeFilter::AnyOf(subs) => assigned_to.map(|a| subs.contains(a)).unwrap_or(false),
        }
    }
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
    /// Rows to SKIP before the page — random access, for a pager offering "page N of M", first and
    /// last. The keyset [`cursor`](Self::cursor) cannot serve those: it only walks forward one page
    /// at a time, so it has no notion of page 7 or of the last page.
    ///
    /// Normally offset is the worse tool, because skipping N rows costs more the deeper it goes. Not
    /// on this engine: `ORDER BY` sorts the matched set on every request regardless, so offset
    /// merely discards from an already-sorted set. Measured at 20,000 rows — page 1 at 97.71 ms and
    /// the LAST page at 101.76 ms, flat — while the cursor swings from 1.77 ms to 166.85 ms
    /// depending on how many rows sit below it.
    ///
    /// Setting both this and `cursor` is contradictory; the cursor wins, because it is the precise
    /// one (an offset can skip or repeat rows when the set changes between pages).
    #[serde(default)]
    pub offset: usize,
    #[serde(default = "default_limit")]
    pub limit: usize,
    /// Rows per page, capped at 500. **`0` means "no rows"** — pair it with `counts` for a counter
    /// tile, and the read skips the row scan entirely rather than fetching records to discard.
    ///
    /// Also return the per-status tally for this filter (`counts` on the reply).
    ///
    /// Off by default, because most reads do not need it: paging to the second page of a roster
    /// re-counts nothing that changed. A UI showing a roster BESIDE its stat tiles sets it once and
    /// gets both from one call instead of two — which is the shape that motivated the flag.
    ///
    /// The tally ignores `filter.status`: it IS the per-status breakdown, so a status-filtered page
    /// still reports all four numbers. That asymmetry is deliberate and shared with
    /// [`crate::count`], which computes it.
    #[serde(default)]
    pub counts: bool,
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
    /// Present iff the query asked for it ([`ListQuery::counts`]). Absent — not zero — when it did
    /// not, so a reader can tell "not requested" from "genuinely none".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub counts: Option<StatusCounts>,
}

/// List insights in workspace `ws` matching `query`, newest-first, keyset-paged.
///
/// `tag_allow` is the tag-facet gate: when `query.filter.tags` is non-empty, the host pre-resolves
/// the matching entity ids through the tag graph (`tags.find`) and passes the id set here (the
/// crate is tag-graph-agnostic — README §7, the wall is the host's). `None` ⇒ no tag facet (or the
/// host resolved "everything"); `Some(set)` ⇒ keep only insights whose id is in the set.
///
/// `assignee` is the same pattern for the owner axis: the host resolves
/// [`ListFilter::assigned_to`]'s wire string (`"me"` needs the principal + their teams) into an
/// [`AssigneeFilter`] and passes it here. `None` ⇒ no owner filter.
// SCOPE: docs/scope/insights/insights-scope.md §"MCP surface" (insight.list)
// SCOPE: docs/scope/insights/insight-triage-scope.md §"How it fits the core" (Get / list)
// SCOPE: docs/scope/datasources/page-cursor-scope.md (the keyset contract)
pub async fn list(
    store: &Store,
    ws: &str,
    query: ListQuery,
    tag_allow: Option<&HashSet<String>>,
    assignee: Option<&AssigneeFilter>,
) -> Result<ListPage, InsightsError> {
    // A search term needs the BM25 index to EXIST — `@@` errors without one. Raise defines it, but a
    // workspace can be searched before anything was ever raised there, so ensure it here too. The
    // statement is `IF NOT EXISTS`, so this costs nothing once it is in place.
    if query.filter.search.is_some() {
        crate::schema::ensure_insight_schema(store, ws).await?;
    }

    let f = &query.filter.clone();

    // `limit: 0` means "no rows, just the tally" — the counter-panel case. Skipping the scan is the
    // whole point: a tile showing one integer has no business transferring 200 records to compute
    // it, and the aggregate is answered by the engine either way. Anything else pages as before.
    if query.limit == 0 {
        let counts = if query.counts {
            Some(crate::count::count(store, ws, f, tag_allow, assignee).await?)
        } else {
            None
        };
        return Ok(ListPage {
            items: Vec::new(),
            next: None,
            counts,
        });
    }

    // NO TALLY ⇒ let the engine do the work. The filters, the (last_ts, id) order, the keyset and
    // the limit all push down, so a 50-row page reads 50 rows instead of the whole table. The scan
    // path below stays for the tally case, where the counts cover the WHOLE matching set and every
    // matching row must therefore be seen.
    if !query.counts {
        let limit = query.limit.min(500);
        let page = crate::list_page::read(
            store,
            ws,
            f,
            tag_allow,
            assignee,
            query.cursor.as_ref(),
            limit,
            query.offset,
        )
        .await?;
        let mut items = page.items;
        let next = if page.has_more {
            items.last().map(|i| PageCursor {
                ts: i.last_ts,
                id: i.id.clone(),
            })
        } else {
            None
        };
        // Same boundary as the scan path: `evidence`/`analysis` are `get`-only.
        for i in &mut items {
            i.evidence = None;
            i.analysis = None;
        }
        return Ok(ListPage {
            items,
            next,
            counts: None,
        });
    }

    // ONE query. Every axis EXCEPT status — the tally is the per-status breakdown, so narrowing by
    // status first would zero three of its four numbers. It comes back already ordered, so the page
    // below is a slice rather than a second sort.
    let matched: Vec<Insight> = crate::list_page::read_all_matching(
        store,
        ws,
        f,
        tag_allow,
        assignee,
        crate::table_scan::MAX_ROWS,
    )
    .await?
    .into_iter()
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
    // The owner axis composes with every other filter by plain AND, and with keyset paging
    // below — it is applied BEFORE the sort/cursor/truncate, so a filtered roster pages
    // correctly instead of returning short pages of a pre-filtered window.
    .filter(|i| {
        assignee
            .map(|a| a.matches(i.assigned_to.as_ref()))
            .unwrap_or(true)
    })
    .collect();

    // The tally, when asked for — counted from the rows ALREADY IN HAND. `list` has just scanned and
    // filtered the table, so this is one more pass over memory; asking the database to re-derive it
    // would be a second scan for an answer we are holding. (`limit: 0` never reaches here — that
    // path skips the scan entirely and uses the SQL aggregate, which is the right tool when there
    // are no rows to count.)
    let counts = if query.counts {
        Some(crate::count::tally(&matched))
    } else {
        None
    };

    let mut items: Vec<Insight> = matched
        .into_iter()
        .filter(|i| f.status.map(|s| i.status == s).unwrap_or(true))
        .collect();

    // Newest-first by (last_ts, id) — id is the ULID tiebreaker for same-ts rows.
    items.sort_by(|a, b| b.last_ts.cmp(&a.last_ts).then_with(|| b.id.cmp(&a.id)));

    // Keyset: strictly after the cursor in the (last_ts DESC, id DESC) order.
    if let Some(cur) = &query.cursor {
        items.retain(|i| (i.last_ts, i.id.as_str()) < (cur.ts, cur.id.as_str()));
    }

    // The offset half of the pager. The pushdown path above expresses this as `START n`; here the
    // matching set is already sorted in memory, so it is a skip. Same precedence as there: a cursor
    // and an offset together are contradictory, so the cursor wins and the offset is ignored rather
    // than compounded.
    //
    // This is NOT optional just because a tally was asked for. Without it, a caller asking for
    // `counts` and page 5 was handed page 1 and no error — the wrong page, silently, which is worse
    // than a refusal because nothing in the reply says so.
    if query.cursor.is_none() && query.offset > 0 {
        let skip = query.offset.min(items.len());
        items.drain(..skip);
    }

    let limit = query.limit.min(500);
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
    //
    // `analysis` is stripped on the same boundary and for the same two reasons — six prose fields
    // per row bloat a roster page for data only the drawer reads, and the `get`-only boundary is
    // what contains free text producers populate from anything in scope. Note the deliberate
    // asymmetry with the tag echo, which DOES ride `list`: the rule is "does a column need it", and
    // tags exist to be columns while analysis exists to fill a drawer.
    // SCOPE: docs/scope/insights/insight-analysis-scope.md §"How it fits" (Get / list)
    //
    // `assigned_to` is deliberately NOT stripped — it is the roster's owner column, so it rides
    // `list` exactly as the tag echo does. Its sibling on the triage plane, the comment thread, is
    // never even read here: comments are the payload most able to make every roster page expensive,
    // and they are `get`-only (insight-triage-scope.md §"Roster width").
    for i in &mut items {
        i.evidence = None;
        i.analysis = None;
    }

    Ok(ListPage {
        items,
        next,
        counts,
    })
}
