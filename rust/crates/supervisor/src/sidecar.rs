//! The live **`Sidecar`** handle — one supervised child, the heart of the native tier (native-tier
//! scope). It owns the child's control channel and drives the lifecycle: handshake on spawn, a
//! correlated `call` request/reply, a `health` poll, cooperative `shutdown`, and `restart` (kill +
//! re-launch from the spec, bounded by the backoff policy).
//!
//! It holds **no durable state** — the spec is a recipe re-read on every (re)spawn, and the only
//! mutable field is the runtime channel + a restart counter the host projects into a record. So a
//! restart re-derives everything from the spec: the stateless-extension guarantee applied to a
//! process (native-tier scope).
//!
//! **The control line is multiplexed** (native-call-concurrency scope). It used to be strictly
//! request/reply: `request` wrote a frame and read replies inline, so the caller's lock covered the
//! whole round-trip and concurrency to one child was 1. Now each channel generation is a [`Conn`]
//! (`conn.rs`) with a reader task + a pending-reply map, and **`call`/`health` take `&self`** — N
//! callers overlap on one line, correlated by `id`.
//!
//! `&self` on the call path is deliberate and load-bearing: a `&mut self` API forces every caller
//! through an exclusive lock, which is precisely the bug being removed. The lifecycle verbs
//! (`restart`/`rearm`/`shutdown`) keep `&mut self` — they replace the generation, so exclusivity is
//! the correct semantics there, and the host's existing `Arc<AsyncMutex<Sidecar>>` provides it.

use std::sync::Arc;

use crate::conn::Conn;
use crate::error::SupervisorError;
use crate::frame::{read_frame, write_frame};
use crate::launcher::{Channel, Launcher};
use crate::rpc::{CallParams, Caller, Method, Reply, Request};
use crate::spec::{RestartPolicy, Spec};

/// A supervised child: its spec, its live multiplexed connection, and how many times it has been
/// restarted. One per `(workspace, ext_id)`; the host keeps these in a runtime map (never in the
/// store — the PID is motion, the record is the truth).
pub struct Sidecar {
    spec: Spec,
    /// The current channel generation. `None` = dead (shut down, or restart-exhausted). Replacing
    /// this is what makes a generation boundary: the outgoing `Conn` is closed (waiters failed,
    /// reader stopped) before the new one takes a frame.
    conn: Option<Arc<Conn>>,
    restarts: u32,
    /// Monotonic id of the CURRENT channel generation, bumped on every replacement.
    ///
    /// Distinct from `restarts`, which is a *budget* that `rearm`/`reset_restarts` deliberately zero
    /// — a counter that goes backwards cannot identify a generation. The host reads this to make its
    /// retry generation-aware: a caller whose generation is still installed knows nothing recovered
    /// the child and fails fast instead of re-attempting against the same corpse (Risk 7 — serially
    /// that cost one wasted retry, multiplexed it costs N).
    generation: u64,
}

impl Sidecar {
    /// Spawn `spec`'s child via `launcher` and perform the `init` handshake. The returned sidecar is
    /// live and ready for `call`. Identity env (`LB_EXT_*`) must already be merged into `spec.env`
    /// by the host before this is called.
    pub async fn spawn<L: Launcher>(spec: Spec, launcher: &L) -> Result<Self, SupervisorError> {
        let channel = launcher.launch(&spec.exec, &spec.args, &spec.env).await?;
        let conn = handshake(channel).await?;
        Ok(Self {
            spec,
            conn: Some(Arc::new(conn)),
            restarts: 0,
            generation: 0,
        })
    }

    /// How many times this sidecar has been restarted (the host projects this into `native_status`).
    pub fn restarts(&self) -> u32 {
        self.restarts
    }

    /// The id of the currently-installed channel generation (see the field docs). Monotonic: it
    /// never resets, so it identifies a generation even across a `rearm` that zeroes the budget.
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Dispatch `tool` with opaque-JSON `input` to the child; return its JSON result string. A
    /// transport failure (the child died mid-call) surfaces as [`SupervisorError::Transport`] — the
    /// host decides whether to restart-and-retry per policy.
    ///
    /// Carries **no caller** — the frame's `caller` is `None` (an old-host-shaped call). Use
    /// [`call_with_caller`](Self::call_with_caller) to stamp the authorized principal into the frame
    /// so the child can enforce per-caller row visibility (native-caller-identity scope).
    pub async fn call(&self, tool: &str, input: &str) -> Result<String, SupervisorError> {
        self.call_with_caller(tool, input, None).await
    }

