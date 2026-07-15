//! `stats` shape + normalize: `skewness`/`kurtosis`/`histogram`, `zscores`/`minmax_scale`/
//! `clip_arr`/`rank`. Shape moments use population moments (Fisher-Pearson g1, excess kurtosis
//! g2); normalizers are positional — missing slots stay `()`.

use rhai::{Array, Dynamic, Engine, EvalAltResult, Map};

use crate::grid::rhai_err;

use super::numeric::{mean_of, need, nums, ranks_of, scalar, slots, slots_to_array, std_of};

pub(super) fn register(engine: &mut Engine) {
    engine.register_fn("skewness", skewness_v);
    engine.register_fn("kurtosis", kurtosis_v);
    engine.register_fn("histogram", histogram_v);
    engine.register_fn("zscores", zscores_v);
    engine.register_fn("minmax_scale", minmax_v);
    engine.register_fn("clip_arr", clip_v);
    engine.register_fn("rank", rank_v);
}

/// Population central moments (m2, m3, m4) about the mean; errors on <2 values or zero variance.
fn moments(a: &Array, verb: &str) -> Result<(f64, f64, f64), Box<EvalAltResult>> {
    let v = nums(a);
    need(&v, 2, verb)?;
    let m = mean_of(&v);
    let (mut m2, mut m3, mut m4) = (0.0, 0.0, 0.0);
    for x in &v {
        let d = x - m;
        m2 += d * d;
        m3 += d * d * d;
        m4 += d * d * d * d;
    }
    let n = v.len() as f64;
    let (m2, m3, m4) = (m2 / n, m3 / n, m4 / n);
    if m2 == 0.0 {
        return Err(rhai_err(format!(
            "{verb}: zero variance (all values equal)"
        )));
    }
    Ok((m2, m3, m4))
}

/// Fisher-Pearson skewness g1 = m3 / m2^(3/2) (population moments).
fn skewness_v(a: Array) -> Result<f64, Box<EvalAltResult>> {
    let (m2, m3, _) = moments(&a, "skewness")?;
    Ok(m3 / m2.powf(1.5))
}

/// Excess kurtosis g2 = m4 / m2² − 3 (population moments; 0.0 for a normal distribution).
fn kurtosis_v(a: Array) -> Result<f64, Box<EvalAltResult>> {
    let (m2, _, m4) = moments(&a, "kurtosis")?;
    Ok(m4 / (m2 * m2) - 3.0)
}

/// Equal-width histogram over [min, max]: `bins` buckets of `#{ lo, hi, n }`; the last bucket is
/// inclusive of the max. All-equal values collapse to one degenerate bucket.
fn histogram_v(a: Array, bins: i64) -> Result<Array, Box<EvalAltResult>> {
    if bins < 1 {
        return Err(rhai_err(format!(
            "histogram: bins must be >= 1, got {bins}"
        )));
    }
    let v = nums(&a);
    need(&v, 1, "histogram")?;
    let lo = v.iter().copied().fold(f64::INFINITY, f64::min);
    let hi = v.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    if lo == hi {
        return Ok(vec![bucket(lo, hi, v.len() as i64)]);
    }
    let bins = bins as usize;
    let width = (hi - lo) / bins as f64;
    let mut counts = vec![0i64; bins];
    for &x in &v {
        counts[(((x - lo) / width) as usize).min(bins - 1)] += 1;
    }
    Ok(counts
        .into_iter()
        .enumerate()
        .map(|(i, n)| {
            let b_hi = if i == bins - 1 {
                hi
            } else {
                lo + (i + 1) as f64 * width
            };
            bucket(lo + i as f64 * width, b_hi, n)
        })
        .collect())
}

fn bucket(lo: f64, hi: f64, n: i64) -> Dynamic {
    let mut m = Map::new();
    m.insert("lo".into(), Dynamic::from_float(lo));
    m.insert("hi".into(), Dynamic::from_float(hi));
    m.insert("n".into(), Dynamic::from_int(n));
    Dynamic::from_map(m)
}

/// Positional z-scores against the array's own sample mean/std; missing slots stay `()`.
fn zscores_v(a: Array) -> Result<Array, Box<EvalAltResult>> {
    let v = nums(&a);
    let (m, s) = (mean_of(&v), std_of(&v, "zscores", true)?);
    Ok(slots_to_array(
        slots(&a)
            .into_iter()
            .map(|o| o.map(|x| (x - m) / s))
            .collect(),
    ))
}

