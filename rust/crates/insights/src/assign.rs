//! `assign` — set, re-assign, or clear an insight's owner (insight-triage-scope.md).
//!
//! One verb for all three gestures: `Some(subject)` assigns/re-assigns, `None` un-assigns. Idempotent
//! — assigning the current assignee is a no-op success, so a double-click or a retried bulk call is
//! harmless.
//!
//! `assigned_to` is **a subject, not a user id** (`user:priya` or `team:mechanical`) — the same
//! discipline `status_by` has. Membership validation is the SERVICE layer's job (it needs the tag/
//! membership planes this crate is deliberately agnostic of); this verb writes what it is told, after
//! establishing the insight exists.
//!
//! Nothing here touches `status`/`status_by`/`status_ts`: ownership and lifecycle are orthogonal
//! axes. An insight can be assigned while `open`, `acked`, or `resolved` — a resolved finding still
//! belongs to whoever closed the loop, and re-assignment after a re-open is a human action.

use lb_store::{write, Store};

use crate::error::InsightsError;
use crate::insight::OCC_TABLE;
use crate::insight_id::record_id;

/// What an [`assign`] call actually did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssignOutcome {
    /// The stored value after the call — echoed so the caller reports what actually landed.
    pub assigned_to: Option<String>,
    /// `false` when the insight ALREADY had this assignee (an idempotent no-op: a double-click, a
    /// retried bulk call, a re-assign to the same owner). Nothing was written.
    ///
    /// The host reads this to decide whether to notify: re-announcing an assignment that did not
    /// change is the notification equivalent of a duplicate, and a retry must not page a queue twice.
    pub changed: bool,
}

/// Assign insight `id` in workspace `ws` to `assignee` (`None` clears). Errors like `ack` does when
/// the insight does not exist.
// SCOPE: docs/scope/insights/insight-triage-scope.md §"How it fits the core" (MCP surface)
pub async fn assign(
    store: &Store,
    ws: &str,
    id: &str,
    assignee: Option<&str>,
) -> Result<AssignOutcome, InsightsError> {
    let Some(mut insight) = crate::get::get(store, ws, id).await? else {
        return Err(InsightsError::BadInput(format!("no such insight: {id}")));
    };
    let next = assignee.map(|s| s.to_string());
    // Idempotent: assigning the current assignee (or clearing an already-clear one) writes nothing.
    if insight.assigned_to == next {
        return Ok(AssignOutcome {
            assigned_to: next,
            changed: false,
        });
    }
    insight.assigned_to = next.clone();
    let value = serde_json::to_value(&insight)
        .map_err(|e| InsightsError::Store(lb_store::StoreError::Decode(e.to_string())))?;
    write(store, ws, OCC_TABLE, &record_id(id), &value).await?;
    Ok(AssignOutcome {
        assigned_to: next,
        changed: true,
    })
}
