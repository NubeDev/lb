//! `stats` outliers + selection: `outliers_iqr`/`outliers_z`/`is_anomaly`, `top_k`/`bottom_k`/
//! `argmax`/`argmin`, and the two seeded order verbs `sample`/`shuffle` (mandatory seed →
//! deterministic in-crate PRNG; elements pass through verbatim). Outlier verbs return indices
//! into the ORIGINAL array.

use rhai::{Array, Dynamic, Engine, EvalAltResult};

use crate::grid::rhai_err;

use super::numeric::{
    floats_to_array, mean_of, need, nums, percentile_of, scalar, slots, sorted, std_of, Prng,
};

pub(super) fn register(engine: &mut Engine) {
    engine.register_fn("outliers_iqr", outliers_iqr_v);
    engine.register_fn("outliers_z", outliers_z_v);
    engine.register_fn("is_anomaly", is_anomaly_v);
    engine.register_fn("top_k", |a: Array, k: i64| take_k(&a, k, "top_k", true));
    engine.register_fn("bottom_k", |a: Array, k: i64| {
        take_k(&a, k, "bottom_k", false)
    });
    engine.register_fn("argmax", |a: Array| arg_v(&a, "argmax", true));
    engine.register_fn("argmin", |a: Array| arg_v(&a, "argmin", false));
    engine.register_fn("sample", sample_v);
    engine.register_fn("shuffle", shuffle_v);
}

/// Indices whose value falls outside `[q1 - k*iqr, q3 + k*iqr]` (Tukey fences).
fn outliers_iqr_v(a: Array, k: Dynamic) -> Result<Array, Box<EvalAltResult>> {
    let k = scalar(&k, "outliers_iqr", "k")?;
    if k < 0.0 {
        return Err(rhai_err(format!("outliers_iqr: k must be >= 0, got {k}")));
    }
    let asc = sorted(&nums(&a));
    let q1 = percentile_of(&asc, 25.0, "outliers_iqr")?;
    let q3 = percentile_of(&asc, 75.0, "outliers_iqr")?;
    let iqr = q3 - q1;
    Ok(indices_where(&a, |x| x < q1 - k * iqr || x > q3 + k * iqr))
}

/// Indices whose |z-score| (sample std) exceeds `thr`.
fn outliers_z_v(a: Array, thr: Dynamic) -> Result<Array, Box<EvalAltResult>> {
    let thr = scalar(&thr, "outliers_z", "thr")?;
    let v = nums(&a);
    let (m, s) = (mean_of(&v), std_of(&v, "outliers_z", true)?);
    Ok(indices_where(&a, |x| ((x - m) / s).abs() > thr))
}

/// Is `x` more than `thr` sample standard deviations from the array's mean?
fn is_anomaly_v(a: Array, x: Dynamic, thr: Dynamic) -> Result<bool, Box<EvalAltResult>> {
    let x = scalar(&x, "is_anomaly", "x")?;
    let thr = scalar(&thr, "is_anomaly", "thr")?;
    let v = nums(&a);
    let (m, s) = (mean_of(&v), std_of(&v, "is_anomaly", true)?);
    Ok(((x - m) / s).abs() > thr)
}

fn indices_where(a: &Array, pred: impl Fn(f64) -> bool) -> Array {
    slots(a)
        .into_iter()
        .enumerate()
        .filter_map(|(i, o)| match o {
            Some(x) if pred(x) => Some(Dynamic::from_int(i as i64)),
            _ => None,
        })
        .collect()
}

/// The `k` largest (`desc`) or smallest numeric values, sorted; `k` past the numeric count
/// returns them all.
fn take_k(a: &Array, k: i64, verb: &str, desc: bool) -> Result<Array, Box<EvalAltResult>> {
    if k < 0 {
        return Err(rhai_err(format!("{verb}: k must be >= 0, got {k}")));
    }
    let mut asc = sorted(&nums(a));
    if desc {
        asc.reverse();
    }
    asc.truncate(k as usize);
    Ok(floats_to_array(asc))
}

/// Index (into the original array) of the first max/min numeric value.
fn arg_v(a: &Array, verb: &str, max: bool) -> Result<i64, Box<EvalAltResult>> {
    let mut best: Option<(usize, f64)> = None;
    for (i, o) in slots(a).into_iter().enumerate() {
        if let Some(x) = o {
            let better = match best {
                None => true,
                Some((_, b)) => {
                    if max {
                        x > b
                    } else {
                        x < b
                    }
                }
            };
            if better {
                best = Some((i, x));
            }
        }
    }
    match best {
        Some((i, _)) => Ok(i as i64),
        None => {
            need(&[], 1, verb)?; // the family's standard "needs at least 1 numeric value" error
            unreachable!()
        }
    }
}

