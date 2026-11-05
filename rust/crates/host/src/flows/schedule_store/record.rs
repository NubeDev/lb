//! The global schedule record's wire shape (`schedule.save` / `get` / `list`) and the evaluated
//! answer (`schedule.evaluate`).
//!
//! The record carries the rubix-ce document under `schedule`, with the same key names, so a rubix-ce
//! document and an lb record map field for field (design §3). The old top-level `weekly` /
//! `exceptions` are gone. A stored row without a `schedule` key is in the old shape and is refused by
//! `read.rs` before it is decoded (serde would otherwise drop the old fields and read it as empty).

use lb_schedules::ScheduleDoc;
use serde::{Deserialize, Serialize};

/// A global, workspace-scoped schedule. Referenced by id from flow nodes and dashboard widgets.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScheduleRecord {
    /// Stable id, unique in the workspace (e.g. `building-hours`).
    pub id: String,
    /// The store's revision: filled on get/list, sent back on save. Never stored in the body (the
    /// store keeps its own `rev` beside the record).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rev: Option<u64>,
    /// Human label for pickers and the widget header.
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub color: String,
    /// Resolution priority (1=local, 10=master, 20=emergency); reported back by `evaluate`.
    #[serde(default = "default_priority")]
    pub priority: i32,
    /// Interpret all times as UTC, ignoring `timezone`.
    #[serde(default)]
    pub use_utc: bool,
    /// IANA zone (e.g. `Australia/Brisbane`). Empty + `use_utc:false` means UTC.
    #[serde(default)]
    pub timezone: String,
    /// A disabled schedule evaluates as permanently inactive but is preserved for re-enabling.
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    /// rubix-ce's output settings. Weekly and event values are checked against min/max when enabled.
    #[serde(default)]
    pub enable_payload: bool,
    #[serde(default)]
    pub min_payload: f64,
    #[serde(default)]
    pub max_payload: f64,
    #[serde(default)]
    pub default_payload: f64,
    /// The rubix-ce document.
    #[serde(default)]
    pub schedule: ScheduleDoc,
}

fn default_priority() -> i32 {
    1
}

fn default_enabled() -> bool {
    true
}

/// The evaluated answer a caller (node leg, widget, verb) gets back for a schedule.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScheduleEvaluation {
    pub id: String,
    pub name: String,
    pub is_active: bool,
    pub weekly_active: bool,
    pub exception_active: bool,
    pub priority: i32,
    /// `weekly:<key>`, `event:<key>`, `none` or `disabled`.
    pub active_source: String,
    /// RFC3339 instant (UTC) of the next change of state or value, when one is known.
    pub next_transition: Option<String>,
    /// The running entry's name; `null` when off.
    pub entry_name: Option<String>,
    /// The running entry's value; `null` when off.
    pub value: Option<f64>,
}

/// What `schedule.list` answers: the records that read, and the rows that do not (old shape or
/// undecodable), each with the `rev` to save over it with.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ScheduleListing {
    pub schedules: Vec<ScheduleRecord>,
    pub unreadable: Vec<UnreadableSchedule>,
}

/// A stored row `schedule.list` could not read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UnreadableSchedule {
    pub id: String,
    pub rev: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A minimal `{id}` post is accepted (the UI's create path) with an empty document.
    #[test]
    fn minimal_record_defaults() {
        let r: ScheduleRecord = serde_json::from_value(serde_json::json!({"id": "s1"})).unwrap();
        assert_eq!(r.rev, None);
        assert_eq!(r.priority, 1);
        assert!(r.enabled);
        assert_eq!(r.schedule, ScheduleDoc::default());
    }

    /// An unset `rev` is left out of the JSON, so a stored body never carries a stale one.
    #[test]
    fn an_unset_rev_is_not_written() {
        let r: ScheduleRecord = serde_json::from_value(serde_json::json!({"id": "s1"})).unwrap();
        let v = serde_json::to_value(&r).unwrap();
        assert!(v.get("rev").is_none());
        assert!(v.get("schedule").is_some());
    }

    #[test]
    fn record_round_trips_through_json() {
        let r: ScheduleRecord = serde_json::from_value(serde_json::json!({
            "id": "s1", "name": "One", "rev": 4, "enable_payload": true, "max_payload": 2
        }))
        .unwrap();
        let back: ScheduleRecord =
            serde_json::from_value(serde_json::to_value(&r).unwrap()).unwrap();
        assert_eq!(r, back);
    }
}
