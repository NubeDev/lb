//! Regression: the **no-progress (stall) ceiling** reaps a STUCK external run — one that spawns but
//! then emits no `RunEvent` (no tool call, no text) — promptly, instead of letting it burn the outer
//! wall-time ceiling. This closes the gap `run-lifecycle-scope.md` flags: the in-house loop self-bounds
//! at `MAX_STEPS`, but an external subprocess was bounded ONLY by wall-time, so a flailing agent (the
//! real incident: an agent shelling `make dev` in a loop) hung for the full 15 minutes before failing.
//!
//! Deterministic + real (rule 9): a real subprocess (`sh -c 'sleep 30'`) is spawned via a scripted
//! wrapper whose `decode_line` never yields an event — so the stream is genuinely silent. With a tiny
//! `no_progress` ceiling the run is reaped in well under the wall/liveness timeout, and the seam returns
//! the distinct stall message (not a deny, not the wall-time message).

use std::time::{Duration, Instant};

use lb_external_agent::wrapper::{AgentWrapper, Decoded};
use lb_external_agent::{AgentProfile, ModelEndpoint};
use lb_host::{AgentRuntime, AllowedTool, Node, RunContext};
use lb_role_external_agent::profiles::ResolvedAgent;
use lb_role_external_agent::AcpRuntime;

/// A wrapper that runs `sh -c 'sleep 30'` and decodes NOTHING — a genuinely silent run.
struct SilentWrapper;

impl AgentWrapper for SilentWrapper {
    fn id(&self) -> &'static str {
        "silent"
    }
    fn command_args(&self, _p: &AgentProfile, _goal: &str, _ws: &str) -> Vec<String> {
        // Spawn, then sleep well past both the liveness timeout and the tiny no-progress ceiling,
        // producing no stdout lines at all.
        vec!["-c".into(), "sleep 30".into()]
    }
    fn decode_line(&self, _line: &str, _turn: u32) -> Decoded {
        Decoded::Ignore
    }
}

fn silent_profile() -> AgentProfile {
    AgentProfile {
        id: "silent".into(),
        binary: "sh".into(),
        model: ModelEndpoint {
            provider: "none".into(),
            model: "none".into(),
            api_key_env: "NONE".into(),
            base_url: None,
        },
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stalled_run_is_reaped_at_the_no_progress_ceiling() {
    let node = std::sync::Arc::new(Node::boot().await.expect("node boots"));
    let ws = "stall-ws";
    let caller = lb_auth::Principal::routed("user:stall", ws, vec!["mcp:agent.invoke:call".into()]);

    let resolved = ResolvedAgent {
        profile: silent_profile(),
        wrapper: Box::new(SilentWrapper),
    };
    // A 250ms stall ceiling — the run must be reaped far before `sleep 30` returns.
    let runtime = AcpRuntime::from_resolved("silent", resolved, std::env::temp_dir())
        .with_no_progress_ceiling(Duration::from_millis(250));

    let ctx = RunContext {
        ws,
        job_id: "stall-1",
        goal: "do nothing",
        caller: &caller,
        agent_caps: &["mcp:agent.invoke:call".to_string()],
        tools: &[] as &[AllowedTool],
        model_override: None,
        persona_catalog: None,
        persona_preset: None,
        ts: 1,
    };

    let started = Instant::now();
    let err = runtime
        .run(&node, ctx)
        .await
        .expect_err("a silent run is reaped, not answered");
    let elapsed = started.elapsed();

    // Reaped promptly — nowhere near `sleep 30`s or the 600s liveness timeout.
    assert!(
        elapsed < Duration::from_secs(5),
        "stall reaped promptly, took {elapsed:?}"
    );
    // PAUSE-AND-ASK: a stall returns the distinct `Stalled` error (not a deny, not a generic fault) so
    // the worker posts an actionable prompt instead of a dead error.
    assert!(
        matches!(err, lb_host::AgentError::Stalled),
        "stall returns AgentError::Stalled, got: {err:?}"
    );

    // The durable job is SUSPENDED (resumable), NOT Failed — a stalled run is paused so the user can
    // "keep going" (resume from the cursor) or "stop"; it is never a terminal failure.
    let job = lb_jobs::load(&node.store, ws, "stall-1")
        .await
        .expect("load")
        .expect("job exists");
    assert!(
        matches!(job.status, lb_jobs::JobStatus::Suspended),
        "stalled run job is Suspended (resumable), got {:?}",
        job.status
    );
}
