//! The **global schedule** record + its CRUD — the store surface behind `schedule.save`/`get`/
//! `list`/`delete` and the resolution seam the `schedule` flow node reads through.
//!
//! A schedule is a **first-class, workspace-scoped record**, not a blob buried in one node's config.
//! That is the whole point of "global schedules": one `Building Hours` record is referenced by any
//! number of flow nodes and dashboard widgets, edited in ONE place, and every reader sees the change
//! on its next evaluation. A node holds a `schedule_id` **reference**; it does not own the data.
//!
//! State, not motion (rule 3): this file owns only the durable shape + its reads/writes. Evaluation
//! is `lb-schedules` (pure, clock-injected); firing is the reactor. Workspace-walled — every read and
//! write is ws-scoped, so a ws-B caller can never see or edit a ws-A schedule.

use chrono::{DateTime, Utc};
use lb_auth::Principal;
use lb_caps::{check, Action, Decision, Request, Surface};
use lb_schedules::{Schedule, ScheduleEvaluator, Weekday};
use lb_store::{read, write, Store};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::error::FlowsError;
use super::scan_all::scan_all;

/// The SurrealDB table holding workspace-scoped global schedule records.
pub const SCHEDULE_TABLE: &str = "schedule";

/// One time-of-day window on a weekday, as authored (`HH:MM`, local to the schedule's zone).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimeRangeSpec {
    pub start: String,
    pub stop: String,
}

/// The weekly pattern for one weekday. `day` is `0=Sunday … 6=Saturday` — the wire convention the Go
/// node and the UI both already speak (chrono's `Weekday` is Monday-based, converted in `to_engine`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DayScheduleSpec {
    pub day: u8,
    #[serde(default)]
    pub time_ranges: Vec<TimeRangeSpec>,
}

/// A date-bounded override that beats the weekly pattern while it is active.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExceptionSpec {
    /// RFC3339 instant.
    pub start: String,
    /// RFC3339 instant.
    pub stop: String,
    #[serde(default = "default_exception_type")]
    pub exception_type: String,
    #[serde(default = "default_exception_priority")]
    pub priority: i32,
}

fn default_exception_type() -> String {
    "override".to_string()
}

fn default_exception_priority() -> i32 {
    50
}

/// A global, workspace-scoped schedule. Referenced by id from flow nodes and dashboard widgets.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScheduleRecord {
    /// Stable id, unique in the workspace (e.g. `building-hours`).
    pub id: String,
    /// Human label for pickers and the widget header.
    #[serde(default)]
    pub name: String,
    /// Resolution priority when several schedules are combined: 1=local, 10=master, 20=emergency.
    #[serde(default = "default_priority")]
    pub priority: i32,
    /// Interpret all times as UTC, ignoring `timezone`.
    #[serde(default)]
    pub use_utc: bool,
    /// IANA zone (e.g. `Australia/Brisbane`). Empty + `use_utc:false` ⇒ UTC.
    #[serde(default)]
    pub timezone: String,
    /// A disabled schedule evaluates as permanently inactive but is preserved for re-enabling.
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default)]
    pub weekly: Vec<DayScheduleSpec>,
    #[serde(default)]
    pub exceptions: Vec<ExceptionSpec>,
}

fn default_priority() -> i32 {
    1
}

fn default_enabled() -> bool {
    true
}

/// The evaluated answer a caller (node leg, widget, verb) gets back for a schedule.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduleEvaluation {
    pub id: String,
    pub name: String,
    pub is_active: bool,
    pub weekly_active: bool,
    pub exception_active: bool,
    pub priority: i32,
    /// The winning source label, e.g. `building-hours-weekly` / `…-exception-holiday`.
    pub active_source: String,
    /// RFC3339 instant of the next state change, when one is known.
    pub next_transition: Option<String>,
}

impl ScheduleRecord {
    /// Build the pure `lb-schedules` engine value from this record.
    ///
    /// Fails on a bad timezone / `HH:MM` / RFC3339 field rather than silently degrading: a schedule
    /// that cannot be built must surface at save, not evaluate as "never active" forever.
    pub fn to_engine(&self) -> Result<Schedule, FlowsError> {
        let mut s = Schedule::new(&self.name, self.use_utc, &self.timezone, self.priority)
            .map_err(|e| FlowsError::BadInput(format!("schedule `{}`: {e}", self.id)))?;

        for day in &self.weekly {
            let weekday = weekday_from_sunday_index(day.day).ok_or_else(|| {
                FlowsError::BadInput(format!(
                    "schedule `{}`: day must be 0..=6 (0=Sunday), got {}",
                    self.id, day.day
                ))
            })?;
            for tr in &day.time_ranges {
                s.add_weekly_time_range(weekday, &tr.start, &tr.stop)
                    .map_err(|e| FlowsError::BadInput(format!("schedule `{}`: {e}", self.id)))?;
            }
        }

        for ex in &self.exceptions {
            let start = parse_rfc3339(&ex.start, &self.id)?;
            let stop = parse_rfc3339(&ex.stop, &self.id)?;
            s.add_exception(start, stop, &ex.exception_type, ex.priority);
        }

        Ok(s)
    }

