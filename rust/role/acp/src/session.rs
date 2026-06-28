//! The ACP **session driver** (agent-run scope Part 4) — translates the ACP v1 turn lifecycle onto
//! the host's run primitives (Part 0–3), under a trusted-session principal bound to ONE workspace.
//! It is the encoder's brain: it owns no kernel logic, only the mapping
//!   - `initialize`            → capability handshake.
//!   - `session/new`           → start a durable run (a job id); reject client `mcpServers`/`cwd`.
//!   - `session/prompt`        → drive a turn (start/resume — NOT a blocking final-answer call); the
//!                               `RunEvent` projection (Part 3 watch) is streamed back as
//!                               `session/update`s; the turn ends with a `StopReason`.
//!   - `session/request_permission` ← a `Suspended` event (Part 2) — issued mid-prompt; if the run
//!                               suspends, the prompt ends with the "suspended" stop reason and the
//!                               decision settles out-of-band.
//!   - `session/cancel`        → the Part-0 cancel hook.
//!   - `session/load`/`resume` ← rehydrate from the Part-0 transcript (the snapshot replay).
//!
//! **Authentication is the trusted-session path, never a bypass** (Part 4 / risk "authn for a local
//! stdio adapter"): the adapter is handed a real session token (minted by `lb_auth`), verifies it
//! with the node key, and binds the session to exactly the workspace in the token. A forged call is
//! as denied as any other.
//!
//! The driver is transport-agnostic: it returns response values + a list of `session/update`
//! notifications, and `stdio.rs` does the byte I/O. That split lets a test drive the driver in-process
//! against a real Node AND lets a test spawn the real binary over a real pipe (rule 9, no fakes).

use std::sync::Arc;

use lb_auth::{verify, Principal};
use lb_host::{cancel_run, invoke, resume, watch_run, AllowedTool, Invocation, Node};
use lb_run_events::{RunEvent, RunOutcome};
use serde_json::{json, Value};

use crate::encode::{encode_update, stop_reason};
use crate::rpc::{codes, Notification};

/// The outcome of handling one ACP method: the JSON-RPC `result` plus any `session/update`
/// notifications to push before the response (the streamed turn). The stdio loop writes the
/// notifications first, then the response — exactly the ACP ordering an editor expects.
pub struct Handled {
    pub result: Value,
    pub notifications: Vec<Notification>,
}

/// A driver error mapped to a JSON-RPC error code + message. Opaque where it must be (auth/deny).
#[derive(Debug)]
pub struct DriverError {
    pub code: i64,
    pub message: String,
}

