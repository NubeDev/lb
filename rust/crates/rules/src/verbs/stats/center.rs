//! `stats` center/spread + quantiles: `sum`/`mean`/`median`/`mode`/`min_of`/`max_of`/`range_of`/
//! `variance`/`std_dev`/`sem`, and `percentile`/`quantiles`/`iqr`. Aggregations skip missing
//! values (module policy); everything undefined on an empty array errors clearly.

use rhai::{Array, Dynamic, Engine, EvalAltResult};

use super::numeric::{
    floats_to_array, mean_of, need, nums, percentile_of, scalar, sorted, variance_of,
};

pub(super) fn register(engine: &mut Engine) {
    engine.register_fn("sum", |a: Array| nums(&a).iter().sum::<f64>());
    engine.register_fn("mean", mean_v);
    engine.register_fn("median", median_v);
    engine.register_fn("mode", mode_v);
    engine.register_fn("min_of", min_v);
    engine.register_fn("max_of", max_v);
    engine.register_fn("range_of", range_v);
    engine.register_fn("variance", |a: Array| variance_of(&nums(&a), "variance"));
    engine.register_fn("std_dev", std_dev_v);
    engine.register_fn("sem", sem_v);
    engine.register_fn("percentile", |a: Array, p: Dynamic| {
        percentile_of(
            &sorted(&nums(&a)),
            scalar(&p, "percentile", "p")?,
            "percentile",
        )
    });
    engine.register_fn("quantiles", quantiles_v);
    engine.register_fn("iqr", iqr_v);
}

fn range_v(a: Array) -> Result<f64, Box<EvalAltResult>> {
    Ok(max_v(a.clone())? - min_v(a)?)
}

fn std_dev_v(a: Array) -> Result<f64, Box<EvalAltResult>> {
    Ok(variance_of(&nums(&a), "std_dev")?.sqrt())
}

fn mean_v(a: Array) -> Result<f64, Box<EvalAltResult>> {
    let v = nums(&a);
    need(&v, 1, "mean")?;
    Ok(mean_of(&v))
}

fn median_v(a: Array) -> Result<f64, Box<EvalAltResult>> {
    percentile_of(&sorted(&nums(&a)), 50.0, "median")
}

/// Most frequent numeric value; ties break to the smallest value (deterministic, documented).
fn mode_v(a: Array) -> Result<f64, Box<EvalAltResult>> {
    let s = sorted(&nums(&a));
    need(&s, 1, "mode")?;
    let (mut best, mut best_n) = (s[0], 0usize);
    let mut i = 0;
    while i < s.len() {
        let mut j = i;
        while j + 1 < s.len() && s[j + 1] == s[i] {
            j += 1;
        }
        if j - i + 1 > best_n {
            best = s[i];
            best_n = j - i + 1;
        }
        i = j + 1;
    }
    Ok(best)
}

fn min_v(a: Array) -> Result<f64, Box<EvalAltResult>> {
    let v = nums(&a);
    need(&v, 1, "min_of")?;
    Ok(v.iter().copied().fold(f64::INFINITY, f64::min))
}

fn max_v(a: Array) -> Result<f64, Box<EvalAltResult>> {
    let v = nums(&a);
    need(&v, 1, "max_of")?;
    Ok(v.iter().copied().fold(f64::NEG_INFINITY, f64::max))
}

/// Standard error of the mean: sample std over sqrt(n).
fn sem_v(a: Array) -> Result<f64, Box<EvalAltResult>> {
    let v = nums(&a);
    Ok(variance_of(&v, "sem")?.sqrt() / (v.len() as f64).sqrt())
}

fn quantiles_v(a: Array, ps: Array) -> Result<Array, Box<EvalAltResult>> {
    let asc = sorted(&nums(&a));
    let mut out = Vec::with_capacity(ps.len());
    for p in &ps {
        out.push(percentile_of(
            &asc,
            scalar(p, "quantiles", "each p")?,
            "quantiles",
        )?);
    }
    Ok(floats_to_array(out))
}

fn iqr_v(a: Array) -> Result<f64, Box<EvalAltResult>> {
    let asc = sorted(&nums(&a));
    Ok(percentile_of(&asc, 75.0, "iqr")? - percentile_of(&asc, 25.0, "iqr")?)
}

#[cfg(test)]
mod tests {
    use super::super::numeric::{close, fa, ga};
    use super::*;

    #[test]
    fn median_odd_and_even() {
        assert!(close(median_v(fa(&[3.0, 1.0, 2.0])).unwrap(), 2.0));
        assert!(close(median_v(fa(&[4.0, 1.0, 3.0, 2.0])).unwrap(), 2.5));
    }

    #[test]
    fn mode_picks_most_frequent_and_breaks_ties_low() {
        assert!(close(mode_v(fa(&[1.0, 2.0, 2.0, 3.0])).unwrap(), 2.0));
        // Tie between 1 and 2 (both twice) → the smallest wins.
        assert!(close(mode_v(fa(&[2.0, 1.0, 2.0, 1.0, 3.0])).unwrap(), 1.0));
    }

    #[test]
    fn variance_and_std_are_sample_n_minus_1() {
        // [1,2,3,4]: mean 2.5, Σdev² = 5, sample var = 5/3.
        let v = nums(&fa(&[1.0, 2.0, 3.0, 4.0]));
        assert!(close(variance_of(&v, "t").unwrap(), 5.0 / 3.0));
        assert!(close(
            variance_of(&v, "t").unwrap().sqrt(),
            (5.0f64 / 3.0).sqrt()
        ));
        // sem = std / sqrt(n).
        let sem = sem_v(fa(&[1.0, 2.0, 3.0, 4.0])).unwrap();
        assert!(close(sem, (5.0f64 / 3.0).sqrt() / 2.0));
    }

    #[test]
    fn percentile_interpolates_linearly() {
        let a = fa(&[1.0, 2.0, 3.0, 4.0]);
        let p = |p: f64| percentile_of(&sorted(&nums(&a)), p, "t").unwrap();
        assert!(close(p(25.0), 1.75)); // rank 0.75 between 1 and 2
        assert!(close(p(50.0), 2.5));
        assert!(close(p(0.0), 1.0));
        assert!(close(p(100.0), 4.0));
    }

    #[test]
    fn quantiles_and_iqr() {
        let qs = quantiles_v(fa(&[1.0, 2.0, 3.0, 4.0, 5.0]), fa(&[25.0, 75.0])).unwrap();
        assert!(close(qs[0].as_float().unwrap(), 2.0));
        assert!(close(qs[1].as_float().unwrap(), 4.0));
        assert!(close(iqr_v(fa(&[1.0, 2.0, 3.0, 4.0, 5.0])).unwrap(), 2.0));
    }

    #[test]
    fn aggregations_skip_missing() {
        // mean([1,(),3]) = 2.0 — the module's canonical policy example.
        assert!(close(
            mean_v(ga(&[Some(1.0), None, Some(3.0)])).unwrap(),
            2.0
        ));
        assert!(close(
            min_v(ga(&[None, Some(5.0), Some(2.0)])).unwrap(),
            2.0
        ));
    }

    #[test]
    fn empty_where_undefined_is_a_clear_error() {
        let err = mean_v(fa(&[])).unwrap_err().to_string();
        assert!(err.contains("mean"), "got: {err}");
        assert!(median_v(ga(&[None])).is_err());
        assert!(variance_of(&nums(&fa(&[1.0])), "variance").is_err());
    }
}
