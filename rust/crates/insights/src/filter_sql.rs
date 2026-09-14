//! The ONE `WHERE` builder shared by [`crate::count`] and [`crate::list_page`] (insights umbrella).
//!
//! Both the tally and the roster page must select exactly the same rows. A tile reading "78 open"
//! beside a list showing a different set is worse than a slow tile, because nothing on screen says
//! which one is lying. Two copies of these predicates is precisely how that drift starts, so there
//! is one copy and both callers use it.
//!
//! `include_status` is the ONLY asymmetry: the tally IS the per-status breakdown, so narrowing by
//! status first would zero three of its four numbers.
//!
//! `tag_allow` and `assignee` arrive pre-resolved from the host (`insight/resolve_filter.rs`) — the
//! tag graph and team membership live in planes this crate is deliberately agnostic of (README §7).

use std::collections::HashSet;

use serde_json::Value;

use crate::list::{AssigneeFilter, ListFilter};
use crate::severity::Severity;

/// The shortest search term that can match, set by the analyzer's `edgengram(2,15)` lower bound.
const MIN_SEARCH_CHARS: usize = 2;

/// A composed clause: the AND-ed predicates and the parameters they bind.
pub(crate) struct Where {
    pub preds: Vec<String>,
    pub bindings: Vec<(String, Value)>,
}

impl Where {
    /// What to append after `FROM type::table($tb)` — EMPTY when nothing is filtered, so an
    /// unfiltered read stays a plain scan rather than `WHERE true`.
    pub(crate) fn clause(&self) -> String {
        if self.preds.is_empty() {
            String::new()
        } else {
            format!(" WHERE {}", self.preds.join(" AND "))
        }
    }
}

/// Compose the filter into SQL. Every axis is AND-ed; an absent axis contributes nothing.
pub(crate) fn build(
    filter: &ListFilter,
    tag_allow: Option<&HashSet<String>>,
    assignee: Option<&AssigneeFilter>,
    include_status: bool,
) -> Where {
    let mut w = Where {
        preds: Vec::new(),
        bindings: Vec::new(),
    };

    if include_status {
        if let Some(status) = filter.status {
            w.preds.push("data.status = $status".into());
            w.bindings.push(("status".into(), json_of(status)));
        }
    }

    // A severity FLOOR, expanded here rather than compared in SQL: the stored value is a lowercase
    // word, and words do not order by severity ("critical" < "info" as bytes). The set is closed and
    // tiny, so listing the members that pass the floor is both correct and index-friendly.
    if let Some(floor) = filter.severity {
        let allowed: Vec<Value> = [Severity::Info, Severity::Warning, Severity::Critical]
            .into_iter()
            .filter(|s| s.at_least(floor))
            .map(json_of)
            .collect();
        w.preds.push("data.severity IN $sevs".into());
        w.bindings.push(("sevs".into(), Value::Array(allowed)));
    }

    // The name search rides the BM25 index (`insight_name`, defined in `schema.rs`). `@@` is
    // SurrealDB's full-text match: it is answered from the index, so this narrows the set the
    // ORDER BY has to sort rather than adding a pass over the rows.
    // Below TWO characters this is not a search. The analyzer indexes prefixes from length 2
    // (`edgengram(2,15)`), so a single letter matches NOTHING — and a roster that empties itself the
    // moment someone types the first letter looks broken. A too-short term is no filter at all, and
    // the floor lives here as well as in the UI so any client behaves the same way.
    if let Some(text) = filter
        .search
        .as_ref()
        .map(|t| t.trim())
        .filter(|t| t.chars().count() >= MIN_SEARCH_CHARS)
    {
        w.preds.push("data.title @@ $q".into());
        w.bindings
            .push(("q".into(), Value::String(text.to_string())));
    }

    // `ref` is a SurrealDB keyword, so the field needs backquoting — the serde name is `ref`
    // (`Origin::reference` is renamed), and the stored document uses the serde name.
    if let Some(origin_ref) = filter.origin_ref.as_ref() {
        w.preds.push("data.origin.`ref` = $oref".into());
        w.bindings
            .push(("oref".into(), Value::String(origin_ref.clone())));
    }

    // Inclusive on both ends, matching the in-memory predicate exactly.
    if let Some((from, to)) = filter.range {
        w.preds
            .push("data.last_ts >= $rfrom AND data.last_ts <= $rto".into());
        w.bindings.push(("rfrom".into(), Value::from(from)));
        w.bindings.push(("rto".into(), Value::from(to)));
    }

    // The tag facet, already resolved to the ids it admits. An EMPTY set admits NOTHING — the same
    // reading `set.contains(id)` gives in memory, and the opposite of "no filter".
    if let Some(ids) = tag_allow {
        let list: Vec<Value> = ids.iter().map(|i| Value::String(i.clone())).collect();
        w.preds.push("data.id IN $tagids".into());
        w.bindings.push(("tagids".into(), Value::Array(list)));
    }

    match assignee {
        None => {}
        Some(AssigneeFilter::Unassigned) => w.preds.push("data.assigned_to IS NONE".into()),
        Some(AssigneeFilter::AnyOf(subs)) => {
            let list: Vec<Value> = subs.iter().map(|s| Value::String(s.clone())).collect();
            w.preds.push("data.assigned_to IN $subs".into());
            w.bindings.push(("subs".into(), Value::Array(list)));
        }
    }

    w
}

/// A closed enum as the lowercase word it is stored as.
fn json_of<T: serde::Serialize>(v: T) -> Value {
    serde_json::to_value(v).unwrap_or(Value::Null)
}
