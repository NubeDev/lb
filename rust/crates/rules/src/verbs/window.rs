//! `window` — rolling/exponential window functions over plain rhai arrays (data-stdlib-scope).
//! Part of the `stats` family (one family, two files); pure compute. Every verb returns an array
//! of the INPUT length: the rolling verbs emit leading `()` until the window (of positions) fills,
//! then aggregate the numeric values inside it under the family's missing-value policy (`stats`
//! module doc) — a window with no usable values (or fewer than 2 for `rolling_std`) emits `()`.
//! `ema` starts at the first numeric value and carries its state across gaps (gap slots stay `()`).

use rhai::{Array, Dynamic, Engine, EvalAltResult};

use crate::grid::rhai_err;
use crate::verbs::stats::numeric::{mean_of, scalar, slots, slots_to_array, variance_of};

/// Register the windowed verbs (free functions — no handle).
pub fn register(engine: &mut Engine) {
    engine.register_fn("rolling_mean", |a: Array, w: i64| {
        rolling(&a, w, "rolling_mean", |v| Some(mean_of(v)))
    });
    engine.register_fn("rolling_sum", |a: Array, w: i64| {
        rolling(&a, w, "rolling_sum", |v| Some(v.iter().sum()))
    });
    engine.register_fn("rolling_min", |a: Array, w: i64| {
        rolling(&a, w, "rolling_min", |v| {
            Some(v.iter().copied().fold(f64::INFINITY, f64::min))
        })
    });
    engine.register_fn("rolling_max", |a: Array, w: i64| {
        rolling(&a, w, "rolling_max", |v| {
            Some(v.iter().copied().fold(f64::NEG_INFINITY, f64::max))
        })
    });
    engine.register_fn("rolling_std", |a: Array, w: i64| {
        rolling(&a, w, "rolling_std", |v| {
            variance_of(v, "rolling_std").ok().map(f64::sqrt)
        })
    });
    engine.register_fn("ema", ema_v);
}

/// The shared rolling frame: for each position from `w-1` on, apply `f` to the numeric values in
/// the trailing `w`-position window (empty window → `()`); earlier positions are `()`.
fn rolling(
    a: &Array,
    w: i64,
    verb: &str,
    f: impl Fn(&[f64]) -> Option<f64>,
) -> Result<Array, Box<EvalAltResult>> {
    if w < 1 {
        return Err(rhai_err(format!("{verb}: window must be >= 1, got {w}")));
    }
    let w = w as usize;
    let sl = slots(a);
    let mut out = Vec::with_capacity(sl.len());
    for i in 0..sl.len() {
        if i + 1 < w {
            out.push(None);
            continue;
        }
        let vals: Vec<f64> = sl[i + 1 - w..=i].iter().flatten().copied().collect();
        out.push(if vals.is_empty() { None } else { f(&vals) });
    }
    Ok(slots_to_array(out))
}

/// Exponential moving average: `e = alpha*x + (1-alpha)*e`, seeded by the first numeric value.
fn ema_v(a: Array, alpha: Dynamic) -> Result<Array, Box<EvalAltResult>> {
    let alpha = scalar(&alpha, "ema", "alpha")?;
    if !(alpha > 0.0 && alpha <= 1.0) {
        return Err(rhai_err(format!(
            "ema: alpha must be in (0, 1], got {alpha}"
        )));
    }
    let mut state: Option<f64> = None;
    let out = slots(&a)
        .into_iter()
        .map(|o| {
            o.map(|x| {
                let e = match state {
                    None => x,
                    Some(e) => alpha * x + (1.0 - alpha) * e,
                };
                state = Some(e);
                e
            })
        })
        .collect();
    Ok(slots_to_array(out))
}

