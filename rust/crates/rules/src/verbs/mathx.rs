//! `mathx` — scalar math extras beyond rhai's standard package (data-stdlib-scope): rounding to
//! places, clamps, interpolation, range mapping, percentages, safe division. Pure compute — no
//! seam, no cap, no I/O. rhai does NOT auto-coerce INT→FLOAT for native functions, so each verb
//! registers its f64 form plus an i64 convenience overload where an integer call site is natural
//! (results stay f64 so `safe_div(10, 4, 0)` is 2.5, never a truncated integer division).
//! Division-by-zero policy: `safe_div` takes the author's explicit default; `pct`/`pct_change`
//! return 0.0 on a zero denominator (a report-friendly value, never a NaN that poisons a body).

use rhai::Engine;

/// Register the scalar-math verbs (free functions — no handle).
pub fn register(engine: &mut Engine) {
    engine.register_fn("round_to", round_to);
    engine.register_fn("round_to", |x: i64, dp: i64| round_to(x as f64, dp));
    engine.register_fn("trunc_to", trunc_to);
    engine.register_fn("trunc_to", |x: i64, dp: i64| trunc_to(x as f64, dp));
    engine.register_fn("sign", sign);
    engine.register_fn("sign", |x: i64| x.signum());
    engine.register_fn("clamp", clamp);
    engine.register_fn("clamp", |x: i64, lo: i64, hi: i64| x.max(lo).min(hi));
    engine.register_fn("lerp", lerp);
    engine.register_fn("lerp", |a: i64, b: i64, t: f64| lerp(a as f64, b as f64, t));
    engine.register_fn("map_range", map_range);
    engine.register_fn(
        "map_range",
        |x: i64, in_lo: i64, in_hi: i64, out_lo: i64, out_hi: i64| {
            map_range(
                x as f64,
                in_lo as f64,
                in_hi as f64,
                out_lo as f64,
                out_hi as f64,
            )
        },
    );
    engine.register_fn("pct", pct);
    engine.register_fn("pct", |part: i64, whole: i64| {
        pct(part as f64, whole as f64)
    });
    engine.register_fn("pct_change", pct_change);
    engine.register_fn("pct_change", |from: i64, to: i64| {
        pct_change(from as f64, to as f64)
    });
    engine.register_fn("safe_div", safe_div);
    engine.register_fn("safe_div", |a: i64, b: i64, default: f64| {
        safe_div(a as f64, b as f64, default)
    });
    engine.register_fn("safe_div", |a: i64, b: i64, default: i64| {
        safe_div(a as f64, b as f64, default as f64)
    });
    engine.register_fn("log_base", log_base);
    engine.register_fn("log_base", |x: i64, b: i64| log_base(x as f64, b as f64));
    engine.register_fn("hypot", hypot);
    engine.register_fn("hypot", |a: i64, b: i64| hypot(a as f64, b as f64));
    engine.register_fn("approx_eq", approx_eq);
}

/// `10^dp`, dp clamped to the f64-exact-ish window so a wild dp can't overflow to inf.
fn pow10(dp: i64) -> f64 {
    10f64.powi(dp.clamp(-18, 18) as i32)
}

/// Round to `dp` decimal places (negative dp rounds to tens/hundreds/…).
fn round_to(x: f64, dp: i64) -> f64 {
    let f = pow10(dp);
    (x * f).round() / f
}

/// Truncate (toward zero) to `dp` decimal places.
fn trunc_to(x: f64, dp: i64) -> f64 {
    let f = pow10(dp);
    (x * f).trunc() / f
}

/// -1 / 0 / 1 (NaN counts as 0 — a non-signal, not a poison value).
fn sign(x: f64) -> i64 {
    if x > 0.0 {
        1
    } else if x < 0.0 {
        -1
    } else {
        0
    }
}

/// Clamp `x` into `[lo, hi]`.
fn clamp(x: f64, lo: f64, hi: f64) -> f64 {
    x.max(lo).min(hi)
}

/// Linear interpolation: `a + (b - a) * t` (t outside 0..1 extrapolates — by design).
fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

/// Map `x` from `[in_lo, in_hi]` onto `[out_lo, out_hi]` (a degenerate input range yields out_lo).
fn map_range(x: f64, in_lo: f64, in_hi: f64, out_lo: f64, out_hi: f64) -> f64 {
    if in_hi == in_lo {
        return out_lo;
    }
    out_lo + (x - in_lo) * (out_hi - out_lo) / (in_hi - in_lo)
}

