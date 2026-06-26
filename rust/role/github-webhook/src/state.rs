//! The webhook receiver's shared state: the in-process node it fronts, the principal each ingest
//! acts as, the workspace it writes into, and the shared HMAC secret GitHub signs deliveries with.
//!
//! The receiver IS a node (symmetric nodes, §3.1) that also exposes one inbound HTTP route so a
//! *GitHub webhook* can reach it. It adds no authority of its own: a verified delivery forwards to
//! `lb_host::ingest_via_bridge` under `principal`, so the SAME two capability gates
//! (`mcp:github-bridge.normalize:call`, then `mcp:workflow.ingest_issue:call`) guard a webhook as
//! guard every other caller (capability-first, §3.5). The signature check is a *transport*
//! authenticity gate (is this really GitHub?), layered BEFORE — never instead of — the cap gates.
//!
//! The secret is a mediated value, not a logged one: it lives here behind `WebhookState`, is read
//! only by the constant-time HMAC check, and never appears in a log line or an error body (a bad
//! signature returns a bare `401`, no detail). A real verified session + a `lb-secrets`-backed
//! secret land later; kept in one place so routes stay thin.

use std::sync::Arc;

use lb_auth::Principal;
use lb_host::Node;

/// The live node + the ingest principal + the workspace + the webhook secret, shared across
/// handlers (`Arc` so axum can clone it into each request). One receiver serves one `(ws,
/// principal, secret)` — a multi-tenant front-door that routes by repo to a workspace is a
/// follow-up; today the wall is enforced by the fixed `ws` every delivery writes into.
#[derive(Clone)]
pub struct WebhookState {
    pub node: Arc<Node>,
    pub principal: Arc<Principal>,
    pub ws: String,
    secret: Arc<Vec<u8>>,
}

impl WebhookState {
    /// Build a receiver around an existing node + the principal it ingests as, the workspace it
    /// writes into, and the GitHub webhook `secret` (the shared HMAC key — the bytes from the repo's
    /// webhook config). The `github-bridge` extension must already be installed in `ws`.
    pub fn new(
        node: Node,
        principal: Principal,
        ws: impl Into<String>,
        secret: impl Into<Vec<u8>>,
    ) -> Self {
        Self::from_shared(Arc::new(node), principal, ws, secret)
    }

    /// Build a receiver around a SHARED node (`Arc<Node>`) — e.g. two receivers fronting one node
    /// for two workspaces, proving the workspace wall holds at the front door (each ingest lands in
    /// its own `ws`, never the other's).
    pub fn from_shared(
        node: Arc<Node>,
        principal: Principal,
        ws: impl Into<String>,
        secret: impl Into<Vec<u8>>,
    ) -> Self {
        Self {
            node,
            principal: Arc::new(principal),
            ws: ws.into(),
            secret: Arc::new(secret.into()),
        }
    }

    /// The shared HMAC secret — read only by the signature check ([`crate::verify`]). Crate-private
    /// on purpose: nothing outside the verifier should touch it, and it must never reach a log line.
    pub(crate) fn secret(&self) -> &[u8] {
        &self.secret
    }
}
