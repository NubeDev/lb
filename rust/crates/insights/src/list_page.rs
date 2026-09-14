//! The PUSHED-DOWN page read — filter, order and limit in the engine (insight-batch scope).
//!
//! # Why this exists beside `scan_all`
//!
//! `list` reads every row and filters in Rust. That is unavoidable when a tally is asked for: the
//! counts cover the whole matching set, so the whole matching set must be seen. It is pure waste
//! otherwise — a roster paging to 50 rows read the entire table to throw nearly all of it away, and
//! `scan_all` issues one query per 200 rows, so a big table cost many round trips for one page.
//!
//! So when no tally is asked for, the engine does the work: the same predicates (from the ONE builder
//! `count` shares), the same `(last_ts, id)` DESC order, the same keyset cursor, and `LIMIT n+1` —
//! the extra row is how "is there a next page" is answered without a second query.
//!
//! Equivalence with the scan path is the whole contract, and `pushdown_returns_what_the_scan_path_returns`
//! asserts it filter by filter: a page must not depend on which path produced it.

use serde_json::Value;

use crate::insight::{Insight, OCC_TABLE};
use crate::list::{AssigneeFilter, ListFilter, PageCursor};
use lb_store::{Store, StoreError};

/// One page plus whether another exists.
pub(crate) struct Page {
    pub items: Vec<Insight>,
    pub has_more: bool,
}

/// Read one page of `ws`'s insights, newest-first, entirely in the engine.
pub(crate) async fn read(
    store: &Store,
    ws: &str,
    filter: &ListFilter,
    tag_allow: Option<&std::collections::HashSet<String>>,
    assignee: Option<&AssigneeFilter>,
    cursor: Option<&PageCursor>,
    limit: usize,
    offset: usize,
) -> Result<Page, StoreError> {
    let mut w = crate::filter_sql::build(filter, tag_allow, assignee, true);

    // The keyset, expressed as SQL: strictly after the cursor in (last_ts DESC, id DESC). The
    // compound form is what makes same-`ts` rows page correctly — a bare `last_ts <` would skip
    // every row sharing the boundary timestamp.
    if let Some(c) = cursor {
        w.preds
            .push("(data.last_ts < $cts OR (data.last_ts = $cts AND data.id < $cid))".into());
        w.bindings.push(("cts".into(), Value::from(c.ts)));
        w.bindings.push(("cid".into(), Value::String(c.id.clone())));
    }

    let where_clause = w.clause();
    let mut bindings = w.bindings;
    bindings.push(("tb".into(), Value::String(OCC_TABLE.to_string())));

    // `+1` is the has-more probe: read one past the page rather than asking the engine a second
    // question. The ordered idioms are SELECTED (`_ts`/`_id`) because SurrealDB requires the ORDER BY
    // expression to appear in the projection (debugging/store/order-by-needs-selected-idiom.md).
    let n = limit.saturating_add(1);
    // `START` is the random-access half: a pager offering "page 7" or "last" cannot use the keyset
    // cursor, which only steps forward. A cursor and an offset together are contradictory, so the
    // cursor wins — it is the precise one — and the offset is ignored rather than compounded.
    let start = if cursor.is_some() { 0 } else { offset };
    let start_clause = if start > 0 {
        format!(" START {start}")
    } else {
        String::new()
    };
    let sql = format!(
        "SELECT data, data.last_ts AS _ts, data.id AS _id FROM type::table($tb){where_clause} \
         ORDER BY _ts DESC, _id DESC LIMIT {n}{start_clause}"
    );

    let mut resp = store.query_ws(ws, &sql, bindings).await?;
    let rows: Vec<Value> = resp
        .take(0)
        .map_err(|e| StoreError::Decode(e.to_string()))?;

    let mut items: Vec<Insight> = rows
        .into_iter()
        .filter_map(|mut v| {
            let inner = v.get_mut("data")?.take();
            serde_json::from_value::<Insight>(inner).ok()
        })
        .collect();

    let has_more = items.len() > limit;
    items.truncate(limit);
    Ok(Page { items, has_more })
}

/// Read EVERY matching row in ONE query — the tally path.
///
/// When a caller asks for counts, the whole matching set must be seen, so there is no LIMIT to push
/// down. What there is no excuse for is reading it in pages: `scan_all` issues one query per 200
/// rows, so a 5,000-row workspace cost 25 round trips to answer one call. This is the same predicates
/// in one statement.
///
/// `status` is deliberately NOT applied: the tally is the per-status breakdown, and the page narrows
/// afterwards in memory over rows already held.
pub(crate) async fn read_all_matching(
    store: &Store,
    ws: &str,
    filter: &ListFilter,
    tag_allow: Option<&std::collections::HashSet<String>>,
    assignee: Option<&AssigneeFilter>,
    cap: usize,
) -> Result<Vec<Insight>, StoreError> {
    let w = crate::filter_sql::build(filter, tag_allow, assignee, false);
    let where_clause = w.clause();
    let mut bindings = w.bindings;
    bindings.push(("tb".into(), Value::String(OCC_TABLE.to_string())));

    // Ordered here so the page below is a slice, not a second sort of the same rows. The cap is the
    // same read-side backstop `scan_all` carried.
    let sql = format!(
        "SELECT data, data.last_ts AS _ts, data.id AS _id FROM type::table($tb){where_clause} \
         ORDER BY _ts DESC, _id DESC LIMIT {cap}"
    );
    let mut resp = store.query_ws(ws, &sql, bindings).await?;
    let rows: Vec<Value> = resp
        .take(0)
        .map_err(|e| StoreError::Decode(e.to_string()))?;
    Ok(rows
        .into_iter()
        .filter_map(|mut v| {
            let inner = v.get_mut("data")?.take();
            serde_json::from_value::<Insight>(inner).ok()
        })
        .collect())
}
