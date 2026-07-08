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
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use lb_external_agent::drive;
use lb_host::{
    get_agent_config, resolve_endpoint_key_host, AgentError, AgentRuntime, Node, RunContext,
    RuntimeRegistry,
};
use lb_jobs::{complete, create, Job, JobStatus};
use lb_run_events::{RunEvent, RunOutcome};

pub mod bridge;
pub mod profiles;
pub mod scratch;
pub mod token;

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

/// **No-progress (stall) ceiling** — the wall-clock quiet period after which a run that has emitted no
/// `RunEvent` (no tool call, no text, no reasoning) is reaped as *stuck*, distinct from the outer
/// wall-time ceiling that bounds a run that is *making progress but slow*. This closes the gap
/// `run-lifecycle-scope.md` flags open: the in-house loop self-bounds at `MAX_STEPS`, but an external
/// subprocess was bounded ONLY by wall-time — so a flailing agent (e.g. one shelling `make dev` in a
/// loop) burned the full 15-minute ceiling before failing. A stall is reaped in `NO_PROGRESS_CEILING`
/// instead: the `drive` future is dropped (closing the ACP session + subprocess stdio, the same reaper
/// seam the wall uses), and the run ends with a distinct, honest message so the caller can tell a
/// *stuck* agent from a *slow* one.
///
/// Reset by EVERY streamed `RunEvent`, so a run that is genuinely working (streaming tool calls / text)
/// never trips it — only true silence does. A model that thinks for a while WITHOUT emitting any event
/// is the edge case; the transport emits reasoning/step events during real work, so quiet this long is
/// a strong stuck signal. Tunable per runtime via [`AcpRuntime::with_no_progress_ceiling`] (a test uses
/// a tiny value against a scripted silent runtime).
const NO_PROGRESS_CEILING: Duration = Duration::from_secs(90);

/// The ONE external runtime (external-agent #2, `AcpRuntime`). One per registered profile id; the body
/// is identical across agents (the difference is the [`ResolvedAgent`] it holds). Named `AcpRuntime`
/// after its integration target even though this slice's wire is `exec --json` — the ACP swap is
/// additive and does not change this type's role in the seam.
pub struct AcpRuntime {
    id: String,
    agent: ResolvedAgent,
    scratch_base: PathBuf,
    timeout: Duration,
    /// The no-progress (stall) ceiling — a run silent this long is reaped as stuck. Defaults to
    /// [`NO_PROGRESS_CEILING`]; a test overrides it to a tiny value.
    no_progress: Duration,
    /// The shim binary name/path the wrapper's MCP config points at. `None` ⇒ `"lb-mcp-shim"`
    /// (expected on the node's `PATH`). Configurable so a deployment can point at an absolute path.
    shim_bin: Option<String>,
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
            no_progress: NO_PROGRESS_CEILING,
            shim_bin: None,
        })
    }

    /// Override the no-progress (stall) ceiling (default: [`NO_PROGRESS_CEILING`]). Used by a test to
    /// reap a scripted silent runtime at a tiny quiet period.
    pub fn with_no_progress_ceiling(mut self, quiet: Duration) -> Self {
        self.no_progress = quiet;
        self
    }

    /// Construct an `AcpRuntime` from an explicit [`ResolvedAgent`] (profile + wrapper) rather than a
    /// built-in id. This is the seam a **test** uses to drive a scripted wrapper/binary (e.g. a silent
    /// `sleep` to exercise the no-progress ceiling) without a real agent CLI — the run body is
    /// identical to a built-in profile's.
    pub fn from_resolved(id: &str, agent: ResolvedAgent, scratch_base: PathBuf) -> Self {
        Self {
            id: id.to_string(),
            agent,
            scratch_base,
            timeout: DEFAULT_RUN_TIMEOUT,
            no_progress: NO_PROGRESS_CEILING,
            shim_bin: None,
        }
    }

    /// Override the shim binary path (default: `"lb-mcp-shim"` on `PATH`). Used when the node binary
    /// is installed in a known location and the shim sits beside it.
    pub fn with_shim_bin(mut self, shim_bin: impl Into<String>) -> Self {
        self.shim_bin = Some(shim_bin.into());
        self
    }
}

impl AgentRuntime for AcpRuntime {
    fn id(&self) -> &str {
        &self.id
    }