    /// Dispatch `tool` like [`call`](Self::call), but additionally stamp `caller` — a minimal,
    /// non-replayable projection of the principal the host already authorized — into the frame
    /// (native-caller-identity scope). The child receives it as `CallParams.caller` and may attribute
    /// its row-filter decision to the real caller. `None` is byte-for-byte the old `call` frame.
    ///
    /// `&self`: concurrent calls to one child overlap (native-call-concurrency scope). Each carries
    /// its OWN `caller` on its OWN frame — identity is per-call, never shared connection state, so
    /// multiplexing cannot leak one caller's identity onto another's dispatch.
    pub async fn call_with_caller(
        &self,
        tool: &str,
        input: &str,
        caller: Option<Caller>,
    ) -> Result<String, SupervisorError> {
        let params = serde_json::to_string(&CallParams {
            tool: tool.to_string(),
            input: input.to_string(),
            caller,
        })
        .map_err(|e| SupervisorError::Transport(e.to_string()))?;
        self.request(Method::Call, params).await
    }

    /// Send a `health` request; `Ok(())` if the child replied in time. A transport/timeout error
    /// means the child is unhealthy (the host triggers the restart policy). `&self` — a health poll
    /// no longer queues behind in-flight tool calls.
    pub async fn health(&self) -> Result<(), SupervisorError> {
        self.request(Method::Health, String::new())
            .await
            .map(|_| ())
    }

    /// Cooperatively stop the child: send `shutdown`, then close the generation (which stops the
    /// reader, fails any remaining waiters, and kills the process group, awaiting its exit).
    ///
    /// The `shutdown` reply is read through the reader task like any other correlated request — the
    /// reader owns the read half, so this verb cannot read its own reply off the wire. Best-effort:
    /// a dead/uncooperative child still gets killed below. After this the sidecar is dead, and any
    /// caller still waiting is failed rather than orphaned.
    pub async fn shutdown(&mut self) {
        if let Some(conn) = self.conn.as_ref() {
            let _ = conn.request(Method::Shutdown, String::new()).await;
        }
        if let Some(conn) = self.conn.take() {
            conn.close().await;
        }
    }

    /// Restart the child: close the current generation (killing it and failing its waiters), then
    /// re-launch from the **same spec** and re-handshake. Bounded by `backoff.max_restarts` — past
    /// that, [`SupervisorError::RestartExhausted`] and the sidecar is left dead (no infinite loop).
    /// The caller (host) applies the backoff delay; this verb does the kill+relaunch.
    ///
    /// **The generation boundary is the correctness-critical part.** The outgoing `Conn` is closed
    /// FIRST — its pending map sealed and every waiter failed — before the new channel exists. The
    /// fresh generation starts its ids at 0 again, but its map is a *different map*, so a
    /// post-restart id 3 can never satisfy a pre-restart waiter on id 3.
    pub async fn restart<L: Launcher>(&mut self, launcher: &L) -> Result<(), SupervisorError> {
        if self.spec.restart == RestartPolicy::Never {
            return Err(SupervisorError::RestartExhausted(self.restarts));
        }
        if self.restarts >= self.spec.backoff.max_restarts {
            return Err(SupervisorError::RestartExhausted(self.restarts));
        }
        self.replace_generation(launcher).await?;
        self.restarts += 1;
        Ok(())
    }

    /// The delay the host should wait before the next restart (exponential backoff on the count).
    pub fn next_backoff(&self) -> std::time::Duration {
        self.spec.backoff.delay_for(self.restarts + 1)
    }

    /// The cool-off window from the spec — how long a sidecar must serve calls cleanly before its
    /// restart count may decay (the host owns the clock and calls [`reset_restarts`] when it elapses).
    pub fn cooloff(&self) -> std::time::Duration {
        self.spec.backoff.cooloff
    }

