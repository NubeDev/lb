//! The timeseries plan-builders — `rollup`/`align`/`interpolate`/`gapfill`/`resample`/`lag`/`delta`/
//! `rate`. **Ported from rubix-cube's `verbs/timeseries.rs`** (pure plan-builders over a `(ts,value)`
//! grid). **Re-targeted dialect**: rubix-cube emitted DataFusion (`date_bin`, window functions);
//! lb-rules platform grids are SurrealQL, so `rollup` uses `time::group` bucketing. A federation grid
//! keeps the ANSI form. Each builder just wraps the plan in a new outer query — nothing scans.

use rhai::{Engine, EvalAltResult};

use crate::grid::{quote_ident, rhai_err, Grid};
use crate::seam::SourceKind;

use super::duration::duration_to_surql;

pub fn register(engine: &mut Engine) {
    engine.register_fn("rollup", |g: &mut Grid, every: &str, agg: &str| {
        rollup(g, every, agg)
    });
    engine.register_fn("lag", |g: &mut Grid, col: &str, n: i64| lag(g, col, n));
    engine.register_fn("delta", |g: &mut Grid, col: &str| delta(g, col));
    engine.register_fn("rate", |g: &mut Grid, col: &str| rate(g, col));
    // interpolate/gapfill/resample: validate the args; identity plan for v1 (the spine is the store's
    // job, not the rule's — same posture rubix-cube took for the non-Timescale path). Documented.
    engine.register_fn("interpolate", |g: &mut Grid, method: &str| {
        interpolate(g, method)
    });
    engine.register_fn("gapfill", |g: &mut Grid, every: &str| gapfill(g, every));
    engine.register_fn("resample", |g: &mut Grid, every: &str, agg: &str| {
        rollup(g, every, agg)
    });
}

/// Time-bucket + aggregate. Result column is named after the aggregate (referenceable downstream).
fn rollup(g: &Grid, every: &str, agg: &str) -> Result<Grid, Box<EvalAltResult>> {
    let interval = duration_to_surql(every).map_err(rhai_err)?;
    let agg = agg.to_lowercase();
    let out = quote_ident(&agg)?;
    match g.kind() {
        SourceKind::Platform => {
            let agg_expr = match agg.as_str() {
                "avg" | "mean" => "math::mean(value)",
                "min" => "math::min(value)",
                "max" => "math::max(value)",
                "sum" => "math::sum(value)",
                "count" => "count()",
                "last" => "array::last(value)",
                "first" => "array::first(value)",
                other => return Err(rhai_err(format!("unknown rollup aggregate {other:?}"))),
            };
            // time::group buckets ts to the floor of the interval; GROUP BY the bucket.
            Ok(g.wrap(format!(
                "SELECT time::group(ts, {interval}) AS ts, {agg_expr} AS {out} \
                 FROM {} GROUP BY ts ORDER BY ts",
                g.subquery()
            )))
        }
        SourceKind::Federation => {
            let agg_expr = match agg.as_str() {
                "avg" | "mean" | "min" | "max" | "sum" | "count" => format!("{agg}(value)"),
                "last" => "last_value(value)".to_string(),
                "first" => "first_value(value)".to_string(),
                other => return Err(rhai_err(format!("unknown rollup aggregate {other:?}"))),
            };
            Ok(g.wrap(format!(
                "SELECT date_bin(INTERVAL '{interval}', ts, TIMESTAMP '1970-01-01T00:00:00') AS ts, \
                 {agg_expr} AS {out} FROM {} GROUP BY 1 ORDER BY 1",
                g.subquery()
            )))
        }
    }
}

fn lag(g: &Grid, col: &str, n: i64) -> Result<Grid, Box<EvalAltResult>> {
    let c = quote_ident(col)?;
    Ok(g.wrap(format!(
        "SELECT *, LAG({c}, {n}) OVER (ORDER BY ts) AS {c}_lag FROM {}",
        g.subquery()
    )))
}

fn delta(g: &Grid, col: &str) -> Result<Grid, Box<EvalAltResult>> {
    let c = quote_ident(col)?;
    Ok(g.wrap(format!(
        "SELECT *, ({c} - LAG({c}) OVER (ORDER BY ts)) AS {c}_delta FROM {}",
        g.subquery()
    )))
}

fn rate(g: &Grid, col: &str) -> Result<Grid, Box<EvalAltResult>> {
    let c = quote_ident(col)?;
    Ok(g.wrap(format!(
        "SELECT *, ({c} - LAG({c}) OVER (ORDER BY ts)) AS {c}_rate FROM {}",
        g.subquery()
    )))
}

/// LOCF or identity. v1 keeps the plan an identity (documented) unless `method` is unknown.
fn interpolate(g: &Grid, method: &str) -> Result<Grid, Box<EvalAltResult>> {
    match method.to_lowercase().as_str() {
        "locf" | "none" => Ok(g.clone()),
        other => Err(rhai_err(format!("unknown interpolate method {other:?}"))),
    }
}

/// Validate the cadence; identity plan for v1 (regular-grid gapfill deferred to the store).
fn gapfill(g: &Grid, every: &str) -> Result<Grid, Box<EvalAltResult>> {
    duration_to_surql(every).map_err(rhai_err)?;
    Ok(g.clone())
}
