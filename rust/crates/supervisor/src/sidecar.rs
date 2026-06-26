//! The live **`Sidecar`** handle — one supervised child, the heart of the native tier (native-tier
//! scope). It owns the child's control channel and drives the lifecycle: handshake on spawn, a
//! correlated `call` request/reply, a `health` poll, cooperative `shutdown`, and `restart` (kill +
//! re-launch from the spec, bounded by the backoff policy).
//!
//! It holds **no durable state** — the spec is a recipe re-read on every (re)spawn, and the only
//! mutable field is the runtime channel + a restart counter the host projects into a record. So a
//! restart re-derives everything from the spec: the stateless-extension guarantee applied to a
//! process (native-tier scope). The control line is request/reply (low-rate), so a `call` writes one
//! framed request and reads framed replies until the matching `id` — no background reader to race.

use crate::error::SupervisorError;
use crate::frame::{read_frame, write_frame};
use crate::launcher::{Channel, Launcher};
use crate::rpc::{CallParams, Method, Reply, Request};
use crate::spec::{RestartPolicy, Spec};

/// A supervised child: its spec, its live control channel, and how many times it has been restarted.
/// One per `(workspace, ext_id)`; the host keeps these in a runtime map (never in the store — the
/// PID is motion, the record is the truth).
pub struct Sidecar {
    spec: Spec,
    channel: Option<Channel>,
    next_id: u64,
    restarts: u32,
}

impl Sidecar {
    /// Spawn `spec`'s child via `launcher` and perform the `init` handshake. The returned sidecar is
    /// live and ready for `call`. Identity env (`LB_EXT_*`) must already be merged into `spec.env`
    /// by the host before this is called.
    pub async fn spawn<L: Launcher>(spec: Spec, launcher: &L) -> Result<Self, SupervisorError> {
        let channel = launcher.launch(&spec.exec, &spec.args, &spec.env).await?;
        let mut sidecar = Self {
            spec,
            channel: Some(channel),
            next_id: 0,
            restarts: 0,
        };
        sidecar.request(Method::Init, String::new()).await?;
        Ok(sidecar)
    }

    /// How many times this sidecar has been restarted (the host projects this into `native_status`).
    pub fn restarts(&self) -> u32 {
        self.restarts
    }

    /// Dispatch `tool` with opaque-JSON `input` to the child; return its JSON result string. A
    /// transport failure (the child died mid-call) surfaces as [`SupervisorError::Transport`] — the
    /// host decides whether to restart-and-retry per policy.
    pub async fn call(&mut self, tool: &str, input: &str) -> Result<String, SupervisorError> {
        let params = serde_json::to_string(&CallParams {
            tool: tool.to_string(),
            input: input.to_string(),
        })
        .map_err(|e| SupervisorError::Transport(e.to_string()))?;
        self.request(Method::Call, params).await
    }

    /// Send a `health` request; `Ok(())` if the child replied in time. A transport/timeout error
    /// means the child is unhealthy (the host triggers the restart policy).
    pub async fn health(&mut self) -> Result<(), SupervisorError> {
        self.request(Method::Health, String::new())
            .await
            .map(|_| ())
    }

    /// Cooperatively stop the child: send `shutdown`, then kill the process group (the launcher's
    /// kill awaits exit). Best-effort on the notification — a child that ignores it is killed after
    /// the supervisor stops waiting (the host owns the grace window). After this the sidecar is dead.
    pub async fn shutdown(&mut self) {
        // Notify; ignore the result — a dead/uncooperative child still gets killed below.
        let _ = self.request(Method::Shutdown, String::new()).await;
        if let Some(channel) = self.channel.take() {
            channel.kill.kill().await;
        }
    }

    /// Restart the child: kill the current one (awaiting its exit so there is no overlap), then
    /// re-launch from the **same spec** and re-handshake. Bounded by `backoff.max_restarts` — past
    /// that, [`SupervisorError::RestartExhausted`] and the sidecar is left dead (no infinite loop).
    /// The caller (host) applies the backoff delay; this verb does the kill+relaunch.
    pub async fn restart<L: Launcher>(&mut self, launcher: &L) -> Result<(), SupervisorError> {
        if self.spec.restart == RestartPolicy::Never {
            return Err(SupervisorError::RestartExhausted(self.restarts));
        }
        if self.restarts >= self.spec.backoff.max_restarts {
            return Err(SupervisorError::RestartExhausted(self.restarts));
        }
        // Kill the predecessor first and await its exit — a respawn must not race a living child.
        if let Some(channel) = self.channel.take() {
            channel.kill.kill().await;
        }
        let channel = launcher
            .launch(&self.spec.exec, &self.spec.args, &self.spec.env)
            .await?;
        self.channel = Some(channel);
        self.restarts += 1;
        self.next_id = 0;
        self.request(Method::Init, String::new()).await?;
        Ok(())
    }

    /// The delay the host should wait before the next restart (exponential backoff on the count).
    pub fn next_backoff(&self) -> std::time::Duration {
        self.spec.backoff.delay_for(self.restarts + 1)
    }

    /// Write one framed request and read framed replies until the one whose `id` matches. Any I/O
    /// failure is a transport error (the child is treated as dead). A child `error` reply maps to
    /// [`SupervisorError::Child`].
    async fn request(&mut self, method: Method, params: String) -> Result<String, SupervisorError> {
        let channel = self
            .channel
            .as_mut()
            .ok_or_else(|| SupervisorError::Transport("sidecar is not running".into()))?;
        let id = self.next_id;
        self.next_id += 1;

        let req = Request { id, method, params };
        let bytes =
            serde_json::to_vec(&req).map_err(|e| SupervisorError::Transport(e.to_string()))?;
        write_frame(&mut channel.write, &bytes).await?;

        loop {
            let body = read_frame(&mut channel.read).await?;
            let reply: Reply = serde_json::from_slice(&body)
                .map_err(|e| SupervisorError::Transport(format!("bad reply json: {e}")))?;
            if reply.id != id {
                continue; // a stale/out-of-order reply; keep reading for ours
            }
            if let Some(err) = reply.error {
                return Err(SupervisorError::Child(err));
            }
            return Ok(reply.result.unwrap_or_default());
        }
    }
}
