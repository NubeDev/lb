//! The lazy, column-oriented `Grid` / `Col` / `GroupedGrid` / `Span`. **Ported from rubix-cube**
//! (`rules/grid.rs`, MIT/Apache-2.0), **re-targeted**: the composition logic is unchanged (every method
//! appends to the plan by wrapping in a subquery; nothing scans until `collect`/a `Col` reduction/
//! `records`/return), but `collect` calls the host [`DataSeam`] (→ `store.query`/`series.*` or
//! `federation.query`) instead of a local DataFusion engine. The grid carries its [`SourceKind`] so a
//! platform grid composes SurrealQL and a federation grid composes ANSI SQL — one author surface, two
//! correct dialects beneath. The deliberate absence of a per-row iterator is kept (stay vectorized;
//! `head(n)` + `Col` reductions for inspection).

use std::sync::Arc;

use rhai::{Array, Dynamic, EvalAltResult, Map};
use serde_json::Value;

use crate::runtime::GridJson;
use crate::seam::{DataSeam, SourceKind};

/// Shared per-run context for grid collection — the data seam (closed over the pinned workspace).
pub struct GridCtx {
    pub data: Arc<dyn DataSeam>,
}

/// A typed time/duration window (`span("24h")` / `last("7d")`) — never a raw string into SQL.
#[derive(Debug, Clone)]
pub struct Span {
    /// Already-validated interval text, dialect-appropriate (e.g. `"24h"` for SurrealQL `duration`).
    pub raw: String,
}

/// A lazy grid: a composed query + the source it reads + the collect context. Cloning is cheap (an
/// `Arc` + two strings); chaining never copies data.
#[derive(Clone)]
pub struct Grid {
    pub(crate) kind: SourceKind,
    pub(crate) source: String,
    /// The composed query text in the source's dialect.
    pub(crate) sql: String,
    pub(crate) ctx: Arc<GridCtx>,
}

/// A handle to one column of a grid — the target of a reduction (`max`/`avg`/…).
#[derive(Clone)]
pub struct Col {
    grid: Grid,
    name: String,
}

/// A grid with a pending `GROUP BY` — `agg(...)` turns it back into a `Grid`.
#[derive(Clone)]
pub struct GroupedGrid {
    grid: Grid,
    keys: Vec<String>,
}

/// Make a rhai eval error from any string.
pub fn rhai_err(msg: impl Into<String>) -> Box<EvalAltResult> {
    Box::new(EvalAltResult::ErrorRuntime(
        Dynamic::from(msg.into()),
        rhai::Position::NONE,
    ))
}

/// Quote an identifier defensively (backtick), rejecting backticks in the name so a source/column
/// can't break out of the quoting. Both SurrealQL and ANSI-ish backends accept backtick quoting in
/// our composed plans.
pub fn quote_ident(name: &str) -> Result<String, Box<EvalAltResult>> {
    if name.contains('`') || name.contains('\n') {
        return Err(rhai_err(format!("invalid identifier: {name:?}")));
    }
    Ok(format!("`{name}`"))
}

impl Grid {
    pub fn new(kind: SourceKind, source: String, sql: String, ctx: Arc<GridCtx>) -> Self {
        Self {
            kind,
            source,
            sql,
            ctx,
        }
    }

    pub fn kind(&self) -> SourceKind {
        self.kind
    }

    /// The current plan as a subquery fragment (parenthesized) for wrapping.
    pub(crate) fn subquery(&self) -> String {
        format!("({})", self.sql)
    }

    /// Wrap the current plan in a new outer query (the universal composition primitive).
    pub(crate) fn wrap(&self, sql: String) -> Grid {
        Grid {
            kind: self.kind,
            source: self.source.clone(),
            sql,
            ctx: self.ctx.clone(),
        }
    }

