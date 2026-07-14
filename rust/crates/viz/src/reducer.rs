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
        // ── Tranche 2 (viz grafana-parity-backend scope) — defined over raw values, before the
        // numeric guard (they answer even for all-null / non-numeric columns).
        "allIsNull" => Value::from(values.iter().all(Value::is_null)),
        "allIsZero" => Value::from(!nums.is_empty() && nums.iter().all(|n| *n == 0.0)),
        "distinctCount" => {
            let mut seen: Vec<&Value> = Vec::new();
            for v in values.iter().filter(|v| !v.is_null()) {
                if !seen.contains(&v) {
                    seen.push(v);
                }
            }
            Value::from(seen.len())
        }
        "changeCount" => {
            // Changes between consecutive NON-NULL values (Grafana skips nulls in the walk).
            let non_null: Vec<&Value> = values.iter().filter(|v| !v.is_null()).collect();
            Value::from(non_null.windows(2).filter(|w| w[0] != w[1]).count())
        }
        _ if nums.is_empty() => Value::Null,
        // ── Tranche 2 numeric calcs (Grafana `doStandardCalcs` semantics, pinned):
        // diff = last − first; diffperc = diff / first (a RATIO, not ×100 — the client's percent
        // unit scales it); delta = Σ positive increments; step = min consecutive difference.
        "diff" => num(nums[nums.len() - 1] - nums[0]),
        "diffperc" => {
            if nums[0] == 0.0 {
                Value::Null // Grafana leaves diffperc unset when first is 0 — honest null, not inf.
            } else {
                num((nums[nums.len() - 1] - nums[0]) / nums[0])
            }
        }
        "delta" => num(nums.windows(2).map(|w| (w[1] - w[0]).max(0.0)).sum::<f64>()),
        "step" => {
            if nums.len() < 2 {
                Value::Null // no consecutive pair — nothing to step over.
            } else {
                num(nums
                    .windows(2)
                    .map(|w| w[1] - w[0])
                    .fold(f64::INFINITY, f64::min))
            }
        }
        "median" => {
            let mut sorted = nums.clone();
            sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            let mid = sorted.len() / 2;
            if sorted.len() % 2 == 1 {
                num(sorted[mid])
            } else {
                num((sorted[mid - 1] + sorted[mid]) / 2.0) // even count → mean of the two middles.
            }
        }
        // Population variance/stdDev (÷ n, Grafana's calculateStdDev).
        "variance" => num(variance(&nums)),
        "stdDev" => num(variance(&nums).sqrt()),
        // The general `pNN` pattern (1–99): any imported percentile COMPUTES rather than degrades
        // (the scope's pin). Grafana's calculatePercentile: sorted[floor(p/100 · (n−1))] — nearest
        // rank on the sorted column, no interpolation.
        p if percentile_of(p).is_some() => {
            let pct = percentile_of(p).expect("guard matched");
            let mut sorted = nums.clone();
            sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            let idx = ((pct / 100.0) * (sorted.len() - 1) as f64).floor() as usize;
            num(sorted[idx])
        }
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

/// `"pNN"` → `Some(NN as f64)` for NN in 1..=99, else `None` — the general percentile pattern
/// (grafana-parity scope: the picker offers p25/p50/…, but ANY imported `pNN` computes).
fn percentile_of(calc: &str) -> Option<f64> {
    let n: u32 = calc.strip_prefix('p')?.parse().ok()?;
    (1..=99).contains(&n).then_some(n as f64)
}

/// Population variance (÷ n — Grafana's `calculateStdDev`). Caller guarantees non-empty.
fn variance(nums: &[f64]) -> f64 {
    let mean = nums.iter().sum::<f64>() / nums.len() as f64;
    nums.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / nums.len() as f64
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

    /// The tranche-2 calc table over a shared numeric fixture (grafana-parity scope's testing
    /// plan), incl. the null/non-numeric skip discipline.
    #[test]
    fn tranche_2_numeric_calcs() {
        // Fixture: 10, (null), 20, 5, 5 — nums = [10, 20, 5, 5].
        let v = vec![json!(10), json!(null), json!(20), json!(5), json!(5)];
        assert_eq!(reduce_field("diff", &v), json!(-5.0)); // last − first
        assert_eq!(reduce_field("diffperc", &v), json!(-0.5)); // ratio, not ×100
        assert_eq!(reduce_field("delta", &v), json!(10.0)); // only the +10 rise counts
        assert_eq!(reduce_field("step", &v), json!(-15.0)); // min consecutive difference
        assert_eq!(reduce_field("median", &v), json!(7.5)); // even count → mean of middles
        assert_eq!(reduce_field("variance", &v), json!(37.5)); // population (÷ n)
        assert_eq!(reduce_field("stdDev", &v).as_f64().unwrap(), 37.5f64.sqrt());
        assert_eq!(reduce_field("distinctCount", &v), json!(3)); // 10, 20, 5 (null skipped)
        assert_eq!(reduce_field("changeCount", &v), json!(2)); // 10→20, 20→5 (5→5 no change)
        assert_eq!(reduce_field("allIsZero", &v), json!(false));
        assert_eq!(reduce_field("allIsNull", &v), json!(false));
    }

    /// The general `pNN` pattern: any 1–99 computes (Grafana's floor(p/100·(n−1)) nearest rank);
    /// out-of-range or malformed stays an unknown calc (null).
    #[test]
    fn pnn_percentiles_compute_for_any_1_to_99() {
        let v: Vec<Value> = (1..=10).map(|n| json!(n)).collect();
        assert_eq!(reduce_field("p50", &v), json!(5.0)); // floor(0.5·9)=4 → sorted[4]=5
        assert_eq!(reduce_field("p90", &v), json!(9.0)); // floor(0.9·9)=8 → 9
        assert_eq!(reduce_field("p1", &v), json!(1.0));
        assert_eq!(reduce_field("p99", &v), json!(9.0)); // floor(0.99·9)=8
        assert_eq!(reduce_field("p37", &v), json!(4.0)); // arbitrary NN computes too
        assert_eq!(reduce_field("p0", &v), Value::Null); // out of the 1–99 pattern
        assert_eq!(reduce_field("p100", &v), Value::Null);
        assert_eq!(reduce_field("pxx", &v), Value::Null);
    }

    /// The boolean/count calcs answer even where numeric calcs go null; edge shapes honest.
    #[test]
    fn tranche_2_edges() {
        assert_eq!(reduce_field("allIsNull", &[json!(null)]), json!(true));
        assert_eq!(reduce_field("allIsNull", &[]), json!(true));
        assert_eq!(
            reduce_field("allIsZero", &[json!(0), json!(0.0)]),
            json!(true)
        );
        assert_eq!(reduce_field("allIsZero", &[]), json!(false)); // no values ≠ all zero
        assert_eq!(
            reduce_field("distinctCount", &[json!("a"), json!("a"), json!("b")]),
            json!(2)
        ); // non-numeric values still count
        assert_eq!(reduce_field("step", &[json!(1)]), Value::Null); // no pair to step over
        assert_eq!(reduce_field("median", &[json!(7)]), json!(7.0));
        assert_eq!(reduce_field("diffperc", &[json!(0), json!(5)]), Value::Null); // first = 0 → honest null, not inf
        assert_eq!(reduce_field("delta", &[json!("x")]), Value::Null); // all-non-numeric
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
