//! The roster's column sort, as SQL — one `ORDER BY` for BOTH read paths (the pushed-down page and the
//! counted scan), so page 1 (counted) and page 2 (not) can never disagree about the order and skip or
//! repeat a row between them.
//!
//! **lb names no column.** A sort is a list of SOURCES tried left to right, the first non-empty one
//! wins: `title`, `tag:<key>`, or one of the numeric facts (`severity`, `last_ts`, `first_ts`,
//! `count`). A roster whose Name column shows `short_name`, else the `insight` tag, else the title
//! sorts by `["tag:short_name", "tag:insight", "title"]`. The case lens's `case_stage`,
//! `case_status`, `case_owner` and `case_ref` are validated here but sorted in memory (`lens_sort`).
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

use crate::case_scope::CaseScope;
use crate::error::InsightsError;
use crate::text_expr::{column_text, text_source};

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
            Some(Source::Numeric(
                lens @ ("case_stage" | "case_status" | "case_owner" | "case_ref"),
            )) if case.is_none() => return bad(format!("`{lens}` needs a case lens")),
            _ => {}
        }
    }
    Ok(())
}

enum Source<'a> {
    Text,
    Numeric(&'a str),
}

fn source(s: &str) -> Option<Source<'_>> {
    match s {
        "severity" | "last_ts" | "first_ts" | "count" | "case_stage" | "case_status"
        | "case_owner" | "case_ref" => Some(Source::Numeric(s)),
        _ => text_source(s).map(|_| Source::Text),
    }
}

/// Render `spec` (already [`validate`]d) as SQL. `None` ⇒ the default order, unchanged.
pub(crate) fn order(spec: Option<&SortSpec>) -> Order {
    // The case lens's sorts are done in memory over the matched set (`lens_sort`), not here.
    let Some(spec) = spec.filter(|_| crate::lens_sort::lens_source(spec).is_none()) else {
        return Order {
            select: String::new(),
            order_by: DEFAULT_ORDER.into(),
        };
    };
    let key = match spec.by.as_slice() {
        [one] if matches!(source(one), Some(Source::Numeric(_))) => numeric(one),
        many => column_text(many),
    };
    let dir = if spec.desc { "DESC" } else { "ASC" };
    Order {
        select: format!(", {key} AS _k, ({key} IS NONE OR {key} = '') AS _e"),
        order_by: format!("_e ASC, _k {dir}, {DEFAULT_ORDER}"),
    }
}

fn numeric(src: &str) -> String {
    match src {
        // The closed set, ranked as `Severity::rank` ranks it (a stored word does not order).
        "severity" => "(IF data.severity = 'critical' THEN 2 \
                       ELSE IF data.severity = 'warning' THEN 1 ELSE 0 END)"
            .into(),
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