/// `n` elements drawn without replacement (partial Fisher-Yates over the seeded stream);
/// elements pass through verbatim.
fn sample_v(a: Array, n: i64, seed: i64) -> Result<Array, Box<EvalAltResult>> {
    if n < 0 {
        return Err(rhai_err(format!("sample: n must be >= 0, got {n}")));
    }
    if n as usize > a.len() {
        return Err(rhai_err(format!(
            "sample: n ({n}) exceeds the array length ({})",
            a.len()
        )));
    }
    let mut pool = a;
    let mut rng = Prng::new(seed);
    for i in 0..n as usize {
        let j = i + rng.below(pool.len() - i);
        pool.swap(i, j);
    }
    pool.truncate(n as usize);
    Ok(pool)
}

/// Full Fisher-Yates shuffle over the seeded stream; elements pass through verbatim.
fn shuffle_v(a: Array, seed: i64) -> Array {
    let mut out = a;
    let mut rng = Prng::new(seed);
    for i in (1..out.len()).rev() {
        let j = rng.below(i + 1);
        out.swap(i, j);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::super::numeric::fa;
    use super::*;

    fn ints(a: &Array) -> Vec<i64> {
        a.iter().map(|d| d.as_int().unwrap()).collect()
    }

    #[test]
    fn outliers_iqr_on_a_known_set() {
        // [1,2,3,4,100]: q1=2, q3=4, iqr=2 → fences [-1, 7] → only index 4 (100).
        let out = outliers_iqr_v(fa(&[1.0, 2.0, 3.0, 4.0, 100.0]), Dynamic::from_float(1.5));
        assert_eq!(ints(&out.unwrap()), vec![4]);
    }

    #[test]
    fn outliers_z_and_is_anomaly() {
        // Nine 1s + one 11: mean 2, sample std sqrt(10) ≈ 3.162 → z(11) ≈ 2.85.
        let a = fa(&[1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 11.0]);
        assert_eq!(
            ints(&outliers_z_v(a.clone(), Dynamic::from_float(2.5)).unwrap()),
            vec![9]
        );
        // [1..5]: mean 3, std ≈ 1.581 → x=10 is ~4.4σ out, x=3 is 0σ.
        let b = fa(&[1.0, 2.0, 3.0, 4.0, 5.0]);
        assert!(is_anomaly_v(
            b.clone(),
            Dynamic::from_float(10.0),
            Dynamic::from_float(3.0)
        )
        .unwrap());
        assert!(!is_anomaly_v(b, Dynamic::from_float(3.0), Dynamic::from_float(3.0)).unwrap());
    }

    #[test]
    fn top_bottom_arg_selection() {
        let f = |a: &Array| -> Vec<f64> { a.iter().map(|d| d.as_float().unwrap()).collect() };
        assert_eq!(
            f(&take_k(&fa(&[5.0, 1.0, 4.0]), 2, "t", true).unwrap()),
            vec![5.0, 4.0]
        );
        assert_eq!(
            f(&take_k(&fa(&[5.0, 1.0, 4.0]), 2, "t", false).unwrap()),
            vec![1.0, 4.0]
        );
        assert_eq!(f(&take_k(&fa(&[2.0]), 9, "t", true).unwrap()), vec![2.0]); // clamps
        assert_eq!(arg_v(&fa(&[1.0, 9.0, 3.0]), "argmax", true).unwrap(), 1);
        assert_eq!(arg_v(&fa(&[1.0, 9.0, 3.0]), "argmin", false).unwrap(), 0);
        assert!(arg_v(&fa(&[]), "argmax", true).is_err());
    }

    #[test]
    fn sample_and_shuffle_are_seed_deterministic() {
        let base = || fa(&[1.0, 2.0, 3.0, 4.0, 5.0]);
        let s1 = shuffle_v(base(), 42);
        let s2 = shuffle_v(base(), 42);
        assert_eq!(
            format!("{s1:?}"),
            format!("{s2:?}"),
            "same seed → same order"
        );
        // Still a permutation of the input.
        let mut sorted_back: Vec<f64> = s1.iter().map(|d| d.as_float().unwrap()).collect();
        sorted_back.sort_by(f64::total_cmp);
        assert_eq!(sorted_back, vec![1.0, 2.0, 3.0, 4.0, 5.0]);
        let p1 = sample_v(base(), 3, 7).unwrap();
        let p2 = sample_v(base(), 3, 7).unwrap();
        assert_eq!(format!("{p1:?}"), format!("{p2:?}"));
        assert_eq!(p1.len(), 3);
        assert!(sample_v(base(), 6, 7).is_err(), "n past the length errors");
    }
}