    /// Materialize: collect via the host data seam. The ONE behavioral change from rubix-cube — this
    /// calls `store.query`/`series.*` or `federation.query`, where the host re-runs `caps::check`.
    pub(crate) fn collect_json(&self) -> Result<GridJson, Box<EvalAltResult>> {
        self.ctx
            .data
            .collect(self.kind, &self.source, &self.sql)
            .map_err(rhai_err)
    }

    // ---- plan-builders (lift verbatim; pure string composition) ----

    pub fn filter(&self, expr: &str) -> Grid {
        self.wrap(format!("SELECT * FROM {} WHERE {expr}", self.subquery()))
    }

    pub fn select(&self, cols: Array) -> Result<Grid, Box<EvalAltResult>> {
        let names = string_list(cols)?;
        if names.is_empty() {
            return Err(rhai_err("select() needs at least one column"));
        }
        let projected = names
            .iter()
            .map(|c| quote_ident(c))
            .collect::<Result<Vec<_>, _>>()?
            .join(", ");
        Ok(self.wrap(format!("SELECT {projected} FROM {}", self.subquery())))
    }

    pub fn add_col(&self, name: &str, expr: &str) -> Result<Grid, Box<EvalAltResult>> {
        Ok(self.wrap(format!(
            "SELECT *, {expr} AS {} FROM {}",
            quote_ident(name)?,
            self.subquery()
        )))
    }

    pub fn rename(&self, from: &str, to: &str) -> Result<Grid, Box<EvalAltResult>> {
        Ok(self.wrap(format!(
            "SELECT *, {} AS {} FROM {}",
            quote_ident(from)?,
            quote_ident(to)?,
            self.subquery()
        )))
    }

    pub fn group_by(&self, keys: Array) -> Result<GroupedGrid, Box<EvalAltResult>> {
        let names = string_list(keys)?;
        if names.is_empty() {
            return Err(rhai_err("group_by() needs at least one key"));
        }
        Ok(GroupedGrid {
            grid: self.clone(),
            keys: names,
        })
    }

    pub fn join(&self, other: Grid, on: &str, how: &str) -> Result<Grid, Box<EvalAltResult>> {
        if other.kind != self.kind {
            return Err(rhai_err("cannot join grids from different source kinds"));
        }
        let how = match how.to_lowercase().as_str() {
            "inner" => "INNER",
            "left" => "LEFT",
            "right" => "RIGHT",
            "outer" | "full" => "FULL OUTER",
            other => return Err(rhai_err(format!("unknown join type {other:?}"))),
        };
        Ok(self.wrap(format!(
            "SELECT * FROM {} AS a {how} JOIN {} AS b ON {on}",
            self.subquery(),
            other.subquery()
        )))
    }

    pub fn col(&self, name: &str) -> Col {
        Col {
            grid: self.clone(),
            name: name.to_string(),
        }
    }

    pub fn head(&self, n: i64) -> Grid {
        let n = n.max(0);
        self.wrap(format!("SELECT * FROM {} LIMIT {n}", self.subquery()))
    }

    pub fn size(&self) -> Result<i64, Box<EvalAltResult>> {
        let g = self.wrap(format!(
            "SELECT count() AS v FROM {} GROUP ALL",
            self.subquery()
        ));
        let grid = g.collect_json()?;
        Ok(grid
            .rows
            .first()
            .and_then(|r| r.get("v"))
            .and_then(|v| v.as_i64())
            .unwrap_or(0))
    }

    pub fn columns(&self) -> Result<Vec<String>, Box<EvalAltResult>> {
        Ok(self.head(1).collect_json()?.columns)
    }

