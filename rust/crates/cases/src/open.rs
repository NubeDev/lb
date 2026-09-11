//! `open` — mint a case over one primary insight (case-plane scope).
//!
//! Three writes, in one order that matters: the case record, the PRIMARY membership, then the
//! `opened` event. The membership goes through [`crate::member_add`], so a case can never be opened
//! over an insight another OPEN case already holds — the exclusivity invariant is enforced on the
//! way in, not audited afterwards.
//!
//! A case opens in [`Workflow::ToAction`] with no resolution and `closed: false`. Everything the
//! sla-clock reactor owns (`policy_id`, `respond_by`, `due_at`) is absent here and stamped by that
//! reactor — this verb does no deadline arithmetic.

use lb_store::{new_ulid, write, Store};

use crate::case::{Case, Grouping, Workflow, TABLE};
use crate::case_event::EventKind;
use crate::case_member::MemberRole;
use crate::error::CasesError;
use crate::event_append::append_event;
use crate::member_add::member_add;

/// What a caller states when opening a case. Everything else on [`Case`] is either derived here or
/// owned by a later verb/reactor.
#[derive(Debug, Clone, Default)]
pub struct OpenInput {
    /// One-line human title.
    pub title: String,
    /// Why these insights are one case.
    pub grouping: Grouping,
    /// The insight the case is fundamentally about — for a verdict case the ROOT finding, not the
    /// record that named it.
    pub primary_insight: String,
    /// The severity, echoed from the primary insight.
    pub severity: String,
    /// The workspace facets, echoed from the primary insight. Opaque strings (rule 10).
    pub category: Option<String>,
    pub site: Option<String>,
    pub scope: Option<String>,
    /// The owner, if known at open (the triage backfill copies the insight's assignee across).
    pub assigned_to: Option<String>,
    /// True when the primary insight carried open data-quality caveats.
    pub caveated: bool,
    /// True when a PERSON opened this case (the `case.open` verb), false for the reactors. It is
    /// stamped onto the primary membership, where it stops a reactor moving the member later.
    pub human_placed: bool,
}

/// Open a case in workspace `ws` as `opened_by` at logical ts `ts`. Returns the stored record.
// SCOPE: docs/scope/insights/case-plane-scope.md §"Verbs" (case.open)
pub async fn open(
    store: &Store,
    ws: &str,
    input: OpenInput,
    opened_by: &str,
    ts: u64,
) -> Result<Case, CasesError> {
    if input.title.trim().is_empty() {
        return Err(CasesError::BadInput(
            "case title is empty — a case is a piece of work and must say which".into(),
        ));
    }
    if input.primary_insight.trim().is_empty() {
        return Err(CasesError::BadInput(
            "case has no primary insight — a case is a record that cites insights".into(),
        ));
    }

    let case = Case {
        id: new_ulid(),
        title: input.title,
        workflow: Workflow::ToAction,
        resolution: None,
        resolved_ts: None,
        resolved_by: None,
        waiting_on: None,
        assigned_to: input.assigned_to,
        snooze_until: None,
        snooze_reason: None,
        snoozed_by: None,
        grouping: input.grouping,
        primary_insight: input.primary_insight.clone(),
        category: input.category,
        site: input.site,
        scope: input.scope,
        severity: input.severity,
        policy_id: None,
        respond_by: None,
        due_at: None,
        breached_ts: None,
        breach_waiting_on: None,
        impact_rate: None,
        impact_tier: None,
        cost_to_fix: None,
        verified_saving: None,
        saving_accepted_by: None,
        saving_accepted_ts: None,
        external_ref: None,
        opened_ts: ts,
        last_activity_ts: ts,
        reopened_count: 0,
        caveated: input.caveated,
        closed: false,
    };

    let value = serde_json::to_value(&case).map_err(CasesError::decode)?;
    write(store, ws, TABLE, &case.id, &value).await?;

    // The primary membership goes through the invariant-enforcing verb. If the primary is already
    // held by another open case this REFUSES — leaving a case row with no members, which the next
    // reconcile pass is free to see. Refusing loudly beats silently double-citing a detection.
    member_add(
        store,
        ws,
        &case.id,
        &input.primary_insight,
        MemberRole::Primary,
        opened_by,
        input.human_placed,
        ts,
    )
    .await?;

    append_event(
        store,
        ws,
        &case.id,
        EventKind::Opened,
        opened_by,
        serde_json::json!({
            "grouping": case.grouping,
            "primary_insight": case.primary_insight,
            "severity": case.severity,
        }),
        ts,
    )
    .await?;

    Ok(case)
}
