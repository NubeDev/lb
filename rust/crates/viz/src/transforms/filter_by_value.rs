//! The `filterByValue` transformer — Grafana's `FilterByValueTransformer` (viz transformations
//! scope). Filters ROWS (not fields) by per-field value matchers. Option shape verbatim:
//! `filters: [{fieldName, config: {id, options}}]`, `type: include|exclude`, `match: any|all`. One
//! responsibility: row selection by value. Pure: rebuilds every field to the kept rows and `relen`s.
//! Honest: a `ValueMatcherID` we don't know never matches (never a silent keep-all).

use serde_json::Value;

use crate::frame::{Field, Frame, Frames};

/// Apply `filterByValue` per frame. For each row, evaluate every filter against its named field's
/// value, combine by `match` (any/all), and keep the row when `(type==include) == combined`.
pub fn apply(frames: Frames, options: &Value) -> Frames {
    let filters = match options.get("filters").and_then(Value::as_array) {
        Some(f) if !f.is_empty() => f.clone(),
        _ => return frames, // no filters → nothing to do (honest no-op).
    };
    let include = options
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("include")
        == "include";
    let match_all = options
        .get("match")
        .and_then(Value::as_str)
        .unwrap_or("any")
        == "all";
    frames
        .into_iter()
        .map(|f| filter_rows(f, &filters, include, match_all))
        .collect()
}

fn filter_rows(frame: Frame, filters: &[Value], include: bool, match_all: bool) -> Frame {
    let kept: Vec<usize> = (0..frame.length)
        .filter(|&row| {
            let mut results = filters.iter().map(|flt| eval_filter(&frame, flt, row));
            let combined = if match_all {
                results.all(|r| r)
            } else {
                results.any(|r| r)
            };
            include == combined
        })
        .collect();
    let fields: Vec<Field> = frame
        .fields
        .iter()
        .map(|f| {
            let values: Vec<Value> = kept.iter().map(|&i| f.at(i)).collect();
            Field::typed(f.name.clone(), f.ty, values)
        })
        .collect();
    let mut out = Frame::new(fields).relen();
    out.ref_id = frame.ref_id;
    out.name = frame.name;
    out
}

/// Evaluate one filter against `row`: read the named field's value and test it with the configured
/// `ValueMatcherID`. A missing field or unknown matcher → false (honest non-match).
fn eval_filter(frame: &Frame, filter: &Value, row: usize) -> bool {
    let field_name = match filter.get("fieldName").and_then(Value::as_str) {
        Some(n) => n,
        None => return false,
    };
    let value = match frame.field(field_name) {
        Some(f) => f.at(row),
        None => return false,
    };
    let config = filter.get("config");
    let id = config
        .and_then(|c| c.get("id"))
        .and_then(Value::as_str)
        .unwrap_or("");
    let opts = config.and_then(|c| c.get("options"));
    matches_value(id, opts, &value)
}

/// A Grafana `ValueMatcherID` test over a single cell value. Numeric comparisons skip non-numeric
/// cells (honest false, never a coerced 0).
fn matches_value(id: &str, opts: Option<&Value>, value: &Value) -> bool {
    let n = value.as_f64();
    let opt_num = opts.and_then(|o| o.get("value")).and_then(Value::as_f64);
    match id {
        "greater" => bin(n, opt_num, |a, b| a > b),
        "greaterOrEqual" => bin(n, opt_num, |a, b| a >= b),
        "lower" => bin(n, opt_num, |a, b| a < b),
        "lowerOrEqual" => bin(n, opt_num, |a, b| a <= b),
        "equal" => equal(value, opts),
        "notEqual" => !equal(value, opts),
        "between" => {
            let from = opts.and_then(|o| o.get("from")).and_then(Value::as_f64);
            let to = opts.and_then(|o| o.get("to")).and_then(Value::as_f64);
            match (n, from, to) {
                (Some(x), Some(lo), Some(hi)) => x >= lo && x <= hi,
                _ => false,
            }
        }
        "isNull" => value.is_null(),
        "isNotNull" => !value.is_null(),
        "regex" => {
            let pat = opts.and_then(|o| o.get("value")).and_then(Value::as_str);
            match (pat, value.as_str()) {
                (Some(p), Some(s)) => crate::config::Matcher {
                    id: "byRegexp".into(),
                    options: Value::from(p),
                }
                .matches_field(s, ""),
                _ => false,
            }
        }
        _ => false,
    }
}

/// A numeric binary test that is false unless both sides are numbers (honest skip of non-numeric).
fn bin(a: Option<f64>, b: Option<f64>, f: impl Fn(f64, f64) -> bool) -> bool {
    matches!((a, b), (Some(x), Some(y)) if f(x, y))
}

/// Equality against the matcher's `options.value` — numeric when both are numbers, else string-wise,
/// else raw JSON equality (so a bool/string target still compares honestly).
fn equal(value: &Value, opts: Option<&Value>) -> bool {
    let target = match opts.and_then(|o| o.get("value")) {
        Some(t) => t,
        None => return false,
    };
    if let (Some(a), Some(b)) = (value.as_f64(), target.as_f64()) {
        return a == b;
    }
    if let (Some(a), Some(b)) = (value.as_str(), target.as_str()) {
        return a == b;
    }
    value == target
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn seeded() -> Frames {
        vec![Frame::new(vec![
            Field::new("v", vec![json!(5), json!(15), json!(25), json!(null)]),
            Field::new(
                "label",
                vec![json!("a"), json!("b"), json!("c"), json!("d")],
            ),
        ])]
    }

    #[test]
    fn include_greater_keeps_matching_rows() {
        let out = apply(
            seeded(),
            &json!({
                "type": "include",
                "match": "any",
                "filters": [{ "fieldName": "v", "config": { "id": "greater", "options": { "value": 10 } } }],
            }),
        );
        assert_eq!(
            out[0].field("v").unwrap().values,
            vec![json!(15), json!(25)]
        );
        assert_eq!(
            out[0].field("label").unwrap().values,
            vec![json!("b"), json!("c")]
        );
        assert_eq!(out[0].length, 2);
    }

    #[test]
    fn match_all_combines_filters() {
        let out = apply(
            seeded(),
            &json!({
                "type": "include",
                "match": "all",
                "filters": [
                    { "fieldName": "v", "config": { "id": "greaterOrEqual", "options": { "value": 5 } } },
                    { "fieldName": "v", "config": { "id": "lower", "options": { "value": 25 } } },
                ],
            }),
        );
        assert_eq!(out[0].field("v").unwrap().values, vec![json!(5), json!(15)]);
    }

    #[test]
    fn exclude_is_complement() {
        let out = apply(
            seeded(),
            &json!({
                "type": "exclude",
                "match": "any",
                "filters": [{ "fieldName": "v", "config": { "id": "isNull", "options": {} } }],
            }),
        );
        // exclude nulls → drop the row where v is null.
        assert_eq!(out[0].length, 3);
        assert_eq!(
            out[0].field("v").unwrap().values,
            vec![json!(5), json!(15), json!(25)]
        );
    }

    #[test]
    fn non_numeric_field_never_matches_numeric_filter() {
        let out = apply(
            seeded(),
            &json!({
                "type": "include",
                "match": "any",
                "filters": [{ "fieldName": "label", "config": { "id": "greater", "options": { "value": 0 } } }],
            }),
        );
        // no string row passes a numeric `greater` → empty result, not a fabricated keep-all.
        assert_eq!(out[0].length, 0);
    }
}