    /// Collect to a rhai array of maps — the bounded "give me the rows" escape hatch. The catalog
    /// promises `Array<Map>` and every consumer (the `chart` helpers' `as_map`, `emit` data, a plain
    /// `for r in rows { r.col }`) reads named fields, so a positional row is normalized HERE, not at
    /// each call site. Two seam shapes feed this: the **platform** path (`store.query`/SurrealDB)
    /// returns JSON OBJECTS (`{"col": v, …}`) which pass through unchanged; the **federation** path
    /// (DataFusion over sqlite/postgres — see `extensions/federation/src/query.rs::shape`) returns
    /// column-aligned ARRAYS (`[v, …]`) which `row_to_map` zips with `columns` into a map here.
    /// Honoring the contract at the seam boundary is what makes `timeseries(query(...).records(), "ts")`
    /// a complete chart-ready rule on every source kind, not just the platform one.
    pub fn records(&self) -> Result<Array, Box<EvalAltResult>> {
        let grid = self.collect_json()?;
        Ok(grid
            .rows
            .into_iter()
            .map(|r| Dynamic::from(row_to_map(&r, &grid.columns)))
            .collect())
    }
}

impl GroupedGrid {
    /// Apply aggregate expressions, producing a grid grouped by the keys.
    pub fn agg(&self, exprs: Array) -> Result<Grid, Box<EvalAltResult>> {
        let aggs = string_list(exprs)?;
        if aggs.is_empty() {
            return Err(rhai_err("agg() needs at least one expression"));
        }
        let keys = self
            .keys
            .iter()
            .map(|k| quote_ident(k))
            .collect::<Result<Vec<_>, _>>()?
            .join(", ");
        let agg_list = aggs.join(", ");
        Ok(self.grid.wrap(format!(
            "SELECT {keys}, {agg_list} FROM {} GROUP BY {keys}",
            self.grid.subquery()
        )))
    }
}

impl Col {
    fn reduce(&self, expr_sql: &str) -> Result<Value, Box<EvalAltResult>> {
        let g = self.grid.wrap(format!(
            "SELECT {expr_sql} AS v FROM {}",
            self.grid.subquery()
        ));
        let grid = g.collect_json()?;
        Ok(grid
            .rows
            .into_iter()
            .next()
            .and_then(|r| r.get("v").cloned())
            .unwrap_or(Value::Null))
    }

    fn col_sql(&self) -> Result<String, Box<EvalAltResult>> {
        quote_ident(&self.name)
    }

    pub fn max(&self) -> Result<Dynamic, Box<EvalAltResult>> {
        json_to_scalar(self.reduce(&format!("math::max({})", self.col_sql()?))?)
    }
    pub fn min(&self) -> Result<Dynamic, Box<EvalAltResult>> {
        json_to_scalar(self.reduce(&format!("math::min({})", self.col_sql()?))?)
    }
    pub fn avg(&self) -> Result<Dynamic, Box<EvalAltResult>> {
        json_to_scalar(self.reduce(&format!("math::mean({})", self.col_sql()?))?)
    }
    pub fn sum(&self) -> Result<Dynamic, Box<EvalAltResult>> {
        json_to_scalar(self.reduce(&format!("math::sum({})", self.col_sql()?))?)
    }
    pub fn count(&self) -> Result<Dynamic, Box<EvalAltResult>> {
        json_to_scalar(self.reduce("count()")?)
    }
    pub fn std(&self) -> Result<Dynamic, Box<EvalAltResult>> {
        json_to_scalar(self.reduce(&format!("math::stddev({})", self.col_sql()?))?)
    }
    pub fn first(&self) -> Result<Dynamic, Box<EvalAltResult>> {
        json_to_scalar(self.reduce(&format!("array::first({})", self.col_sql()?))?)
    }
    pub fn last(&self) -> Result<Dynamic, Box<EvalAltResult>> {
        json_to_scalar(self.reduce(&format!("array::last({})", self.col_sql()?))?)
    }
    pub fn p(&self, pct: i64) -> Result<Dynamic, Box<EvalAltResult>> {
        json_to_scalar(self.reduce(&format!("math::percentile({}, {pct})", self.col_sql()?))?)
    }
}

