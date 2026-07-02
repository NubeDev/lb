//! `HostCtx` — the CE sidecar's one handle to the host: the `lb-sidecar-client` callback transport
//! plus the sidecar's own parsed grant (its `LB_EXT_TOKEN` caps + workspace). Two jobs, both
//! host-mediated (the ROS-sidecar idiom, reused verbatim — control-engine scope §"caps"):
//!
//! 1. **Per-verb capability self-check** (`require`). The inbound path to a native sidecar tool is the
//!    host's `native.call` verb, gated only by `mcp:native.call:call` — the control line carries NO
//!    caller principal/caps. So the finer `mcp:control-engine.appliance.add:call` etc. granularity the
//!    scope demands is enforced HERE, by the handler, against the sidecar's own grant (`requested ∩
//!    admin_approved`, a node-signed JWT the host already verified to mint). Read with
//!    `claims_unverified` (no key needed — the host verified it at inject; the child cannot forge a
//!    grant it was not given).
//!
//! 2. **Host callbacks** (`client`). The `ce_appliance` registry CRUD reaches the store through the one
//!    `SidecarClient`: `store.write` (add), `store.query` (list/resolve), `store.delete` (remove) —
//!    each denied identically by the host's capability + workspace gate (a `403` → `CallError::Denied`).
//!
//! **Workspace isolation is structural, not a per-call arg.** The sidecar is spawned per-(ws,ext_id)
//! with a fixed `LB_EXT_WS`; every callback authenticates with the ws-scoped token, so a ws-A sidecar
//! can never read/write ws-B — the wall is the token, not anything a handler passes (§7).
//!
//! **Local-vs-remote is the HOST's job, not this sidecar's (symmetric nodes, §3.1).** The sidecar
//! never opens a Zenoh session and never decides "route elsewhere": any `control-engine.*` call that
//! REACHES it is for an appliance this node owns, because the host router forwarded it to the owning
//! node (the existing routed-MCP hop, by ext id). So `resolve.rs` reads the `ce_appliance` record only
//! to recover the CE **base** to connect to locally; a record absent in this workspace is a clean
//! not-found (the isolation wall). The record's `node` field is recorded for the registry listing +
//! a future discovery layer that populates the remote-routing entry — the sidecar does not branch on it.

use lb_sidecar_client::{CallError, SidecarClient};

/// The handler-facing error: a capability denial (opaque, like the host's gate) or a host/transport
/// failure surfaced from a callback. Never carries the token.
#[derive(Debug, thiserror::Error)]
pub enum HostError {
    #[error("denied")]
    Denied,
    #[error("not found")]
    NotFound,
    #[error("bad input: {0}")]
    BadInput(String),
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

/// The CE sidecar's host handle: the callback client + its own granted caps + workspace.
#[derive(Clone)]
pub struct HostCtx {
    client: SidecarClient,
    caps: Vec<String>,
    ws: String,
}

impl HostCtx {
    /// Build from the injected env: the callback client (`SidecarClient::from_env`) + the grant parsed
    /// from `LB_EXT_TOKEN`. A missing token/gateway is a callback error (the sidecar can serve nothing
    /// durable without the host, but the loop stays up to report it).
    pub fn from_env() -> Result<Self, HostError> {
        let client = SidecarClient::from_env()?;
        let token = std::env::var(lb_sidecar_client::TOKEN_ENV).unwrap_or_default();
        let (caps, ws) = match lb_auth::claims_unverified(&token) {
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

    /// The sidecar's workspace (diagnostics + record key scoping). The wire workspace on a callback is
    /// the token's, never this — this is the same value, kept for the handler's own record keys.
    pub fn ws(&self) -> &str {
        &self.ws
    }

    /// The callback client, for the `ce_appliance` registry `store.*` calls.
    pub fn client(&self) -> &SidecarClient {
        &self.client
    }

    /// Enforce that the sidecar's grant includes `mcp:<verb>:call` — the per-verb gate the host's
    /// coarse `mcp:native.call:call` cannot express. `Err(Denied)` is opaque (no "which cap" oracle),
    /// exactly like the host gate. This runs FIRST in every handler, before any callback or CE call.
    pub fn require(&self, verb: &str) -> Result<(), HostError> {
        let needed = format!("mcp:{verb}:call");
        if self.caps.iter().any(|c| c == &needed) {
            Ok(())
        } else {
            Err(HostError::Denied)
        }
    }
}
