//! The schedule document as rubix-ce and the box store it, field for field. Defaults fill what a
//! rubix-ce document leaves out (design §3); unknown fields (rubix-ce screen settings) are dropped.
//! Maps are `BTreeMap` so entry keys iterate in order, which is how overlap ties are broken.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// The `schedule` object of a record.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ScheduleDoc {
    #[serde(default)]
    pub config: DocConfig,
    #[serde(default)]
    pub schedules: DocSchedules,
}

/// The entry names a schedule offers and the one new entries take.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DocConfig {
    #[serde(default = "default_names")]
    pub names: Vec<String>,
    #[serde(default = "default_name")]
    pub default_name: String,
}

impl Default for DocConfig {
    fn default() -> Self {
        Self {
            names: default_names(),
            default_name: default_name(),
        }
    }
}

fn default_names() -> Vec<String> {
    vec![default_name()]
}

fn default_name() -> String {
    "Default".to_string()
}

/// The three entry maps, keyed by entry key.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct DocSchedules {
    #[serde(default)]
    pub weekly: BTreeMap<String, WeeklyEntry>,
    #[serde(default)]
    pub events: BTreeMap<String, DatedEntry>,
    #[serde(default)]
    pub exception: BTreeMap<String, DatedEntry>,
}

/// A weekly entry: `days` are lowercase day names, `start`/`end` `HH:MM` with the end inclusive.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WeeklyEntry {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub days: Vec<String>,
    #[serde(default)]
    pub start: String,
    #[serde(default)]
    pub end: String,
    #[serde(default)]
    pub value: f64,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub color: String,
    #[serde(default = "default_enable")]
    pub enable: bool,
}

fn default_enable() -> bool {
    true
}

/// An event (adds hours) or an exception (switches the schedule off). `dates` are local wall times
/// `YYYY-MM-DDTHH:MM`, the end exact. `kind`, `label` and `yearly` are lb's additions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DatedEntry {
    #[serde(default)]
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(default)]
    pub dates: Vec<DateWindow>,
    #[serde(default)]
    pub value: f64,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub color: String,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub yearly: bool,
}

/// One dated window.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DateWindow {
    pub start: String,
    pub end: String,
}