    fn run<'a>(
        &'a self,
        node: &'a Arc<Node>,
        ctx: RunContext<'a>,
    ) -> Pin<Box<dyn Future<Output = Result<String, AgentError>> + Send + 'a>> {
        Box::pin(async move {
            // WORKSPACE-ISOLATION SEAL (filesystem): a per-run scratch dir + cwd under `{base}/{ws}/
            // {job_id}`. Two runs never share a dir; a ws=A run's tree can't appear under ws=B. This
            // is the local, testable half of the zero-cross-ws-bleed invariant (#5); #3 adds the
            // kernel confinement of this same dir.
            let scratch = ScratchDir::create(&self.scratch_base, ctx.ws, ctx.job_id)
                .map_err(|e| AgentError::BadInput(format!("scratch dir: {e}")))?;

            // DURABLE JOB RECORD (run-lifecycle #5 + agent-key-lifecycle D3): create the job so the
            // gateway's run-status gate (`verify_token` → `lb_jobs::load`) can refuse a cancelled
            // run's token instantly. The payload carries the persona's Ask floor (`ask` list) so the
            // gateway's `/mcp/call` can enforce the same Ask suspension for bridged calls that the
            // in-house loop enforces at the model-proposal layer. Marked Done/Failed at the end.
            let ask_floor = ctx
                .persona_preset
                .map(|p| p.ask.clone())
                .unwrap_or_default();
            let payload = serde_json::to_string(&RunPayload {
                goal: ctx.goal.to_string(),
                ask: ask_floor,
            })
            .unwrap_or_else(|_| String::new());
            let _ = create(
                &node.store,
                ctx.ws,
                &Job::new(ctx.job_id, "agent-session", &payload, ctx.ts),
            )
            .await;

            // LIVE MOTION: publish each `RunEvent` the moment its stdout line decodes, so a watcher
            // (agent-run Part 3 / the channel run-feed) sees the agent work in real time — tool calls,
            // reasoning, text — not a burst at the end. `drive` taps every event to this unbounded
            // channel as it streams; a detached publisher forwards them onto the ws-walled run subject.
            // (Best-effort: the durable transcript is #5's job; this slice's authority is the answer.)
            let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<RunEvent>();
            let bus = node.bus.clone();
            let ws = ctx.ws.to_string();
            let job = ctx.job_id.to_string();
            // NO-PROGRESS WATCHDOG (run-lifecycle #5, stall ceiling): a shared "last activity" tick,
            // bumped by the publisher on every streamed `RunEvent`. A separate watchdog future polls it
            // and fires when the run has been silent past `no_progress`. Racing `drive` against the
            // watchdog lets a STUCK run be reaped promptly (drop `drive` → reap the subprocess) instead
            // of burning the whole outer wall-time ceiling. Seeded to "now" so the clock starts at spawn.
            let progress = Arc::new(tokio::sync::Notify::new());
            let progress_pub = progress.clone();
            let publisher = tokio::spawn(async move {
                while let Some(event) = rx.recv().await {
                    // Any event = progress; wake the watchdog so it re-arms from now.
                    progress_pub.notify_one();
                    lb_host::publish_run_event(&bus, &ws, &job, &event).await;
                }
            });

            // SEALED-KEY RESOLUTION (agent-catalog test-and-secrets scope): resolve the model key with
            // precedence **workspace `agent.config` sealed secret → node env → unset**. The active
            // `agent.config.model_endpoint.api_key_secret` (set self-serve via the UI/`secret.set`) is a
            // PATH into `lb-secrets`; absent it, the profile's `api_key_env` NAME is the fallback (the
            // pre-sealed-key behavior — the operator's node env). The resolved VALUE is injected into the
            // child env under the name the WRAPPER tells the CLI to read (`env_key=<profile.api_key_env>`)
            // so the injected value and the CLI's `env_key` always agree — for that one child, never a
            // record or log (names-only holds; §6.7).
            //
            // HOST-MEDIATED read: the model key is workspace INFRASTRUCTURE (like a federated DSN),
            // resolved by the HOST on the run's behalf — NOT under the end user's caps. A run executes
            // under a derived `agent:` actor that legitimately holds no `secret:<path>:get`, and the
            // delegation clamp (gate 2b) would block even a handed-in cap; `resolve_endpoint_key_host`
            // reads the `Workspace`-visibility key wall-first (this `ws`) without that per-user gate. It
            // cannot widen isolation (a ws-B run can't name ws-A's path) and a `Private` key is never
            // resolvable here — only the workspace-shared model key the admin sealed for the run.
            let key_env = self.agent.profile.model.api_key_env.as_str();
            let secret_path = get_agent_config(&node.store, ctx.ws)
                .await
                .ok()
                .flatten()
                .and_then(|c| c.model_endpoint)
                .and_then(|e| e.api_key_secret);
            let key_value = resolve_endpoint_key_host(
                &node.store,
                ctx.ws,
                secret_path.as_deref(),
                Some(key_env),
            )
            .await;
            // Only inject when we resolved a value; `None` lets the child inherit the process env
            // (the fallback), so a workspace that set nothing keeps working exactly as before.
            let key = key_value.as_deref().map(|v| (key_env, v));

            // MCP-SHIM BRIDGE (external-agent-authoring scope S1d): mint a short-TTL run-scoped
            // token for the derived principal, build the narrowed menu JSON, write the wrapper's
            // MCP config into the scratch dir, and produce the bridge env vars the child carries.
            // The shim (spawned by the agent as its MCP server) inherits these env vars, reads the
            // menu by path, and forwards every `tools/call` to the gateway — which re-checks caps
            // exactly as it does for the UI. `None` ⇒ the wrapper has no MCP bridge today; the run
            // proceeds pre-bridge (the agent has no host-tool reach, same as before this slice).
            //
            // The token never appears in the goal, a transcript, or a log — only in the per-child
            // env map + the per-run config file inside the sealed scratch dir (mode 0600).
            let derived = ctx.caller.derive("agent:session", ctx.agent_caps.to_vec());
            let now = ctx.ts;
            let node_key = node.key();
            let run_tok = token::mint_run_token(
                &node_key,
                &derived,
                ctx.job_id,
                now,
                token::DEFAULT_RUN_TOKEN_TTL_SECS,
            );
            let bridge_env = match bridge::build(
                self.agent.wrapper.as_ref(),
                scratch.path(),
                ctx.job_id,
                ctx.tools,
                &run_tok,
                self.shim_bin.as_deref().unwrap_or("lb-mcp-shim"),
            ) {
                Ok(Some(b)) => b.env,
                Ok(None) => Vec::new(),
                Err(e) => {
                    return Err(AgentError::BadInput(format!(
                        "write MCP bridge config: {e}"
                    )))
                }
            };

            // Drive the real subprocess over the shipped `exec --json` transport, streaming each event
            // live (above) AND collecting them for the final answer. The cwd is the sealed scratch dir —
            // NOT the caller's workspace path (the `drive(.., workspace, ..)` cwd gap #5 flagged).
            let cwd = scratch.path().to_string_lossy().into_owned();
            let run_fut = drive(
                self.agent.wrapper.as_ref(),
                &self.agent.profile,
                ctx.goal,
                &cwd,
                self.timeout,
                key,
                &bridge_env,
                Some(&tx),
            );
            // RACE the run against the no-progress watchdog. The watchdog waits `no_progress`; each
            // streamed event (`progress.notified()`) restarts the wait. If it elapses with no event,
            // the run is STUCK — we drop `run_fut` (reaping the subprocess, same as the wall ceiling)
            // and return a distinct stall error so the caller can tell "stuck" from "slow"/"denied".
            let quiet = self.no_progress;
            let events = {
                tokio::pin!(run_fut);
                loop {
                    let watchdog = async {
                        loop {
                            match tokio::time::timeout(quiet, progress.notified()).await {
                                // An event arrived within the quiet window → re-arm from now.
                                Ok(()) => continue,
                                // Silence past the ceiling → stuck.
                                Err(_) => return,
                            }
                        }
                    };
                    tokio::select! {
                        r = &mut run_fut => break r
                            .map_err(|e| AgentError::BadInput(
                                format!("external agent run failed: {e}"),
                            ))?,
                        () = watchdog => {
                            // Dropping `run_fut` on the next scope exit reaps the child. Instead of
                            // FAILING the run (a dead end), PAUSE it — `suspend` leaves the job
                            // `Suspended`/resumable from its cursor — and return the distinct
                            // `Stalled` error so the worker posts an actionable "keep going / stop"
                            // prompt. A user "keep going" is the shipped `resume_run` (re-enqueue +
                            // rehydrate from the cursor); "stop" is `stop_run` (cancel). The subprocess
                            // is reaped either way (drop); a resume re-spawns a fresh one from the goal.
                            let _ = lb_jobs::suspend(&node.store, ctx.ws, ctx.job_id).await;
                            return Err(AgentError::Stalled);
                        }
                    }
                }
            };
            // Close the sink and let the publisher drain every streamed event before we return.
            drop(tx);
            let _ = publisher.await;

            // The answer + fail-closed terminal outcome. `terminal_outcome` maps the projected finish
            // (an unrecognised agent status → `Failed`, never `Done` — the untrusted-agent rule the
            // leaf's `outcome_of` already enforces). We surface a failed run as an error at the seam;
            // #5 will make the *job/exit* authoritative over this hint.
            let answer = final_answer(&events);
            match run_outcome(&events) {
                Some(RunOutcome::Failed) => {
                    // On failure the useful text is the terminal `RunFinish.answer` (e.g. a provider
                    // "429 Too Many Requests" or a tool error) — NOT the assistant `TextDelta`s, which
                    // are usually empty on a failed turn. Fall back to it so the channel `agent_error`
                    // carries a real reason instead of an empty string.
                    let reason = finish_message(&events).unwrap_or(answer);
                    let _ = complete(&node.store, ctx.ws, ctx.job_id, JobStatus::Failed).await;
                    Err(AgentError::BadInput(format!(
                        "external agent run ended failed: {reason}"
                    )))
                }
                _ => {
                    let _ = complete(&node.store, ctx.ws, ctx.job_id, JobStatus::Done).await;
                    Ok(answer)
                }
            }
        })
    }
}