impl DriverError {
    fn new(code: i64, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

type DriverResult = Result<Handled, DriverError>;

/// One authenticated ACP session, bound to a workspace by its token. Generic over the model access
/// `M` (the real `AiGateway` in the binary; the same trait in tests) so the driver build-depends only
/// on the host trait, never a concrete provider.
pub struct AcpSession<M: lb_host::ModelAccess> {
    node: Arc<Node>,
    model: Arc<M>,
    /// The verified principal — workspace + caps come from the token (the wall, §7).
    principal: Principal,
    /// The agent actor's own caps (intersected with the caller's per the no-widening rule).
    agent_caps: Vec<String>,
    /// The tools the model may propose this session (qualified MCP names).
    tools: Vec<AllowedTool>,
    /// A monotonic logical clock for `ts` (no wall-clock in core — testing §3). The caller seeds it;
    /// each turn bumps it so successive events order.
    clock: u64,
}

impl<M: lb_host::ModelAccess + Send + Sync + 'static> AcpSession<M> {
    /// Authenticate a session from a bearer `token`, verified with the node `key` at logical `now`.
    /// `Err` (opaque UNAUTHENTICATED) if the token is missing/forged/expired — the trusted-session
    /// wall. On success the session is bound to the token's workspace.
    #[allow(clippy::too_many_arguments)]
    pub fn authenticate(
        node: Arc<Node>,
        model: Arc<M>,
        key: &lb_auth::SigningKey,
        token: &str,
        now: u64,
        agent_caps: Vec<String>,
        tools: Vec<AllowedTool>,
    ) -> Result<Self, DriverError> {
        let principal = verify(key, token, now).map_err(|_| {
            DriverError::new(codes::UNAUTHENTICATED, "invalid or expired session token")
        })?;
        Ok(Self {
            node,
            model,
            principal,
            agent_caps,
            tools,
            clock: now,
        })
    }

    /// The workspace this session is bound to (from the token).
    pub fn ws(&self) -> &str {
        self.principal.ws()
    }

    /// Dispatch one ACP method by name. Unknown methods → METHOD_NOT_FOUND.
    pub async fn handle(&mut self, method: &str, params: &Value) -> DriverResult {
        match method {
            "initialize" => self.initialize(),
            "session/new" => self.session_new(params),
            "session/prompt" => self.session_prompt(params).await,
            "session/cancel" => self.session_cancel(params).await,
            "session/load" | "session/resume" => self.session_load(params).await,
            other => Err(DriverError::new(
                codes::METHOD_NOT_FOUND,
                format!("unknown method: {other}"),
            )),
        }
    }

    /// `initialize` — advertise the ACP protocol version + the adapter's capabilities. We expose only
    /// our already-known internal MCP tools; client-provided MCP servers are NOT supported (declared
    /// here so the editor knows up front, and enforced in `session/new`).
    fn initialize(&self) -> DriverResult {
        Ok(Handled {
            result: json!({
                "protocolVersion": 1,
                "agentCapabilities": {
                    "loadSession": true,
                    "promptCapabilities": { "image": false, "audio": false },
                    "mcpCapabilities": { "http": false, "sse": false }
                }
            }),
            notifications: Vec::new(),
        })
    }

    /// `session/new` — start a durable run. RESOLVED DECISION (Part 4): a client-provided
    /// `mcpServers`/`cwd` is **rejected cleanly** (not silently dropped) — bridging client-side tools
    /// needs a `net:*`-style grant that is a future scope. The session id IS the durable job id.
    fn session_new(&self, params: &Value) -> DriverResult {
        if params
            .get("mcpServers")
            .is_some_and(|v| !v.is_null() && v.as_array().is_none_or(|a| !a.is_empty()))
            || params.get("cwd").is_some_and(|v| !v.is_null())
        {
            return Err(DriverError::new(
                codes::UNSUPPORTED_CLIENT_SERVERS,
                "client-provided mcpServers/cwd are not supported in v1 (would require a net:* grant)",
            ));
        }
        // The client may suggest a session id; else we mint a deterministic one from the clock (no
        // wall-clock — the caller's logical now). The id is the durable job id the run lives on.
        let session_id = params
            .get("sessionId")
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .unwrap_or_else(|| format!("acp-{}", self.clock));
        Ok(Handled {
            result: json!({ "sessionId": session_id }),
            notifications: Vec::new(),
        })
    }

    /// `session/prompt` — drive ONE turn against the run. Subscribes to the run's `RunEvent` feed
    /// FIRST (Part 3 watch), drives the run (`invoke` for a fresh prompt — the start/resume-vs-watch
    /// split means we are NOT relying on the blocking return for the stream), drains the feed into
    /// `session/update`s, and ends with a `StopReason` derived from the terminal outcome. If the run
    /// suspends on an Ask (Part 2), the turn ends with the "suspended" stop reason — the editor's
    /// permission request is implicit in the streamed `Suspended` (a real editor would answer via a
    /// separate decision channel; the disconnect-mid-permission contract holds because the run is
    /// durably suspended regardless of the connection).
    async fn session_prompt(&mut self, params: &Value) -> DriverResult {
        let session_id = self.require_session_id(params)?;
        let prompt = extract_prompt_text(params);
        self.clock += 1;

        // Subscribe before driving so no early delta is missed (Part 3).
        let watch = watch_run(
            &self.node.store,
            &self.node.bus,
            &self.principal,
            self.ws(),
            &session_id,
        )
        .await
        .map_err(|_| DriverError::new(codes::DENIED, "watch denied"))?;

        // Drive the run concurrently with draining its event feed. `invoke` creates-or-continues the
        // durable job (idempotent on the id); a fresh session_id starts a run, an existing one with a
        // new prompt continues it (the job's goal is the first prompt; subsequent prompts are a S5
        // follow-up — v1 drives the run to its first terminal/suspend point per prompt).
        let node = self.node.clone();
        let model = self.model.clone();
        let principal = self.principal.clone();
        let agent_caps = self.agent_caps.clone();
        let tools = self.tools.clone();
        let ws = self.ws().to_string();
        let sid = session_id.clone();
        let goal = prompt.clone();
        let ts = self.clock;
        let driver = tokio::spawn(async move {
            invoke(
                &node,
                model.as_ref(),
                &principal,
                &agent_caps,
                &ws,
                Invocation {
                    job_id: &sid,
                    goal: &goal,
                    skill: None,
                    doc: None,
                    tools: &tools,
                    ts,
                },
            )
            .await
        });

        // Drain the live feed into ACP updates until a terminal RunFinish (or a bounded cap so a
        // dropped finish can't hang the turn).
        let (notifications, outcome) =
            drain_to_updates(&session_id, &watch.snapshot, &watch.stream).await;
        let _ = driver.await;

        Ok(Handled {
            result: json!({ "stopReason": stop_reason(outcome) }),
            notifications,
        })
    }

    /// `session/cancel` — the Part-0 durable cancel hook. Idempotent; leaves a terminal, restorable
    /// transcript. A notification, in ACP, but we accept it as a method and ack.
    async fn session_cancel(&self, params: &Value) -> DriverResult {
        let session_id = self.require_session_id(params)?;
        cancel_run(&self.node, self.ws(), &session_id)
            .await
            .map_err(|_| DriverError::new(codes::DENIED, "cancel denied"))?;
        Ok(Handled {
            result: json!({ "ok": true }),
            notifications: Vec::new(),
        })
    }

    /// `session/load` / `session/resume` — rehydrate from the durable transcript (Part 0) and replay
    /// its `RunEvent` projection as `session/update`s, restoring the editor's view. Then `resume`
    /// continues the run if it is resumable (e.g. a suspension was settled out-of-band). The replay +
    /// continue is the same start/resume-vs-watch split (Part 3): load is a watch, resume is a drive.
    async fn session_load(&mut self, params: &Value) -> DriverResult {
        let session_id = self.require_session_id(params)?;
        self.clock += 1;

        // Snapshot (the catch-up replay) + live feed.
        let watch = watch_run(
            &self.node.store,
            &self.node.bus,
            &self.principal,
            self.ws(),
            &session_id,
        )
        .await
        .map_err(|_| DriverError::new(codes::DENIED, "watch denied"))?;

        // Continue the run from the durable cursor (rehydrates Part 0; applies a settled suspension
        // Part 2). Run concurrently so live deltas stream while it resumes.
        let node = self.node.clone();
        let model = self.model.clone();
        let principal = self.principal.clone();
        let agent_caps = self.agent_caps.clone();
        let tools = self.tools.clone();
        let ws = self.ws().to_string();
        let sid = session_id.clone();
        let ts = self.clock;
        let driver = tokio::spawn(async move {
            resume(
                &node,
                model.as_ref(),
                &principal,
                &agent_caps,
                &ws,
                &sid,
                &tools,
                ts,
            )
            .await
        });

        let (notifications, outcome) =
            drain_to_updates(&session_id, &watch.snapshot, &watch.stream).await;
        let _ = driver.await;

        Ok(Handled {
            result: json!({ "stopReason": stop_reason(outcome), "loaded": true }),
            notifications,
        })
    }

    fn require_session_id(&self, params: &Value) -> Result<String, DriverError> {
        params
            .get("sessionId")
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .ok_or_else(|| DriverError::new(codes::INVALID_PARAMS, "missing sessionId"))
    }
}

/// Drain a run's snapshot + live feed into ACP `session/update` notifications, returning them plus the
/// run's terminal [`RunOutcome`] (defaulting to `Done` if the feed closes without an explicit finish —
/// a closed bus with no finish means the run ended; the durable transcript is the authority anyway).
async fn drain_to_updates(
    session_id: &str,
    snapshot: &[RunEvent],
    stream: &lb_host::RunEventSub,
) -> (Vec<Notification>, RunOutcome) {
    let mut notifications = Vec::new();
    let mut outcome = RunOutcome::Done;

    // Phase 1: the catch-up snapshot (events that already landed before we attached).
    for event in snapshot {
        if let RunEvent::RunFinish { outcome: o, .. } = event {
            outcome = *o;
        }
        if let Some(params) = encode_update(session_id, event) {
            notifications.push(Notification::new("session/update", params));
        }
    }

    // Phase 2: live deltas until a terminal finish (bounded so a missed finish can't hang the turn).
    for _ in 0..256 {
        match tokio::time::timeout(std::time::Duration::from_secs(10), stream.recv()).await {
            Ok(Some(event)) => {
                if let RunEvent::RunFinish { outcome: o, .. } = &event {
                    outcome = *o;
                    if let Some(params) = encode_update(session_id, &event) {
                        notifications.push(Notification::new("session/update", params));
                    }
                    break;
                }
                if let Some(params) = encode_update(session_id, &event) {
                    notifications.push(Notification::new("session/update", params));
                }
            }
            // Closed or timed out — the run ended (the transcript is the record). Stop draining.
            _ => break,
        }
    }
    (notifications, outcome)
}

/// Pull the prompt text out of an ACP `session/prompt` params. ACP carries a content-block array; we
/// concatenate the text blocks (image/audio are declared unsupported). A bare `prompt` string is also
/// accepted for ergonomics.
fn extract_prompt_text(params: &Value) -> String {
    if let Some(s) = params.get("prompt").and_then(|v| v.as_str()) {
        return s.to_string();
    }
    params
        .get("prompt")
        .and_then(|v| v.as_array())
        .map(|blocks| {
            blocks
                .iter()
                .filter_map(|b| b.get("text").and_then(|t| t.as_str()))
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default()
}