/// Catalog rows for the windowed half of the `stats` family (the rest live in
/// `stats/rows.rs::CATALOG` — one family, two files).
#[rustfmt::skip]
pub(crate) const CATALOG: &[crate::catalog::FnEntry] = &[
    crate::catalog::FnEntry { name: "rolling_mean", family: "stats", signature: "rolling_mean(a: Array, w: Int) -> Array",
        description: "Trailing w-position mean; () until the window fills or when it holds no numeric value." },
    crate::catalog::FnEntry { name: "rolling_sum", family: "stats", signature: "rolling_sum(a: Array, w: Int) -> Array",
        description: "Trailing w-position sum; () until the window fills or when it holds no numeric value." },
    crate::catalog::FnEntry { name: "rolling_min", family: "stats", signature: "rolling_min(a: Array, w: Int) -> Array",
        description: "Trailing w-position minimum; () until the window fills or when it holds no numeric value." },
    crate::catalog::FnEntry { name: "rolling_max", family: "stats", signature: "rolling_max(a: Array, w: Int) -> Array",
        description: "Trailing w-position maximum; () until the window fills or when it holds no numeric value." },
    crate::catalog::FnEntry { name: "rolling_std", family: "stats", signature: "rolling_std(a: Array, w: Int) -> Array",
        description: "Trailing w-position sample standard deviation; () until the window holds 2 numeric values." },
    crate::catalog::FnEntry { name: "ema", family: "stats", signature: "ema(a: Array, alpha: Float) -> Array",
        description: "Exponential moving average with smoothing alpha in (0, 1], seeded by the first numeric value." },
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::verbs::stats::numeric::{close, fa, ga};

    #[test]
    fn rolling_mean_fills_after_the_window() {
        // [1,2,3,4], w=2 → [(), 1.5, 2.5, 3.5].
        let out = rolling(&fa(&[1.0, 2.0, 3.0, 4.0]), 2, "t", |v| Some(mean_of(v))).unwrap();
        assert_eq!(out.len(), 4, "output keeps the input length");
        assert!(out[0].is_unit());
        assert!(close(out[1].as_float().unwrap(), 1.5));
        assert!(close(out[3].as_float().unwrap(), 3.5));
    }

    #[test]
    fn rolling_skips_missing_inside_the_window() {
        // [1,(),3], w=2: pos1 window {1,()} → mean 1; pos2 window {(),3} → 3.
        let out = rolling(&ga(&[Some(1.0), None, Some(3.0)]), 2, "t", |v| {
            Some(mean_of(v))
        })
        .unwrap();
        assert!(close(out[1].as_float().unwrap(), 1.0));
        assert!(close(out[2].as_float().unwrap(), 3.0));
        // An all-missing window emits ().
        let out = rolling(&ga(&[Some(1.0), None, None]), 2, "t", |v| Some(mean_of(v))).unwrap();
        assert!(out[2].is_unit());
    }

    #[test]
    fn rolling_sum_min_max_std_on_a_fixture() {
        let a = || fa(&[1.0, 2.0, 4.0]);
        let s = rolling(&a(), 2, "t", |v| Some(v.iter().sum())).unwrap();
        assert!(close(s[2].as_float().unwrap(), 6.0));
        let mn = rolling(&a(), 2, "t", |v| {
            Some(v.iter().copied().fold(f64::INFINITY, f64::min))
        })
        .unwrap();
        assert!(close(mn[2].as_float().unwrap(), 2.0));
        // std of [2,4] (sample) = sqrt(2); a 1-numeric window for std yields ().
        let sd = rolling(&a(), 2, "t", |v| variance_of(v, "t").ok().map(f64::sqrt)).unwrap();
        assert!(close(sd[2].as_float().unwrap(), 2.0f64.sqrt()));
        let sd = rolling(&ga(&[Some(1.0), None]), 2, "t", |v| {
            variance_of(v, "t").ok().map(f64::sqrt)
        })
        .unwrap();
        assert!(sd[1].is_unit());
        assert!(
            rolling(&a(), 0, "t", |_| None).is_err(),
            "window < 1 errors"
        );
    }

    #[test]
    fn ema_recursion_and_gaps() {
        // alpha 0.5 over [1,2,3] → [1, 1.5, 2.25].
        let out = ema_v(fa(&[1.0, 2.0, 3.0]), Dynamic::from_float(0.5)).unwrap();
        assert!(close(out[0].as_float().unwrap(), 1.0));
        assert!(close(out[1].as_float().unwrap(), 1.5));
        assert!(close(out[2].as_float().unwrap(), 2.25));
        // A gap stays () but the state carries: [1,(),3] → [1, (), 2.0].
        let out = ema_v(ga(&[Some(1.0), None, Some(3.0)]), Dynamic::from_float(0.5)).unwrap();
        assert!(out[1].is_unit());
        assert!(close(out[2].as_float().unwrap(), 2.0));
        assert!(ema_v(fa(&[1.0]), Dynamic::from_float(0.0)).is_err());
        assert!(ema_v(fa(&[1.0]), Dynamic::from_float(1.5)).is_err());
    }
}
