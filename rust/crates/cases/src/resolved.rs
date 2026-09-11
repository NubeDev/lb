//! `resolved_cases` — every RESOLVED case in a workspace, optionally windowed on `resolved_ts`
//! (case-plane scope §7). The read half of the scorecard; [`crate::scorecard`] is the arithmetic.
//!
//! Why not `crate::list` with `include_closed: true`? Because that verb is a *lane* — it pages
//! (`MAX_CASE_PAGE`), it sorts by deadline, and it reads a member count per row. A scorecard wants
//! EVERY resolved case and none of the member counts: truncating at 200 would silently compute a
//! precision over the first 200 rows and present it as the rule's precision, which is the exact
//! shape of a lie this scope is trying to remove. So this is its own read, deliberately unpaged.
//!
//! Read-only, and rule 10 holds: `site` is an opaque string compared for equality, never parsed.

use lb_store::{scan_all, Store};

use crate::case::{Case, TABLE};
use crate::error::CasesError;
use crate::list::unwrap_case;

/// The window + site filter a scorecard read may narrow by. All axes optional and ANDed.
#[derive(Debug, Clone, Default)]
pub struct ResolvedFilter<'a> {
    /// Only cases at this site. `None` means every site INCLUDING the cases that have none.
    pub site: Option<&'a str>,
    /// Only cases resolved at or after this logical timestamp (epoch ms).
    pub since: Option<u64>,
    /// Only cases resolved at or before this logical timestamp (epoch ms).
    pub until: Option<u64>,
}

/// Every case in `ws` that closed with a resolution and falls inside `filter`.
///
/// A case is "resolved" here iff it carries BOTH a `resolution` and a `resolved_ts` — the pair
/// [`crate::workflow`] writes together. A row missing either cannot contribute an outcome or a
/// duration, so counting it would inflate `raised` with a case nothing can be said about.
// SCOPE: docs/scope/insights/case-plane-scope.md §"Verbs" (rule.scorecard)
pub async fn resolved_cases(
    store: &Store,
    ws: &str,
    filter: &ResolvedFilter<'_>,
) -> Result<Vec<Case>, CasesError> {
    Ok(scan_all(store, ws, TABLE)
        .await?
        .into_iter()
        // `scan` returns the whole `{ data, rev }` write envelope (unlike `read`/`list`, which hand
        // back the inner value), so the row is unwrapped TWICE — once off the `Row`, once out of
        // the envelope. Getting this wrong yields an empty, green, error-free result.
        .filter_map(|row| unwrap_case(row.data))
        .filter(|c| matches(c, filter))
        .collect())
}

fn matches(case: &Case, filter: &ResolvedFilter<'_>) -> bool {
    let (Some(_), Some(resolved_ts)) = (case.resolution, case.resolved_ts) else {
        return false;
    };
    if let Some(site) = filter.site {
        if case.site.as_deref() != Some(site) {
            return false;
        }
    }
    if filter.since.is_some_and(|s| resolved_ts < s) {
        return false;
    }
    if filter.until.is_some_and(|u| resolved_ts > u) {
        return false;
    }
    true
}
