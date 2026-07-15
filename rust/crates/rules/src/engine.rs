//! `RuleEngine` — orchestrates one evaluation. **Ported from rubix-cube's `rules/mod.rs::RuleEngine`**,
//! re-seamed: it owns the [`DataSeam`] + [`AiSeam`] + limits, builds a fresh sandboxed engine per run,
//! registers the verb library closing over the run's pinned state, evaluates the body on a blocking
//! thread (CPU-bound rhai + the governors), drains the collectors, and classifies the output.
//!
//! The host constructs one `RuleEngine` per run (the seams close over the workspace + caller), so the
//! engine itself holds no durable state (rule 4).

use std::sync::Arc;

use crate::control::{RunControl, ABORT_CANCELLED, ABORT_PAUSED};
use crate::grid::GridCtx;
use crate::meter::{AiMeter, WriteMeter};
use crate::runtime::{AiBudget, GridJson, Rule, RuleError, RuleOutput, RuleRun};
use crate::sandbox::{build_engine_with_control, RuleLimits};
use crate::seam::{AiSeam, DataSeam, JobSeam, MessagingSeam};
use crate::verbs::{register, Collectors, JobWiring, RunWiring};

/// Per-run options beyond the constructor's seams (long-running-rules-scope). A synchronous
/// `rules.run` uses `RunOptions::default()`; a job-backed run passes the shared control + the
/// durable job binding.
#[derive(Default)]
pub struct RunOptions {
    /// The cooperative pause/cancel intent, shared with the host's control verbs. Observed by the
    /// per-operation governor — no author cooperation needed.
    pub control: Option<Arc<RunControl>>,
    /// The durable checkpoint binding: job id + seam + the persisted state folded from the
    /// transcript (a resume's memoized `job.step` lookups).
    pub job: Option<JobBinding>,
}

/// The durable half of [`RunOptions`] for a job-backed run.
pub struct JobBinding {
    pub id: String,
    pub seam: Arc<dyn JobSeam>,
    pub state: rhai::Map,
}

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
    messaging: Arc<dyn MessagingSeam>,
    limits: RuleLimits,
    ai_limits: AiLimits,
    /// The per-run write budget (`env::rules::MAX_WRITES`, default 32). Charged by every
    /// motion-producing messaging write; reads are uncharged (rules-messaging-scope).
    max_writes: u32,
    /// The run's read-only flag (rules-for-widgets slice 2). `false` = a panel repaint: the `insight`
    /// handle no-ops (charges nothing, logs a skip) so a dashboard refresh doesn't stamp durable
    /// records + notify fan-out on every paint (rule-raises-insight-scope §route:false). Defaults to
    /// `true` (a normal routed run) via [`RuleEngine::new`]; a caller opts a panel run out with
    /// [`RuleEngine::with_route`].
    route: bool,
}

impl RuleEngine {
    pub fn new(
        data: Arc<dyn DataSeam>,
        ai: Arc<dyn AiSeam>,
        messaging: Arc<dyn MessagingSeam>,
        limits: RuleLimits,
        ai_limits: AiLimits,
        max_writes: u32,
    ) -> Self {
        Self {
            data,
            ai,
            messaging,
            limits,
            ai_limits,
            max_writes,
            route: true,
        }
    }

    /// Set the run's `route` flag (rules-for-widgets slice 2). `false` = a read-only panel run: the
    /// `insight` handle no-ops. The host passes the run's `route` here so the cage can honor it.
    pub fn with_route(mut self, route: bool) -> Self {
        self.route = route;
        self
    }

    /// Evaluate `rule.body` against `run`. Returns the typed output; `run.findings`/`run.log`/
    /// `run.ai_spend` are filled from the collectors. Runs synchronously (callers spawn it on a
    /// blocking thread when inside async — the body is CPU-bound and uses the wall-clock governor).
    pub fn run(&self, rule: &Rule, run: &mut RuleRun) -> Result<RuleOutput, RuleError> {
        self.run_with(rule, run, RunOptions::default())
    }

    /// [`RuleEngine::run`] with per-run options: a shared [`RunControl`] (cooperative pause/cancel)
    /// and/or a durable [`JobBinding`] (checkpoints + resume state) for a job-backed run.
    pub fn run_with(
        &self,
        rule: &Rule,
        run: &mut RuleRun,
        opts: RunOptions,
    ) -> Result<RuleOutput, RuleError> {
        let mut engine = build_engine_with_control(&self.limits, opts.control.clone());

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
        let write_meter = Arc::new(WriteMeter::new(self.max_writes));

        let job = opts.job.map(|j| JobWiring {
            id: j.id,
            seam: j.seam,
            state: j.state,
            // The handle's `should_stop()` reads the same control the governor observes; a run
            // without one gets a fresh never-stopping default inside the handle.
            control: opts.control.clone().unwrap_or_default(),
        });

        let handles = register(
            &mut engine,
            RunWiring {
                ctx,
                data: self.data.clone(),
                allow,
                inputs: inputs.clone(),
                collectors: collectors.clone(),
                ai_seam: self.ai.clone(),
                meter: meter.clone(),
                context_rows: self.ai_limits.context_rows,
                messaging: self.messaging.clone(),
                write_meter,
                now: run.now,
                route: self.route,
                origin_ref: rule.name.clone(),
                limits: self.limits.clone(),
                job,
            },
        );

        // Build the scope: the handles + each bound param.
        let mut scope = rhai::Scope::new();
        scope.push("ai", handles.ai);
        scope.push("inbox", handles.inbox);
        scope.push("outbox", handles.outbox);
        scope.push("channel", handles.channel);
        scope.push("insight", handles.insight);
        scope.push("time", handles.time);
        scope.push("job", handles.job);
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
            Ok(value) => classify(value, !run.findings.is_empty()),
            Err(e) => Err(map_eval_error(*e)),
        }
    }
}

/// Classify the body's last value into a `RuleOutput` (port verbatim). A grid materializes via the
/// seam; a materialization fault (a source SQL/planning error, a sidecar fault) is AUTHOR FEEDBACK —
/// propagate it verbatim, never swallow it into `Nothing` (the workbench "honest, never blank" rule).
fn classify(value: rhai::Dynamic, has_findings: bool) -> Result<RuleOutput, RuleError> {
    if value.is_unit() {
        return Ok(if has_findings {
            RuleOutput::Findings
        } else {
            RuleOutput::Nothing
        });
    }
    // A returned grid materializes; anything else becomes a scalar JSON.
    if let Some(grid) = value.clone().try_cast::<crate::grid::Grid>() {
        return grid_to_json(&grid).map(RuleOutput::Grid);
    }
    Ok(RuleOutput::Scalar(crate::grid::dynamic_to_json(&value)))
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
    // The cooperative-control aborts (long-running-rules-scope) — typed so the host can park
    // (suspend) or finalize (cancel) the job instead of recording an author error. The token rides
    // the `ErrorTerminated` payload (its Display omits it), so match the variant, not the text.
    if let rhai::EvalAltResult::ErrorTerminated(token, _) = &e {
        match token.clone().into_string().as_deref() {
            Ok(ABORT_PAUSED) => return RuleError::Paused,
            Ok(ABORT_CANCELLED) => return RuleError::Cancelled,
            _ => {} // the deadline token stays an Eval (author feedback, unchanged)
        }
    }
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
