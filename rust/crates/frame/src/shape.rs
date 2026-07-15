//! Shape — the projection/ordering verbs: `select`/`drop`/`rename`/`with_col_from`,
//! `sort` (asc/desc), `unique`/`unique_by`, `reverse`. All stable/deterministic (sorts and
//! uniques keep first-seen order) so a re-run stays byte-identical (scope determinism contract).

use polars::prelude::{IntoColumn, SortMultipleOptions, UniqueKeepStrategy};
use rhai::{Engine, EvalAltResult};

use crate::convert::{array_to_series, string_list};
use crate::value::{perr, Frame};

/// Register the shape verbs.
pub(crate) fn register(engine: &mut Engine) {
    engine.register_fn(
        "select",
        |f: &mut Frame, cols: rhai::Array| -> Result<Frame, Box<EvalAltResult>> {
            let names = string_list(&cols)?;
            f.with_df(f.df.select(names).map_err(perr)?)
        },
    );
    engine.register_fn(
        "drop",
        |f: &mut Frame, cols: rhai::Array| -> Result<Frame, Box<EvalAltResult>> {
            let names = string_list(&cols)?;
            let mut df = f.df.clone();
            for n in &names {
                df = df.drop(n).map_err(perr)?;
            }
            f.with_df(df)
        },
    );
    engine.register_fn(
        "rename",
        |f: &mut Frame, from: &str, to: &str| -> Result<Frame, Box<EvalAltResult>> {
            let mut df = f.df.clone();
            df.rename(from, to.into()).map_err(perr)?;
            f.with_df(df)
        },
    );
    engine.register_fn(
        "with_col_from",
        |f: &mut Frame, name: &str, values: rhai::Array| -> Result<Frame, Box<EvalAltResult>> {
            let series = array_to_series(name, &values)?;
            let mut df = f.df.clone();
            df.with_column(series.into_column()).map_err(perr)?;
            f.with_df(df)
        },
    );
    engine.register_fn(
        "sort",
        |f: &mut Frame, column: &str| -> Result<Frame, Box<EvalAltResult>> {
            sort(f, column, false)
        },
    );
    engine.register_fn(
        "sort",
        |f: &mut Frame, column: &str, desc: bool| -> Result<Frame, Box<EvalAltResult>> {
            sort(f, column, desc)
        },
    );
    engine.register_fn(
        "unique",
        |f: &mut Frame| -> Result<Frame, Box<EvalAltResult>> {
            f.with_df(
                f.df.unique_stable(None, UniqueKeepStrategy::First, None)
                    .map_err(perr)?,
            )
        },
    );
    engine.register_fn(
        "unique_by",
        |f: &mut Frame, cols: rhai::Array| -> Result<Frame, Box<EvalAltResult>> {
            let names = string_list(&cols)?;
            f.with_df(
                f.df.unique_stable(Some(&names), UniqueKeepStrategy::First, None)
                    .map_err(perr)?,
            )
        },
    );
    engine.register_fn("reverse", |f: &mut Frame| f.with_df(f.df.reverse()));
}

fn sort(f: &Frame, column: &str, desc: bool) -> Result<Frame, Box<EvalAltResult>> {
    let opts = SortMultipleOptions::default()
        .with_order_descending(desc)
        .with_nulls_last(true)
        .with_maintain_order(true);
    f.with_df(f.df.sort([column], opts).map_err(perr)?)
}