/// The serialized job payload for an external-agent run. Stored in the durable `job:{id}` record
/// so the gateway can read it on each bridged call (the Ask floor) and the run-status gate can
/// consult the status. Minimal: the goal (for audit) + the persona's Ask tool list (for the
/// gateway's bridged-call gate). Kept as a plain struct so the gateway can deserialize it without
/// a dep on this role crate.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RunPayload {
    pub goal: String,
    pub ask: Vec<String>,
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

/// The message carried by the terminal `RunFinish` (the failure reason on a failed run), if any and
/// non-empty. Used to give a failed run an honest error string instead of the empty `TextDelta` join.
fn finish_message(events: &[RunEvent]) -> Option<String> {
    events.iter().rev().find_map(|e| match e {
        RunEvent::RunFinish { answer, .. } if !answer.trim().is_empty() => Some(answer.clone()),
        _ => None,
    })
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
pub fn register(
    registry: &mut RuntimeRegistry,
    model: ModelEndpoint,
    scratch_base: Option<PathBuf>,
) {
    let base = scratch_base.unwrap_or_else(default_scratch_base);
    // Resolve the MCP shim binary the wrappers' config points the agent at. Default `"lb-mcp-shim"`
    // assumes it's on `PATH` — which is FALSE for a `make dev`/`cargo run` node (the shim is a sibling
    // binary in the target dir, not installed). If we leave the bare name, the agent's MCP-server child
    // fails to spawn, the agent gets NO host tools, and falls back to flailing in its own shell (the
    // real incident behind the no-progress reap). So prefer a shim sitting NEXT TO the node binary
    // (`<current_exe_dir>/lb-mcp-shim`), which covers both the dev target dir and an installed layout;
    // fall back to the PATH name only if we can't locate the node exe or the sibling is absent.
    let shim_bin = resolve_shim_bin();
    for id in BUILTIN_IDS {
        if let Some(rt) = AcpRuntime::new(id, model.clone(), base.clone()) {
            let rt = match &shim_bin {
                Some(path) => rt.with_shim_bin(path.clone()),
                None => rt,
            };
            registry.register(Arc::new(rt));
        }
    }
}

/// Locate the `lb-mcp-shim` binary sitting beside the running node binary. Returns the absolute path
/// when a sibling `lb-mcp-shim[.exe]` exists (dev target dir OR an installed `bin/` layout), else
/// `None` (fall back to the bare `PATH` name). Pure filesystem probe — no spawn.
fn resolve_shim_bin() -> Option<String> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    let name = if cfg!(windows) {
        "lb-mcp-shim.exe"
    } else {
        "lb-mcp-shim"
    };
    let sibling = dir.join(name);
    sibling
        .exists()
        .then(|| sibling.to_string_lossy().into_owned())
}
