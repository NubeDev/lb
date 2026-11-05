//! `schedule.save`: validate and write a schedule record.

use lb_auth::Principal;
use lb_schedules::{validate, PayloadLimits};
use lb_store::{write, Store};

use super::authorize::authorize_write;
use super::record::ScheduleRecord;
use super::SCHEDULE_TABLE;
use crate::flows::error::FlowsError;

/// Persist a schedule after validating its document. `rev` is never written into the body.
pub async fn schedule_save(
    store: &Store,
    principal: &Principal,
    ws: &str,
    record: &ScheduleRecord,
) -> Result<String, FlowsError> {
    authorize_write(principal, ws)?;
    if record.id.trim().is_empty() {
        return Err(FlowsError::BadInput(
            "schedule needs a non-empty `id`".into(),
        ));
    }
    let limits = PayloadLimits {
        enable_payload: record.enable_payload,
        min_payload: record.min_payload,
        max_payload: record.max_payload,
    };
    validate(&record.schedule, &limits)
        .map_err(|e| FlowsError::BadInput(format!("schedule `{}`: {e}", record.id)))?;
    let mut body = record.clone();
    body.rev = None;
    let value = serde_json::to_value(&body).map_err(|e| FlowsError::Internal(e.to_string()))?;
    write(store, ws, SCHEDULE_TABLE, &record.id, &value)
        .await
        .map_err(|e| FlowsError::Internal(e.to_string()))?;
    Ok(record.id.clone())
}
