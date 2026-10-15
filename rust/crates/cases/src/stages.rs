//! `stages` — every case's id and workflow stage in one read, each marked with whether it passes a
//! filter (case-plane scope; the Detections roster's case lens).
//!
//! **Why this exists beside `list`.** The insight roster pages detections on the node and counts them
//! by their CASE's stage ("to action", "actioned", …) and by the queue's case filters. The insights
//! crate knows nothing of cases (its README §7), so the host hands it a case-id → stage map instead,
//! and this is the read that builds it. `list` cannot: it pages, sorts, reads member counts per row,
//! and drops closed cases, when this caller needs every case, closed ones included (a resolved case's
//! detections are what the "resolved" count counts), and nothing else.
//!
//! The filter check is `list`'s own (`matches_filter`), so a case the queue shows under a filter is
//! exactly a case the roster keeps under the same filter.
//!
//! **Unfiltered reads ask for two fields, not the case.** With no filter axis set (the roster's default
//! — every page open), `matches` is `true` for every case and only `id` + `workflow` are read, so the
//! engine projects those two in ONE statement instead of paging every full case record through
//! `scan_all` and decoding it. On ESR's 5,194 cases that is ~0.37 s → a fraction of it per call, and
//! the roster makes several such calls per open.
//!
//! **A number lookup asks for four.** A search by case reference ("INS-005422") filters on the
//! number alone (`seq` / `seq_match`, plus the caller's site scope), so the same projection carries
//! `seq` and `site` too and the check runs on those, instead of decoding every case to compare one
//! number: on ESR that was ~3 s a call. Any other filtered read still decodes whole cases, because
//! `matches_filter` reads their other fields.
//!
//! One responsibility: the stage of every case, and whether it passes the filter.

use lb_store::{scan_all, Store};
use serde::Deserialize;

use crate::case::Workflow;
use crate::error::CasesError;
use crate::list::{matches_filter, unwrap_case, ListFilter};

/// One case, reduced to what the roster's case lens reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaseStage {
    pub id: String,
    pub workflow: Workflow,
    /// Whether the case passes the filter [`stages`] was given (always `true` for an empty one).
    pub matches: bool,
}

/// Every case in `ws` with its stage, closed ones included, each marked against `filter` evaluated at
/// `now` (the `snoozed` axis is relative to it). One scan of the case table.
///
/// `party` is refused, as `list` refuses it: the plane that could answer it does not exist yet, and
/// a filter that silently matched everything would make the roster lie.
// SCOPE: docs/scope/insights/case-plane-scope.md §"Verbs" (case.list)
pub async fn stages(
    store: &Store,
    ws: &str,
    filter: &ListFilter,
    now: u64,
) -> Result<Vec<CaseStage>, CasesError> {
    if filter.party.is_some() {
        return Err(CasesError::BadInput(
            "filter `party` needs the case_request plane, which is wave 2 of the case-plane scope \
             — refused rather than silently matching every case"
                .into(),
        ));
    }
    if is_unfiltered(filter) {
        return projected(store, ws, |_| true).await;
    }
    if is_number_only(filter) {
        return projected(store, ws, |r| number_matches(r, filter)).await;
    }
    Ok(scan_all(store, ws, crate::case::TABLE)
        .await?
        .into_iter()
        .filter_map(|row| unwrap_case(row.data))
        .map(|case| CaseStage {
            matches: matches_filter(&case, filter, now),
            id: case.id,
            workflow: case.workflow,
        })
        .collect())
}

/// No axis set: every case matches, so nothing but `id` and `workflow` is needed. Decided by the
/// filter's own serialized shape — every axis is skip-if-none — so a new axis can never be missed here.
fn is_unfiltered(filter: &ListFilter) -> bool {
    serde_json::to_value(filter)
        .map(|v| v.as_object().is_some_and(|o| o.is_empty()))
        .unwrap_or(false)
        && filter.sites_allowed.is_none()
}

/// Only the number axes are set (`seq`, `seq_match`), with or without the caller's site scope: the
/// projection below can answer it. Decided, like `is_unfiltered`, from the serialized shape, so an
/// axis added later is never silently ignored here.
fn is_number_only(filter: &ListFilter) -> bool {
    serde_json::to_value(filter)
        .ok()
        .and_then(|v| v.as_object().cloned())
        .is_some_and(|o| !o.is_empty() && o.keys().all(|k| k == "seq" || k == "seq_match"))
}

/// The number axes and the site scope, on the projected row — the same checks `matches_filter` makes.
fn number_matches(r: &StageRow, f: &ListFilter) -> bool {
    if let Some(n) = f.seq {
        if r.seq != Some(n) {
            return false;
        }
    }
    if let Some(m) = &f.seq_match {
        if !r.seq.is_some_and(|n| m.matches(n)) {
            return false;
        }
    }
    if let Some(allowed) = &f.sites_allowed {
        if !r.site.as_deref().is_some_and(|s| allowed.contains(s)) {
            return false;
        }
    }
    true
}

/// The row the projected reads ask for: the stage, and what a number lookup checks.
#[derive(Deserialize)]
struct StageRow {
    id: Option<String>,
    workflow: Option<Workflow>,
    seq: Option<u64>,
    site: Option<String>,
}

/// `id`, `workflow`, `seq` and `site` of every case, projected by the engine in one statement, each
/// marked by `matches`. A row missing `id` or `workflow` (a record that would not decode as a case) is
/// skipped, as `unwrap_case` skips it on the full read.
async fn projected(
    store: &Store,
    ws: &str,
    matches: impl Fn(&StageRow) -> bool,
) -> Result<Vec<CaseStage>, CasesError> {
    let sql =
        "SELECT data.id AS id, data.workflow AS workflow, data.seq AS seq, data.site AS site \
               FROM type::table($tb)";
    let mut resp = store
        .query_ws(
            ws,
            sql,
            vec![(
                "tb".into(),
                serde_json::Value::String(crate::case::TABLE.into()),
            )],
        )
        .await?;
    let rows: Vec<serde_json::Value> = resp
        .take(0)
        .map_err(|e| lb_store::StoreError::Decode(e.to_string()))?;
    Ok(rows
        .into_iter()
        .filter_map(|v| serde_json::from_value::<StageRow>(v).ok())
        .filter_map(|r| {
            let matches = matches(&r);
            Some(CaseStage {
                id: r.id?,
                workflow: r.workflow?,
                matches,
            })
        })
        .collect())
}
