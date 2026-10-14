//! `list` — the three work lanes and their filters (case-plane scope).
//!
//! **Sorted `due_at` ASCENDING, then severity DESCENDING.** That order is the whole product
//! argument in one line: the queue is ranked by *when the commitment expires*, not by how loud the
//! detection was — a `warning` due this afternoon is above a `critical` due next month, because the
//! first one is about to become a broken promise. Severity only breaks ties. A case with no
//! deadline sorts last: nothing is more urgent than a stated deadline, including an un-clocked
//! `critical`.
//!
//! **Returns the member COUNT, never the members.** A storm case cites hundreds of detections; a
//! roster that carried them would make every lane page cost what the drawer costs. `case.members`
//! is the paged read for that.
//!
//! This crate resolves no identities. The `Mine` lane matches against a `subjects` set the HOST
//! computed — the person plus every team they belong to — reusing the ONE definition of "mine" the
//! shipped `SubFilter.assignee: "me"` semantics already established
//! (`lb_insights::OwnerSubjects` / `host/src/insight/assignee.rs::owner_subjects_for`). A second
//! definition here is how "my work" silently stops showing team-owned jobs.

use std::collections::BTreeSet;

use lb_store::Store;
use serde::{Deserialize, Serialize};

use crate::case::{severity_rank, Case, WaitingOn, Workflow};
use crate::error::CasesError;
use crate::list_read::read_cases;
use crate::member_counts::member_counts;
use crate::sla_state::{sla_state, SlaState, DEFAULT_DUE_SOON_MS};

/// Which work queue the caller is asking for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Lane {
    /// Assigned to the caller or to a team they are on.
    Mine,
    /// Blocked on somebody — `waiting_on_po`, or any case with a `waiting_on` party set. The lane
    /// that exists because "waiting" is the most common honest answer and it needs its own screen.
    Waiting,
    /// Everything else the caller can see — the unassigned and other people's work. Named
    /// `watching` because that is what looking at it is: oversight, not ownership.
    Watching,
}

/// The filter axes, all optional and all ANDed. Every value is an opaque workspace string except
/// the two closed enums (rule 10: this crate knows no category, site or party VALUE).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ListFilter {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub site: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subsystem: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workflow: Option<Workflow>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub waiting_on: Option<WaitingOn>,
    /// The external party a case is waiting on. **Not yet answerable**: the `case_request` plane
    /// that records which party was asked what is wave 2 of this scope. Supplying it is REFUSED
    /// rather than silently ignored — a filter that quietly matches everything is how a queue lies.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub party: Option<String>,
    /// `Some(true)` = only parked cases, `Some(false)` = only live ones, `None` = both. Evaluated
    /// against `query.now`, so an expired snooze reads as live without a sweep having run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snoozed: Option<bool>,
    /// Only cases in this SLA state (breached, due soon, on track, no deadline, met). Evaluated
    /// against `query.now`, as `snoozed` is: see [`crate::sla_state`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sla: Option<SlaState>,
    /// The due-soon window for the `sla` axis, in milliseconds before `due_at`. Absent ⇒
    /// [`DEFAULT_DUE_SOON_MS`]. Read only with `sla`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub due_soon_ms: Option<u64>,
    /// The sites the CALLER may see (entity-scoped data), set by the host from server state and
    /// never read from the wire (`serde(skip)` — a client cannot send it). `None` = unrestricted;
    /// `Some` = a case must carry one of these sites (a case with no site is hidden).
    #[serde(skip)]
    pub sites_allowed: Option<BTreeSet<String>>,
    /// Only these cases, by id — for a caller that already knows which cases it needs (a roster
    /// labelling the rows on screen with their case's stage) and should not read whole lanes to find
    /// them. At most [`MAX_CASE_PAGE`] ids; more is refused rather than silently truncated.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ids: Option<Vec<String>>,
}

/// One row of the roster: the case plus its member count.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CaseRow {
    #[serde(flatten)]
    pub case: Case,
    /// How many insights this case cites. The count, never the members.
    pub member_count: usize,
}

/// A lane request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListQuery {
    pub lane: Lane,
    #[serde(default)]
    pub filter: ListFilter,
    /// The caller's logical now — the `snoozed` axis is relative to it (no wall clock here).
    #[serde(default)]
    pub now: u64,
    /// Include closed cases. Off by default: a lane is a work queue, and closed work is not in it.
    #[serde(default)]
    pub include_closed: bool,
    /// Rows per page. Clamped to [`MAX_CASE_PAGE`].
    #[serde(default = "default_limit")]
    pub limit: usize,
}

fn default_limit() -> usize {
    50
}

/// The most rows one lane page may carry.
pub const MAX_CASE_PAGE: usize = 200;

/// One page of a lane.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ListPage {
    pub items: Vec<CaseRow>,
    /// How many cases matched in total, before the page limit — the lane's badge count.
    pub total: usize,
}

