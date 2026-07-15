//! Inspection — the read-only verbs: `shape`/`height`/`width`/`columns`/`dtypes`,
//! `head`/`tail`/`slice`, `describe()` (hand-rolled: polars 0.54's Rust API has no `describe`,
//! it lives Python-side only), `null_count()`, `is_empty()`.

use polars::prelude::{col, Column, DataFrame, DataType, IntoLazy};
use rhai::{Dynamic, Engine, EvalAltResult};

use crate::value::{perr, Frame};

/// Register the inspection verbs.
pub(crate) fn register(engine: &mut Engine) {
    engine.register_fn("shape", |f: &mut Frame| -> rhai::Array {
        vec![
            Dynamic::from_int(f.df.height() as i64),
            Dynamic::from_int(f.df.width() as i64),
        ]
    });
    engine.register_fn("height", |f: &mut Frame| f.df.height() as i64);
    engine.register_fn("width", |f: &mut Frame| f.df.width() as i64);
    engine.register_fn("is_empty", |f: &mut Frame| f.df.height() == 0);
    engine.register_fn("columns", |f: &mut Frame| -> rhai::Array {
        f.df.get_column_names()
            .iter()
            .map(|n| Dynamic::from(n.to_string()))
            .collect()
    });
    engine.register_fn("dtypes", |f: &mut Frame| -> rhai::Map {
        let mut m = rhai::Map::new();
        for c in f.df.columns() {
            m.insert(
                c.name().as_str().into(),
                Dynamic::from(c.dtype().to_string()),
            );
        }
        m
    });
    engine.register_fn("null_count", |f: &mut Frame| -> rhai::Map {
        let mut m = rhai::Map::new();
        for c in f.df.columns() {
            m.insert(
                c.name().as_str().into(),
                Dynamic::from_int(c.null_count() as i64),
            );
        }
        m
    });
    engine.register_fn("head", |f: &mut Frame, n: i64| {
        f.with_df(f.df.head(Some(n.max(0) as usize)))
    });
    engine.register_fn("tail", |f: &mut Frame, n: i64| {
        f.with_df(f.df.tail(Some(n.max(0) as usize)))
    });
    engine.register_fn("slice", |f: &mut Frame, offset: i64, n: i64| {
        f.with_df(f.df.slice(offset, n.max(0) as usize))
    });
    engine.register_fn(
        "describe",
        |f: &mut Frame| -> Result<Frame, Box<EvalAltResult>> { f.with_df(describe(&f.df)?) },
    );
}

/// The seven summary statistics per numeric column, as a small frame:
/// `statistic | <col> | <col> | …` with rows count/null_count/mean/std/min/max/median.
fn describe(df: &DataFrame) -> Result<DataFrame, Box<EvalAltResult>> {
    const STATS: [&str; 7] = ["count", "null_count", "mean", "std", "min", "max", "median"];
    let numeric: Vec<String> = df
        .columns()
        .iter()
        .filter(|c| c.dtype().is_primitive_numeric())
        .map(|c| c.name().to_string())
        .collect();

    let mut exprs = Vec::with_capacity(numeric.len() * STATS.len());
    for c in &numeric {
        let base = col(c.as_str()).cast(DataType::Float64);
        exprs.push(
            base.clone()
                .count()
                .cast(DataType::Float64)
                .alias(format!("{c}\u{0}count")),
        );
        exprs.push(
            base.clone()
                .null_count()
                .cast(DataType::Float64)
                .alias(format!("{c}\u{0}null_count")),
        );
        exprs.push(base.clone().mean().alias(format!("{c}\u{0}mean")));
        exprs.push(base.clone().std(1).alias(format!("{c}\u{0}std")));
        exprs.push(base.clone().min().alias(format!("{c}\u{0}min")));
        exprs.push(base.clone().max().alias(format!("{c}\u{0}max")));
        exprs.push(base.median().alias(format!("{c}\u{0}median")));
    }

    let mut out = vec![Column::new("statistic".into(), STATS.as_slice())];
    if numeric.is_empty() || df.height() == 0 {
        // No numeric columns (or nothing to summarize): the statistic column alone is honest.
        if !numeric.is_empty() {
            for c in &numeric {
                out.push(Column::new(
                    c.as_str().into(),
                    vec![None::<f64>; STATS.len()],
                ));
            }
        }
        return DataFrame::new_infer_height(out).map_err(perr);
    }

    let summary = df.clone().lazy().select(exprs).collect().map_err(perr)?;
    for c in &numeric {
        let values: Vec<Option<f64>> = STATS
            .iter()
            .map(|s| {
                summary
                    .column(&format!("{c}\u{0}{s}"))
                    .ok()
                    .and_then(|col| col.get(0).ok())
                    .and_then(|av| av.extract::<f64>())
                    .filter(|v| v.is_finite())
            })
            .collect();
        out.push(Column::new(c.as_str().into(), values));
    }
    DataFrame::new_infer_height(out).map_err(perr)
}
