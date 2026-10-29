//! The case lens's column sorts (`case_stage`, `case_status`, `case_owner`, `case_ref`), done in
//! memory over the matched rows rather than in the statement.
//!
//! **Why not SQL.** Each of these is a fact of the CASE, not of the detection, so the statement had
//! to look every row's `case_id` up in a bound list or object of every known case. SurrealDB walks
//! (or copies) that binding per row and per mention, so on ESR (thousands of cases) a 50-row page
//! sorted by status or ref took ~5 s against ~1 s for any other column. Here it is a hash lookup.
//!
//! **One order for both read paths.** `list` sends every lens-sorted read through the matched-set
//! path (the tally path, whether or not counts are asked), so page 1 and page 2 are sliced from the
//! same sort and can never skip or repeat a row between them.
//!
//! Same rules as `sort_sql`: empty values (no case, unranked word, nobody holds it, no number) sort
//! last in BOTH directions; ties fall back to the default order, newest first.
//!
//! One responsibility: order detections by a case lens fact.

use std::cmp::Ordering;

use crate::case_scope::CaseScope;
use crate::insight::Insight;
use crate::sort_sql::SortSpec;

/// The lens sources, all sorted here.
const LENS_SOURCES: [&str; 4] = ["case_stage", "case_status", "case_owner", "case_ref"];

/// The lens source `spec` sorts by, if it is one (a lens source stands alone, `sort_sql::validate`).
pub(crate) fn lens_source(spec: Option<&SortSpec>) -> Option<&str> {
    match spec?.by.as_slice() {
        [one] if LENS_SOURCES.contains(&one.as_str()) => Some(one.as_str()),
        _ => None,
    }
}

/// One row's sort key. Within one sort every key is the same variant.
#[derive(PartialEq, Eq, PartialOrd, Ord)]
enum Key<'a> {
    Rank(usize),
    Text(&'a str),
    Num(u64),
}

fn rank(order: &[String], label: &str) -> Option<usize> {
    order.iter().position(|l| l == label)
}

fn key<'a>(scope: &'a CaseScope, src: &str, insight: &Insight) -> Option<Key<'a>> {
    let case = insight.case_id.as_deref()?;
    match src {
        "case_stage" => rank(&scope.stage_order, scope.stage_of.get(case)?).map(Key::Rank),
        "case_status" => rank(&scope.status_order, scope.status_of.get(case)?).map(Key::Rank),
        "case_owner" => scope
            .owner_of
            .get(case)
            .filter(|o| !o.is_empty())
            .map(|o| Key::Text(o)),
        "case_ref" => scope.seq_of.get(case).map(|n| Key::Num(*n)),
        _ => None,
    }
}

/// Sort `items` by the lens fact `src`, `desc` or not; empty keys last, then newest first.
pub(crate) fn sort(items: &mut [Insight], src: &str, desc: bool, scope: &CaseScope) {
    items.sort_by(|a, b| {
        let newest = (b.last_ts, &b.id).cmp(&(a.last_ts, &a.id));
        match (key(scope, src, a), key(scope, src, b)) {
            (Some(x), Some(y)) => {
                let o = x.cmp(&y);
                if desc { o.reverse() } else { o }.then(newest)
            }
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (None, None) => newest,
        }
    });
}
