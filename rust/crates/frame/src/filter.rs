//! Filter — the row-selection verbs: `filter_eq/ne/gt/ge/lt/le`, `filter_in`, `filter_between`,
//! `filter_null`/`filter_not_null`, and deterministic `sample(n, seed)` (seed MANDATORY — the
//! cage has no ambient randomness; an inline xorshift64*, never the `rand` crate).

use polars::prelude::{col, lit, IdxCa, IdxSize, IntoLazy};
use rhai::{Dynamic, Engine, EvalAltResult};

use crate::convert::{array_to_series, dynamic_to_lit};
use crate::value::{perr, rerr, Frame};

/// Register the filter verbs.
pub(crate) fn register(engine: &mut Engine) {
    macro_rules! cmp {
        ($name:literal, $method:ident) => {
            engine.register_fn(
                $name,
                |f: &mut Frame, column: &str, v: Dynamic| -> Result<Frame, Box<EvalAltResult>> {
                    let value = dynamic_to_lit(&v)?;
                    f.collect(f.df.clone().lazy().filter(col(column).$method(value)))
                },
            );
        };
    }
    cmp!("filter_eq", eq);
    cmp!("filter_ne", neq);
    cmp!("filter_gt", gt);
    cmp!("filter_ge", gt_eq);
    cmp!("filter_lt", lt);
    cmp!("filter_le", lt_eq);

    engine.register_fn(
        "filter_in",
        |f: &mut Frame, column: &str, values: rhai::Array| -> Result<Frame, Box<EvalAltResult>> {
            let series = array_to_series("", &values)?;
            f.collect(
                f.df.clone()
                    .lazy()
                    .filter(col(column).is_in(lit(series), false)),
            )
        },
    );
    engine.register_fn(
        "filter_between",
        |f: &mut Frame,
         column: &str,
         lo: Dynamic,
         hi: Dynamic|
         -> Result<Frame, Box<EvalAltResult>> {
            let lo = dynamic_to_lit(&lo)?;
            let hi = dynamic_to_lit(&hi)?;
            f.collect(
                f.df.clone()
                    .lazy()
                    .filter(col(column).gt_eq(lo).and(col(column).lt_eq(hi))),
            )
        },
    );
    engine.register_fn(
        "filter_null",
        |f: &mut Frame, column: &str| -> Result<Frame, Box<EvalAltResult>> {
            f.collect(f.df.clone().lazy().filter(col(column).is_null()))
        },
    );
    engine.register_fn(
        "filter_not_null",
        |f: &mut Frame, column: &str| -> Result<Frame, Box<EvalAltResult>> {
            f.collect(f.df.clone().lazy().filter(col(column).is_not_null()))
        },
    );
    engine.register_fn(
        "sample",
        |f: &mut Frame, n: i64, seed: i64| -> Result<Frame, Box<EvalAltResult>> {
            sample(f, n, seed)
        },
    );
}

/// Deterministic sample without replacement: a partial Fisher–Yates over the row indices, driven
/// by xorshift64* seeded from the author's mandatory `seed`. Same seed + same frame → same rows,
/// byte-identical across re-runs (scope: "stochastic verbs take a mandatory seed").
fn sample(f: &Frame, n: i64, seed: i64) -> Result<Frame, Box<EvalAltResult>> {
    if n < 0 {
        return Err(rerr("sample(n, seed): n must be >= 0"));
    }
    let h = f.df.height();
    let n = (n as usize).min(h);
    let mut indices: Vec<IdxSize> = (0..h as IdxSize).collect();
    // xorshift64* — a zero seed would be a fixed point, so displace it by the golden ratio.
    let mut state: u64 = (seed as u64) ^ 0x9E37_79B9_7F4A_7C15;
    for i in 0..n {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        let r = state.wrapping_mul(0x2545_F491_4F6C_DD1D);
        let j = i + (r % (h - i) as u64) as usize;
        indices.swap(i, j);
    }
    indices.truncate(n);
    let idx = IdxCa::from_vec("".into(), indices);
    f.with_df(f.df.take(&idx).map_err(perr)?)
}
