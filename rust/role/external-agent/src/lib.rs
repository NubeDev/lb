//! `lb-role-external-agent` — the external-agent **role** crate (external-agent sub-scope #1/#2). It
//! supplies the [`AcpRuntime`] `impl lb_host::AgentRuntime` and the [`register`] hook that populates a
//! node's [`RuntimeRegistry`](lb_host::RuntimeRegistry) with one external runtime per built-in
//! profile — **only** when the `node` binary is built with the `external-agent` feature (the feature
//! gates the *dependency*, so a feature-off node never compiles this crate; runtime-seam #1).
//!
//! **The load-bearing property: swappability.** `AcpRuntime` is one code path. Which external agent it
//! drives — Open Interpreter (default), VT Code, or Codex — is the `profile_id` it was registered
//! under (resolved in [`profiles`]). Replacing Open Interpreter with VT Code one day is a config
//! change (register a different id / set the invoke `runtime` arg), never a code change. The swap test
//! (this crate's tests) proves it by driving two profiles through the identical body.
//!
//! **Transport (implementation status).** This slice reaches the agent over the shipped
//! `lb_external_agent::drive` (`exec --json` NDJSON) — the verified seam-proof. The full ACP SDK swap
//! is the next slice and is *additive* (the seam is transport-agnostic). Recorded, not faked.
//!
//! **Named, linked seams NOT built here (per this session's scope):**
//! - **#3 capability-wall / OS sandbox + built-ins-off:** the scratch-dir cwd seal ([`scratch`]) is in;
//!   the kernel egress/fs confinement and the fail-closed built-ins-off assertion are `capability-wall
//!   -scope.md`. TODO(#3): wrap [`scratch::ScratchDir`] in the OS sandbox before spawn.
//! - **#4 model-routing / served OpenAI-compat endpoint + scoped token:** the profile carries a
//!   `ModelEndpoint` (config), but the gateway does not yet *serve* an OpenAI face. TODO(#4):
//!   `model-routing-scope.md` (blocking prereq — no owner yet).
//! - **#5 durable job / resume / supervision:** `run` emits `RunEvent`s that a #5 job would persist,
//!   but the run here is a collect-then-forward, not yet a supervised durable job with resume.
//!   TODO(#5): `run-lifecycle-scope.md`.

use std::future::Future;
use std::pin::Pin;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use lb_external_agent::{drive, ModelEndpoint};
use lb_host::{AgentError, AgentRuntime, Node, RunContext, RuntimeRegistry};
use lb_run_events::{RunEvent, RunOutcome};

pub mod profiles;
pub mod scratch;

/// Re-exported so a caller (the node binary) names the model-endpoint config type without depending
/// on the leaf `lb-external-agent` crate directly.
pub use lb_external_agent::ModelEndpoint;

use profiles::{resolve_builtin, ResolvedAgent, BUILTIN_IDS};
use scratch::{default_scratch_base, ScratchDir};

/// A liveness bound for a run driven over `exec --json`. This is NOT the #5 supervision ceiling (which
/// is wall-time + token/iteration and owns kill/restart/reap) — it is the same local liveness timeout
/// the leaf `drive(..)` already enforces, surfaced as a runtime knob. TODO(#5): replace with the
/// supervisor's bounded, killable, reap-on-every-exit-path run.
const DEFAULT_RUN_TIMEOUT: Duration = Duration::from_secs(600);

/// The ONE external runtime (external-agent #2, `AcpRuntime`). One per registered profile id; the body
/// is identical across agents (the difference is the [`ResolvedAgent`] it holds). Named `AcpRuntime`
/// after its integration target even though this slice's wire is `exec --json` — the ACP swap is
/// additive and does not change this type's role in the seam.
pub struct AcpRuntime {
    id: String,
    agent: ResolvedAgent,
    scratch_base: PathBuf,
    timeout: Duration,
}

impl AcpRuntime {
    /// Build an `AcpRuntime` for built-in profile `id` over `model`. Returns `None` if `id` is not a
    /// built-in profile (the caller/registry treats that as unconfigured). `scratch_base` is the
    /// node's scratch root (config; defaults to the OS temp dir).
    pub fn new(id: &str, model: ModelEndpoint, scratch_base: PathBuf) -> Option<Self> {
        let agent = resolve_builtin(id, model)?;
        Some(Self {
            id: id.to_string(),
            agent,
            scratch_base,
            timeout: DEFAULT_RUN_TIMEOUT,
        })
    }
}

