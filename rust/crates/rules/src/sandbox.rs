//! The cage — `RuleLimits` + `build_engine_with_control`. **Lifted verbatim from rubix-cube** (`rules/sandbox.rs`,
//! MIT/Apache-2.0). The security model is "absence of capability + presence of limits": the builder
//! registers NO file/net/process API and sets every resource governor, so a rule can do nothing but
//! call the verbs the host hands it — exactly the capability-first posture (rule 5), realized in-process.
//! Defense in depth: the governors bound *work and time* (DoS); `caps::check` (at every seam verb) bounds
//! *authority*.

use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::control::{RunControl, ABORT_CANCELLED, ABORT_PAUSED};

/// The resource governors for one rule run. Defaults come from node config (the `env::rules::*` knobs
/// rubix-cube uses); a per-workspace override is additive, not v1 (rules-engine-scope "Resolved").
#[derive(Debug, Clone)]
pub struct RuleLimits {
    /// Max bytecode operations before the engine aborts (bounds an infinite loop fast).
    pub max_operations: u64,
    /// Max function-call nesting (bounds unbounded recursion).
    pub max_call_levels: usize,
    /// Max bytes in any one string value.
    pub max_string_bytes: usize,
    /// Max elements in any one array.
    pub max_array_len: usize,
    /// Max entries in any one object map.
    pub max_map_len: usize,
    /// Wall-clock deadline for the whole run (bounds a legitimately-large-but-slow run).
    pub timeout: Duration,
    /// Max rows a `Frame` may materialize (data-stdlib-scope governors — the deadline cannot
    /// interrupt a native polars call, so the bound moves to the inputs).
    pub max_frame_rows: usize,
    /// Max cells (rows × cols) any one `Frame` op may produce (bounds a join/pivot explosion).
    pub max_frame_cells: usize,
}

impl Default for RuleLimits {
    fn default() -> Self {
        // Conservative in-process defaults. The host overrides from config at construction.
        Self {
            max_operations: 5_000_000,
            max_call_levels: 64,
            max_string_bytes: 256 * 1024,
            max_array_len: 100_000,
            max_map_len: 100_000,
            timeout: Duration::from_secs(10),
            max_frame_rows: 200_000,
            max_frame_cells: 2_000_000,
        }
    }
}

/// Build a fresh, fully-governed rhai engine with ZERO I/O surface. A new engine is built per run,
/// so the wall-clock deadline starts now and no state leaks between runs (rule 4 — stateless).
///
/// `control` is the run's optional cooperative [`RunControl`] (long-running-rules-scope): a
/// job-backed run shares it with the host's `rules.runs.suspend`/`cancel` verbs, and the
/// per-operation governor observes it — pause/cancel bite between bytecode ops without any author
/// cooperation. A synchronous `rules.run` passes `None` (nothing can park it).
pub fn build_engine_with_control(
    limits: &RuleLimits,
    control: Option<Arc<RunControl>>,
) -> rhai::Engine {
    let mut engine = rhai::Engine::new();
    engine.set_max_operations(limits.max_operations);
    engine.set_max_call_levels(limits.max_call_levels);
    engine.set_max_string_size(limits.max_string_bytes);
    engine.set_max_array_size(limits.max_array_len);
    engine.set_max_map_size(limits.max_map_len);
    engine.set_max_expr_depths(64, 64);

    // Strip the two escape hatches: dynamic `eval` and module imports. A rule is a leaf script, never
    // a loader (rules-engine-scope non-goal: "never an extension-loading mechanism").
    engine.disable_symbol("eval");
    engine.set_max_modules(0);

    // Determinism (data-stdlib-scope): rhai's built-in `timestamp()` is a live wall-clock `Instant`.
    // Shadow it with an author error so the only clock a body can read is the run's injected `time`
    // handle — a re-run with the same inputs and `now` stays byte-identical.
    engine.register_fn(
        "timestamp",
        || -> Result<rhai::Dynamic, Box<rhai::EvalAltResult>> {
            Err("timestamp() is disabled; use time.now() (the run's injected clock)".into())
        },
    );

    // Wall-clock deadline + cooperative control via the progress callback (fires per operation).
    // `Some(token)` aborts the run; the engine maps the token onto the typed RuleError.
    let deadline = Instant::now() + limits.timeout;
    engine.on_progress(move |_ops| {
        if let Some(control) = &control {
            match control.intent() {
                crate::control::ControlIntent::Cancel => return Some(ABORT_CANCELLED.into()),
                crate::control::ControlIntent::Pause => return Some(ABORT_PAUSED.into()),
                crate::control::ControlIntent::Run => {}
            }
        }
        if Instant::now() >= deadline {
            Some("rule exceeded time budget".into())
        } else {
            None
        }
    });

    engine
}
