//! The ONE `WHERE` builder shared by [`crate::count`] and [`crate::list_page`] (insights umbrella).
//!
//! Both the tally and the roster page must select exactly the same rows. A tile reading "78 open"
//! beside a list showing a different set is worse than a slow tile, because nothing on screen says
//! which one is lying. Two copies of these predicates is precisely how that drift starts, so there
//! is one copy and both callers use it.
//!
//! `include_status` and `include_stage` are the ONLY asymmetries: each tally IS the breakdown by one
//! of them, so narrowing by it first would zero the other numbers.
//!
//! `tag_allow` and `assignee` arrive pre-resolved from the host (`insight/resolve_filter.rs`) — the
//! tag graph and team membership live in planes this crate is deliberately agnostic of (README §7).

use std::collections::HashSet;

use serde_json::Value;

use crate::list::{AssigneeFilter, ListFilter};
use crate::search_columns::plain_ident;
use crate::severity::Severity;
use crate::text_expr::{column_text, text_source, TextSource};

/// The shortest search term: one letter would match nearly every row, which is noise, not a search.
const MIN_SEARCH_CHARS: usize = 2;

/// A composed clause: the AND-ed predicates and the parameters they bind.
pub(crate) struct Where {
    pub preds: Vec<String>,
    pub bindings: Vec<(String, Value)>,
    /// The search's index lookup, run as an INNER query that the rest of the clause filters.
    ///
    /// Measured on SurrealDB 3.2.4: full-text matches joined by `OR` are answered from their indexes
    /// only when nothing else is AND-ed to them; add any other predicate (a severity, the exact
    /// check) and the planner falls back to `Iterate Table`, a full scan. As an inner query the
    /// matches still use the indexes, and every other predicate runs over those candidates only.
    candidates: Option<String>,
}

impl Where {
    /// What follows `FROM`: the table, or the search's candidates drawn from it.
    pub(crate) fn source(&self) -> String {
        match &self.candidates {
            None => "type::table($tb)".into(),
            Some(c) => format!("(SELECT * FROM type::table($tb) WHERE {c})"),
        }
    }

    /// What to append after `FROM <source>` — EMPTY when nothing is filtered, so an
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
    include_stage: bool,
) -> Where {
    let mut w = Where {
        preds: Vec::new(),
        bindings: Vec::new(),
        candidates: None,
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

    // The search rides the full-text indexes (the title's in `schema.rs`, one per tag key in
    // `search_columns.rs`), then an exact check on each column's shown value. `@@` is
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
        let (candidates, exact) = search_predicate(&filter.search_columns);
        w.candidates = Some(candidates);
        w.preds.push(exact);
        w.bindings
            .push(("q".into(), Value::String(text.to_string())));
        w.bindings
            .push(("ql".into(), Value::String(text.to_lowercase())));
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

    // The host-set entity limit (entity-scoped data): the flat tag ECHO is the one source of truth
    // for "which entity is this", so the predicate reads it directly. The key is a plain identifier
    // (the host validates it at policy set); anything else fails closed.
    if let Some((key, values)) = &filter.entity {
        if plain_ident(key) {
            w.preds.push(format!("data.tags.{key} IN $entity_ids"));
            let list: Vec<Value> = values.iter().map(|v| Value::String(v.clone())).collect();
            w.bindings.push(("entity_ids".into(), Value::Array(list)));
        } else {
            w.preds.push("false".into());
        }
    }

    // "No <key>": the echo lacks the key, or carries it empty (`list` validated the key).
    if let Some(key) = filter.tag_missing.as_ref().filter(|k| plain_ident(k)) {
        w.preds
            .push(format!("(data.tags.{key} IS NONE OR data.tags.{key} = '')"));
    }

    // The tag facet, already resolved to the ids it admits. An EMPTY set admits NOTHING — the same
    // reading `set.contains(id)` gives in memory, and the opposite of "no filter".
    if let Some(ids) = tag_allow {
        let list: Vec<Value> = ids.iter().map(|i| Value::String(i.clone())).collect();
        w.preds.push("data.id IN $tagids".into());
        w.bindings.push(("tagids".into(), Value::Array(list)));
    }

    // The case lens, host-resolved to plain ids (`case_scope.rs`). The allowlist: a detection
    // with no case never passes (`IN` is false for NONE), exactly as `CaseScope::allows` reads.
    if let Some(scope) = &filter.case {
        if let Some(ids) = &scope.allow {
            let list: Vec<Value> = ids.iter().map(|i| Value::String(i.clone())).collect();
            w.preds.push("data.case_id IN $case_allow".into());
            w.bindings.push(("case_allow".into(), Value::Array(list)));
        }
        if let (true, Some(stages)) = (include_stage, &scope.stages) {
            let ids: Vec<Value> = scope
                .ids_in_stages(stages)
                .into_iter()
                .map(Value::String)
                .collect();
            w.bindings.push(("stage_ids".into(), Value::Array(ids)));
            if stages.contains(crate::case_scope::NO_CASE) {
                // "No case" also covers an echo pointing at a case the host did not return (the
                // same fallback `CaseScope::stage_of` takes), hence the known-ids test.
                let known: Vec<Value> = scope
                    .stage_of
                    .keys()
                    .map(|k| Value::String(k.clone()))
                    .collect();
                w.bindings.push(("known_cases".into(), Value::Array(known)));
                w.preds.push(
                    "(data.case_id IN $stage_ids OR data.case_id IS NONE \
                     OR data.case_id NOT IN $known_cases)"
                        .into(),
                );
            } else {
                w.preds.push("data.case_id IN $stage_ids".into());
            }
        }
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

/// The search, in two parts (`search_columns.rs` explains both), returned separately because they
/// run at different levels (`Where::candidates`):
///
/// 1. `title @0@ q OR tags.k1 @1@ q OR …`: candidates from the full-text indexes, one branch per
///    source field the columns read. Each branch has its own match reference: SurrealDB ties a match
///    to one index through that number.
/// 2. `string::contains(<column 1 as shown>, ql) OR …`: the exact check, so a row matches only when
///    the typed text really is in a column's shown value.
///
/// No columns ⇒ the title alone. Sources that do not parse are skipped (validated at boot too).
fn search_predicate(columns: &[Vec<String>]) -> (String, String) {
    let title_only = [vec!["title".to_string()]];
    let columns: &[Vec<String>] = if columns.is_empty() {
        &title_only
    } else {
        columns
    };
    let mut fields: Vec<String> = Vec::new();
    for src in columns.iter().flatten() {
        let field = match text_source(src) {
            Some(TextSource::Title) => "data.title".to_string(),
            Some(TextSource::Tag(k)) if plain_ident(k) => format!("data.tags.{k}"),
            _ => continue,
        };
        if !fields.contains(&field) {
            fields.push(field);
        }
    }
    let branches: Vec<String> = fields
        .iter()
        .enumerate()
        .map(|(i, f)| format!("{f} @{i}@ $q"))
        .collect();
    let exact: Vec<String> = columns
        .iter()
        .map(|c| format!("string::contains({}, $ql)", column_text(c)))
        .collect();
    (branches.join(" OR "), format!("({})", exact.join(" OR ")))
}