/// Coerce a rhai array of string-likes into `Vec<String>`.
pub fn string_list(arr: Array) -> Result<Vec<String>, Box<EvalAltResult>> {
    arr.into_iter()
        .map(|d| {
            d.into_string()
                .map_err(|_| rhai_err("expected a string in the array"))
        })
        .collect()
}

/// Normalize one collected row into a rhai Map, regardless of which seam shape produced it:
///   - a JSON OBJECT (`{"col": v, …}`, the platform/Surreal path) → map of the same keys;
///   - a JSON ARRAY (`[v, …]`, the federation column-aligned path) → map keyed by `columns`, in order;
///   - a bare scalar (rare; a single-column result reduced to one cell) → single-cell map under the
///     first column name (or `"value"` if `columns` is somehow empty — an honest shape, never a crash).
///
/// This is the seam boundary where the two wire shapes collapse to the one shape every cage consumer
/// reads: a named map. Centralizing it here is what makes `records()` honor its `Array<Map>` catalog
/// contract on every source kind, so the `chart` helpers and a plain `r.col` access work uniformly.
pub fn row_to_map(row: &Value, columns: &[String]) -> Dynamic {
    match row {
        Value::Object(_) => json_to_dynamic(row),
        Value::Array(cells) => {
            let mut m = Map::new();
            for (i, c) in cells.iter().enumerate() {
                let key = columns
                    .get(i)
                    .cloned()
                    .unwrap_or_else(|| format!("col_{i}"));
                m.insert(key.into(), json_to_dynamic(c));
            }
            Dynamic::from_map(m)
        }
        other => {
            let mut m = Map::new();
            let key = columns.first().cloned().unwrap_or_else(|| "value".into());
            m.insert(key.into(), json_to_dynamic(other));
            Dynamic::from_map(m)
        }
    }
}

/// serde_json::Value → a rhai Dynamic (recursive).
pub fn json_to_dynamic(v: &Value) -> Dynamic {
    match v {
        Value::Null => Dynamic::UNIT,
        Value::Bool(b) => Dynamic::from_bool(*b),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Dynamic::from_int(i)
            } else {
                Dynamic::from_float(n.as_f64().unwrap_or(0.0))
            }
        }
        Value::String(s) => Dynamic::from(s.clone()),
        Value::Array(a) => {
            let arr: Array = a.iter().map(json_to_dynamic).collect();
            Dynamic::from_array(arr)
        }
        Value::Object(o) => {
            let mut m = Map::new();
            for (k, val) in o {
                m.insert(k.as_str().into(), json_to_dynamic(val));
            }
            Dynamic::from_map(m)
        }
    }
}

/// A scalar JSON value → a rhai Dynamic (for reductions; objects/arrays returned as-is).
fn json_to_scalar(v: Value) -> Result<Dynamic, Box<EvalAltResult>> {
    Ok(json_to_dynamic(&v))
}

/// A rhai Dynamic → serde_json::Value (recursive) — for `emit`/finding capture.
pub fn dynamic_to_json(d: &Dynamic) -> Value {
    if d.is_unit() {
        Value::Null
    } else if let Ok(b) = d.as_bool() {
        Value::Bool(b)
    } else if let Ok(i) = d.as_int() {
        Value::Number(i.into())
    } else if let Ok(f) = d.as_float() {
        serde_json::Number::from_f64(f)
            .map(Value::Number)
            .unwrap_or(Value::Null)
    } else if let Some(s) = d.read_lock::<String>() {
        Value::String(s.clone())
    } else if d.is_array() {
        let arr = d.clone().into_array().unwrap_or_default();
        Value::Array(arr.iter().map(dynamic_to_json).collect())
    } else if d.is_map() {
        let map = d.read_lock::<Map>();
        match map {
            Some(m) => {
                let mut o = serde_json::Map::new();
                for (k, v) in m.iter() {
                    o.insert(k.to_string(), dynamic_to_json(v));
                }
                Value::Object(o)
            }
            None => Value::Null,
        }
    } else {
        Value::String(d.to_string())
    }
}
