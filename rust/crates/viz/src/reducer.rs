//! The shared field **reducer** — Grafana's `ReducerID` calc set over a column of canonical values
//! (viz transformations scope). One place so `reduce`, `groupBy`, and `calculateField`'s reduce-row
//! mode never drift. Pure: numeric calcs skip non-numeric/null cells and return `None` for an
//! empty/all-non-numeric column (an HONEST no-value, never a fabricated 0 — the no-mock rule).

use serde_json::Value;

/// Apply a Grafana `ReducerID` over a column. Returns the reduced value, or `Null` when there is
/// nothing to reduce (empty / all non-numeric for a numeric calc) — never a fabricated `0`.
pub fn reduce_field(calc: &str, values: &[Value]) -> Value {
    let nums: Vec<f64> = values.iter().filter_map(Value::as_f64).collect();
    match calc {
        // Count is defined even with no numbers (it counts non-null cells, Grafana's `count`).
        "count" => Value::from(values.iter().filter(|v| !v.is_null()).count()),
        "first" | "firstNotNull" => values
            .iter()
            .find(|v| calc == "first" || !v.is_null())
            .cloned()
            .unwrap_or(Value::Null),
        "last" | "lastNotNull" => values
            .iter()
            .rev()
            .find(|v| calc == "last" || !v.is_null())
            .cloned()
            .unwrap_or(Value::Null),
        _ if nums.is_empty() => Value::Null,
        "sum" => num(nums.iter().sum()),
        "mean" | "avg" => num(nums.iter().sum::<f64>() / nums.len() as f64),
        "min" => num(nums.iter().cloned().fold(f64::INFINITY, f64::min)),
        "max" => num(nums.iter().cloned().fold(f64::NEG_INFINITY, f64::max)),
        "range" => {
            let mn = nums.iter().cloned().fold(f64::INFINITY, f64::min);
            let mx = nums.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            num(mx - mn)
        }
        // Unknown calc → honest null (never a guessed value).
        _ => Value::Null,
    }
}

/// JSON number from an f64, guarding NaN/inf (→ Null, never an invalid wire value).
fn num(f: f64) -> Value {
    if f.is_finite() {
        serde_json::Number::from_f64(f)
            .map(Value::Number)
            .unwrap_or(Value::Null)
    } else {
        Value::Null
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn numeric_calcs() {
        let v = vec![json!(1), json!(3), json!(2)];
        assert_eq!(reduce_field("sum", &v), json!(6.0));
        assert_eq!(reduce_field("mean", &v), json!(2.0));
        assert_eq!(reduce_field("min", &v), json!(1.0));
        assert_eq!(reduce_field("max", &v), json!(3.0));
        assert_eq!(reduce_field("last", &v), json!(2));
        assert_eq!(reduce_field("count", &v), json!(3));
    }

    #[test]
    fn empty_and_non_numeric_is_null_not_zero() {
        assert_eq!(reduce_field("sum", &[]), Value::Null);
        assert_eq!(
            reduce_field("mean", &[json!("x"), json!(null)]),
            Value::Null
        );
        // count still works over non-numeric (counts non-null cells).
        assert_eq!(reduce_field("count", &[json!("x"), json!(null)]), json!(1));
    }
}