    /// Evaluate this schedule now. A disabled schedule is inactive by definition (and reports so),
    /// which is what lets an operator park a schedule without deleting it.
    pub fn evaluate(&self) -> Result<ScheduleEvaluation, FlowsError> {
        if !self.enabled {
            return Ok(ScheduleEvaluation {
                id: self.id.clone(),
                name: self.name.clone(),
                is_active: false,
                weekly_active: false,
                exception_active: false,
                priority: self.priority,
                active_source: "disabled".to_string(),
                next_transition: None,
            });
        }

        let engine = self.to_engine()?;
        let stats = engine.check_combined();

        let mut evaluator = ScheduleEvaluator::new();
        evaluator.add_schedule(engine);
        let state = evaluator.get_state();

        Ok(ScheduleEvaluation {
            id: self.id.clone(),
            name: self.name.clone(),
            is_active: state.is_active,
            weekly_active: stats.weekly_active,
            exception_active: stats.exception_active,
            priority: state.active_priority,
            active_source: state.active_source,
            next_transition: state.next_transition.map(|t| t.to_rfc3339()),
        })
    }
}

/// Map the wire convention (`0=Sunday … 6=Saturday`) onto chrono's Monday-based `Weekday`.
fn weekday_from_sunday_index(day: u8) -> Option<Weekday> {
    Some(match day {
        0 => Weekday::Sun,
        1 => Weekday::Mon,
        2 => Weekday::Tue,
        3 => Weekday::Wed,
        4 => Weekday::Thu,
        5 => Weekday::Fri,
        6 => Weekday::Sat,
        _ => return None,
    })
}

fn parse_rfc3339(s: &str, id: &str) -> Result<DateTime<Utc>, FlowsError> {
    DateTime::parse_from_rfc3339(s)
        .map(|t| t.with_timezone(&Utc))
        .map_err(|e| FlowsError::BadInput(format!("schedule `{id}`: bad RFC3339 `{s}`: {e}")))
}

// --- store surface ---

/// Persist a schedule after validating it actually builds (a bad zone/time is rejected here, not at
/// the next reactor pass).
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
    record.to_engine()?; // validate before write
    let value = serde_json::to_value(record).map_err(|e| FlowsError::Internal(e.to_string()))?;
    write(store, ws, SCHEDULE_TABLE, &record.id, &value)
        .await
        .map_err(|e| FlowsError::Internal(e.to_string()))?;
    Ok(record.id.clone())
}

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

/// Read a schedule WITHOUT the caller gate — for internal readers (the reactor and the node leg),
/// which have already been authorized at the flow surface they arrived through.
pub async fn read_schedule_internal(
    store: &Store,
    ws: &str,
    id: &str,
) -> Result<Option<ScheduleRecord>, FlowsError> {
    let raw = read(store, ws, SCHEDULE_TABLE, id)
        .await
        .map_err(|e| FlowsError::Internal(e.to_string()))?;
    match raw {
        Some(v) => serde_json::from_value(v)
            .map(Some)
            .map_err(|e| FlowsError::Internal(e.to_string())),
        None => Ok(None),
    }
}

/// Every schedule in the workspace (the picker feed for nodes and widgets).
pub async fn schedule_list(
    store: &Store,
    principal: &Principal,
    ws: &str,
) -> Result<Vec<ScheduleRecord>, FlowsError> {
    authorize_read(principal, ws)?;
    let rows = scan_all(store, ws, SCHEDULE_TABLE)
        .await
        .map_err(|e| FlowsError::Internal(e.to_string()))?;
    // A scanned row wraps the written value in a `data` envelope (the shape `flows_list` unwraps).
    // A row that fails to deserialize is skipped rather than failing the whole listing — one corrupt
    // record must not make every schedule unreachable in the picker.
    let mut out: Vec<ScheduleRecord> = rows
        .into_iter()
        .map(|row| match row.data {
            Value::Object(mut o) => o.remove("data").unwrap_or(Value::Null),
            other => other,
        })
        .filter_map(|v| serde_json::from_value(v).ok())
        .collect();
    out.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(out)
}

/// Delete a schedule. Nodes referencing it then evaluate inactive with an explicit
/// `schedule-not-found` source rather than failing the run — one deleted record must not wedge every
/// flow that pointed at it.
pub async fn schedule_delete(
    store: &Store,
    principal: &Principal,
    ws: &str,
    id: &str,
) -> Result<(), FlowsError> {
    authorize_write(principal, ws)?;
    lb_store::delete(store, ws, SCHEDULE_TABLE, id)
        .await
        .map_err(|e| FlowsError::Internal(e.to_string()))
}

