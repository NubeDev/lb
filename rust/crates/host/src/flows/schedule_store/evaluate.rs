//! `schedule.evaluate`: is the schedule on now, which entry is running, and when it next changes.
//! The box's rules (`lb_schedules`) in the record's zone; the flow node reads `is_active` only.

use chrono::{DateTime, Utc};
use lb_auth::Principal;
use lb_schedules::{evaluate_local, next_change, Zone};
use lb_store::Store;

use super::read::schedule_get;
use super::record::{ScheduleEvaluation, ScheduleRecord};
use crate::flows::error::FlowsError;

impl ScheduleRecord {
    /// Evaluate this schedule now.
    pub fn evaluate(&self) -> Result<ScheduleEvaluation, FlowsError> {
        self.evaluate_at(Utc::now())
    }

    /// Evaluate at `now`. A disabled schedule is inactive by definition (and reports so), which is
    /// what lets an operator park a schedule without deleting it.
    pub fn evaluate_at(&self, now: DateTime<Utc>) -> Result<ScheduleEvaluation, FlowsError> {
        let mut answer = ScheduleEvaluation {
            id: self.id.clone(),
            name: self.name.clone(),
            is_active: false,
            weekly_active: false,
            exception_active: false,
            priority: self.priority,
            active_source: "disabled".to_string(),
            next_transition: None,
            entry_name: None,
            value: None,
        };
        if !self.enabled {
            return Ok(answer);
        }
        let zone = Zone::new(self.use_utc, &self.timezone)
            .map_err(|e| FlowsError::BadInput(format!("schedule `{}`: {e}", self.id)))?;
        let local = zone.local(now);
        let at = evaluate_local(&self.schedule, local);
        answer.active_source = "none".to_string();
        answer.weekly_active = at.weekly_active;
        answer.exception_active = at.exception_active;
        answer.next_transition = next_change(&self.schedule, local)
            .map(|t| zone.instant(t).with_timezone(&Utc).to_rfc3339());
        if let Some(run) = at.running {
            answer.is_active = true;
            answer.active_source = run.source;
            answer.entry_name = Some(run.name);
            answer.value = Some(run.value);
        }
        Ok(answer)
    }
}

/// Evaluate one schedule by id (the verb behind the widget's poll and the node's read).
pub async fn schedule_evaluate(
    store: &Store,
    principal: &Principal,
    ws: &str,
    id: &str,
) -> Result<ScheduleEvaluation, FlowsError> {
    schedule_get(store, principal, ws, id).await?.evaluate()
}
