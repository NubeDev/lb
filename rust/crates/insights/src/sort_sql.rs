//! The roster's column sort, as SQL — one `ORDER BY` for BOTH read paths (the pushed-down page and the
//! counted scan), so page 1 (counted) and page 2 (not) can never disagree about the order and skip or
//! repeat a row between them.
//!
//! **lb names no column.** A sort is a list of SOURCES tried left to right, the first non-empty one
//! wins: `title`, `tag:<key>`, or one of the numeric facts (`severity`, `last_ts`, `first_ts`,
//! `count`, `case_stage`). A roster whose Name column shows `short_name`, else the `insight` tag, else
//! the title sorts by `["tag:short_name", "tag:insight", "title"]`.
//!
//! **Empty values sort last in BOTH directions** (`_e` is ordered before the key): a column of dashes
//! is not information, and a descending sort that put every blank first would bury the rows that
//! have a value. Text compares case-insensitively. Ties fall back to the default order, newest first.
//!
//! **Cost.** Every order here is a sort over the matched set, which the default order already is
//! (`insight-indexes-scope.md`: `ORDER BY` keeps `SortTopKByKey` in every plan shape), so a column
//! sort costs what the default costs.
//!
//! One responsibility: validate a sort and render it as SQL.

use serde_json::Value;

use crate::case_scope::CaseScope;
use crate::error::InsightsError;
use crate::search_tags::plain_ident;

/// A column sort (`ListQuery::sort`).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SortSpec {
    /// Sources, first non-empty wins. Text sources (`title`, `tag:<key>`) may be chained; a numeric
    /// source stands alone.
    pub by: Vec<String>,
    /// Descending. Empty values stay last either way.
    #[serde(default)]
    pub desc: bool,
}

/// The most sources one sort may chain.
const MAX_SOURCES: usize = 4;

/// What a sort adds to the statement.
pub(crate) struct Order {
    /// Extra projections (`, <expr> AS _k, <expr> AS _e`), empty for the default order.
    pub select: String,
    /// The `ORDER BY` list.
    pub order_by: String,
    pub bindings: Vec<(String, Value)>,
}

/// The default order: newest activity first, the id breaking same-`ts` ties.
const DEFAULT_ORDER: &str = "_ts DESC, _id DESC";

/// Refuse a sort the SQL could not express safely. Run before any read, so a bad sort is an error,
/// never a silently different order.
pub(crate) fn validate(spec: &SortSpec, case: Option<&CaseScope>) -> Result<(), InsightsError> {
    let bad = |m: String| Err(InsightsError::BadInput(format!("sort: {m}")));
    if spec.by.is_empty() || spec.by.len() > MAX_SOURCES {
        return bad(format!("`by` needs 1 to {MAX_SOURCES} sources"));
    }
    for src in &spec.by {
        match source(src) {
            None => return bad(format!("unknown source {src:?}")),
            Some(Source::Numeric(_)) if spec.by.len() > 1 => {
                return bad(format!("{src:?} cannot be chained with other sources"))
            }
            Some(Source::Numeric("case_stage")) if case.is_none() => {
                return bad("`case_stage` needs a case lens".into())
            }
            _ => {}
        }
    }
    Ok(())
}

enum Source<'a> {
    Title,
    Tag(&'a str),
    Numeric(&'a str),
}

fn source(s: &str) -> Option<Source<'_>> {
    match s {
        "title" => Some(Source::Title),
        "severity" | "last_ts" | "first_ts" | "count" | "case_stage" => Some(Source::Numeric(s)),
        _ => s
            .strip_prefix("tag:")
            .filter(|k| plain_ident(k))
            .map(Source::Tag),
    }
}

/// Render `spec` (already [`validate`]d) as SQL. `None` ⇒ the default order, unchanged.
pub(crate) fn order(spec: Option<&SortSpec>, case: Option<&CaseScope>) -> Order {
    let Some(spec) = spec else {
        return Order {
            select: String::new(),
            order_by: DEFAULT_ORDER.into(),
            bindings: Vec::new(),
        };
    };
    let mut bindings = Vec::new();
    let key = match spec.by.as_slice() {
        [one] if matches!(source(one), Some(Source::Numeric(_))) => {
            numeric(one, case, &mut bindings)
        }
        many => text(many),
    };
    let dir = if spec.desc { "DESC" } else { "ASC" };
    Order {
        select: format!(", {key} AS _k, ({key} IS NONE OR {key} = '') AS _e"),
        order_by: format!("_e ASC, _k {dir}, {DEFAULT_ORDER}"),
        bindings,
    }
}

/// First non-empty text source, lowercased. `''` when every source is empty.
fn text(sources: &[String]) -> String {
    // Built right to left: IF a is non-empty THEN a ELSE (the rest).
    let mut expr = "''".to_string();
    for src in sources.iter().rev() {
        let field = match source(src) {
            Some(Source::Tag(k)) => format!("data.tags.{k}"),
            _ => "data.title".to_string(),
        };
        expr = format!("(IF {field} != NONE AND {field} != '' THEN {field} ELSE {expr} END)");
    }
    format!("string::lowercase({expr})")
}

fn numeric(src: &str, case: Option<&CaseScope>, bindings: &mut Vec<(String, Value)>) -> String {
    match src {
        // The closed set, ranked as `Severity::rank` ranks it (a stored word does not order).
        "severity" => "(IF data.severity = 'critical' THEN 2 \
                       ELSE IF data.severity = 'warning' THEN 1 ELSE 0 END)"
            .into(),
        // The host's stage order; a detection with no known case has no stage (empty ⇒ last).
        "case_stage" => {
            let Some(scope) = case else {
                return "NONE".into();
            };
            let mut expr = "NONE".to_string();
            for (rank, stage) in scope.stage_order.iter().enumerate().rev() {
                let name = format!("stage_rank_{rank}");
                let ids: Vec<Value> = scope
                    .stage_of
                    .iter()
                    .filter(|(_, s)| *s == stage)
                    .map(|(id, _)| Value::String(id.clone()))
                    .collect();
                bindings.push((name.clone(), Value::Array(ids)));
                expr = format!("(IF data.case_id IN ${name} THEN {rank} ELSE {expr} END)");
            }
            expr
        }
        other => format!("data.{other}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(by: &[&str]) -> SortSpec {
        SortSpec {
            by: by.iter().map(|s| s.to_string()).collect(),
            desc: false,
        }
    }

    #[test]
    fn text_sources_chain_and_a_numeric_one_stands_alone() {
        assert!(validate(&spec(&["tag:short_name", "tag:insight", "title"]), None).is_ok());
        assert!(validate(&spec(&["severity"]), None).is_ok());
        assert!(validate(&spec(&["severity", "title"]), None).is_err());
        assert!(validate(&spec(&[]), None).is_err());
    }

    #[test]
    fn a_source_that_could_break_the_sql_is_refused() {
        for bad in [
            "tag:Site",
            "tag:site;DROP",
            "tags.site",
            "data.title",
            "tag:",
        ] {
            assert!(validate(&spec(&[bad]), None).is_err(), "{bad:?}");
        }
        assert!(
            validate(&spec(&["case_stage"]), None).is_err(),
            "no lens, no stage"
        );
    }
}
