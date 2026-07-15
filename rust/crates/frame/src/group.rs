//! Group/combine — `group_agg([keys], #{col: "agg"})`, `join(other, on, how)`, `vstack(other)`,
//! `pivot(idx, cols, vals, agg)`, `melt([ids], [vals])`. These are the frame-EXPLODING verbs, so
//! their outputs all pass the row/cell caps (via `Frame::new` inside `with_df`/`collect`) — the
//! honest bound when the deadline can't interrupt native polars (scope Risk "uninterruptible").

use std::sync::Arc;

use polars::prelude::{
    by_name, col, element, DataFrame, Expr, IntoLazy, JoinArgs, JoinType, SortMultipleOptions,
    UniqueKeepStrategy, UnpivotDF,
};
use rhai::{Engine, EvalAltResult};

use polars::frame::PivotColumnNaming;

use crate::convert::string_list;
use crate::value::{perr, rerr, Frame};

/// Register the group/combine verbs.
pub(crate) fn register(engine: &mut Engine) {
    engine.register_fn(
        "group_agg",
        |f: &mut Frame, keys: rhai::Array, aggs: rhai::Map| -> Result<Frame, Box<EvalAltResult>> {
            group_agg(f, keys, aggs)
        },
    );
    engine.register_fn(
        "join",
        |f: &mut Frame, other: Frame, on: &str, how: &str| -> Result<Frame, Box<EvalAltResult>> {
            let jt = match how {
                "inner" => JoinType::Inner,
                "left" => JoinType::Left,
                "outer" => JoinType::Full,
                "anti" => JoinType::Anti,
                other => {
                    return Err(rerr(format!(
                        "join: unknown how {other:?} (expected inner|left|outer|anti)"
                    )))
                }
            };
            f.collect(f.df.clone().lazy().join(
                other.df.clone().lazy(),
                [col(on)],
                [col(on)],
                JoinArgs::new(jt),
            ))
        },
    );
    engine.register_fn(
        "vstack",
        |f: &mut Frame, other: Frame| -> Result<Frame, Box<EvalAltResult>> {
            f.with_df(f.df.vstack(&other.df).map_err(perr)?)
        },
    );
    engine.register_fn(
        "pivot",
        |f: &mut Frame,
         idx: &str,
         columns: &str,
         values: &str,
         agg: &str|
         -> Result<Frame, Box<EvalAltResult>> { pivot(f, idx, columns, values, agg) },
    );
    engine.register_fn(
        "melt",
        |f: &mut Frame, ids: rhai::Array, vals: rhai::Array| -> Result<Frame, Box<EvalAltResult>> {
            let ids = string_list(&ids)?;
            let vals = string_list(&vals)?;
            f.with_df(f.df.unpivot(Some(vals), ids).map_err(perr)?)
        },
    );
}

/// `group_agg(["series"], #{ value: "mean", ts: "max" })` — stable group order (first-seen), the
/// aggregated column keeps its own name.
fn group_agg(f: &Frame, keys: rhai::Array, aggs: rhai::Map) -> Result<Frame, Box<EvalAltResult>> {
    let keys = string_list(&keys)?;
    if keys.is_empty() {
        return Err(rerr("group_agg: needs at least one key column"));
    }
    let mut exprs: Vec<Expr> = Vec::with_capacity(aggs.len());
    for (name, agg) in aggs.iter() {
        let agg = agg
            .clone()
            .into_string()
            .map_err(|_| rerr("group_agg: aggregation must be a string"))?;
        exprs.push(agg_expr(name.as_str(), &agg)?);
    }
    if exprs.is_empty() {
        return Err(rerr(
            "group_agg: needs at least one column -> aggregation entry",
        ));
    }
    let key_exprs: Vec<Expr> = keys.iter().map(|k| col(k.as_str())).collect();
    f.collect(f.df.clone().lazy().group_by_stable(key_exprs).agg(exprs))
}

/// One aggregation name → the polars expression, keeping the source column's name.
fn agg_expr(column: &str, agg: &str) -> Result<Expr, Box<EvalAltResult>> {
    let e = col(column);
    Ok(match agg {
        "mean" | "avg" => e.mean(),
        "median" => e.median(),
        "sum" => e.sum(),
        "min" => e.min(),
        "max" => e.max(),
        "std" => e.std(1),
        "var" => e.var(1),
        "count" => e.count(),
        "n_unique" => e.n_unique(),
        "first" => e.first(),
        "last" => e.last(),
        other => {
            return Err(rerr(format!(
                "group_agg: unknown aggregation {other:?} (expected mean|median|sum|min|max|\
                 std|var|count|n_unique|first|last)"
            )))
        }
    }
    .alias(column))
}

/// `pivot("ts", "series", "value", "mean")` — wide-format reshape. The distinct `on` values are
/// computed eagerly and SORTED so the output column order is deterministic across re-runs.
fn pivot(
    f: &Frame,
    idx: &str,
    columns: &str,
    values: &str,
    agg: &str,
) -> Result<Frame, Box<EvalAltResult>> {
    let agg = pivot_agg(agg)?;
    let distinct: DataFrame =
        f.df.select([columns])
            .map_err(perr)?
            .unique_stable(None, UniqueKeepStrategy::First, None)
            .map_err(perr)?
            .sort([columns], SortMultipleOptions::default())
            .map_err(perr)?;
    // The distinct on-values become output COLUMNS — pre-check the pivot's width against the
    // cell cap before polars runs the reshape (rows can only shrink; width is the explosion).
    f.limits
        .check_frame(f.df.height().min(1).max(1), distinct.height() + 1)
        .map_err(rerr)?;
    let lf = f.df.clone().lazy().pivot(
        by_name([columns], true, false),
        Arc::new(distinct),
        by_name([idx], true, false),
        by_name([values], true, false),
        agg,
        true,
        "_".into(),
        PivotColumnNaming::Auto,
    );
    f.collect(lf)
}

/// The pivot aggregation over the group's cells (`element()` is polars' placeholder).
fn pivot_agg(agg: &str) -> Result<Expr, Box<EvalAltResult>> {
    Ok(match agg {
        "mean" | "avg" => element().mean(),
        "median" => element().median(),
        "sum" => element().sum(),
        "min" => element().min(),
        "max" => element().max(),
        "count" => element().count(),
        "first" => element().first(),
        "last" => element().last(),
        other => {
            return Err(rerr(format!(
                "pivot: unknown aggregation {other:?} (expected mean|median|sum|min|max|\
                 count|first|last)"
            )))
        }
    })
}