/// Evaluate one schedule by id (the verb behind the widget's poll + the node's read).
pub async fn schedule_evaluate(
    store: &Store,
    principal: &Principal,
    ws: &str,
    id: &str,
) -> Result<ScheduleEvaluation, FlowsError> {
    let record = schedule_get(store, principal, ws, id).await?;
    record.evaluate()
}

fn authorize_write(principal: &Principal, ws: &str) -> Result<(), FlowsError> {
    let req = Request::new(ws, Surface::Store, SCHEDULE_TABLE, Action::Write);
    match check(principal, &req) {
        Decision::Allowed => Ok(()),
        Decision::Denied(_) => Err(FlowsError::Denied),
    }
}

fn authorize_read(principal: &Principal, ws: &str) -> Result<(), FlowsError> {
    let req = Request::new(ws, Surface::Store, SCHEDULE_TABLE, Action::Read);
    match check(principal, &req) {
        Decision::Allowed => Ok(()),
        Decision::Denied(_) => Err(FlowsError::Denied),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn weekly_record(day: u8, start: &str, stop: &str) -> ScheduleRecord {
        ScheduleRecord {
            id: "bh".into(),
            name: "Building Hours".into(),
            priority: 1,
            use_utc: true,
            timezone: String::new(),
            enabled: true,
            weekly: vec![DayScheduleSpec {
                day,
                time_ranges: vec![TimeRangeSpec {
                    start: start.into(),
                    stop: stop.into(),
                }],
            }],
            exceptions: vec![],
        }
    }

    /// The wire day index is Sunday-based; chrono's is Monday-based. A silent mismatch here would
    /// shift every schedule by a day, so it is pinned.
    #[test]
    fn sunday_indexed_days_map_onto_chrono() {
        assert_eq!(weekday_from_sunday_index(0), Some(Weekday::Sun));
        assert_eq!(weekday_from_sunday_index(1), Some(Weekday::Mon));
        assert_eq!(weekday_from_sunday_index(6), Some(Weekday::Sat));
        assert_eq!(weekday_from_sunday_index(7), None);
    }

    #[test]
    fn a_valid_record_builds_an_engine() {
        assert!(weekly_record(1, "09:00", "17:00").to_engine().is_ok());
    }

    /// A bad day index / time / zone must fail loudly at build (and so at save), never degrade into
    /// a schedule that is silently never active.
    #[test]
    fn invalid_fields_are_rejected() {
        assert!(weekly_record(9, "09:00", "17:00").to_engine().is_err());
        assert!(weekly_record(1, "not-a-time", "17:00").to_engine().is_err());

        let mut bad_zone = weekly_record(1, "09:00", "17:00");
        bad_zone.use_utc = false;
        bad_zone.timezone = "Mars/Olympus".into();
        assert!(bad_zone.to_engine().is_err());
    }

    #[test]
    fn a_bad_exception_instant_is_rejected() {
        let mut r = weekly_record(1, "09:00", "17:00");
        r.exceptions = vec![ExceptionSpec {
            start: "yesterday".into(),
            stop: "2026-12-25T23:59:59Z".into(),
            exception_type: "holiday".into(),
            priority: 50,
        }];
        assert!(r.to_engine().is_err());
    }

    /// A disabled schedule reports inactive without consulting the engine — the park-without-delete
    /// affordance the UI's enable toggle relies on.
    #[test]
    fn disabled_schedule_is_inactive() {
        let mut r = weekly_record(1, "00:00", "23:59");
        r.enabled = false;
        let ev = r.evaluate().unwrap();
        assert!(!ev.is_active);
        assert_eq!(ev.active_source, "disabled");
        assert_eq!(ev.next_transition, None);
    }

    /// An always-on window evaluates active, and reports WHY (weekly, not exception).
    #[test]
    fn an_all_week_window_is_active() {
        let mut r = weekly_record(0, "00:00", "23:59");
        for day in 1..=6u8 {
            r.weekly.push(DayScheduleSpec {
                day,
                time_ranges: vec![TimeRangeSpec {
                    start: "00:00".into(),
                    stop: "23:59".into(),
                }],
            });
        }
        let ev = r.evaluate().unwrap();
        assert!(ev.is_active, "a 24/7 window should be active");
        assert!(ev.weekly_active);
        assert!(!ev.exception_active);
    }

    /// Round-trips through JSON with the camelCase-free wire shape the UI posts.
    #[test]
    fn record_round_trips_through_json() {
        let r = weekly_record(1, "09:00", "17:00");
        let v = serde_json::to_value(&r).unwrap();
        let back: ScheduleRecord = serde_json::from_value(v).unwrap();
        assert_eq!(r, back);
    }

    /// Optional fields default so a minimal `{id}` post is accepted (the UI's create path).
    #[test]
    fn minimal_record_defaults() {
        let r: ScheduleRecord = serde_json::from_value(serde_json::json!({"id": "s1"})).unwrap();
        assert_eq!(r.priority, 1);
        assert!(r.enabled);
        assert!(r.weekly.is_empty());
    }
}
