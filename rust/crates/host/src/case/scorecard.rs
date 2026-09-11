//! `rule.scorecard` — "how good is this detector?", per `origin.ref` × site (case-plane scope §7,
//! "Feedback from resolution to detection").
//!
//! `origin.ref` names the producer that raised the finding; `case.resolution` names what a human
//! concluded when the work finished. This verb is the join: scan the workspace's RESOLVED cases,
//! resolve each case's `primary_insight` to its `origin.ref`, and hand the flattened rows to
//! [`lb_cases::scorecard`], which owns every piece of arithmetic.
//!
//! **Read-only, and it stays read-only.** Nothing here writes a record, and the scope's
//! `rule_policy` demotion ("under a precision floor, raise at Info") is an explicit FAST-FOLLOW
//! that is deliberately not built: a number that silently changes how a detector fires is the
//! "silent code" §7 rules out. v1 reports; a person decides.
//!
//! **VIEWER.** A precision score is a read over outcomes a viewer can already see one at a time
//! through `case.get`/`case.list` — it is an aggregate of visible facts, not a new disclosure, and
//! the people who most need to know a detector is crying wolf are exactly the ones who only hold
//! read caps.
//!
//! **Rule 10.** `rule_ref` is `origin.reference` verbatim — an opaque string this file groups by
//! and never parses, prefixes or matches. The verb works identically for a rule, a flow, an agent
//! or an extension tool, because it never asks which it is.
//!
//! ## The read cost — this IS an N+1, and knowingly so
//!
//! One scan of the `case` table, then one point read of the primary insight PER resolved case. On a
//! workspace with thousands of resolved cases that is thousands of point reads for one call. It is
//! acceptable for v1 (the caller is a rules page, not a hot path, and the alternative below is a
//! record change) but it should not stay that way. **The fix is an `origin_ref` echo written onto
//! the case at open** — the same discipline `case_id`, the owner echo and the tag echo already use:
//! host-computed at open, self-healing on the next grouping pass, and it would collapse this verb
//! to a single scan with no reads at all. That is a wave-1 RECORD change and wave 1 is committed,
//! so it is named here rather than done here.

use std::collections::HashMap;

use lb_auth::Principal;
use lb_cases::{ResolvedCase, ResolvedFilter, ScorecardRow};
use lb_mcp::authorize_tool;
use lb_store::Store;

use super::error::CaseSvcError;

/// The `rule_ref` a case is filed under when its primary insight cannot be read.
///
/// A resolved case whose primary insight has since been deleted still HAPPENED: somebody opened it,
/// worked it and closed it with an outcome. Dropping it would quietly shrink a denominator and make
/// the surviving precision a lie about a smaller population than the caller thinks they are seeing,
/// so it is counted here instead, under a name that visibly says "we no longer know which detector
/// this was" — and a `warn!` is logged so the disappearance is traceable.
///
/// *Rejected: skip the row with a warn.* A log line nobody reads is not a disclosure; a row in the
/// output the reader can see is. *Rejected: fail the verb.* One deleted insight must not take the
/// whole scorecard down.
///
/// A producer whose `origin.ref` is literally `"unknown"` would merge into this bucket. That is
/// accepted: the string is opaque (rule 10 forbids reserving a namespace inside it), the collision
/// is harmless — both halves mean "no identifiable detector" — and the alternative is a sentinel
/// this file would have to teach every consumer about.
pub const UNKNOWN_RULE_REF: &str = "unknown";

/// Compute the scorecard for `ws`. Gated by `mcp:rule.scorecard:call` (workspace-first §7).
///
/// `rule_ref` narrows to one producer, `site` to one site, and `since`/`until` bound `resolved_ts`
/// (epoch ms, inclusive on both ends). All four are optional.
// SCOPE: docs/scope/insights/case-plane-scope.md §"Verbs" (rule.scorecard)
pub async fn rule_scorecard(
    store: &Store,
    principal: &Principal,
    ws: &str,
    rule_ref: Option<&str>,
    site: Option<&str>,
    since: Option<u64>,
    until: Option<u64>,
) -> Result<Vec<ScorecardRow>, CaseSvcError> {
    authorize_tool(principal, ws, "rule.scorecard").map_err(|_| CaseSvcError::Denied)?;

    let cases = lb_cases::resolved_cases(store, ws, &ResolvedFilter { site, since, until }).await?;

    // One insight may be the primary of several cases (a hold-down reopen closes and reopens the
    // same finding), so the reads are memoized. It does not stop this being an N+1 — it only stops
    // it being worse than one.
    let mut refs: HashMap<String, String> = HashMap::new();
    let mut rows = Vec::with_capacity(cases.len());
    for case in cases {
        let rule = match refs.get(&case.primary_insight) {
            Some(r) => r.clone(),
            None => {
                let resolved = match lb_insights::get(store, ws, &case.primary_insight).await? {
                    Some(insight) => insight.origin.reference,
                    None => {
                        tracing::warn!(
                            case = %case.id,
                            insight = %case.primary_insight,
                            "rule.scorecard: a resolved case's primary insight no longer exists; \
                             counting it under `{UNKNOWN_RULE_REF}` rather than dropping it"
                        );
                        UNKNOWN_RULE_REF.to_string()
                    }
                };
                refs.insert(case.primary_insight.clone(), resolved.clone());
                resolved
            }
        };
        // The `rule_ref` filter is applied HERE and not in the store read, because the rule is a
        // property of the insight the case points at, not of the case row itself.
        if rule_ref.is_some_and(|want| want != rule) {
            continue;
        }
        let (Some(resolution), Some(resolved_ts)) = (case.resolution, case.resolved_ts) else {
            // `resolved_cases` already refuses these; the destructure is here so this file never
            // fabricates a `0` timestamp to satisfy the type.
            continue;
        };
        rows.push(ResolvedCase {
            rule_ref: rule,
            site: case.site.clone(),
            resolution,
            opened_ts: case.opened_ts,
            resolved_ts,
        });
    }

    Ok(lb_cases::scorecard(&rows))
}
