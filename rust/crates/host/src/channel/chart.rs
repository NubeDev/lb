//! The **chart picker** — a pure, deterministic function that turns a result row-set into a chart
//! spec (channels-query-charts scope). It runs host-side and is embedded in the `query_result`
//! payload so EVERY subscriber renders the same chart without re-deriving it (a host decision, not
//! a per-client guess). Conservative by design: it fails safe to `chart: null` (table-only) rather
//! than render a misleading chart.
//!
//! The rule (scope "Intent"):
//!   - a **temporal** first column (x) + numeric columns → a **line** chart;
//!   - a **categorical** first column (x) + numeric columns → a **bar** chart;
//!   - a single numeric column, many rows, no category → a **histogram**;
//!   - otherwise nothing plottable → `None` (the UI shows the table only).
//!
//! Column types are INFERRED from the row values (the federation result carries names, not types):
//! numeric = all sampled values are JSON numbers; temporal = all sampled values are ISO-8601 date
//! strings (`YYYY-MM-DD…`); categorical = anything else. Inferring from data (not a header guess)
//! is what makes the picker honest about what is actually plottable.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A chart series — one numeric column plotted against the x axis.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChartSeries {
    pub field: String,
}

/// The chart spec embedded in a `query_result` payload. `x` is the category/temporal axis;
/// `series` the numeric columns plotted. `type` is the renderer hint the UI switches on. `bins`
/// is present only for a histogram (the suggested bucket count).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChartSpec {
    #[serde(rename = "type")]
    pub kind: ChartKind,
    pub x: String,
    pub series: Vec<ChartSeries>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bins: Option<u32>,
}

/// The chart kinds the picker emits. Rendered verbatim as the `type` field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChartKind {
    Line,
    Bar,
    Histogram,
}

/// The semantic type inferred for one column from its values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ColType {
    Numeric,
    Temporal,
    Categorical,
}

/// Pick a chart for a result set with `columns` and `rows` (JSON objects keyed by column). Returns
/// `None` when nothing is safely plottable — the UI then shows the table only. Pure: no IO, no
/// wall-clock; deterministic for a fixed input (testing §3).
pub fn pick_chart(columns: &[String], rows: &[Value]) -> Option<ChartSpec> {
    if columns.is_empty() || rows.is_empty() {
        return None;
    }

    let types: Vec<(String, ColType)> = columns
        .iter()
        .map(|c| (c.clone(), infer_column_type(c, rows)))
        .collect();

    let numeric: Vec<&String> = types
        .iter()
        .filter(|(_, t)| *t == ColType::Numeric)
        .map(|(c, _)| c)
        .collect();
    let temporal: Vec<&String> = types
        .iter()
        .filter(|(_, t)| *t == ColType::Temporal)
        .map(|(c, _)| c)
        .collect();
    let categorical: Vec<&String> = types
        .iter()
        .filter(|(_, t)| *t == ColType::Categorical)
        .map(|(c, _)| c)
        .collect();

    // Line: a temporal x axis with at least one numeric series.
    if let Some(x) = temporal.first() {
        if !numeric.is_empty() {
            return Some(ChartSpec {
                kind: ChartKind::Line,
                x: x.to_string(),
                series: numeric
                    .iter()
                    .map(|f| ChartSeries {
                        field: (*f).clone(),
                    })
                    .collect(),
                bins: None,
            });
        }
    }

    // Bar: a categorical x axis with at least one numeric series.
    if let Some(x) = categorical.first() {
        if !numeric.is_empty() {
            return Some(ChartSpec {
                kind: ChartKind::Bar,
                x: x.to_string(),
                series: numeric
                    .iter()
                    .map(|f| ChartSeries {
                        field: (*f).clone(),
                    })
                    .collect(),
                bins: None,
            });
        }
    }

    // Histogram: exactly one numeric column and enough rows to bucket.
    if numeric.len() == 1 && rows.len() >= 4 {
        let field = numeric[0];
        return Some(ChartSpec {
            kind: ChartKind::Histogram,
            x: field.clone(),
            series: vec![ChartSeries {
                field: field.clone(),
            }],
            bins: Some(suggest_bins(rows.len())),
        });
    }

    None
}

