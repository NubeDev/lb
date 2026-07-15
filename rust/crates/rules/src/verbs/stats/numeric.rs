//! Shared numeric plumbing for the `stats` family (and `window.rs`): the missing-value coercion,
//! moments, sorting/percentiles, average ranks, positional array builders, and the seeded PRNG.

use rhai::{Array, Dynamic, EvalAltResult};

use crate::grid::rhai_err;

/// Coerce one element under the family's missing-value policy: `INT` and finite `FLOAT` are
/// numeric; `()`, non-numeric types, and non-finite floats (`NaN`/`inf`) are missing.
pub(crate) fn num_of(d: &Dynamic) -> Option<f64> {
    if let Ok(i) = d.as_int() {
        return Some(i as f64);
    }
    if let Ok(f) = d.as_float() {
        if f.is_finite() {
            return Some(f);
        }
    }
    None
}

/// The aggregation view: numeric values only, missing skipped.
pub(crate) fn nums(a: &Array) -> Vec<f64> {
    a.iter().filter_map(num_of).collect()
}

/// The positional view: one slot per input element, `None` where missing.
pub(crate) fn slots(a: &Array) -> Vec<Option<f64>> {
    a.iter().map(num_of).collect()
}

/// Error unless the verb has at least `min` numeric values to work with.
pub(crate) fn need(v: &[f64], min: usize, verb: &str) -> Result<(), Box<EvalAltResult>> {
    if v.len() < min {
        return Err(rhai_err(format!(
            "{verb}: needs at least {min} numeric value(s), got {}",
            v.len()
        )));
    }
    Ok(())
}

/// Coerce a scalar argument (INT or FLOAT), or a clear author error naming the argument.
pub(crate) fn scalar(d: &Dynamic, verb: &str, arg: &str) -> Result<f64, Box<EvalAltResult>> {
    num_of(d).ok_or_else(|| rhai_err(format!("{verb}: {arg} must be a number")))
}

pub(crate) fn mean_of(v: &[f64]) -> f64 {
    v.iter().sum::<f64>() / v.len() as f64
}

/// Sample variance (n-1 denominator); errors below 2 numeric values.
pub(crate) fn variance_of(v: &[f64], verb: &str) -> Result<f64, Box<EvalAltResult>> {
    need(v, 2, verb)?;
    let m = mean_of(v);
    Ok(v.iter().map(|x| (x - m) * (x - m)).sum::<f64>() / (v.len() - 1) as f64)
}

/// Sample standard deviation; errors on zero variance when `nonzero` (z-scores need a scale).
pub(crate) fn std_of(v: &[f64], verb: &str, nonzero: bool) -> Result<f64, Box<EvalAltResult>> {
    let s = variance_of(v, verb)?.sqrt();
    if nonzero && s == 0.0 {
        return Err(rhai_err(format!(
            "{verb}: zero variance (all values equal)"
        )));
    }
    Ok(s)
}

pub(crate) fn sorted(v: &[f64]) -> Vec<f64> {
    let mut s = v.to_vec();
    s.sort_by(f64::total_cmp);
    s
}

/// Linear-interpolation percentile over an ASCENDING slice; `p` in `0..=100`.
pub(crate) fn percentile_of(asc: &[f64], p: f64, verb: &str) -> Result<f64, Box<EvalAltResult>> {
    need(asc, 1, verb)?;
    if !(0.0..=100.0).contains(&p) {
        return Err(rhai_err(format!(
            "{verb}: percentile must be in 0..=100, got {p}"
        )));
    }
    let rank = p / 100.0 * (asc.len() - 1) as f64;
    let lo = rank.floor() as usize;
    let hi = (rank.ceil() as usize).min(asc.len() - 1);
    Ok(asc[lo] + (asc[hi] - asc[lo]) * (rank - lo as f64))
}

/// 1-based average ranks (ties share the mean of their positions) — feeds `rank` and `spearman`.
pub(crate) fn ranks_of(v: &[f64]) -> Vec<f64> {
    let mut idx: Vec<usize> = (0..v.len()).collect();
    idx.sort_by(|&a, &b| v[a].total_cmp(&v[b]));
    let mut ranks = vec![0.0; v.len()];
    let mut i = 0;
    while i < idx.len() {
        let mut j = i;
        while j + 1 < idx.len() && v[idx[j + 1]] == v[idx[i]] {
            j += 1;
        }
        let avg = (i + j) as f64 / 2.0 + 1.0;
        for &k in &idx[i..=j] {
            ranks[k] = avg;
        }
        i = j + 1;
    }
    ranks
}

/// Positional result: `Some(x)` → FLOAT, `None` → `()`.
pub(crate) fn slots_to_array(v: Vec<Option<f64>>) -> Array {
    v.into_iter()
        .map(|s| match s {
            Some(x) => Dynamic::from_float(x),
            None => Dynamic::UNIT,
        })
        .collect()
}

pub(crate) fn floats_to_array(v: Vec<f64>) -> Array {
    v.into_iter().map(Dynamic::from_float).collect()
}

/// The cage's deterministic PRNG: a splitmix64 seed scramble feeding an xorshift64* stream.
/// In-crate on purpose — no `rand`, no OS entropy, no wall clock: the same seed always yields the
/// same stream (the determinism contract; the seed argument is mandatory at every call site).
pub(crate) struct Prng(u64);

impl Prng {
    pub(crate) fn new(seed: i64) -> Self {
        let mut z = (seed as u64).wrapping_add(0x9E37_79B9_7F4A_7C15);
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^= z >> 31;
        // xorshift needs a nonzero state; the scramble of 0 is nonzero, but guard anyway.
        Self(if z == 0 { 0x9E37_79B9_7F4A_7C15 } else { z })
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// Uniform-ish index in `0..n` (`n > 0`; modulo bias is irrelevant at these sizes).
    pub(crate) fn below(&mut self, n: usize) -> usize {
        (self.next_u64() % n as u64) as usize
    }
}

// ---- test fixtures shared by the family's unit tests ----

/// Build an array of floats (test fixture).
#[cfg(test)]
pub(crate) fn fa(xs: &[f64]) -> Array {
    xs.iter().copied().map(Dynamic::from_float).collect()
}

/// Build an array with gaps: `Some(x)` → FLOAT, `None` → `()` (test fixture).
#[cfg(test)]
pub(crate) fn ga(xs: &[Option<f64>]) -> Array {
    xs.iter()
        .map(|x| match x {
            Some(v) => Dynamic::from_float(*v),
            None => Dynamic::UNIT,
        })
        .collect()
}

/// Approximate float equality for hand-computed fixtures.
#[cfg(test)]
pub(crate) fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}
