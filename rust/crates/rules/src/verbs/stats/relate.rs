//! `stats` relate: `corr` (Pearson), `spearman`, `cov` (sample), `linreg` (least squares →
//! `#{slope, intercept, r2}`), `predict`, `forecast_linear`. Two-array verbs require equal input
//! lengths and pair positions where BOTH sides are numeric (pairwise deletion of missing).

use rhai::{Array, Dynamic, Engine, EvalAltResult, Map};

use crate::grid::rhai_err;

use super::numeric::{floats_to_array, mean_of, need, num_of, ranks_of, scalar, slots};

pub(super) fn register(engine: &mut Engine) {
    engine.register_fn("corr", |a: Array, b: Array| {
        let (xs, ys) = pairs(&a, &b, "corr")?;
        pearson(&xs, &ys, "corr")
    });
    engine.register_fn("spearman", |a: Array, b: Array| {
        let (xs, ys) = pairs(&a, &b, "spearman")?;
        pearson(&ranks_of(&xs), &ranks_of(&ys), "spearman")
    });
    engine.register_fn("cov", cov_v);
    engine.register_fn("linreg", |xs: Array, ys: Array| {
        let (xs, ys) = pairs(&xs, &ys, "linreg")?;
        linreg_core(&xs, &ys, "linreg")
    });
    engine.register_fn("predict", predict_v);
    engine.register_fn("forecast_linear", forecast_v);
}

/// Pair two same-length arrays position-wise, keeping only slots numeric on BOTH sides.
fn pairs(a: &Array, b: &Array, verb: &str) -> Result<(Vec<f64>, Vec<f64>), Box<EvalAltResult>> {
    if a.len() != b.len() {
        return Err(rhai_err(format!(
            "{verb}: arrays must be the same length ({} vs {})",
            a.len(),
            b.len()
        )));
    }
    Ok(slots(a)
        .into_iter()
        .zip(slots(b))
        .filter_map(|(x, y)| Some((x?, y?)))
        .unzip())
}

/// Centered sums Σdx², Σdy², Σdxdy over an already-paired sample (n ≥ 2 enforced).
fn csums(xs: &[f64], ys: &[f64], verb: &str) -> Result<(f64, f64, f64), Box<EvalAltResult>> {
    need(xs, 2, verb)?;
    let (mx, my) = (mean_of(xs), mean_of(ys));
    let (mut sxx, mut syy, mut sxy) = (0.0, 0.0, 0.0);
    for (x, y) in xs.iter().zip(ys) {
        sxx += (x - mx) * (x - mx);
        syy += (y - my) * (y - my);
        sxy += (x - mx) * (y - my);
    }
    Ok((sxx, syy, sxy))
}

fn pearson(xs: &[f64], ys: &[f64], verb: &str) -> Result<f64, Box<EvalAltResult>> {
    let (sxx, syy, sxy) = csums(xs, ys, verb)?;
    if sxx == 0.0 || syy == 0.0 {
        return Err(rhai_err(format!("{verb}: zero variance on one side")));
    }
    Ok(sxy / (sxx * syy).sqrt())
}

/// Sample covariance (n-1 denominator) over pairwise-complete positions.
fn cov_v(a: Array, b: Array) -> Result<f64, Box<EvalAltResult>> {
    let (xs, ys) = pairs(&a, &b, "cov")?;
    let (_, _, sxy) = csums(&xs, &ys, "cov")?;
    Ok(sxy / (xs.len() - 1) as f64)
}

/// Least-squares fit. r2 = 1 − SSres/SStot (1.0 when ys are constant — the fit is exact).
fn linreg_core(xs: &[f64], ys: &[f64], verb: &str) -> Result<Map, Box<EvalAltResult>> {
    let (sxx, syy, sxy) = csums(xs, ys, verb)?;
    if sxx == 0.0 {
        return Err(rhai_err(format!(
            "{verb}: zero variance in xs (slope undefined)"
        )));
    }
    let slope = sxy / sxx;
    let intercept = mean_of(ys) - slope * mean_of(xs);
    let ss_res: f64 = xs
        .iter()
        .zip(ys)
        .map(|(x, y)| (y - (slope * x + intercept)).powi(2))
        .sum();
    let r2 = if syy == 0.0 { 1.0 } else { 1.0 - ss_res / syy };
    let mut m = Map::new();
    m.insert("slope".into(), Dynamic::from_float(slope));
    m.insert("intercept".into(), Dynamic::from_float(intercept));
    m.insert("r2".into(), Dynamic::from_float(r2));
    Ok(m)
}

