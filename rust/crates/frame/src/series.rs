//! Series ops — the per-column derivations: rolling windows, `ewm_mean`, `diff`/`pct_change`/
//! `cumsum`/`shift`, `rank`, `zscore`, `clip`. Derivations ADD a new column named
//! `<col>_<op>` (the source stays); `clip` alone replaces its column in place (it bounds the
//! same measure rather than deriving a new one). Windows lead with nulls until they fill.

use polars::prelude::{
    col, lit, DataType, EWMOptions, Expr, IntoLazy, RankMethod, RankOptions,
    RollingOptionsFixedWindow,
};
use rhai::{Dynamic, Engine, EvalAltResult};

use polars::series::ops::NullBehavior;

use crate::convert::dynamic_to_lit;
use crate::value::{rerr, Frame};

/// Register the series verbs.
pub(crate) fn register(engine: &mut Engine) {
    macro_rules! rolling {
        ($name:literal, $suffix:literal, $method:ident) => {
            engine.register_fn(
                $name,
                |f: &mut Frame, column: &str, w: i64| -> Result<Frame, Box<EvalAltResult>> {
                    let opts = window(w)?;
                    with_derived(f, column, $suffix, |e| e.$method(opts))
                },
            );
        };
    }
    rolling!("rolling_mean", "rolling_mean", rolling_mean);
    rolling!("rolling_sum", "rolling_sum", rolling_sum);
    rolling!("rolling_min", "rolling_min", rolling_min);
    rolling!("rolling_max", "rolling_max", rolling_max);
    rolling!("rolling_std", "rolling_std", rolling_std);

    engine.register_fn(
        "ewm_mean",
        |f: &mut Frame, column: &str, alpha: f64| -> Result<Frame, Box<EvalAltResult>> {
            if !(alpha > 0.0 && alpha <= 1.0) {
                return Err(rerr("ewm_mean(col, alpha): alpha must be in (0, 1]"));
            }
            let opts = EWMOptions {
                alpha,
                adjust: false,
                bias: false,
                min_periods: 1,
                ignore_nulls: true,
            };
            with_derived(f, column, "ewm_mean", |e| e.ewm_mean(opts))
        },
    );
    engine.register_fn(
        "diff",
        |f: &mut Frame, column: &str| -> Result<Frame, Box<EvalAltResult>> {
            with_derived(f, column, "diff", |e| e.diff(lit(1), NullBehavior::Ignore))
        },
    );
    engine.register_fn(
        "pct_change",
        |f: &mut Frame, column: &str| -> Result<Frame, Box<EvalAltResult>> {
            with_derived(f, column, "pct_change", |e| e.pct_change(lit(1)))
        },
    );
    engine.register_fn(
        "cumsum",
        |f: &mut Frame, column: &str| -> Result<Frame, Box<EvalAltResult>> {
            with_derived(f, column, "cumsum", |e| e.cum_sum(false))
        },
    );
    engine.register_fn(
        "shift",
        |f: &mut Frame, column: &str, n: i64| -> Result<Frame, Box<EvalAltResult>> {
            with_derived(f, column, "shift", |e| e.shift(lit(n)))
        },
    );
    engine.register_fn(
        "rank",
        |f: &mut Frame, column: &str| -> Result<Frame, Box<EvalAltResult>> {
            let opts = RankOptions {
                method: RankMethod::Average,
                descending: false,
            };
            with_derived(f, column, "rank", |e| e.rank(opts, None))
        },
    );
    engine.register_fn(
        "zscore",
        |f: &mut Frame, column: &str| -> Result<Frame, Box<EvalAltResult>> {
            with_derived(f, column, "zscore", |e| {
                let v = e.cast(DataType::Float64);
                (v.clone() - v.clone().mean()) / v.std(1)
            })
        },
    );
    engine.register_fn(
        "clip",
        |f: &mut Frame,
         column: &str,
         lo: Dynamic,
         hi: Dynamic|
         -> Result<Frame, Box<EvalAltResult>> {
            let lo = dynamic_to_lit(&lo)?;
            let hi = dynamic_to_lit(&hi)?;
            let name = column.to_string();
            f.collect(
                f.df.clone()
                    .lazy()
                    .with_column(col(&name).clip(lo, hi).alias(name.clone())),
            )
        },
    );
}

/// Add the derived column `<col>_<suffix>` computed by `build` over `col`.
fn with_derived(
    f: &Frame,
    column: &str,
    suffix: &str,
    build: impl FnOnce(Expr) -> Expr,
) -> Result<Frame, Box<EvalAltResult>> {
    let out = format!("{column}_{suffix}");
    f.collect(
        f.df.clone()
            .lazy()
            .with_column(build(col(column)).alias(out)),
    )
}

/// A fixed rolling window: full-window semantics (leading nulls until it fills).
fn window(w: i64) -> Result<RollingOptionsFixedWindow, Box<EvalAltResult>> {
    if w < 1 {
        return Err(rerr("rolling window size must be >= 1"));
    }
    Ok(RollingOptionsFixedWindow {
        window_size: w as usize,
        min_periods: w as usize,
        weights: None,
        center: false,
        fn_params: None,
    })
}
