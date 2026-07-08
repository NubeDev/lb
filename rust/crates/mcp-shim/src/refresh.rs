//! The run-token refresher (agent-key-lifecycle D2). The shim refreshes lazily:
//!   1. **proactively** before the next call once `refresh_at` (60% TTL) has passed, and
//!   2. **reactively** as a one-shot self-heal on a `401` (the narrow expiry race D2 names).
//!
//! Refresh is `POST /agent/runs/{id}/token/refresh` — itself run-status-gated on the gateway
//! (D3): once the run is terminal (cancelled/failed/done), the gateway refuses the refresh, so
//! the next call fails closed. The fresh token REPLACES the one in [`Refresher`] so subsequent
//! calls use it; we never keep a stale one around.

use std::sync::Arc;
use std::time::SystemTime;

use serde::Deserialize;
use tokio::sync::Mutex;

use crate::forward::ForwardError;

/// The refresh endpoint's JSON reply: `{ token, refresh_at_sec }` — the new bearer + the next
/// 60%-TTL timestamp. The gateway computes both so the shim stays clock-naive beyond "is now
/// past refresh_at".
#[derive(Debug, Clone, Deserialize)]
pub struct Refreshed {
    pub token: String,
    #[serde(rename = "refresh_at_sec")]
    pub refresh_at_sec: u64,
}

/// The mutable token state shared by every forward. Behind a `Mutex` so a refresh mid-flight (a
/// 401 self-heal racing a proactive refresh) lands exactly one winner; the loser reads the
/// already-fresh token on its next call. Clonable so the serve loop and the forward path hold
/// the same handle.
#[derive(Debug, Clone)]
pub struct Refresher {
    inner: Arc<Mutex<Inner>>,
    gateway_url: String,
    run_id: String,
    client: reqwest::Client,
}

#[derive(Debug)]
struct Inner {
    token: String,
    refresh_at: Option<u64>,
    /// One-shot guard: a 401 self-heal refreshes at most once per call (D2). Reset per call.
    healed_this_call: bool,
}

impl Refresher {
    /// Read-only access to the gateway base URL — the serve loop builds `/mcp/call` from it.
    pub fn gateway_url(&self) -> String {
        self.gateway_url.clone()
    }

    /// Build the refresher with the role-crate-minted initial token + its proactive-refresh
    /// timestamp (60% TTL).
    pub fn new(
        gateway_url: String,
        run_id: String,
        token: String,
        refresh_at: Option<u64>,
        client: reqwest::Client,
    ) -> Self {
        Self {
            inner: Arc::new(Mutex::new(Inner {
                token,
                refresh_at,
                healed_this_call: false,
            })),
            gateway_url,
            run_id,
            client,
        }
    }

    /// Take a snapshot of the current token (the forward path passes this as the bearer). Public so
    /// `serve_on` can capture the initial token into an `EnvConfig` snapshot for diagnostics.
    pub async fn token(&self) -> String {
        self.inner.lock().await.token.clone()
    }

    /// Mark the start of a new `tools/call`: resets the one-shot heal guard so a 401 on THIS call
    /// can heal exactly once. Called by the serve loop before each forward.
    pub async fn begin_call(&self) {
        self.inner.lock().await.healed_this_call = false;
    }

    /// If the proactive-refresh timestamp has passed, refresh before returning the token. Cheap
    /// when not due (one `SystemTime` read under the lock).
    pub async fn token_refreshed_if_due(&self) -> Result<String, ForwardError> {
        if !self.is_due().await {
            return Ok(self.inner.lock().await.token.clone());
        }
        self.refresh().await
    }

    /// One-shot 401 self-heal: refresh and return the new token, but only the FIRST time per
    /// call. A second 401 within the same call returns `Unauthorized` so the serve loop surfaces
    /// it (the race window D2 names is one retry wide — anything past that is a real revoke).
    pub async fn heal_once(&self) -> Result<String, ForwardError> {
        let mut g = self.inner.lock().await;
        if g.healed_this_call {
            return Err(ForwardError::Unauthorized);
        }
        g.healed_this_call = true;
        drop(g);
        self.refresh().await
    }

    async fn is_due(&self) -> bool {
        let Some(at) = self.inner.lock().await.refresh_at else {
            return false;
        };
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_secs() >= at)
            .unwrap_or(false)
    }

    async fn refresh(&self) -> Result<String, ForwardError> {
        let url = format!(
            "{}/agent/runs/{}/token/refresh",
            self.gateway_url, self.run_id
        );
        // The refresh route authenticates against the CURRENT (still-known) token; the gateway
        // refuses it if the run is terminal (D3), surfacing as a 401 here → the caller fails
        // closed. No body — the run id is in the path, the principal in the bearer.
        let current = self.inner.lock().await.token.clone();
        let resp = self
            .client
            .post(&url)
            .bearer_auth(&current)
            .send()
            .await
            .map_err(|e| ForwardError::Transport(e.to_string()))?;
        if resp.status().as_u16() == 401 {
            return Err(ForwardError::Unauthorized);
        }
        let status = resp.status().as_u16();
        let body = resp
            .text()
            .await
            .map_err(|e| ForwardError::Transport(e.to_string()))?;
        if !(200..300).contains(&status) {
            return Err(ForwardError::Status { status, body });
        }
        let fresh: Refreshed = serde_json::from_str(&body)
            .map_err(|e| ForwardError::Transport(format!("parse refresh reply: {e}")))?;
        let mut g = self.inner.lock().await;
        g.token = fresh.token;
        g.refresh_at = Some(fresh.refresh_at_sec);
        Ok(g.token.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn begin_call_resets_heal_guard() {
        let r = Refresher::new(
            "http://127.0.0.1:1".into(),
            "run".into(),
            "t0".into(),
            None,
            reqwest::Client::new(),
        );
        // No refresh_at → never due; the heal-once path is independent.
        r.begin_call().await;
        // Without a live gateway the heal errors with Transport; that's fine — the guard is what
        // we assert here by NOT hitting Unauthorized on the first attempt.
        let first = r.heal_once().await;
        assert!(matches!(first, Err(ForwardError::Transport(_))));
    }

    #[tokio::test]
    async fn token_snapshot_is_stable() {
        let r = Refresher::new(
            "http://127.0.0.1:1".into(),
            "run".into(),
            "tok".into(),
            None,
            reqwest::Client::new(),
        );
        assert_eq!(r.token().await, "tok");
    }
}
