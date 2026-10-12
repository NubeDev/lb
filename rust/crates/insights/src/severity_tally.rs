//! The per-severity tally `list` returns when asked (`ListQuery::severity_counts`) — a roster's
//! Critical / Warning / Info chips in one read instead of three floor reads and two subtractions.
//!
//! It ignores the severity filter (it IS the per-severity breakdown) and honours every other axis,
//! the status and the case stage included. The severity floor normally narrows inside the statement,
//! so when the tally is asked for the scan leaves it out and `list` applies it in memory afterwards.
//!
//! One responsibility: the severity breakdown of a matched set.

use std::collections::BTreeMap;

use crate::insight::Insight;
use crate::list::ListFilter;

/// The filter the scan runs with: the caller's, minus the severity floor when a tally needs the rows
/// that floor would drop.
pub(crate) fn scan_filter(f: &ListFilter, tallying: bool) -> ListFilter {
    if tallying {
        ListFilter {
            severity: None,
            ..f.clone()
        }
    } else {
        f.clone()
    }
}

/// `critical` / `warning` / `info` → how many of `matched` the status and stage filters keep. All
/// three keys are always present, so a severity with none reads 0 rather than missing.
pub(crate) fn tally(matched: &[Insight], f: &ListFilter) -> BTreeMap<String, u64> {
    let mut t: BTreeMap<String, u64> = ["critical", "warning", "info"]
        .into_iter()
        .map(|k| (k.to_string(), 0))
        .collect();
    let kept = matched.iter().filter(|i| {
        f.status.map(|s| i.status == s).unwrap_or(true)
            && f.case.as_ref().map(|c| c.in_stages(i)).unwrap_or(true)
    });
    for i in kept {
        let k = serde_json::to_value(i.severity)
            .ok()
            .and_then(|v| v.as_str().map(str::to_string))
            .unwrap_or_default();
        *t.entry(k).or_default() += 1;
    }
    t
}