/// Infer a column's semantic type from a sample of its non-null values (up to 64 rows). Empty /
/// all-null → Categorical (the safe default — never claim numeric/temporal we can't back up).
fn infer_column_type(col: &str, rows: &[Value]) -> ColType {
    let mut numeric = 0usize;
    let mut temporal = 0usize;
    let mut other = 0usize;
    for r in rows.iter().take(64) {
        match r.get(col) {
            Some(Value::Number(_)) => numeric += 1,
            Some(Value::String(s)) if looks_temporal(s) => temporal += 1,
            Some(Value::String(_))
            | Some(Value::Bool(_))
            | Some(Value::Object(_))
            | Some(Value::Array(_)) => other += 1,
            Some(Value::Null) | None => {} // null/absent → no signal
        }
    }
    if other == 0 && temporal == 0 && numeric > 0 {
        ColType::Numeric
    } else if other == 0 && numeric == 0 && temporal > 0 {
        ColType::Temporal
    } else if other == 0 && temporal > 0 {
        // mixed numeric+temporal with no plain strings → call it temporal (dates may sort)
        ColType::Temporal
    } else {
        ColType::Categorical
    }
}

/// Does `s` look like an ISO-8601 date/datetime (`YYYY-MM-DD…`)? Conservative — only the canonical
/// calendar prefix counts, so a free-text column is never mistaken for temporal.
fn looks_temporal(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() >= 10
        && b[4] == b'-'
        && b[7] == b'-'
        && b[..4].iter().all(|c| c.is_ascii_digit())
        && b[5..7].iter().all(|c| c.is_ascii_digit())
        && b[8..10].iter().all(|c| c.is_ascii_digit())
}

/// Suggest a histogram bucket count for `n` rows — the square-root rule (cheap, stable, good
/// enough for an auto-plot; clamped to a sane 5..32 range so a tiny or huge set still reads).
fn suggest_bins(n: usize) -> u32 {
    let raw = (n as f64).sqrt().round() as u32;
    raw.clamp(5, 32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn rows(values: &[Value]) -> Vec<Value> {
        values.iter().cloned().collect()
    }

    #[test]
    fn line_for_temporal_x_and_numeric_series() {
        let cols = vec!["day".into(), "signups".into()];
        let rs = rows(&[
            json!({ "day": "2024-01-01", "signups": 3 }),
            json!({ "day": "2024-01-02", "signups": 5 }),
            json!({ "day": "2024-01-03", "signups": 7 }),
        ]);
        let chart = pick_chart(&cols, &rs).expect("plottable");
        assert_eq!(chart.kind, ChartKind::Line);
        assert_eq!(chart.x, "day");
        assert_eq!(chart.series.len(), 1);
        assert_eq!(chart.series[0].field, "signups");
    }

    #[test]
    fn bar_for_categorical_x_and_numeric_series() {
        let cols = vec!["region".into(), "sales".into()];
        let rs = rows(&[
            json!({ "region": "north", "sales": 10 }),
            json!({ "region": "south", "sales": 20 }),
        ]);
        let chart = pick_chart(&cols, &rs).expect("plottable");
        assert_eq!(chart.kind, ChartKind::Bar);
        assert_eq!(chart.x, "region");
        assert_eq!(chart.series[0].field, "sales");
    }

    #[test]
    fn histogram_for_single_numeric_many_rows() {
        let cols = vec!["latency_ms".into()];
        let rs: Vec<Value> = (0..20).map(|i| json!({ "latency_ms": i * 10 })).collect();
        let chart = pick_chart(&cols, &rs).expect("plottable");
        assert_eq!(chart.kind, ChartKind::Histogram);
        assert_eq!(chart.x, "latency_ms");
        assert!(chart.bins.is_some());
    }

    #[test]
    fn none_when_nothing_plottable() {
        // All-text columns, single row.
        let cols = vec!["name".into(), "note".into()];
        let rs = rows(&[json!({ "name": "a", "note": "hi" })]);
        assert!(pick_chart(&cols, &rs).is_none());
    }

    #[test]
    fn none_for_empty_result() {
        assert!(pick_chart(&["x".into()], &[]).is_none());
        assert!(pick_chart(&[], &[json!({})]).is_none());
    }

    #[test]
    fn categorical_takes_priority_when_no_temporal() {
        // A categorical x with a numeric series → bar, even when a second numeric column exists.
        let cols = vec!["kind".into(), "count".into(), "total".into()];
        let rs = rows(&[
            json!({ "kind": "a", "count": 1, "total": 2 }),
            json!({ "kind": "b", "count": 3, "total": 4 }),
        ]);
        let chart = pick_chart(&cols, &rs).expect("plottable");
        assert_eq!(chart.kind, ChartKind::Bar);
        assert_eq!(chart.series.len(), 2);
    }

    #[test]
    fn nulls_do_not_spoil_type_inference() {
        let cols = vec!["day".into(), "v".into()];
        let rs = rows(&[
            json!({ "day": "2024-01-01", "v": 1 }),
            json!({ "day": "2024-01-02", "v": null }),
            json!({ "day": "2024-01-03", "v": 3 }),
        ]);
        let chart = pick_chart(&cols, &rs).expect("plottable");
        assert_eq!(chart.kind, ChartKind::Line);
    }
}
