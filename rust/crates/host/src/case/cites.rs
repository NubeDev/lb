//! "Is some OPEN case already waiting for this finding?" — the straggler lookup that makes the
//! **verdict-first** ordering work (case-plane scope, "case-group" reactor).
//!
//! A scheduled rule produces the two citation orderings in whichever order its queries finish:
//!
//!   - **verdict-last** — the cited findings already sit in `single` cases when the verdict record
//!     arrives. The grouping pass MERGES those singles into the verdict case. Handled in
//!     `group.rs`; nothing here is needed.
//!   - **verdict-first** — the verdict record arrives naming findings that do not exist yet. Its
//!     `explains[]` entries resolve to nothing and are skipped. When a straggler is finally raised
//!     minutes later, the ordinary grouping path would open it a `single` case and the verdict
//!     grouping would be silently, permanently wrong.
//!
//! This file closes that second window: before opening a `single`, ask whether an open verdict case
//! ALREADY cites this finding, and if so join it instead. The lookup walks the open verdict cases'
//! members and re-reads their bodies through [`super::verdict`], so the citation grammar is read in
//! exactly one place and this file learns nothing new about `body`.
//!
//! The walk is bounded and only ever runs on the "this finding has no case" path — the reconcile
//! loop's path and a first raise, never a re-raise of an already-grouped finding.

use lb_cases::{Case, Grouping};
use lb_store::Store;

use super::error::CaseSvcError;
use super::verdict::resolve_verdict;

/// The open verdict case that already cites `insight_id`, if any.
///
/// Matches on the RESOLVED citation (an insight id), so it agrees exactly with what the grouping
/// pass would have done had the records arrived in the other order — the two orderings converge on
/// one case rather than on two that merely look similar.
pub(super) async fn find_citing_case(
    store: &Store,
    ws: &str,
    insight_id: &str,
) -> Result<Option<Case>, CaseSvcError> {
    for case in open_verdict_cases(store, ws).await? {
        let members = lb_cases::members(store, ws, &case.id, lb_cases::MAX_MEMBER_PAGE, None)
            .await?
            .items;
        for member in members {
            let Some(insight) = lb_insights::get(store, ws, &member.insight_id).await? else {
                continue;
            };
            let Some(verdict) = resolve_verdict(store, ws, &insight).await? else {
                continue;
            };
            if verdict.primary == insight_id || verdict.explained.iter().any(|e| e == insight_id) {
                return Ok(Some(case));
            }
        }
    }
    Ok(None)
}

/// Every OPEN case with `grouping: verdict`. The only cases that can be waiting for a straggler —
/// a `single` case cites exactly one finding and cannot be short of another.
async fn open_verdict_cases(store: &Store, ws: &str) -> Result<Vec<Case>, CaseSvcError> {
    let page = lb_cases::list(
        store,
        ws,
        &lb_cases::ListQuery {
            lane: lb_cases::Lane::Watching,
            filter: lb_cases::ListFilter::default(),
            now: 0,
            include_closed: false,
            limit: lb_cases::MAX_CASE_PAGE,
        },
        &Default::default(),
    )
    .await?;
    Ok(page
        .items
        .into_iter()
        .map(|row| row.case)
        .filter(|c| c.grouping == Grouping::Verdict)
        .collect())
}
