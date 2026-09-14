//! The per-status tally behind the insights stat tiles (insights umbrella scope).
//!
//! # Why it exists
//!
//! The tiles show four integers — total / open / acked / resolved. Before this, `insight.list` was
//! the only read, so a caller counting by status had to fetch every matching ROW and tally it
//! client-side, once per tile. Measured on a live 200-insight workspace, one page load issued
//! **four** `insight.list` calls returning **709 KB** to render four numbers, and each one
//! re-resolved the same tag facet and re-scanned the table before throwing the rows away.
//!
//! Both paths here are for [`crate::list`]. [`tally`] counts rows it is fetching anyway; [`count`]
//! is one `GROUP BY` for `limit: 0`, where the caller wants the four integers and no records.
//!
//! # The one deliberate asymmetry with [`crate::list`]
//!
//! [`ListFilter::status`] is **ignored** here. The reply IS the per-status breakdown, so honouring a
//! status constraint would make three of the four numbers zero by construction. Every other axis
//! composes exactly as it does in `list`, so a tile and its list cannot disagree about what they
//! count.

use std::collections::HashSet;

use lb_store::Store;
use serde_json::Value;

use crate::error::InsightsError;
use crate::insight::OCC_TABLE;
use crate::list::{AssigneeFilter, ListFilter};

/// The four numbers the stat tiles show. `total` is the sum of the three states — carried
/// explicitly rather than left to the caller so every reader adds it up the same way.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct StatusCounts {
    pub total: u64,
    pub open: u64,
    pub acked: u64,
    pub resolved: u64,
}

/// One `GROUP BY status` row: the lowercase status literal and its count.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct TallyRow {
    status: Option<String>,
    n: u64,
}
lb_store::surreal_value_via_serde!(TallyRow);

/// Tally an ALREADY-MATCHED set of insights, in memory.
///
/// This is the path [`crate::list`] uses whenever it is fetching rows anyway: it has just scanned
/// and filtered the table, so counting them is one more pass over data already in hand. Asking the
/// database to re-derive that is a second scan for an answer we are holding.
///
/// `rows` must be the set matching every filter axis EXCEPT `status` — the reply IS the per-status
/// breakdown, so narrowing by status first would zero three of the four numbers. That is the same
/// contract the SQL in [`count`] keeps, and the count tests pin the two against each other.
pub fn tally(rows: &[crate::insight::Insight]) -> StatusCounts {
    let mut c = StatusCounts {
        total: rows.len() as u64,
        ..StatusCounts::default()
    };
    for r in rows {
        match r.status {
            crate::status::Status::Open => c.open += 1,
            crate::status::Status::Acked => c.acked += 1,
            crate::status::Status::Resolved => c.resolved += 1,
        }
    }
    c
}

/// Count insights in `ws` per status, under the same filter axes [`crate::list`] accepts.
///
/// The SQL path — for `insight.list` with `limit: 0`, where the caller wants the tally and no
/// records at all. When rows are being fetched anyway, use [`tally`] instead.
///
/// `tag_allow` and `assignee` are the host-resolved arguments, identical in meaning to `list`'s:
/// the crate is tag-graph- and membership-agnostic, so the service layer resolves them first.
pub async fn count(
    store: &Store,
    ws: &str,
    filter: &ListFilter,
    tag_allow: Option<&HashSet<String>>,
    assignee: Option<&AssigneeFilter>,
) -> Result<StatusCounts, InsightsError> {
    // The predicates come from the ONE builder `list`'s pushdown also uses — `include_status:
    // false` because this reply IS the per-status breakdown.
    let w = crate::filter_sql::build(filter, tag_allow, assignee, false);
    let where_clause = w.clause();
    let mut bindings = w.bindings;
    bindings.push(("tb".into(), Value::String(OCC_TABLE.to_string())));

    // `status` is the SELECTED idiom the GROUP BY names — SurrealDB requires the grouped expression
    // to appear in the projection (debugging/store/order-by-needs-selected-idiom.md, same rule).
    let sql = format!(
        "SELECT data.status AS status, count() AS n FROM type::table($tb){where_clause} \
         GROUP BY status"
    );

    let mut resp = store.query_ws(ws, &sql, bindings).await?;
    let rows: Vec<TallyRow> = resp
        .take(0)
        .map_err(|e| lb_store::StoreError::Decode(e.to_string()))?;

    let mut c = StatusCounts::default();
    for row in rows {
        match row.status.as_deref() {
            Some("open") => c.open = row.n,
            Some("acked") => c.acked = row.n,
            Some("resolved") => c.resolved = row.n,
            // A row whose status is absent or unknown still exists, so it still counts toward the
            // total — dropping it would make the tiles disagree with the roster.
            _ => {}
        }
        c.total += row.n;
    }
    Ok(c)
}
