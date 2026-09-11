//! `CaseEvent` — the append-only history of a case (case-plane scope).
//!
//! **Append-only, and it never evicts.** The occurrence ring evicts because firings are
//! machine-generated and individually low-value; a case's history is the audit trail a client
//! reads back six months later to see who was told what, when. Deleting that is a trust failure of
//! the kind `insight_comment`'s count cap exists to avoid — so the bound here is a per-event size
//! REFUSAL ([`validate_event_size`]), never a silent drop of an older row.
//!
//! `actor` is `user: | team: | party: | system:` — a contractor's reply is attributed to the
//! **party**, not to a login they never had. That is the whole reason `actor` is a free subject
//! string rather than a user id.

use serde::{Deserialize, Serialize};

use crate::error::CasesError;

/// The store table event rows live in. `case_id` is a `data` field, so a case's history is a
/// single-field equality filter — the layout the insight comment thread uses.
pub const TABLE: &str = "case_event";

/// The hard size cap on one event's `data` serialized. Exceeding it rejects the WHOLE append
/// before any write (never silent truncation) — the contract `validate_occurrence_size` and
/// `validate_comment` already hold.
pub const MAX_EVENT_DATA_BYTES: usize = 4 * 1024;

/// What happened. A closed set: an event kind nothing renders is an event nobody reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    Opened,
    Merged,
    Split,
    Workflow,
    Assigned,
    Snoozed,
    /// A deadline was SET (or recomputed) and which `service_policy` clause produced it. Not in the
    /// scope's original list because that list was written before the sla-clock had to be
    /// defensible: a `due_at` a client can be held to must be traceable to a contract, and the only
    /// place that trace can live is the append-only history. Folding it into `workflow` was
    /// rejected — a deadline is not a state transition, and mixing them corrupts the one stream a
    /// reader scans to answer "what happened to this case".
    Sla,
    /// The primary insight's caveat state CHANGED — the data underneath this case became untrusted,
    /// or became trustworthy again. Additive, and separate from `workflow` for the same reason
    /// `Sla` is: it is a fact about the evidence, not a transition of the work. An operator asking
    /// "why did the contractor button switch off?" has no other place to find the answer.
    Caveat,
    RequestSent,
    RequestOpened,
    /// We took an ask back. NOT in the case-plane scope's original list, and added deliberately:
    /// a withdrawal is a thing that happened to a party, and a history that showed `request_sent`
    /// with no counterpart would read as "we asked them and they ignored us" for ever. Additive, so
    /// a row written before it existed decodes unchanged.
    RequestWithdrawn,
    Reply,
    Nudge,
    Breach,
    FmsTicket,
    Reopened,
    Resolved,
    Verified,
    SavingAccepted,
    Comment,
}

/// One row of a case's history.
///
/// **Serialized field names matter**: these rows are written through `lb_store::write`, so the
/// stored body carries a `case_id` field beside the event's own. The monotone per-case sequence
/// serializes as **`eseq`** to stay clear of any store-injected `seq` — the `oseq`/`cseq`
/// precedent.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CaseEvent {
    /// Monotone per-case sequence, host-assigned (previous max + 1). Stable: nothing evicts, so a
    /// `seq` is a permanent handle to a moment in the case's history.
    #[serde(rename = "eseq")]
    pub seq: u64,
    /// Logical timestamp (no wall-clock in this crate — testing §3).
    pub ts: u64,
    /// What happened.
    pub kind: EventKind,
    /// Who did it — `user:` / `team:` / `party:` / `system:`. Host-stamped from the principal (or
    /// the reactor's system subject); never caller-supplied.
    pub actor: String,
    /// The event's payload — the transition's before/after, the comment text, the reply. Opaque
    /// JSON, ≤ [`MAX_EVENT_DATA_BYTES`] serialized.
    #[serde(default, skip_serializing_if = "serde_json::Value::is_null")]
    pub data: serde_json::Value,
}

/// Validate an event's `data` against the size cap WITHOUT writing, so an oversize payload rejects
/// the append and leaves the existing history exactly as it was.
pub fn validate_event_size(event: &CaseEvent) -> Result<(), CasesError> {
    if event.data.is_null() {
        return Ok(());
    }
    let bytes = serde_json::to_vec(&event.data)
        .map_err(|e| CasesError::Store(lb_store::StoreError::Decode(e.to_string())))?;
    if bytes.len() > MAX_EVENT_DATA_BYTES {
        return Err(CasesError::BadInput(format!(
            "case event data {} bytes exceeds the {MAX_EVENT_DATA_BYTES}-byte cap — nothing was \
             appended and the existing history is untouched; slim the payload or link the detail",
            bytes.len()
        )));
    }
    Ok(())
}
