//! Missing data — `drop_nulls()` / `drop_nulls([cols])`, `fill_null(v)` (whole frame), and
//! `fill_null_strategy(col, "forward"|"backward"|"mean"|"zero")`. The scope's NaN/null policy:
//! missing is `()` ↔ `null`; these verbs are the author's repair kit for it.

use polars::prelude::{col, FillNullStrategy, IntoLazy};
use rhai::{Dynamic, Engine, EvalAltResult};

use crate::convert::{dynamic_to_lit, string_list};
use crate::value::{perr, rerr, Frame};

/// Register the missing-data verbs.
pub(crate) fn register(engine: &mut Engine) {
    engine.register_fn(
        "drop_nulls",
        |f: &mut Frame| -> Result<Frame, Box<EvalAltResult>> {
            f.with_df(f.df.drop_nulls::<String>(None).map_err(perr)?)
        },
    );
    engine.register_fn(
        "drop_nulls",
        |f: &mut Frame, cols: rhai::Array| -> Result<Frame, Box<EvalAltResult>> {
            let names = string_list(&cols)?;
            f.with_df(f.df.drop_nulls(Some(&names)).map_err(perr)?)
        },
    );
    engine.register_fn(
        "fill_null",
        |f: &mut Frame, v: Dynamic| -> Result<Frame, Box<EvalAltResult>> {
            let value = dynamic_to_lit(&v)?;
            f.collect(f.df.clone().lazy().fill_null(value))
        },
    );
    engine.register_fn(
        "fill_null_strategy",
        |f: &mut Frame, column: &str, strategy: &str| -> Result<Frame, Box<EvalAltResult>> {
            let strat = match strategy {
                "forward" => FillNullStrategy::Forward(None),
                "backward" => FillNullStrategy::Backward(None),
                "mean" => FillNullStrategy::Mean,
                "zero" => FillNullStrategy::Zero,
                other => {
                    return Err(rerr(format!(
                        "fill_null_strategy: unknown strategy {other:?} \
                         (expected forward|backward|mean|zero)"
                    )))
                }
            };
            let name = column.to_string();
            f.collect(
                f.df.clone().lazy().with_column(
                    col(&name)
                        .fill_null_with_strategy(strat)
                        .alias(name.clone()),
                ),
            )
        },
    );
}
