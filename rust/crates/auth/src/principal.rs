//! The Principal — a verified identity resolved from a token, ready for `caps::check`.
//!
//! This is the post-verification view the rest of the host uses: who, which workspace, and
//! what capabilities. The raw JWT and signing never leave the `auth` crate.

use serde::{Deserialize, Serialize};

/// RBAC roles (README §6.6). Ordered most→least privileged is not encoded here on purpose —
/// the check path reads `caps`, not `role`; roles only gate what is minted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Role {
    SuperAdmin,
    WorkspaceAdmin,
    Member,
}

/// A verified actor. Construct it only via `auth::verify` (or [`Principal::derive`], which only
/// ever *narrows* an existing one) — there is no public raw constructor, so an unverified or
/// widened principal cannot exist by accident.
///
/// **Delegation (the agent's derived principal, agent + auth-caps scopes).** A delegated actor
/// (e.g. the central agent acting for a caller) carries its own `caps` PLUS an optional
/// `constraint` — the *caller's* caps. The check layer requires a request to match `caps` **and**,
/// when present, `constraint`. That is exact set intersection (`caller ∩ agent`) with no pattern
/// algebra: an actor can never do something *either* side forbids, so an agent can never widen its
/// access beyond what it was delegated. A non-delegated principal has `constraint == None` and is
/// checked against `caps` alone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Principal {
    sub: String,
    ws: String,
    role: Role,
    caps: Vec<String>,
    /// The delegation upper bound: when `Some`, the check layer ALSO requires a match here. `None`
    /// for an ordinary (non-delegated) principal. Set only by [`derive`](Principal::derive).
    constraint: Option<Vec<String>>,
}

impl Principal {
    /// Crate-internal: built by `verify` after the signature and expiry check pass. No constraint
    /// (an ordinary actor is bounded only by its own caps).
    pub(crate) fn new(sub: String, ws: String, role: Role, caps: Vec<String>) -> Self {
        Self {
            sub,
            ws,
            role,
            caps,
            constraint: None,
        }
    }

    /// Reconstruct a caller principal on a node that received a **routed** request carrying the
    /// caller's `sub`/`ws`/`caps` (agent scope: the hub runs the agent loop and must check each tool
    /// call under the caller's grant). This is the S5 **co-trust** path: the carried caps are NOT
    /// signed, so it is only sound because the edge and hub are co-trusted in-process and the
    /// workspace-scoped bus key already enforces isolation (a ws-B caller can only emit on
    /// `ws/B/...`). Signing the carried grant (token-on-the-bus) is the mcp-scope "serve-side
    /// authorization" open question — recorded, not built at S5. Named loudly so this trust
    /// assumption is impossible to use by accident.
    pub fn routed(sub: impl Into<String>, ws: impl Into<String>, caps: Vec<String>) -> Principal {
        Principal {
            sub: sub.into(),
            ws: ws.into(),
            role: Role::Member,
            caps,
            constraint: None,
        }
    }

    /// Build a verified principal for a **machine/API-key** actor (api-keys scope). A bearer key
    /// from an untrusted appliance is a different trust context from [`routed`]'s co-trust path: here
    /// the gateway has ALREADY verified the secret (constant-time HMAC) and resolved the key's caps
    /// server-side from the durable grant store, so the principal's inputs are trusted even though
    /// the caller is not. This dedicated constructor states that invariant loudly, rather than
    /// silently inheriting `routed`'s caveat whose justification (in-process co-trust) does not
    /// apply. `role` is `Member` in v1 (a key is never more privileged than a member); a future
    /// admin key could set it explicitly here. No `constraint` — a key is bounded only by its own
    /// resolved caps (it delegates nothing).
    pub fn for_key(sub: impl Into<String>, ws: impl Into<String>, caps: Vec<String>) -> Principal {
        Principal {
            sub: sub.into(),
            ws: ws.into(),
            role: Role::Member,
            caps,
            constraint: None,
        }
    }

    /// Derive a **narrower** principal that acts on behalf of `self` (the caller). The result has:
    /// a distinct `sub` (e.g. `agent:summarize`) so audit shows the agent acted; the **agent's own
    /// caps** as its caps; and `self`'s caps as the `constraint` — so the check layer enforces
    /// `agent ∩ caller`. The workspace is inherited from `self` and **cannot** be changed
    /// (delegation never crosses the hard wall, §3.6). This is the only way to mint a delegated
    /// actor, and it can only ever *narrow* — never widen — the effective access (agent scope, the
    /// auth-caps "grant delegation" resolution).
    pub fn derive(&self, sub: impl Into<String>, agent_caps: Vec<String>) -> Principal {
        Principal {
            sub: sub.into(),
            ws: self.ws.clone(), // inherited — delegation cannot cross workspaces
            role: Role::Member,  // a delegated actor is never more privileged than a member
            caps: agent_caps,
            // The delegation bound. When `self` is ALREADY a derived (on-behalf-of) principal — e.g. a
            // re-entrant host-callback chain `caller → ext-A → ext-B` (host-callback scope) — keep the
            // ORIGINAL caller's constraint, NOT `self.caps`. Otherwise a nested derive would re-bound
            // against the agent's own (possibly wider) caps and could widen across hops. Preserving the
            // root constraint guarantees the effective grant never exceeds the original caller at ANY
            // depth. For a first (non-delegated) derive, `self.constraint` is `None`, so the bound is
            // `self.caps` — the caller's own caps, as before.
            constraint: Some(self.constraint.clone().unwrap_or_else(|| self.caps.clone())),
        }
    }

    /// The global identity (`user:…` / `key:…`).
    pub fn sub(&self) -> &str {
        &self.sub
    }

    /// The workspace this principal is scoped to — the hard wall, checked first.
    pub fn ws(&self) -> &str {
        &self.ws
    }

    pub fn role(&self) -> Role {
        self.role
    }

    /// The held capability strings (auth-caps grammar). Read by `caps::check`.
    pub fn caps(&self) -> &[String] {
        &self.caps
    }

    /// The delegation upper bound, if this is a derived (on-behalf-of) principal. When `Some`, the
    /// check layer requires a request to match these caps too — enforcing `agent ∩ caller`. `None`
    /// for an ordinary principal (bounded only by `caps`). Read by `caps::check`.
    pub fn constraint(&self) -> Option<&[String]> {
        self.constraint.as_deref()
    }
}