/// Apply a `linreg` model map to one x: `slope * x + intercept`.
fn predict_v(model: Map, x: Dynamic) -> Result<f64, Box<EvalAltResult>> {
    let field = |k: &str| {
        model.get(k).and_then(num_of).ok_or_else(|| {
            rhai_err(format!(
                "predict: model must have a numeric {k:?} (from linreg)"
            ))
        })
    };
    Ok(field("slope")? * scalar(&x, "predict", "x")? + field("intercept")?)
}

/// Fit index→value over the numeric slots (index = original position), then extend the line for
/// the next `n` positions past the end of the input.
fn forecast_v(a: Array, n: i64) -> Result<Array, Box<EvalAltResult>> {
    if n < 0 {
        return Err(rhai_err(format!(
            "forecast_linear: n must be >= 0, got {n}"
        )));
    }
    let (xs, ys): (Vec<f64>, Vec<f64>) = slots(&a)
        .into_iter()
        .enumerate()
        .filter_map(|(i, o)| Some((i as f64, o?)))
        .unzip();
    let m = linreg_core(&xs, &ys, "forecast_linear")?;
    let (slope, intercept) = (
        m["slope"].as_float().unwrap(),
        m["intercept"].as_float().unwrap(),
    );
    Ok(floats_to_array(
        (0..n as usize)
            .map(|k| slope * (a.len() + k) as f64 + intercept)
            .collect(),
    ))
}

#[cfg(test)]
mod tests {
    use super::super::numeric::{close, fa, ga};
    use super::*;

    fn corr_of(a: Array, b: Array) -> Result<f64, Box<EvalAltResult>> {
        let (xs, ys) = pairs(&a, &b, "corr")?;
        pearson(&xs, &ys, "corr")
    }

    #[test]
    fn corr_plus_one_minus_one_zero() {
        let x = || fa(&[1.0, 2.0, 3.0]);
        assert!(close(corr_of(x(), fa(&[3.0, 5.0, 7.0])).unwrap(), 1.0)); // y = 2x+1
        assert!(close(corr_of(x(), fa(&[-1.0, -2.0, -3.0])).unwrap(), -1.0));
        assert!(close(corr_of(x(), fa(&[1.0, 2.0, 1.0])).unwrap(), 0.0)); // Σdxdy = 0
        assert!(corr_of(x(), fa(&[4.0, 4.0, 4.0])).is_err()); // zero variance
        assert!(corr_of(x(), fa(&[1.0])).is_err()); // length mismatch
    }

    #[test]
    fn spearman_is_rank_pearson() {
        // Monotone but nonlinear → Spearman 1, on ranks.
        let (xs, ys) = pairs(&fa(&[1.0, 2.0, 3.0]), &fa(&[1.0, 10.0, 100.0]), "t").unwrap();
        assert!(close(
            pearson(&ranks_of(&xs), &ranks_of(&ys), "t").unwrap(),
            1.0
        ));
    }

    #[test]
    fn cov_sample_hand_computed() {
        // x=[1,2,3], y=[2,4,6]: Σdxdy = 4, n-1 = 2 → cov 2.
        assert!(close(
            cov_v(fa(&[1.0, 2.0, 3.0]), fa(&[2.0, 4.0, 6.0])).unwrap(),
            2.0
        ));
    }

    #[test]
    fn linreg_on_a_known_line_with_r2() {
        // y = 3x + 2, exact fit → r2 = 1.
        let m = linreg_core(&[0.0, 1.0, 2.0, 3.0], &[2.0, 5.0, 8.0, 11.0], "t").unwrap();
        assert!(close(m["slope"].as_float().unwrap(), 3.0));
        assert!(close(m["intercept"].as_float().unwrap(), 2.0));
        assert!(close(m["r2"].as_float().unwrap(), 1.0));
        // Off-line point → r2 < 1 but positive.
        let m2 = linreg_core(&[0.0, 1.0, 2.0], &[0.0, 1.0, 3.0], "t").unwrap();
        let r2 = m2["r2"].as_float().unwrap();
        assert!(r2 > 0.9 && r2 < 1.0, "r2 was {r2}");
        // predict applies the model.
        assert!(close(
            predict_v(m, Dynamic::from_float(10.0)).unwrap(),
            32.0
        ));
    }

    #[test]
    fn forecast_extends_the_index_line_skipping_gaps() {
        let out = forecast_v(fa(&[1.0, 2.0, 3.0]), 2).unwrap();
        assert!(close(out[0].as_float().unwrap(), 4.0));
        assert!(close(out[1].as_float().unwrap(), 5.0));
        // A gap keeps its position: [1,(),3] is still slope 1 over indices 0 and 2.
        let out = forecast_v(ga(&[Some(1.0), None, Some(3.0)]), 1).unwrap();
        assert!(close(out[0].as_float().unwrap(), 4.0));
    }
}
