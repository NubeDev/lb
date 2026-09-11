//! `find_by_insight` — the OTHER direction of the citation edge: "which case holds this insight?"
//! (case-plane scope).
//!
//! This is where the exclusivity invariant is actually checked. **Every open insight is in exactly
//! one OPEN case** — so [`find_open_case_for_insight`] returning `Some` is the whole test
//! [`crate::member_add`] applies before it writes, and the case-group reactor's "have I already
//! grouped this?" fast path.
//!
//! An insight may legitimately appear in MANY closed cases (a chiller that short-cycles every
//! summer is one detection key and three jobs over three years), which is exactly why the filter is
//! on the case's `closed` flag and not on membership alone.

use lb_store::{list as store_list, Store};

use crate::case::Case;
use crate::case_member::{CaseMember, TABLE};
use crate::error::CasesError;

/// Every membership row citing `insight_id`, across open AND closed cases, newest-first by `ts`.
pub async fn memberships_of_insight(
    store: &Store,
    ws: &str,
    insight_id: &str,
) -> Result<Vec<CaseMember>, CasesError> {
    let rows = store_list(store, ws, TABLE, "insight_id", insight_id).await?;
    let mut items: Vec<CaseMember> = rows
        .into_iter()
        .filter_map(|v| serde_json::from_value(v).ok())
        .collect();
    items.sort_by_key(|m| std::cmp::Reverse(m.ts));
    Ok(items)
}

/// The ONE open case holding `insight_id`, with its membership row — or `None` when the insight is
/// ungrouped (the state the reconcile backstop exists to end).
///
/// If the invariant has somehow been broken (a crash between a remove and an add), this returns the
/// most recently added open case rather than failing: a read that refuses to answer would take the
/// roster down over an inconsistency the next reconcile pass repairs.
pub async fn find_open_case_for_insight(
    store: &Store,
    ws: &str,
    insight_id: &str,
) -> Result<Option<(Case, CaseMember)>, CasesError> {
    for member in memberships_of_insight(store, ws, insight_id).await? {
        let Some(case) = crate::get::get(store, ws, &member.case_id).await? else {
            // A membership whose case was hard-deleted. Skip it — the row is debris, not a claim.
            continue;
        };
        if !case.closed {
            return Ok(Some((case, member)));
        }
    }
    Ok(None)
}

/// The most recently CLOSED case that cited `insight_id` — the hold-down reactor's question ("did
/// the last repair hold?"). Ordered by the close time, not the membership time: the case that
/// closed last is the repair being judged.
pub async fn last_closed_case_for_insight(
    store: &Store,
    ws: &str,
    insight_id: &str,
) -> Result<Option<Case>, CasesError> {
    let mut best: Option<Case> = None;
    for member in memberships_of_insight(store, ws, insight_id).await? {
        let Some(case) = crate::get::get(store, ws, &member.case_id).await? else {
            continue;
        };
        if !case.closed {
            continue;
        }
        let better = match (&best, case.resolved_ts) {
            (None, _) => true,
            (Some(b), Some(ts)) => b.resolved_ts.is_none_or(|bt| ts > bt),
            (Some(_), None) => false,
        };
        if better {
            best = Some(case);
        }
    }
    Ok(best)
}