/// Positional min-max scaling to [0, 1]; errors when the range is zero (all values equal).
fn minmax_v(a: Array) -> Result<Array, Box<EvalAltResult>> {
    let v = nums(&a);
    need(&v, 1, "minmax_scale")?;
    let lo = v.iter().copied().fold(f64::INFINITY, f64::min);
    let hi = v.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    if lo == hi {
        return Err(rhai_err("minmax_scale: zero range (all values equal)"));
    }
    Ok(slots_to_array(
        slots(&a)
            .into_iter()
            .map(|o| o.map(|x| (x - lo) / (hi - lo)))
            .collect(),
    ))
}

/// Positional clamp into [lo, hi]; missing slots stay `()`.
fn clip_v(a: Array, lo: Dynamic, hi: Dynamic) -> Result<Array, Box<EvalAltResult>> {
    let lo = scalar(&lo, "clip_arr", "lo")?;
    let hi = scalar(&hi, "clip_arr", "hi")?;
    if lo > hi {
        return Err(rhai_err(format!(
            "clip_arr: lo ({lo}) must be <= hi ({hi})"
        )));
    }
    Ok(slots_to_array(
        slots(&a)
            .into_iter()
            .map(|o| o.map(|x| x.clamp(lo, hi)))
            .collect(),
    ))
}

/// Positional 1-based ranks (ascending); ties share the average rank; missing slots stay `()`.
fn rank_v(a: Array) -> Array {
    let sl = slots(&a);
    let present: Vec<f64> = sl.iter().flatten().copied().collect();
    let ranks = ranks_of(&present);
    let mut it = ranks.into_iter();
    slots_to_array(sl.into_iter().map(|o| o.and_then(|_| it.next())).collect())
}

#[cfg(test)]
mod tests {
    use super::super::numeric::{close, fa, ga};
    use super::*;

    #[test]
    fn skewness_zero_and_kurtosis_on_uniform_five() {
        // [1..5]: symmetric → skew 0; m2 = 2, m4 = 6.8 → g2 = 6.8/4 − 3 = −1.3.
        assert!(close(
            skewness_v(fa(&[1.0, 2.0, 3.0, 4.0, 5.0])).unwrap(),
            0.0
        ));
        assert!(close(
            kurtosis_v(fa(&[1.0, 2.0, 3.0, 4.0, 5.0])).unwrap(),
            -1.3
        ));
    }

    #[test]
    fn histogram_bin_edges_and_counts() {
        // [1..8], 2 bins: width 3.5 → [1, 4.5) holds 1..4 (n=4), [4.5, 8] holds 5..8 (n=4).
        let h = histogram_v(fa(&[1., 2., 3., 4., 5., 6., 7., 8.]), 2).unwrap();
        let b = |i: usize, k: &str| h[i].read_lock::<Map>().unwrap()[k].clone();
        assert!(close(b(0, "lo").as_float().unwrap(), 1.0));
        assert!(close(b(0, "hi").as_float().unwrap(), 4.5));
        assert_eq!(b(0, "n").as_int().unwrap(), 4);
        assert!(close(b(1, "hi").as_float().unwrap(), 8.0));
        assert_eq!(b(1, "n").as_int().unwrap(), 4);
        // Max lands in the last (inclusive) bucket; all-equal collapses to one bucket.
        assert_eq!(histogram_v(fa(&[5.0, 5.0]), 3).unwrap().len(), 1);
    }

    #[test]
    fn zscores_on_1_2_3() {
        // std([1,2,3]) = 1 (sample) → z = [-1, 0, 1]; a gap stays ().
        let z = zscores_v(ga(&[Some(1.0), None, Some(2.0), Some(3.0)])).unwrap();
        assert!(close(z[0].as_float().unwrap(), -1.0));
        assert!(z[1].is_unit());
        assert!(close(z[2].as_float().unwrap(), 0.0));
        assert!(close(z[3].as_float().unwrap(), 1.0));
    }

    #[test]
    fn minmax_clip_and_rank() {
        let m = minmax_v(fa(&[1.0, 2.0, 3.0])).unwrap();
        assert!(close(m[1].as_float().unwrap(), 0.5));
        assert!(minmax_v(fa(&[4.0, 4.0])).is_err());
        let c = clip_v(
            fa(&[1.0, 5.0, 10.0]),
            Dynamic::from_float(2.0),
            Dynamic::from_float(8.0),
        )
        .unwrap();
        assert!(close(c[0].as_float().unwrap(), 2.0));
        assert!(close(c[2].as_float().unwrap(), 8.0));
        // Ties share the average rank: [10,20,20,30] → [1, 2.5, 2.5, 4].
        let r = rank_v(fa(&[10.0, 20.0, 20.0, 30.0]));
        assert!(close(r[1].as_float().unwrap(), 2.5));
        assert!(close(r[3].as_float().unwrap(), 4.0));
    }
}
