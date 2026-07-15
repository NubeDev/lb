//! Aggregate — the column-to-scalar reductions (`mean`/`median`/`sum`/`min`/`max`/`std`/`var`/
//! `quantile`), `count()`, `n_unique(col)`, and `value_counts(col)`. Scalars come back through
//! the JSON boundary so NaN → `()` like every other export (scope NaN/null policy). `std`/`var`
//! use ddof = 1 (sample statistics — matches the `stats` array family).

use polars::prelude::{col, lit, DataType, Expr, IntoLazy, QuantileMethod};
use rhai::{Dynamic, Engine, EvalAltResult};

use polars::prelude::SeriesMethods;

use crate::convert::value_to_dynamic;
use crate::json::any_value_to_json;
use crate::value::{perr, rerr, Frame};

/// Register the aggregate verbs.
pub(crate) fn register(engine: &mut Engine) {
    macro_rules! agg {
        ($name:literal, $build:expr) => {
            engine.register_fn(
                $name,
                |f: &mut Frame, column: &str| -> Result<Dynamic, Box<EvalAltResult>> {
                    let build: fn(Expr) -> Expr = $build;
                    scalar(f, build(col(column)))
                },
            );
        };
    }
    agg!("mean", |e| e.mean());
    agg!("median", |e| e.median());
    agg!("sum", |e| e.sum());
    agg!("min", |e| e.min());
    agg!("max", |e| e.max());
    agg!("std", |e| e.std(1));
    // `var` is a rhai RESERVED keyword — the verb is `variance` (the catalog documents it).
    agg!("variance", |e| e.var(1));

    engine.register_fn(
        "quantile",
        |f: &mut Frame, column: &str, q: f64| -> Result<Dynamic, Box<EvalAltResult>> {
            if !(0.0..=1.0).contains(&q) {
                return Err(rerr("quantile(col, q): q must be within 0.0..=1.0"));
            }
            scalar(
                f,
                col(column)
                    .cast(DataType::Float64)
                    .quantile(lit(q), QuantileMethod::Linear),
            )
        },
    );
    engine.register_fn("count", |f: &mut Frame| f.df.height() as i64);
    engine.register_fn(
        "n_unique",
        |f: &mut Frame, column: &str| -> Result<i64, Box<EvalAltResult>> {
            let d = scalar(f, col(column).n_unique())?;
            d.as_int().map_err(|_| rerr("n_unique: non-integer result"))
        },
    );
    engine.register_fn(
        "value_counts",
        |f: &mut Frame, column: &str| -> Result<Frame, Box<EvalAltResult>> {
            let series = f.df.column(column).map_err(perr)?.as_materialized_series();
            // sort=true (by count, desc), parallel=false (deterministic tie order).
            let counts = series
                .value_counts(true, false, "count".into(), false)
                .map_err(perr)?;
            f.with_df(counts)
        },
    );
}

/// Run one aggregate expression eagerly and return the single cell as a rhai scalar.
fn scalar(f: &Frame, expr: Expr) -> Result<Dynamic, Box<EvalAltResult>> {
    let out =
        f.df.clone()
            .lazy()
            .select([expr.alias("v")])
            .collect()
            .map_err(perr)?;
    let av = out.column("v").map_err(perr)?.get(0).map_err(perr)?;
    Ok(value_to_dynamic(&any_value_to_json(&av)))
}
