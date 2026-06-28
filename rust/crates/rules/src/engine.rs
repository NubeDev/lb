//! `RuleEngine` — orchestrates one evaluation. **Ported from rubix-cube's `rules/mod.rs::RuleEngine`**,
//! re-seamed: it owns the [`DataSeam`] + [`AiSeam`] + limits, builds a fresh sandboxed engine per run,
//! registers the verb library closing over the run's pinned state, evaluates the body on a blocking
//! thread (CPU-bound rhai + the governors), drains the collectors, and classifies the output.
//!
//! The host constructs one `RuleEngine` per run (the seams close over the workspace + caller), so the
//! engine itself holds no durable state (rule 4).

use std::sync::Arc;

use crate::grid::GridCtx;
use crate::meter::AiMeter;
use crate::runtime::{AiBudget, GridJson, Rule, RuleError, RuleOutput, RuleRun};
use crate::sandbox::{build_engine, RuleLimits};
use crate::seam::{AiSeam, DataSeam};
use crate::verbs::{register, Collectors};

/// The AI budget knobs for a run (mirrors rubix-cube's `env::rules::AI_*`).
#[derive(Debug, Clone, Copy)]
pub struct AiLimits {
    pub max_calls: u32,
    pub max_tokens: u32,
    pub context_rows: usize,
}

impl Default for AiLimits {
    fn default() -> Self {
        Self {
            max_calls: 8,
            max_tokens: 20_000,
            context_rows: 200,
        }
    }
}

/// An evaluatable rule engine for ONE run. Build it with the seams (already closed over the workspace
/// + caller) and the limits; call [`RuleEngine::run`].
pub struct RuleEngine {
    data: Arc<dyn DataSeam>,
    ai: Arc<dyn AiSeam>,
    limits: RuleLimits,
    ai_limits: AiLimits,
}

impl RuleEngine {
    pub fn new(
        data: Arc<dyn DataSeam>,
        ai: Arc<dyn AiSeam>,
        limits: RuleLimits,
        ai_limits: AiLimits,
    ) -> Self {
        Self {
            data,
            ai,
            limits,
            ai_limits,
        }
    }

    /// Evaluate `rule.body` against `run`. Returns the typed output; `run.findings`/`run.log`/
    /// `run.ai_spend` are filled from the collectors. Runs synchronously (callers spawn it on a
    /// blocking thread when inside async — the body is CPU-bound and uses the wall-clock governor).
    pub fn run(&self, rule: &Rule, run: &mut RuleRun) -> Result<RuleOutput, RuleError> {
        let mut engine = build_engine(&self.limits);

        let ctx = Arc::new(GridCtx {
            data: self.data.clone(),
        });
        let allow = run.allowed_sources.clone();
        let inputs = Arc::new(run.inputs.clone());
        let collectors = Arc::new(Collectors::default());
        let meter = Arc::new(AiMeter::new(
            self.ai_limits.max_calls,
            self.ai_limits.max_tokens,
        ));

        let ai_handle = register(
            &mut engine,
            ctx,
            self.data.clone(),
            allow,
            inputs.clone(),
            collectors.clone(),
            self.ai.clone(),
            meter.clone(),
            self.ai_limits.context_rows,
        );

        // Build the scope: the `ai` handle + each bound param as a top-level variable.
        let mut scope = rhai::Scope::new();
        scope.push("ai", ai_handle);
        for (name, value) in inputs.iter() {
            scope.push_dynamic(name.as_str(), value.clone());
        }

        let result = engine.eval_with_scope::<rhai::Dynamic>(&mut scope, &rule.body);

        // Drain collectors regardless of outcome (findings emitted before a later error still count).
        run.findings = collectors.drain_findings();
        run.log = collectors.drain_log();
        run.ai_spend = AiBudget {
            calls: meter.calls_used(),
            tokens: meter.tokens_used(),
        };

        match result {
            Ok(value) => Ok(classify(value, !run.findings.is_empty())),
            Err(e) => Err(map_eval_error(*e)),
        }
    }
}

/// Classify the body's last value into a `RuleOutput` (port verbatim).
fn classify(value: rhai::Dynamic, has_findings: bool) -> RuleOutput {
    if value.is_unit() {
        return if has_findings {
            RuleOutput::Findings
        } else {
            RuleOutput::Nothing
        };
    }
    // A returned grid materializes; anything else becomes a scalar JSON.
    if let Some(grid) = value.clone().try_cast::<crate::grid::Grid>() {
        match grid_to_json(&grid) {
            Ok(g) => return RuleOutput::Grid(g),
            Err(_) => return RuleOutput::Nothing,
        }
    }
    RuleOutput::Scalar(crate::grid::dynamic_to_json(&value))
}

fn grid_to_json(grid: &crate::grid::Grid) -> Result<GridJson, RuleError> {
    grid.collect_json()
        .map(|g| GridJson {
            columns: g.columns,
            rows: g.rows,
        })
        .map_err(|e| RuleError::Seam(e.to_string()))
}

/// Map a rhai eval error onto a `RuleError`. A seam error surfaced via our `source not allowed` /
/// `seam error` text is classified so the MCP layer can keep the deny opaque.
fn map_eval_error(e: rhai::EvalAltResult) -> RuleError {
    let msg = e.to_string();
    if msg.contains("source not allowed") {
        // strip our prefix for the typed error
        let name = msg
            .split("source not allowed:")
            .nth(1)
            .map(|s| s.trim().to_string())
            .unwrap_or_default();
        RuleError::SourceNotAllowed(name)
    } else {
        RuleError::Eval(msg)
    }
}
