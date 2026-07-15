//! Time bucketing — `bucket(ts_col, "15m")`: epoch-aware truncation of an integer timestamp
//! column to a duration boundary (pairs with `time.floor` on scalars). Accepts epoch SECONDS or
//! MILLISECONDS per value (heuristic: |v| >= 1e11 is ms — that's year 5138 in seconds), so the
//! rows a source actually returned bucket correctly whichever unit they arrived in.

use polars::prelude::{DataType, Int64Chunked, IntoColumn, IntoSeries};
use rhai::{Engine, EvalAltResult};

use crate::value::{perr, rerr, Frame};

/// Register the bucket verb.
pub(crate) fn register(engine: &mut Engine) {
    engine.register_fn(
        "bucket",
        |f: &mut Frame, column: &str, dur: &str| -> Result<Frame, Box<EvalAltResult>> {
            bucket(f, column, dur)
        },
    );
}

fn bucket(f: &Frame, column: &str, dur: &str) -> Result<Frame, Box<EvalAltResult>> {
    let secs = parse_duration_secs(dur)?;
    let series =
        f.df.column(column)
            .map_err(perr)?
            .as_materialized_series()
            .cast(&DataType::Int64)
            .map_err(perr)?;
    let ca = series.i64().map_err(perr)?;
    let truncated: Int64Chunked = ca
        .iter()
        .map(|v| {
            v.map(|ts| {
                // Millisecond epochs are ~1.7e12 today; second epochs ~1.7e9. 1e11 splits them.
                let step = if ts.abs() >= 100_000_000_000 {
                    secs * 1000
                } else {
                    secs
                };
                ts - ts.rem_euclid(step)
            })
        })
        .collect();
    let mut df = f.df.clone();
    let new_col = truncated.into_series().with_name(column.into());
    df.replace(column, new_col.into_column()).map_err(perr)?;
    f.with_df(df)
}

/// Parse the `s/m/h/d/w` duration form (`"15m"`, `"1h"`, `"7d"`) → whole seconds.
fn parse_duration_secs(dur: &str) -> Result<i64, Box<EvalAltResult>> {
    let dur = dur.trim();
    let (num, unit) = dur.split_at(dur.len().saturating_sub(1));
    let n: i64 = num.parse().map_err(|_| {
        rerr(format!(
            "bucket: invalid duration {dur:?} (expected e.g. \"15m\")"
        ))
    })?;
    if n <= 0 {
        return Err(rerr("bucket: duration must be positive"));
    }
    let mult = match unit {
        "s" => 1,
        "m" => 60,
        "h" => 3600,
        "d" => 86_400,
        "w" => 604_800,
        _ => {
            return Err(rerr(format!(
                "bucket: invalid duration unit in {dur:?} (expected s|m|h|d|w)"
            )))
        }
    };
    Ok(n * mult)
}