/// List the cases in `ws` matching `query`.
///
/// `subjects` is the host-resolved "who am I" set for the [`Lane::Mine`] lane (the caller's own
/// subject plus each team they are on).
// SCOPE: docs/scope/insights/case-plane-scope.md §"Verbs" (case.list)
pub async fn list(
    store: &Store,
    ws: &str,
    query: &ListQuery,
    subjects: &BTreeSet<String>,
) -> Result<ListPage, CasesError> {
    if query.filter.party.is_some() {
        return Err(CasesError::BadInput(
            "filter `party` needs the case_request plane, which is wave 2 of the case-plane scope \
             — refused rather than silently matching every case"
                .into(),
        ));
    }

    if query
        .filter
        .ids
        .as_ref()
        .is_some_and(|ids| ids.len() > MAX_CASE_PAGE)
    {
        return Err(CasesError::BadInput(format!(
            "filter `ids` takes at most {MAX_CASE_PAGE} ids — ask in pages"
        )));
    }

    let mut matched: Vec<Case> =
        read_cases(store, ws, query.include_closed, query.filter.ids.as_deref())
            .await?
            .into_iter()
            .filter(|c| matches(c, query, subjects))
            .collect();

    // `due_at` ascending with `None` last, then severity DESCENDING, then the open time so the
    // order is total (two cases with the same deadline and severity must not shuffle between
    // pages).
    matched.sort_by(|a, b| {
        let due = a
            .due_at
            .map(|d| (0u8, d))
            .unwrap_or((1, 0))
            .cmp(&b.due_at.map(|d| (0u8, d)).unwrap_or((1, 0)));
        due.then_with(|| severity_rank(&b.severity).cmp(&severity_rank(&a.severity)))
            .then_with(|| a.opened_ts.cmp(&b.opened_ts))
            .then_with(|| a.id.cmp(&b.id))
    });

    let total = matched.len();
    matched.truncate(query.limit.clamp(1, MAX_CASE_PAGE));

    // The counts are read only for the rows that actually ship, not for every match, and in one read.
    let ids: Vec<String> = matched.iter().map(|c| c.id.clone()).collect();
    let counts = member_counts(store, ws, &ids).await?;
    let items = matched
        .into_iter()
        .map(|case| CaseRow {
            member_count: counts.get(&case.id).copied().unwrap_or(0),
            case,
        })
        .collect();
    Ok(ListPage { items, total })
}

/// Unwrap the `{ data, rev }` write envelope `scan` returns and decode the case. A row that will
/// not decode is skipped rather than failing the lane — one bad record must not take the queue down.
pub(crate) fn unwrap_case(row: serde_json::Value) -> Option<Case> {
    let inner = match row {
        serde_json::Value::Object(mut obj) => {
            obj.remove("data").unwrap_or(serde_json::Value::Object(obj))
        }
        other => other,
    };
    serde_json::from_value(inner).ok()
}

/// Every filter axis, ANDed. Split out so the sort above reads as the product decision it is.
fn matches(case: &Case, query: &ListQuery, subjects: &BTreeSet<String>) -> bool {
    if case.closed && !query.include_closed {
        return false;
    }
    if !matches_filter(case, &query.filter, query.now) {
        return false;
    }

    match query.lane {
        Lane::Mine => case
            .assigned_to
            .as_deref()
            .is_some_and(|a| subjects.contains(a)),
        Lane::Waiting => case.workflow == Workflow::WaitingOnPo || case.waiting_on.is_some(),
        // Deliberately NOT "everything": the cases the caller owns are already in `Mine`, and a
        // lane that repeats them makes the two counts add up to more than the workspace has.
        Lane::Watching => !case
            .assigned_to
            .as_deref()
            .is_some_and(|a| subjects.contains(a)),
    }
}

/// The filter axes alone, lane and `closed` aside: the ONE copy of "does this case pass the
/// filter", shared with [`crate::stages`] so the insight roster's case filter cannot drift from the
/// queue's. `party` is refused by the callers before this runs.
pub(crate) fn matches_filter(case: &Case, f: &ListFilter, now: u64) -> bool {
    if let Some(ids) = &f.ids {
        if !ids.contains(&case.id) {
            return false;
        }
    }
    if let Some(allowed) = &f.sites_allowed {
        if !case.site.as_deref().is_some_and(|s| allowed.contains(s)) {
            return false;
        }
    }
    if let Some(site) = &f.site {
        if case.site.as_deref() != Some(site.as_str()) {
            return false;
        }
    }
    if let Some(category) = &f.category {
        if case.category.as_deref() != Some(category.as_str()) {
            return false;
        }
    }
    if let Some(scope) = &f.scope {
        if case.scope.as_deref() != Some(scope.as_str()) {
            return false;
        }
    }
    if let Some(subsystem) = &f.subsystem {
        if case.subsystem.as_deref() != Some(subsystem.as_str()) {
            return false;
        }
    }
    if let Some(w) = f.workflow {
        if case.workflow != w {
            return false;
        }
    }
    if let Some(w) = f.waiting_on {
        if case.waiting_on != Some(w) {
            return false;
        }
    }
    if let Some(want_snoozed) = f.snoozed {
        let snoozed = case.snooze_until.is_some_and(|u| u > now);
        if snoozed != want_snoozed {
            return false;
        }
    }
    if let Some(want) = f.sla {
        if sla_state(case, now, f.due_soon_ms.unwrap_or(DEFAULT_DUE_SOON_MS)) != want {
            return false;
        }
    }
    true
}