impl AgentRuntime for AcpRuntime {
    fn id(&self) -> &str {
        &self.id
    }

    fn run<'a>(
        &'a self,
        node: &'a Node,
        ctx: RunContext<'a>,
    ) -> Pin<Box<dyn Future<Output = Result<String, AgentError>> + Send + 'a>> {
        Box::pin(async move {
            // WORKSPACE-ISOLATION SEAL (filesystem): a per-run scratch dir + cwd under `{base}/{ws}/
            // {job_id}`. Two runs never share a dir; a ws=A run's tree can't appear under ws=B. This
            // is the local, testable half of the zero-cross-ws-bleed invariant (#5); #3 adds the
            // kernel confinement of this same dir.
            let scratch = ScratchDir::create(&self.scratch_base, ctx.ws, ctx.job_id)
                .map_err(|e| AgentError::BadInput(format!("scratch dir: {e}")))?;

            // Drive the real subprocess over the shipped `exec --json` transport, collecting the
            // projected `RunEvent`s. The cwd is the sealed scratch dir — NOT the caller's workspace
            // path (the old `drive(.., workspace, ..)` cwd gap the run-lifecycle scope flagged).
            let cwd = scratch.path().to_string_lossy().into_owned();
            let events = drive(
                self.agent.wrapper.as_ref(),
                &self.agent.profile,
                ctx.goal,
                &cwd,
                self.timeout,
            )
            .await
            .map_err(|e| AgentError::BadInput(format!("external agent run failed: {e}")))?;

            // Forward each event as motion so a watcher observes an external run identically to an
            // in-house one (agent-run Part 3). Best-effort publish (the transcript is authority in #5;
            // this slice has no durable transcript yet — TODO(#5)).
            for event in &events {
                lb_host::publish_run_event(&node.bus, ctx.ws, ctx.job_id, event).await;
            }

            // The answer + fail-closed terminal outcome. `terminal_outcome` maps the projected finish
            // (an unrecognised agent status → `Failed`, never `Done` — the untrusted-agent rule the
            // leaf's `outcome_of` already enforces). We surface a failed run as an error at the seam;
            // #5 will make the *job/exit* authoritative over this hint.
            let answer = final_answer(&events);
            match run_outcome(&events) {
                Some(RunOutcome::Failed) => Err(AgentError::BadInput(format!(
                    "external agent run ended failed: {answer}"
                ))),
                _ => Ok(answer),
            }
        })
    }
}

/// The run's final answer = the concatenation of assistant `TextDelta`s (per-step in this transport,
/// one whole-content delta per model message). Mirrors how a watcher would assemble the answer text.
fn final_answer(events: &[RunEvent]) -> String {
    events
        .iter()
        .filter_map(|e| match e {
            RunEvent::TextDelta { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("")
}

/// The run's terminal outcome, if the stream carried a `RunFinish`. `None` when the agent emitted no
/// terminal event (an empty/aborted stream) — treated as non-failed here; #5 makes exit authoritative.
fn run_outcome(events: &[RunEvent]) -> Option<RunOutcome> {
    events.iter().rev().find_map(|e| match e {
        RunEvent::RunFinish { outcome, .. } => Some(*outcome),
        _ => None,
    })
}

/// The **registration hook** (runtime-seam #1): add one [`AcpRuntime`] per built-in profile to
/// `registry`, over the node's configured `model` + `scratch_base`. Called ONLY from the `node`
/// binary's feature-gated path — a feature-off node never links this crate, so no external entry
/// exists (the OFF build's default-only registry). Additive: the default in-house runtime is
/// untouched.
pub fn register(registry: &mut RuntimeRegistry, model: ModelEndpoint, scratch_base: Option<PathBuf>) {
    let base = scratch_base.unwrap_or_else(default_scratch_base);
    for id in BUILTIN_IDS {
        if let Some(rt) = AcpRuntime::new(id, model.clone(), base.clone()) {
            registry.register(Arc::new(rt));
        }
    }
}
