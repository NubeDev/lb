//! The **case** record — one row per *piece of work* that cites one or more insights
//! (`docs/scope/insights/case-plane-scope.md`).
//!
//! An insight is a **detection**: keyed by `(ws, dedup_key)`, re-raised and re-opened by a machine,
//! and deliberately immune on its human plane (`insight-triage-scope.md`). A case is the **work**:
//! many-to-one with detections, owned by a person, moved through a workflow, closed with a
//! resolution. Putting workflow on the detection row breaks both — so this is a separate record and
//! the raise hot path never learns what a purchase order is.
//!
//! **Rule 10.** A case is "a record that cites insights". Nothing here names a pack, a rule, a
//! category *value* or an extension: `category`/`site`/`scope` are opaque workspace strings
//! (`tag_vocab` data), and `grouping: verdict` is decided by reading `body.explains[]` — a property
//! of the data, never a rule name.
//!
//! State lives in `lb_store` behind the workspace wall; every timestamp is a caller-injected
//! logical epoch-millis value (no wall clock in this crate — testing §3, the host normalizes).

use serde::{Deserialize, Serialize};

/// The store table all cases live in. One table per workspace namespace; `workflow`/`closed`/
/// `assigned_to` are `data` fields, so a lane read is a filtered scan rather than a new table.
pub const TABLE: &str = "case";

/// Where a case sits on the human workflow. The closed set the lanes and the queue index on.
///
/// `Resolved` is the ONLY terminal state, and it is the only one that may carry a [`Resolution`] —
/// see [`crate::workflow`], which refuses both halves of that invariant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Workflow {
    /// Nobody has started. The default a case opens in.
    ToAction,
    /// Someone did something — a visit, a reset, a parameter change.
    Actioned,
    /// Blocked on money/authority, not on effort. Its own state because "waiting" is the single
    /// most common honest answer and folding it into `actioned` loses the queue that matters.
    WaitingOnPo,
    /// Closed. Requires a [`Resolution`], sets `resolved_ts`/`resolved_by` and `closed`.
    Resolved,
}

impl Workflow {
    /// True for the one terminal state. `closed` on the record is this, materialized so the
    /// exclusivity invariant ("every open insight is in exactly one OPEN case") is indexable
    /// without decoding an enum in a filter.
    pub fn is_terminal(self) -> bool {
        matches!(self, Workflow::Resolved)
    }
}

/// How a case ended. Load-bearing beyond reporting: the hold-down reactor reopens a case that
/// closed as [`Resolution::Fixed`] and re-fired inside the policy window ("the repair did not
/// hold"), while every other resolution lets a NEW case open.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Resolution {
    /// We repaired it. The only resolution the hold-down window applies to.
    Fixed,
    /// It stopped on its own — nothing was done.
    SelfCleared,
    /// The detection was wrong.
    FalsePositive,
    /// Real, understood, and deliberately not being fixed.
    AcceptedRisk,
    /// Folded into another case (what [`crate::merge`] closes the losing case as).
    Duplicate,
}

/// Who the case is blocked on while it waits. Orthogonal to [`Workflow`]: a case can be
/// `waiting_on_po` on the client or `actioned` and waiting on a contractor's return visit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WaitingOn {
    Internal,
    Client,
    Contractor,
}

/// WHY these insights are one piece of work. Derived by the case-group reactor from the data, never
/// declared by a rule (rule 10): `Verdict` means one cited the others through `body.explains[]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Grouping {
    /// A record that CITES other findings (`body.explains[]`) — its root cause is the case's
    /// primary and the cited findings are `explained` members.
    Verdict,
    /// Same equipment tree. A fast-follow, derived by query at reactor time.
    Topology,
    /// One incident, many simultaneous detections. A fast-follow, derived by query.
    Storm,
    /// A person grouped them (`case.open` / `case.merge` / `case.split`).
    Human,
    /// One insight, one case — the default, and what the reconcile backstop opens.
    Single,
}

impl Default for Grouping {
    /// A case with nothing said about it is one insight, one case — what the reconcile backstop
    /// opens, and the shape every other grouping is a fold OF.
    fn default() -> Self {
        Grouping::Single
    }
}

/// How much to trust the money on a case. Kept beside the amounts so a UI can never render a
/// modelled saving as a claimed one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImpactTier {
    Claimed,
    Modelled,
    Withheld,
}