    /// **Re-arm** the restart budget and force a fresh child, IGNORING the budget (native-tier
    /// resilience: an operator `reset` recovers a sidecar that already crash-looped past
    /// `max_restarts`). Unlike [`restart`], this does not consult `max_restarts` and works even when
    /// the sidecar is currently dead (`conn == None`, the exhausted state) — it kills any live
    /// predecessor, relaunches from the SAME spec, re-handshakes, and zeroes the counter. The
    /// stateless-extension guarantee makes this safe: the child held no durable state, so a forced
    /// respawn loses nothing. `RestartPolicy::Never` still refuses (a one-shot is never re-armed).
    pub async fn rearm<L: Launcher>(&mut self, launcher: &L) -> Result<(), SupervisorError> {
        if self.spec.restart == RestartPolicy::Never {
            return Err(SupervisorError::RestartExhausted(self.restarts));
        }
        self.replace_generation(launcher).await?;
        self.restarts = 0;
        Ok(())
    }

    /// Zero the in-memory restart counter WITHOUT touching the child — the decay path. The host calls
    /// this once a running sidecar has served calls cleanly for the [`cooloff`] window, so a
    /// subsequent fault gets the full budget again (a transient crash no longer permanently poisons
    /// it). No respawn: the child is already healthy; only the accounting is reset.
    pub fn reset_restarts(&mut self) {
        self.restarts = 0;
    }

    /// Close the live generation and install a freshly launched, handshaken one. The single place a
    /// generation boundary is crossed, shared by `restart` and `rearm` so the ordering (close-then-
    /// launch, waiters failed before the new channel exists) cannot drift between them.
    async fn replace_generation<L: Launcher>(
        &mut self,
        launcher: &L,
    ) -> Result<(), SupervisorError> {
        if let Some(conn) = self.conn.take() {
            conn.close().await;
        }
        let channel = launcher
            .launch(&self.spec.exec, &self.spec.args, &self.spec.env)
            .await?;
        self.conn = Some(Arc::new(handshake(channel).await?));
        self.generation += 1;
        Ok(())
    }

    /// Route one correlated request over the live generation.
    async fn request(&self, method: Method, params: String) -> Result<String, SupervisorError> {
        self.conn()?.request(method, params).await
    }

    /// The live generation as a **detached handle**.
    ///
    /// This is what lets the host stop serializing. The host holds one
    /// `Arc<AsyncMutex<Sidecar>>` per `(ws, ext_id)`; if a caller had to keep that guard for the
    /// duration of its round-trip, the mutex alone would re-impose concurrency 1 no matter how well
    /// `Conn` multiplexes. Cloning the `Arc<Conn>` out under a **short** lock and dropping the guard
    /// before awaiting is the seam: the lock now covers a map lookup, not a remote query.
    ///
    /// The returned handle stays valid for its own generation even if the sidecar is concurrently
    /// restarted — the new generation installs a *different* `Conn`, and this one is closed, so an
    /// in-flight caller is failed with a transport error rather than silently answered by the new
    /// child (the id-collision hazard).
    pub fn conn(&self) -> Result<Arc<Conn>, SupervisorError> {
        self.conn
            .clone()
            .ok_or_else(|| SupervisorError::Transport("sidecar is not running".into()))
    }
}

/// Run the `init` handshake **synchronously on the raw channel**, then start the reader task.
///
/// This resolves the bootstrap ordering the scope flags: `init` is the one request that exists before
/// there is a connection to register a waiter in. Doing it raw — write one frame, read one frame,
/// then hand the channel to [`Conn::start`] — keeps the handshake trivially correct and means the
/// reader task never races the frame that proves the child is alive. Every subsequent request goes
/// through the multiplexer.
async fn handshake(mut channel: Channel) -> Result<Conn, SupervisorError> {
    let req = Request {
        id: 0,
        method: Method::Init,
        params: String::new(),
    };
    let bytes = serde_json::to_vec(&req).map_err(|e| SupervisorError::Transport(e.to_string()))?;
    write_frame(&mut channel.write, &bytes).await?;

    // Read until the handshake's own id (a child that emits anything before its init reply is
    // tolerated, exactly as the old inline loop was).
    loop {
        let body = read_frame(&mut channel.read).await?;
        let reply: Reply = serde_json::from_slice(&body)
            .map_err(|e| SupervisorError::Transport(format!("bad reply json: {e}")))?;
        if reply.id != 0 {
            continue;
        }
        if let Some(err) = reply.error {
            return Err(SupervisorError::Child(err));
        }
        break;
    }
    Ok(Conn::start(channel))
}
