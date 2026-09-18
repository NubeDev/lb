//! `backfill_case_facets` — repair the facet echo on cases opened before a facet key existed.
//!
//! The echo in [`super::facets`] runs at OPEN and only at open, which is correct for a facet that
//! was already there (re-deriving on every raise would move a case between queues under an operator
//! mid-triage). It leaves one gap: when a NEW facet key ships — a pack starts tagging `subsystem` —
//! every case opened before that carries `None` for ever, because nothing re-reads the primary
//! insight after open. The operator sees a filter axis that is real, enabled, and empty across the
//! whole existing estate.
//!
//! This is the migration for that, and it is the same shape as [`super::reconcile`]: derive the
//! answer from the durable record rather than remember it, fill only what is missing, and be safe to
//! run on a timer for ever. It reuses [`super::facets::facets_of`] — the SAME function the open path
//! echoes through — so a backfilled case and a freshly opened one cannot disagree about what the
//! insight said.
//!
//! **Gaps only, never a rewrite.** `lb_cases::refresh_facets` will not overwrite a facet the case
//! already carries, so this cannot reclassify live work; it can only fill blanks. A second pass
//! returns `0`.

use std::sync::Arc;

use lb_cases::FacetFill;
use lb_store::scan_all;

use super::error::CaseSvcError;
use super::facets::facets_of;
use crate::boot::Node;

/// The actor stamped on the history event. Not a person: nobody chose this, an upgrade did.
const ACTOR: &str = "system:facet-backfill";

/// Fill missing facet echoes on every open case in `ws`. Returns how many cases were repaired.
// SCOPE: docs/scope/insights/case-plane-scope.md §"Reactors" (the reconcile backstop + backfill)
pub async fn backfill_case_facets(
    node: &Arc<Node>,
    ws: &str,
    now: u64,
) -> Result<usize, CaseSvcError> {
    let mut repaired = 0usize;
    for case in open_cases(node, ws).await? {
        // Nothing to fill — cheapest check first, and it is the steady state once the pass has run.
        if case.category.is_some()
            && case.site.is_some()
            && case.scope.is_some()
            && case.subsystem.is_some()
        {
            continue;
        }
        let Some(insight) = lb_insights::get(&node.store, ws, &case.primary_insight).await? else {
            // A case whose primary is gone is a different repair than this one; reconcile owns it.
            continue;
        };
        let facets = facets_of(&insight);
        let fill = FacetFill {
            category: facets.category,
            site: facets.site,
            scope: facets.scope,
            subsystem: facets.subsystem,
        };
        match lb_cases::refresh_facets(&node.store, ws, &case.id, &fill, ACTOR, now).await {
            // `None` means nothing was missing that the insight could answer — not a failure.
            Ok(Some(_)) => repaired += 1,
            Ok(None) => {}
            Err(e) => {
                // One unrepairable case must not stop the pass; the next tick retries it.
                tracing::warn!(ws, case_id = %case.id, error = %e,
                    "case facet backfill: refresh failed");
            }
        }
    }
    Ok(repaired)
}

/// Every OPEN case in the workspace. Closed cases are excluded here as well as in the crate verb:
/// what a client was told at resolution does not get rewritten afterwards.
async fn open_cases(node: &Arc<Node>, ws: &str) -> Result<Vec<lb_cases::Case>, CaseSvcError> {
    let rows = scan_all(&node.store, ws, lb_cases::CASE_TABLE).await?;
    Ok(rows
        .into_iter()
        .filter_map(|row| unwrap_case(row.data))
        .filter(|c: &lb_cases::Case| !c.closed)
        .collect())
}

/// Unwrap the `{ data, rev }` write envelope and decode. A row that will not decode is skipped: one
/// bad record must not stop the whole workspace being repaired (the `reconcile` precedent).
fn unwrap_case(row: serde_json::Value) -> Option<lb_cases::Case> {
    let inner = match row {
        serde_json::Value::Object(mut obj) => {
            obj.remove("data").unwrap_or(serde_json::Value::Object(obj))
        }
        other => other,
    };
    serde_json::from_value(inner).ok()
}