/// A durable case record.
///
/// Not `Eq`: the money fields are `f64` (a rate is a real quantity in the workspace's currency, so
/// the float is the honest type). Compare with `PartialEq`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Case {
    /// Stable id (ULID), unique within the workspace. Host-assigned at open.
    pub id: String,
    /// One-line human title.
    pub title: String,
    /// The workflow state (see [`Workflow`]).
    pub workflow: Workflow,
    /// How it closed. Present **iff** `workflow == Resolved` — [`crate::workflow`] enforces both
    /// directions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolution: Option<Resolution>,
    /// Logical timestamp of the close. Cleared on a reopen.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved_ts: Option<u64>,
    /// The subject that closed it. Cleared on a reopen.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved_by: Option<String>,
    /// Who the case is blocked on (see [`WaitingOn`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub waiting_on: Option<WaitingOn>,
    /// Who OWNS the work — a **subject**, not a user id (`user:priya` or `team:mechanical`), the
    /// discipline `insight.assigned_to` already holds. This is the authority; the insight's
    /// `assigned_to` is an echo of it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assigned_to: Option<String>,
    /// Snoozed until this logical timestamp. A severity escalation **punctures** a snooze
    /// ([`crate::snooze::puncture_snooze`]) — a case that got worse is not still parked.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snooze_until: Option<u64>,
    /// Why it was snoozed. Required to snooze (a snooze with no reason is an unexplained silence).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snooze_reason: Option<String>,
    /// Who snoozed it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snoozed_by: Option<String>,
    /// Why these insights are one case (see [`Grouping`]).
    pub grouping: Grouping,
    /// The insight this case is fundamentally ABOUT — for a verdict case the ROOT finding, not the
    /// record that named it.
    pub primary_insight: String,
    /// Workspace vocabulary, echoed from the primary insight's facets. Opaque to this crate
    /// (rule 10): the value set is `tag_vocab` data a pack seeds, and lb ships no default list.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    /// The site facet, echoed from the primary insight. Opaque (see `category`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub site: Option<String>,
    /// The scope facet (`point | device | system | site | portfolio` in the seeded vocabulary),
    /// echoed from the primary insight. Opaque (see `category`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
    /// The severity of the case, echoed from the primary insight's latest firing. A string, not an
    /// enum: this crate does not depend on `lb-insights`, and the queue only ever ranks it
    /// ([`severity_rank`]).
    pub severity: String,
    /// The `service_policy` that produced `respond_by`/`due_at`. Written by the sla-clock reactor.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub policy_id: Option<String>,
    /// The response deadline in business hours (sla-clock reactor).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub respond_by: Option<u64>,
    /// The resolution deadline in business hours — the queue's primary sort key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub due_at: Option<u64>,
    /// When the deadline was breached (sla-clock reactor). Never un-set: a breach is history.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub breached_ts: Option<u64>,
    /// Who the case was blocked on AT the breach — the accountability the report needs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub breach_waiting_on: Option<WaitingOn>,
    /// The money bleeding per unit time while this is open.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub impact_rate: Option<f64>,
    /// How much to trust the money (see [`ImpactTier`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub impact_tier: Option<ImpactTier>,
    /// The quoted/actual cost of the repair.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost_to_fix: Option<f64>,
    /// The saving measured AFTER the repair.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verified_saving: Option<f64>,
    /// The subject who signed off the verified saving.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub saving_accepted_by: Option<String>,
    /// When the saving was signed off.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub saving_accepted_ts: Option<u64>,
    /// A foreign work-order / ticket reference (an FMS id). Opaque.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub external_ref: Option<String>,
    /// Logical timestamp the case was opened.
    pub opened_ts: u64,
    /// Logical timestamp of the last thing that happened on it — bumped by every verb that writes.
    pub last_activity_ts: u64,
    /// How many times this case was REOPENED by the hold-down reactor ("the repair did not hold").
    #[serde(default)]
    pub reopened_count: u32,
    /// True when the primary insight carried open data-quality caveats at open time. A caveated
    /// case is never a breakthrough (`insight-notify-scope.md`).
    #[serde(default)]
    pub caveated: bool,
    /// The derived "this is not an open case" flag — `workflow.is_terminal()`, materialized.
    ///
    /// It exists so the exclusivity invariant ("every open insight is in exactly ONE open case") is
    /// a filter on a stored boolean rather than a decode-and-match over every case in the
    /// workspace. [`crate::workflow`] is the only writer; nothing else may set it.
    #[serde(default)]
    pub closed: bool,
}

/// Rank a severity string low→high for the queue's secondary sort (severity DESCENDING after
/// `due_at` ascending). Unknown values rank lowest — an unrecognised severity must never jump the
/// queue ahead of a known `critical`.
///
/// The three names are lb's own closed severity set (`lb_insights::Severity`), not workspace
/// vocabulary, so ranking them here is not a rule-10 special case. They are matched as strings
/// rather than by depending on `lb-insights`: this crate's dependency set mirrors that crate's, and
/// a case only ever ORDERS a severity — it never interprets one.
pub fn severity_rank(severity: &str) -> u8 {
    match severity {
        "critical" => 3,
        "warning" => 2,
        "info" => 1,
        _ => 0,
    }
}
