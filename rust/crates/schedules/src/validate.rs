//! The rules a schedule document meets before it is saved (design §3, "Validation on save").
//! Overlapping entries are allowed, as on the box. `min_payload <= max_payload` is checked first;
//! then the first entry problem found is reported, naming the entry, in map order weekly, events,
//! exception and keys ascending.

use crate::doc::{DateWindow, DatedEntry, DocConfig, ScheduleDoc, WeeklyEntry};
use crate::error::ScheduleError;
use crate::parse::{minutes_of, parse_local, weekday_from_name};

/// The record's payload settings the weekly and event values are checked against.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PayloadLimits {
    pub enable_payload: bool,
    pub min_payload: f64,
    pub max_payload: f64,
}

/// The dated-entry kinds lb knows.
const KINDS: [&str; 4] = ["closed", "changed", "extra", "holiday"];

pub fn validate(doc: &ScheduleDoc, limits: &PayloadLimits) -> Result<(), ScheduleError> {
    // Checked whether or not payloads are enabled: the box's output settings refuse it, so a record
    // carrying it could never be pushed to an attached box.
    if limits.min_payload > limits.max_payload {
        return Err(ScheduleError::Invalid(format!(
            "min_payload {} is above max_payload {}",
            limits.min_payload, limits.max_payload
        )));
    }
    let s = &doc.schedules;
    let problems = s
        .weekly
        .iter()
        .map(|(k, w)| ("weekly", k, weekly_problem(w, &doc.config, limits)))
        .chain(
            s.events
                .iter()
                .map(|(k, e)| ("event", k, dated_problem(e, &doc.config, Some(limits)))),
        )
        .chain(
            s.exception
                .iter()
                .map(|(k, e)| ("exception", k, dated_problem(e, &doc.config, None))),
        );
    for (map, key, problem) in problems {
        if let Some(p) = problem {
            return Err(ScheduleError::Invalid(format!("{map} `{key}`: {p}")));
        }
    }
    Ok(())
}

fn weekly_problem(w: &WeeklyEntry, config: &DocConfig, limits: &PayloadLimits) -> Option<String> {
    name_problem(config, &w.name)
        .or_else(|| value_problem(limits, w.value))
        .or_else(|| {
            w.days
                .iter()
                .find(|d| weekday_from_name(d).is_none())
                .map(|d| format!("unknown day `{d}`"))
        })
        .or_else(|| match (minutes_of(&w.start), minutes_of(&w.end)) {
            (Ok(start), Ok(end)) if start <= end => None,
            (Ok(_), Ok(_)) => Some(format!(
                "start `{}` is after end `{}`; a weekly entry cannot cross midnight",
                w.start, w.end
            )),
            (Err(e), _) | (_, Err(e)) => Some(e.to_string()),
        })
}

/// `limits` is `None` for exceptions: their values are not checked.
fn dated_problem(
    e: &DatedEntry,
    config: &DocConfig,
    limits: Option<&PayloadLimits>,
) -> Option<String> {
    name_problem(config, &e.name)
        .or_else(|| limits.and_then(|l| value_problem(l, e.value)))
        .or_else(|| {
            e.kind
                .as_deref()
                .filter(|k| !KINDS.contains(k))
                .map(|k| format!("unknown kind `{k}`"))
        })
        .or_else(|| {
            let repeatable = matches!(e.kind.as_deref(), Some("closed") | Some("holiday"));
            (e.yearly && !repeatable)
                .then(|| "`yearly` is allowed only on `closed` and `holiday` entries".to_string())
        })
        .or_else(|| e.dates.iter().find_map(window_problem))
}

fn window_problem(w: &DateWindow) -> Option<String> {
    match (parse_local(&w.start), parse_local(&w.end)) {
        (Ok(start), Ok(end)) if start < end => None,
        (Ok(_), Ok(_)) => Some(format!("start `{}` is not before end `{}`", w.start, w.end)),
        (Err(e), _) | (_, Err(e)) => Some(e.to_string()),
    }
}

fn name_problem(config: &DocConfig, name: &str) -> Option<String> {
    (!config.names.iter().any(|n| n == name))
        .then(|| format!("name `{name}` is not in config.names {:?}", config.names))
}

fn value_problem(l: &PayloadLimits, v: f64) -> Option<String> {
    (l.enable_payload && !(l.min_payload..=l.max_payload).contains(&v))
        .then(|| format!("value {v} is outside {}..={}", l.min_payload, l.max_payload))
}
