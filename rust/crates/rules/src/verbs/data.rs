//! The data verbs — `source`/`query`/`history`/`span`/`last`/`param`. **Ported from rubix-cube's
//! `verbs/data.rs`**, **re-seamed**: every base grid is produced by resolving the source name through
//! the host [`DataSeam`] (the allowlist + workspace pin), then composed lazily. `source("series")`
//! and `source("timescale")` both return a `Grid`; the seam picks platform vs federation. The grid's
//! `collect` later calls `store.query`/`series.*` or `federation.query` — the host re-checks `caps`
//! there (the chokepoint that replaces rubix-cube's per-collect SQL validator).

use std::collections::HashSet;
use std::sync::Arc;

use rhai::{Dynamic, Engine, EvalAltResult};

use crate::grid::{quote_ident, rhai_err, Grid, GridCtx, Span};
use crate::seam::SourceKind;

use super::duration::duration_to_surql;

/// Register the data verbs into `engine`, each closing over the run context + allowlist + inputs.
pub fn register(
    engine: &mut Engine,
    ctx: Arc<GridCtx>,
    allow: Arc<HashSet<String>>,
    inputs: Arc<rhai::Map>,
) {
    // source(name) — the uniform entry. Resolve the name (allowlist + kind) and seed a base grid that
    // reads everything from it. SurrealQL `SELECT * FROM source` for platform; the federation source's
    // own table for external (the host maps the registered source to its physical query).
    {
        let ctx = ctx.clone();
        let allow = allow.clone();
        engine.register_fn("source", move |name: &str| base_grid(name, &allow, &ctx));
    }

    // query(source, sql) — the escape hatch: a hand-written query against a named source. Still
    // resolved through the seam; the host re-validates the SQL at collect.
    {
        let ctx = ctx.clone();
        let allow = allow.clone();
        engine.register_fn(
            "query",
            move |name: &str, sql: &str| -> Result<Grid, Box<EvalAltResult>> {
                let (kind, resolved) = resolve(name, &allow, &ctx)?;
                Ok(Grid::new(kind, resolved, sql.to_string(), ctx.clone()))
            },
        );
    }

    // history(source, point, span) / history(source, point, "24h") — timeseries sugar: the (ts,value)
    // rows of a named point within a window. Platform series live in the series plane (ts is a field).
    {
        let ctx1 = ctx.clone();
        let allow1 = allow.clone();
        engine.register_fn(
            "history",
            move |name: &str, point: &str, span: Span| -> Result<Grid, Box<EvalAltResult>> {
                history_grid(name, point, &span.raw, &allow1, &ctx1)
            },
        );
        let ctx2 = ctx.clone();
        let allow2 = allow.clone();
        engine.register_fn(
            "history",
            move |name: &str, point: &str, dur: &str| -> Result<Grid, Box<EvalAltResult>> {
                let raw = duration_to_surql(dur).map_err(rhai_err)?;
                history_grid(name, point, &raw, &allow2, &ctx2)
            },
        );
    }

    // span("24h") / last("7d") — typed window constructors (validated duration).
    engine.register_fn("span", |s: &str| -> Result<Span, Box<EvalAltResult>> {
        Ok(Span {
            raw: duration_to_surql(s).map_err(rhai_err)?,
        })
    });
    engine.register_fn("last", |s: &str| -> Result<Span, Box<EvalAltResult>> {
        Ok(Span {
            raw: duration_to_surql(s).map_err(rhai_err)?,
        })
    });

    // param(name) — read a bound input by name (also a scope var).
    engine.register_fn("param", move |name: &str| {
        inputs.get(name).cloned().unwrap_or(Dynamic::UNIT)
    });
}

/// Resolve `name` through the seam (allowlist + workspace), returning kind + physical name. The
/// allowlist is the fast local guard; the seam's `resolve` is authoritative (and the collect re-checks
/// `caps`). A name absent from the allowlist is an opaque "not allowed".
fn resolve(
    name: &str,
    allow: &Arc<HashSet<String>>,
    ctx: &Arc<GridCtx>,
) -> Result<(SourceKind, String), Box<EvalAltResult>> {
    if !allow.contains(name) {
        return Err(rhai_err(format!("source not allowed: {name}")));
    }
    ctx.data.resolve(name).map_err(rhai_err)
}

/// A base grid reading all rows of a source.
fn base_grid(
    name: &str,
    allow: &Arc<HashSet<String>>,
    ctx: &Arc<GridCtx>,
) -> Result<Grid, Box<EvalAltResult>> {
    let (kind, resolved) = resolve(name, allow, ctx)?;
    let sql = format!("SELECT * FROM {}", quote_ident(&resolved)?);
    Ok(Grid::new(kind, resolved, sql, ctx.clone()))
}

/// A history grid: `(ts, value)` of `point` over the window. For platform series the host's series
/// plane is read; for federation the source's table is read with a `ts` filter.
fn history_grid(
    name: &str,
    point: &str,
    window: &str,
    allow: &Arc<HashSet<String>>,
    ctx: &Arc<GridCtx>,
) -> Result<Grid, Box<EvalAltResult>> {
    let (kind, resolved) = resolve(name, allow, ctx)?;
    // Platform series carry a LOGICAL ts (a sample timestamp, not wall-clock), so a `now()-window`
    // filter is both nondeterministic and wrong here; the author filters the window explicitly on the
    // returned grid. The federation path keeps the wall-clock window (its ts is a real timestamp).
    let _ = window;
    let sql = match kind {
        // The committed series row stores the numeric under `payload`; normalize it to `value` so the
        // timeseries verbs (which speak `value`) compose over platform + federation uniformly.
        SourceKind::Platform => format!(
            "SELECT ts, payload AS value FROM {} WHERE series = {} ORDER BY ts",
            quote_ident(&resolved)?,
            surql_string(point)
        ),
        SourceKind::Federation => format!(
            "SELECT ts, value FROM {} WHERE point = {} AND ts >= now() - INTERVAL '{window}' ORDER BY ts",
            quote_ident(point)?,
            ansi_string(point)
        ),
    };
    Ok(Grid::new(kind, resolved, sql, ctx.clone()))
}

/// A SurrealQL single-quoted string literal with quotes escaped.
fn surql_string(s: &str) -> String {
    format!("'{}'", s.replace('\'', "\\'"))
}

/// An ANSI single-quoted string literal with quotes doubled.
fn ansi_string(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}
