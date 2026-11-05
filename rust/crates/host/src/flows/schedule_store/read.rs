//! `schedule.get` / `schedule.list` and the internal read the flow node uses. Every record comes back
//! with its store `rev`, the revision a save sends back. A stored row without a `schedule` key is in
//! the old shape and is refused, never read as empty (design §3): `get` and `evaluate` answer
//! `schedule_old_shape`, `list` names it under `unreadable` with its `rev` so it can be saved over.
//! Workspace-walled: every read is ws-scoped, so a ws-B caller never sees a ws-A schedule.

use lb_auth::Principal;
use lb_store::{read_versioned, scan_all, Store, FIRST_REV};
use serde_json::Value;

use super::authorize::authorize_read;
use super::record::{ScheduleListing, ScheduleRecord, UnreadableSchedule};
use super::refusal::old_shape;
use super::SCHEDULE_TABLE;
use crate::flows::error::FlowsError;

/// Read one schedule by id.
pub async fn schedule_get(
    store: &Store,
    principal: &Principal,
    ws: &str,
    id: &str,
) -> Result<ScheduleRecord, FlowsError> {
    authorize_read(principal, ws)?;
    read_schedule_internal(store, ws, id)
        .await?
        .ok_or_else(|| FlowsError::BadInput(format!("schedule `{id}` not found")))
}

/// Read a schedule WITHOUT the caller gate, for internal readers (the reactor and the node leg),
/// which were already authorized at the flow surface they arrived through. An old-shape row is an
/// error here too, so the flow node keeps its last state instead of reading "off".
pub async fn read_schedule_internal(
    store: &Store,
    ws: &str,
    id: &str,
) -> Result<Option<ScheduleRecord>, FlowsError> {
    let live = read_versioned(store, ws, SCHEDULE_TABLE, id)
        .await
        .map_err(|e| FlowsError::Internal(e.to_string()))?;
    let Some(value) = live.value else {
        return Ok(None);
    };
    if value.get("schedule").is_none() {
        return Err(old_shape(id, live.rev));
    }
    let mut record: ScheduleRecord =
        serde_json::from_value(value).map_err(|e| FlowsError::Internal(e.to_string()))?;
    record.rev = Some(live.rev);
    Ok(Some(record))
}

/// Every schedule in the workspace (the picker feed for nodes and widgets), sorted by id. A row that
/// is in the old shape or does not decode goes to `unreadable` instead: one bad record must not make
/// every schedule unreachable, and it must still be findable.
pub async fn schedule_list(
    store: &Store,
    principal: &Principal,
    ws: &str,
) -> Result<ScheduleListing, FlowsError> {
    authorize_read(principal, ws)?;
    let rows = scan_all(store, ws, SCHEDULE_TABLE)
        .await
        .map_err(|e| FlowsError::Internal(e.to_string()))?;
    let prefix = format!("{SCHEDULE_TABLE}:");
    let mut listing = ScheduleListing::default();
    for row in rows {
        let (rev, record) = decode_row(row.data);
        match record {
            Some(mut record) => {
                record.rev = Some(rev);
                listing.schedules.push(record);
            }
            None => listing.unreadable.push(UnreadableSchedule {
                id: row.id.strip_prefix(&prefix).unwrap_or(&row.id).to_string(),
                rev,
            }),
        }
    }
    listing.schedules.sort_by(|a, b| a.id.cmp(&b.id));
    listing.unreadable.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(listing)
}

/// A scanned row is the stored envelope `{data, rev}`: its revision (a row written before revisions
/// existed reads as `FIRST_REV`, as `read_versioned` does) and the record, when `data` has a
/// `schedule` key and decodes.
fn decode_row(row: Value) -> (u64, Option<ScheduleRecord>) {
    let Value::Object(mut envelope) = row else {
        return (FIRST_REV, None);
    };
    let rev = envelope
        .get("rev")
        .and_then(Value::as_u64)
        .unwrap_or(FIRST_REV);
    let record = envelope
        .remove("data")
        .filter(|data| data.get("schedule").is_some())
        .and_then(|data| serde_json::from_value(data).ok());
    (rev, record)
}
