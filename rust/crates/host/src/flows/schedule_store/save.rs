//! `schedule.save`: validate a schedule and write it under its `rev` (design §5).
//!
//! - The write lands only if the stored record is still at the expected revision
//!   (`lb_store::write_if_rev`, check and write in one transaction).
//! - `rev: N` (N > 0) saves over revision N; a mismatch, or N > 0 on a missing id, is
//!   `schedule_stale`.
//! - No `rev`, or `rev: 0`, is a create, so agents and scripts can still create without reading
//!   first: the guarded write at 0, refused as `schedule_exists` when the id is taken (old-shape
//!   rows included).
//! - The removed top-level `weekly` / `exceptions` are refused, so an old client gets an error
//!   instead of a change serde would silently drop.
//! - The document is validated (`lb_schedules::validate`) and the zone must be known.

use lb_auth::Principal;
use lb_schedules::{validate, PayloadLimits, Zone};
use lb_store::{write_if_rev, RevWrite, Store};
use serde_json::Value;

use super::authorize::authorize_write;
use super::record::ScheduleRecord;
use super::refusal::{exists, invalid, legacy_fields, stale};
use super::SCHEDULE_TABLE;
use crate::flows::error::FlowsError;

/// One `schedule.save` call: the record, plus whether the caller sent the removed fields.
#[derive(Debug, Clone)]
pub struct ScheduleSave {
    pub record: ScheduleRecord,
    /// `weekly` or `exceptions` was present at the top level (any value, `null` included).
    pub legacy_sent: bool,
}

/// What a save answers: the id and the revision the record now stands at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavedSchedule {
    pub id: String,
    pub rev: u64,
}

impl ScheduleSave {
    /// Read a `schedule.save` input. The removed fields are checked on the raw input, because the
    /// record type ignores unknown keys; the refusal comes after the write-cap check.
    pub fn from_input(input: &Value) -> Result<Self, FlowsError> {
        let legacy_sent = input.get("weekly").is_some() || input.get("exceptions").is_some();
        let record = serde_json::from_value(input.clone())
            .map_err(|e| FlowsError::BadInput(e.to_string()))?;
        Ok(Self {
            record,
            legacy_sent,
        })
    }
}

pub async fn schedule_save(
    store: &Store,
    principal: &Principal,
    ws: &str,
    save: &ScheduleSave,
) -> Result<SavedSchedule, FlowsError> {
    authorize_write(principal, ws)?;
    let mut record = save.record.clone();
    if record.id.trim().is_empty() {
        return Err(FlowsError::BadInput(
            "schedule needs a non-empty `id`".into(),
        ));
    }
    if save.legacy_sent {
        return Err(legacy_fields(&record.id));
    }
    let limits = PayloadLimits {
        enable_payload: record.enable_payload,
        min_payload: record.min_payload,
        max_payload: record.max_payload,
    };
    validate(&record.schedule, &limits).map_err(|e| invalid(&record.id, e))?;
    Zone::new(record.use_utc, &record.timezone).map_err(|e| invalid(&record.id, e))?;
    // The store keeps `rev` beside the body; it is never written inside it.
    let sent = record.rev.take();
    let value = serde_json::to_value(&record).map_err(|e| FlowsError::Internal(e.to_string()))?;
    match write_if_rev(
        store,
        ws,
        SCHEDULE_TABLE,
        &record.id,
        &value,
        sent.unwrap_or(0),
    )
    .await
    .map_err(|e| FlowsError::Internal(e.to_string()))?
    {
        RevWrite::Written { rev } => Ok(SavedSchedule { id: record.id, rev }),
        RevWrite::Stale if sent.unwrap_or(0) == 0 => Err(exists(&record.id)),
        RevWrite::Stale => Err(stale(&record.id)),
    }
}
