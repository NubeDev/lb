//! The `ai.*` verbs — `ai.ask`/`complete`/`classify`/`embed`. **Ported from rubix-cube's
//! `verbs/ai.rs`**, **re-seamed** to the host [`AiSeam`] (the AI-gateway). The two invariants transfer
//! exactly:
//!   - **the budget meter** ([`crate::meter::AiMeter`]): every call charges; a loop can't overspend;
//!   - **the nsql FENCE**: `ai.ask`'s proposed SQL is re-validated through the SAME path `query()` uses
//!     (`build_grid`/the seam collect) before it can run — there is no path from a model-proposed query
//!     to execution that skips the validator + `caps::check`.

use std::collections::HashSet;
use std::sync::Arc;

use rhai::{Array, Dynamic, Engine, EvalAltResult};

use crate::grid::{rhai_err, string_list, Grid, GridCtx};
use crate::meter::AiMeter;
use crate::seam::{AiSeam, DataSeam, SourceKind};

/// The `ai` scope value — closes over the AI-gateway seam + the data seam (for the fence) + the meter.
#[derive(Clone)]
pub struct AiHandle {
    ai: Arc<dyn AiSeam>,
    ctx: Arc<GridCtx>,
    data: Arc<dyn DataSeam>,
    allow: Arc<HashSet<String>>,
    meter: Arc<AiMeter>,
    context_rows: usize,
}

impl AiHandle {
    pub fn new(
        ai: Arc<dyn AiSeam>,
        ctx: Arc<GridCtx>,
        data: Arc<dyn DataSeam>,
        allow: Arc<HashSet<String>>,
        meter: Arc<AiMeter>,
        context_rows: usize,
    ) -> Self {
        Self {
            ai,
            ctx,
            data,
            allow,
            meter,
            context_rows,
        }
    }

    /// ai.ask(question) → Grid. Propose SQL from the model, then RE-VALIDATE it through the same seam
    /// path a hand-written `query()` takes (the fence). The proposed SQL targets the workspace's own
    /// schemas only. We default the resolved grid to the first granted PLATFORM source's kind — the
    /// host's collect re-checks `caps` regardless.
    pub fn ask(&self, question: &str) -> Result<Grid, Box<EvalAltResult>> {
        self.meter.charge_call().map_err(rhai_err)?;
        // Schemas of the workspace's OWN granted sources only (never cross-tenant).
        let schemas = self.data.schemas().map_err(rhai_err)?;
        let sql = self.ai.propose_sql(question, &schemas).map_err(rhai_err)?;
        // THE FENCE: the proposed SQL becomes a grid over a granted source; the host's collect runs the
        // SAME validator + caps check a `query()` would. We bind it to a representative granted source;
        // a proposed cross-source query is rejected at collect by the workspace pin.
        let source = self
            .allow
            .iter()
            .next()
            .cloned()
            .ok_or_else(|| rhai_err("ai.ask: no granted source to query"))?;
        let (kind, resolved) = self.data.resolve(&source).map_err(rhai_err)?;
        let _ = kind;
        Ok(Grid::new(
            SourceKind::Platform,
            resolved,
            sql,
            self.ctx.clone(),
        ))
    }

    pub fn complete(&self, prompt: &str, context: &str) -> Result<String, Box<EvalAltResult>> {
        self.meter.charge_call().map_err(rhai_err)?;
        let full = if context.is_empty() {
            prompt.to_string()
        } else {
            format!("{prompt}\n\nContext:\n{context}")
        };
        let completion = self.ai.complete(&full).map_err(rhai_err)?;
        self.meter
            .charge_tokens(completion.tokens)
            .map_err(rhai_err)?;
        Ok(completion.text)
    }

    /// Bound the grid to the context cap before sending its rows to the model.
    fn grid_context(&self, g: &Grid) -> Result<String, Box<EvalAltResult>> {
        let bounded = g.head(self.context_rows as i64);
        let grid = bounded.collect_json()?;
        serde_json::to_string(&grid.rows).map_err(|e| rhai_err(e.to_string()))
    }

    /// ai.classify(grid, labels) → an array of `{...row, label}` maps. Rejects an over-large grid
    /// BEFORE charging (filter/head first), charges the budget, sends the bounded rows + the labels to
    /// the model, and attaches the parsed label per row. Returns a rhai array (not a lazy Grid): the
    /// labels are a materialized model output, not a composable SQL plan — a downstream `records()`-
    /// style consumer is what the author wants. (A lazy literal-rows grid is additive later.)
    pub fn classify(&self, g: &Grid, labels: Array) -> Result<Array, Box<EvalAltResult>> {
        let labels = string_list(labels)?;
        if labels.is_empty() {
            return Err(rhai_err("ai.classify() needs at least one label"));
        }
        let total = g.size()?;
        if total as usize > self.context_rows {
            return Err(rhai_err(format!(
                "ai.classify(): grid has {total} rows (> {} cap) — filter or head() first",
                self.context_rows
            )));
        }
        self.meter.charge_call().map_err(rhai_err)?;
        let bounded = g.head(self.context_rows as i64);
        let grid = bounded.collect_json()?;
        let ctx = serde_json::to_string(&grid.rows).map_err(|e| rhai_err(e.to_string()))?;
        let label_list = labels
            .iter()
            .map(|l| format!("\"{l}\""))
            .collect::<Vec<_>>()
            .join(", ");
        let prompt = format!(
            "Classify each JSON row into exactly one of [{label_list}]. \
             Reply with ONLY a JSON array of label strings in row order, no prose.\n\nRows:\n{ctx}"
        );
        let completion = self.ai.complete(&prompt).map_err(rhai_err)?;
        self.meter
            .charge_tokens(completion.tokens)
            .map_err(rhai_err)?;
        let parsed: Vec<String> = serde_json::from_str(completion.text.trim()).unwrap_or_default();
        // Build the labelled rows as a rhai array of maps.
        let mut out = Array::new();
        for (i, row) in grid.rows.iter().enumerate() {
            let mut obj = row.clone();
            if let Some(map) = obj.as_object_mut() {
                let label = parsed.get(i).cloned().unwrap_or_default();
                map.insert("label".to_string(), serde_json::Value::String(label));
            }
            out.push(crate::grid::json_to_dynamic(&obj));
        }
        Ok(out)
    }

    pub fn embed(&self, text: &str) -> Result<Vec<Dynamic>, Box<EvalAltResult>> {
        self.meter.charge_call().map_err(rhai_err)?;
        let v = self.ai.embed(text).map_err(rhai_err)?;
        Ok(v.into_iter().map(Dynamic::from_float).collect())
    }
}

pub fn register(engine: &mut Engine) {
    engine.register_type_with_name::<AiHandle>("Ai");
    engine.register_fn("ask", |a: &mut AiHandle, q: &str| a.ask(q));
    engine.register_fn("complete", |a: &mut AiHandle, p: &str| a.complete(p, ""));
    engine.register_fn("complete", |a: &mut AiHandle, p: &str, ctx: &str| {
        a.complete(p, ctx)
    });
    engine.register_fn("complete", |a: &mut AiHandle, p: &str, grid: Grid| {
        let ctx = a.grid_context(&grid)?;
        a.complete(p, &ctx)
    });
    engine.register_fn("classify", |a: &mut AiHandle, g: Grid, labels: Array| {
        a.classify(&g, labels)
    });
    engine.register_fn("embed", |a: &mut AiHandle, t: &str| a.embed(t));
}
