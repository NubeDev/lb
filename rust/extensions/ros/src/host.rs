//! `HostCtx` — the sidecar's one handle to the host: the `lb-sidecar-client` callback transport plus
//! the sidecar's own parsed grant (its `LB_EXT_TOKEN` caps + workspace). Two jobs, both host-mediated:
//!
//! 1. **Per-verb capability self-check** (`require`). The inbound path to a native sidecar tool is the
//!    host's `native.call` verb, gated only by `mcp:native.call:call` — the control line carries NO
//!    caller principal/caps (native-tier scope: the control plane is tiny on purpose). So the finer
//!    `mcp:ros.list:call` / `mcp:point.write:call` granularity the scope demands is enforced HERE, by
//!    the handler, against the sidecar's own grant (`requested ∩ admin_approved`, a node-signed JWT the
//!    host already verified to mint). This is defense-in-depth on top of `mcp:native.call:call`, and it
//!    is exactly the deny path the mandatory tests drive: install `ros` without `mcp:point.write:call`
//!    → the handler refuses before any REST write leaves the node. Read with `claims_unverified` (no
//!    key needed — the host verified it at inject; the child cannot forge a grant it was not given).
//!
//! 2. **Host callbacks** (`client`). The config-shadow CRUD (`assets.*`), the poller (`ingest.write`),
//!    and `point.write` (`outbox.enqueue`) all reach the host through the one `SidecarClient` — each
//!    denied identically by the host's capability + workspace gate (a `403` → `CallError::Denied`).
//!
//! **Workspace isolation is structural, not a per-call arg.** The sidecar is spawned per-(ws,ext_id)
//! with a fixed `LB_EXT_WS`; every callback authenticates with the ws-scoped token, so a ws-A sidecar
//! can never read/write/poll ws-B — the wall is the token, not anything a handler passes (§7).

use lb_sidecar_client::{CallError, SidecarClient};

/// The handler-facing error: a capability denial (opaque, like the host's gate) or a host/transport
/// failure surfaced from a callback. Never carries the token.
#[derive(Debug, thiserror::Error)]
pub enum HostError {
    #[error("denied")]
    Denied,
    #[error("host callback failed: {0}")]
    Callback(String),
    #[error("bad host response: {0}")]
    BadResponse(String),
}

impl From<CallError> for HostError {
    fn from(e: CallError) -> Self {
        match e {
            CallError::Denied => HostError::Denied,
            other => HostError::Callback(other.to_string()),
        }
    }
}

/// The sidecar's host handle: the callback client + its own granted caps + workspace. Constructed once
/// at sidecar start (`from_env`) and shared across handlers; cheap to clone the caps vector rarely.
#[derive(Clone)]
pub struct HostCtx {
    client: SidecarClient,
    caps: Vec<String>,
    ws: String,
}

impl HostCtx {
    /// Build from the injected env: the callback client (`SidecarClient::from_env`) + the grant parsed
    /// from `LB_EXT_TOKEN`. A missing token/gateway is surfaced as a callback error (the sidecar can
    /// serve nothing useful without the host, but the loop stays up to report it).
    pub fn from_env() -> Result<Self, HostError> {
        let client = SidecarClient::from_env()?;
        let token = std::env::var(lb_sidecar_client::TOKEN_ENV).unwrap_or_default();
        let claims = lb_auth::claims_unverified(&token);
        let (caps, ws) = match claims {
            Some(c) => (c.caps, c.ws),
            None => (
                Vec::new(),
                std::env::var(lb_sidecar_client::WS_ENV).unwrap_or_default(),
            ),
        };
        Ok(Self { client, caps, ws })
    }

    /// Build from explicit parts (tests): a `SidecarClient` over a known gateway + an explicit grant.
    /// No process env — so a test never races the global env.
    pub fn with_parts(client: SidecarClient, caps: Vec<String>, ws: impl Into<String>) -> Self {
        Self {
            client,
            caps,
            ws: ws.into(),
        }
    }

    /// The sidecar's workspace (diagnostics + shadow id scoping). The wire workspace on a callback is
    /// the token's, never this — this is the same value, kept for the handler's own record keys.
    pub fn ws(&self) -> &str {
        &self.ws
    }

    /// The callback client, for `assets.*` / `ingest.write` / `outbox.enqueue`.
    pub fn client(&self) -> &SidecarClient {
        &self.client
    }

    /// Enforce that the sidecar's grant includes `mcp:<verb>:call` — the per-verb gate the host's
    /// coarse `mcp:native.call:call` cannot express. `Err(Denied)` is opaque (no "which cap" oracle),
    /// exactly like the host gate. This runs FIRST in every handler, before any REST call or callback.
    pub fn require(&self, verb: &str) -> Result<(), HostError> {
        let needed = format!("mcp:{verb}:call");
        if self.caps.iter().any(|c| c == &needed) {
            Ok(())
        } else {
            Err(HostError::Denied)
        }
    }
}
