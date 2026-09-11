//! `append_event` — the append-only case-history write (case-plane scope).
//!
//! The size cap is checked BEFORE any write, so a rejected append leaves the existing history
//! exactly as it was. `seq` is host-assigned (previous max + 1, monotone per case); `actor` is
//! host-stamped by the service layer from the principal, never caller-supplied.
//!
//! Nothing evicts here — see `case_event.rs`'s module doc. If you are here to make this a ring, you
//! are deleting an audit trail, not fixing an inconsistency.

use lb_store::{write, Store};

use crate::case_event::{validate_event_size, CaseEvent, EventKind, TABLE};
use crate::error::CasesError;
use crate::events::events_all;

/// Append one event to `case_id` in workspace `ws`, returning the assigned `seq`.
pub async fn append_event(
    store: &Store,
    ws: &str,
    case_id: &str,
    kind: EventKind,
    actor: &str,
    data: serde_json::Value,
    ts: u64,
) -> Result<u64, CasesError> {
    // Read the existing history FIRST: it gives the next `seq`, and it means the size refusal
    // below happens before anything is written.
    let existing = events_all(store, ws, case_id).await?;
    let seq = existing.iter().map(|e| e.seq).max().unwrap_or(0) + 1;

    let event = CaseEvent {
        seq,
        ts,
        kind,
        actor: actor.to_string(),
        data,
    };
    validate_event_size(&event)?;

    // The stored body carries `case_id` (the filter the history read uses) beside the event's own
    // fields. Row id is `{case_id}:{seq}` — stable, unique, readable in a store dump.
    let mut body = serde_json::to_value(&event).map_err(CasesError::decode)?;
    if let Some(obj) = body.as_object_mut() {
        obj.insert("case_id".into(), serde_json::json!(case_id));
    }
    write(store, ws, TABLE, &format!("{case_id}:{seq}"), &body).await?;
    Ok(seq)
}