/// `part` as a percentage of `whole` (0.0 when whole is 0).
fn pct(part: f64, whole: f64) -> f64 {
    if whole == 0.0 {
        0.0
    } else {
        part / whole * 100.0
    }
}

/// Percentage change from `from` to `to` (0.0 when from is 0).
fn pct_change(from: f64, to: f64) -> f64 {
    if from == 0.0 {
        0.0
    } else {
        (to - from) / from * 100.0
    }
}

/// `a / b`, or the author's `default` when b is 0.
fn safe_div(a: f64, b: f64, default: f64) -> f64 {
    if b == 0.0 {
        default
    } else {
        a / b
    }
}

/// Logarithm of `x` in base `b`.
fn log_base(x: f64, b: f64) -> f64 {
    x.log(b)
}

/// `sqrt(a² + b²)` without intermediate overflow.
fn hypot(a: f64, b: f64) -> f64 {
    a.hypot(b)
}

/// True when `|a - b| <= eps` (the float-comparison idiom, made explicit).
fn approx_eq(a: f64, b: f64, eps: f64) -> bool {
    (a - b).abs() <= eps
}

/// Catalog rows for the `mathx` family — one row per verb; overload arities share the row
/// (the catalog's `names_are_unique` contract; the `history` row precedent).
pub(crate) const CATALOG: &[crate::catalog::FnEntry] = &[
    crate::catalog::FnEntry {
        name: "round_to",
        family: "mathx",
        signature: "round_to(x: f64|i64, dp: i64) -> f64",
        description: "Round to dp decimal places (negative dp rounds to tens/hundreds).",
    },
    crate::catalog::FnEntry {
        name: "trunc_to",
        family: "mathx",
        signature: "trunc_to(x: f64|i64, dp: i64) -> f64",
        description: "Truncate toward zero to dp decimal places.",
    },
    crate::catalog::FnEntry {
        name: "sign",
        family: "mathx",
        signature: "sign(x: f64|i64) -> i64",
        description: "-1, 0 or 1 by the sign of x (NaN counts as 0).",
    },
    crate::catalog::FnEntry {
        name: "clamp",
        family: "mathx",
        signature: "clamp(x: f64, lo: f64, hi: f64) -> f64  |  clamp(x: i64, lo: i64, hi: i64) -> i64",
        description: "Clamp x into the closed range [lo, hi].",
    },
    crate::catalog::FnEntry {
        name: "lerp",
        family: "mathx",
        signature: "lerp(a: f64|i64, b: f64|i64, t: f64) -> f64",
        description: "Linear interpolation a + (b - a) * t (t past 0..1 extrapolates).",
    },
    crate::catalog::FnEntry {
        name: "map_range",
        family: "mathx",
        signature: "map_range(x, in_lo, in_hi, out_lo, out_hi) -> f64",
        description: "Map x from [in_lo, in_hi] onto [out_lo, out_hi] (degenerate input range yields out_lo).",
    },
    crate::catalog::FnEntry {
        name: "pct",
        family: "mathx",
        signature: "pct(part: f64|i64, whole: f64|i64) -> f64",
        description: "part as a percentage of whole (0.0 when whole is 0).",
    },
    crate::catalog::FnEntry {
        name: "pct_change",
        family: "mathx",
        signature: "pct_change(from: f64|i64, to: f64|i64) -> f64",
        description: "Percentage change from from to to (0.0 when from is 0).",
    },
    crate::catalog::FnEntry {
        name: "safe_div",
        family: "mathx",
        signature: "safe_div(a: f64|i64, b: f64|i64, default: f64|i64) -> f64",
        description: "a / b, or the author's default when b is 0 (always true division, never truncating).",
    },
    crate::catalog::FnEntry {
        name: "log_base",
        family: "mathx",
        signature: "log_base(x: f64|i64, b: f64|i64) -> f64",
        description: "Logarithm of x in base b.",
    },
    crate::catalog::FnEntry {
        name: "hypot",
        family: "mathx",
        signature: "hypot(a: f64|i64, b: f64|i64) -> f64",
        description: "sqrt(a² + b²) without intermediate overflow.",
    },
    crate::catalog::FnEntry {
        name: "approx_eq",
        family: "mathx",
        signature: "approx_eq(a: f64, b: f64, eps: f64) -> bool",
        description: "True when |a - b| <= eps (the explicit float-comparison idiom).",
    },
];
