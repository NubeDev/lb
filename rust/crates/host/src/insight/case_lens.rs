//! The case lens on `insight.list` — the request half: what the caller asks (the case queue's filters,
//! the stages picked), resolved here against the case plane into the `lb_insights::CaseScope` the
//! insights crate filters and counts with (case-plane scope; the Detections roster).
//!
//! The insights crate knows no cases (README §7), so this is where the two planes meet, as tag facets
//! meet in `resolve_filter`. One scan of the case table per request that carries a lens; the
//! Detections roster sends one on every page, and there are far fewer cases than detections.
//!
//! **A lens reads the case plane, so it needs the case plane's read grant.** `mcp:case.list:call` is
//! checked here, beside the `insight.list` gate the verb already ran: a caller who may not list cases
//! must not learn their stages through detection counts either.
//!
//! One responsibility: validate and resolve the case lens.

use std::collections::HashSet;

use lb_auth::Principal;
use lb_cases::Workflow;
use lb_insights::{CaseScope, NO_CASE};
use lb_mcp::authorize_tool;
use lb_store::Store;
use serde::Deserialize;

use super::error::InsightSvcError;

/// The `case` object an `insight.list` call may carry beside its filter.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct CaseLens {
    /// The case queue's own filters (`case.list`'s axes); a detection passes when its case does. An
    /// empty filter narrows nothing.
    #[serde(default)]
    pub filter: lb_cases::ListFilter,
    /// Keep only detections whose case is at one of these stages: lb's workflow words
    /// (`to_action`, `actioned`, `resolved`) or `none` for no case. Empty ⇒ every
    /// stage. The reply's `case_counts` tallies every stage regardless of this.
    #[serde(default)]
    pub stages: Vec<String>,
    /// The caller's logical now, for the `snoozed` and `sla` axes (no wall clock in the verb).
    #[serde(default)]
    pub now: u64,
}

/// The retired fourth stage's word, still accepted on the wire as `actioned`.
const RETIRED_PO: &str = "waiting_on_po";

/// Every stage, in lb's workflow order.
const WORKFLOW_ORDER: [Workflow; 3] = [Workflow::ToAction, Workflow::Actioned, Workflow::Resolved];

/// The workflow word as it travels on the wire (`to_action`, …) — serde's own spelling, so this
/// file and `lb_cases` can never disagree about it.
fn stage_word(w: Workflow) -> String {
    serde_json::to_value(w)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_default()
}

/// Resolve `lens` into the scope `lb_insights::list` applies. Refuses an unknown stage word rather
/// than matching nothing with it, and a caller without `case.list`.
pub(super) async fn resolve_case_lens(
    store: &Store,
    principal: &Principal,
    ws: &str,
    lens: &CaseLens,
) -> Result<CaseScope, InsightSvcError> {
    authorize_tool(principal, ws, "case.list").map_err(|_| InsightSvcError::Denied)?;

    let stages = if lens.stages.is_empty() {
        None
    } else {
        let known: HashSet<String> = WORKFLOW_ORDER
            .into_iter()
            .map(stage_word)
            .chain([NO_CASE.to_string()])
            .collect();
        // `waiting_on_po` is a retired stage (it reads as `actioned` now); a caller built before the
        // retirement still sends it beside `actioned`, so it is taken as `actioned`, not refused.
        let words: Vec<String> = lens
            .stages
            .iter()
            .map(|s| {
                if s == RETIRED_PO {
                    "actioned".to_string()
                } else {
                    s.clone()
                }
            })
            .collect();
        if let Some(bad) = words.iter().find(|s| !known.contains(*s)) {
            return Err(InsightSvcError::BadInput(format!(
                "case stage {bad:?} is not one of {known:?}"
            )));
        }
        Some(words.into_iter().collect())
    };

    // `pinned` reads the caller's own pins (`case.pin`); the subject is the session's, never the wire's.
    let mut filter = lens.filter.clone();
    filter.pinned_for = Some(principal.sub().to_string());
    let cases = lb_cases::stages(store, ws, &filter, lens.now).await?;
    // An empty filter serializes to `{}` (every axis is skip-if-none): no allowlist at all, rather
    // than one listing every case, which would wrongly drop the detections that have no case.
    let filtered = serde_json::to_value(&lens.filter)
        .map(|v| v.as_object().is_some_and(|o| !o.is_empty()))
        .unwrap_or(false);
    let allow = filtered.then(|| {
        cases
            .iter()
            .filter(|c| c.matches)
            .map(|c| c.id.clone())
            .collect()
    });
    // The three stage words once, not one serialisation per case (thousands of them).
    let words: Vec<(Workflow, String)> = WORKFLOW_ORDER
        .into_iter()
        .map(|w| (w, stage_word(w)))
        .collect();
    let word = |w: Workflow| {
        words
            .iter()
            .find(|(k, _)| *k == w)
            .map(|(_, s)| s.clone())
            .unwrap_or_else(|| stage_word(w))
    };
    Ok(CaseScope {
        stage_of: cases
            .iter()
            .map(|c| (c.id.clone(), word(c.workflow)))
            .collect(),
        allow,
        stages,
        // lb's own workflow order, the order a `case_stage` sort ranks by.
        stage_order: WORKFLOW_ORDER.into_iter().map(stage_word).collect(),
    })
}
