//! The child wire protocol — the request/response shapes the supervisor and a sidecar speak over the
//! framed line (native-tier scope, re-authored from rubix-cube's child contract). A small, closed
//! method set: `init` (handshake), `health` (liveness poll), `call` (dispatch a tool), `shutdown`
//! (cooperative drain). JSON over `Content-Length` framing (see `frame`).
//!
//! Deliberately minimal: this is the *control* line, not a data firehose. A sidecar that needs host
//! capabilities calls back through the routed MCP namespace with its injected scoped token, not this
//! line (native-tier scope non-goal). Keeping the protocol tiny keeps the security surface tiny.

use serde::{Deserialize, Serialize};

/// A request from the supervisor to the child. `id` correlates the reply; `method` is the verb.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Request {
    pub id: u64,
    pub method: Method,
    /// Method-specific arguments as a raw JSON string (the same opaque-JSON ABI the wasm tier uses,
    /// mcp scope — richer schemas stay host-side). Empty for `init`/`health`/`shutdown`.
    #[serde(default)]
    pub params: String,
}

/// The closed set of control methods. A new method is a deliberate protocol change, like a new
/// capability surface — not an ad-hoc string.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Method {
    /// Handshake: the child reports it is ready. Sent once, right after spawn.
    Init,
    /// Liveness poll: the child must reply `ok` within the health window or be treated as dead.
    Health,
    /// Dispatch a tool: `params` carries `{ "tool": "<name>", "input": "<json>" }`.
    Call,
    /// Cooperative shutdown: the child should drain and exit; escalated to a kill after the grace.
    Shutdown,
}

/// A reply from the child, correlated by `id`. Exactly one of `result`/`error` is set.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Reply {
    pub id: u64,
    /// The success payload (a raw JSON string), present when the call succeeded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<String>,
    /// The error message, present when the call failed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl Reply {
    pub fn ok(id: u64, result: impl Into<String>) -> Self {
        Self {
            id,
            result: Some(result.into()),
            error: None,
        }
    }
    pub fn err(id: u64, error: impl Into<String>) -> Self {
        Self {
            id,
            result: None,
            error: Some(error.into()),
        }
    }
}

/// The `params` shape for a [`Method::Call`]: which tool, its opaque-JSON input, and — additively —
/// **who** the host already authorized for this call ([`Caller`]).
///
/// `caller` is **additive-by-absence** (native-caller-identity scope): an old host omits it
/// (`skip_serializing_if`), an old child ignores an unknown field (`#[serde(default)]`), so the
/// frame stays backward compatible across a host/child version skew — the same rule the manifest's
/// `input_schema`/`emits_external` fields use. A child that DOES read it can attribute its per-call
/// row-filter decision to the real caller instead of a synthetic admin.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CallParams {
    pub tool: String,
    pub input: String,
    /// A minimal, **non-replayable** projection of the principal the host authorized for this call.
    /// `None` on an old-host frame (or a call with no resolvable caller). See [`Caller`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub caller: Option<Caller>,
}

/// A minimal projection of the routed caller the host stamps into a [`CallParams`] frame
/// (native-caller-identity scope). It is deliberately the *least* a per-caller row filter needs:
/// **who** (`sub`), **which tenant** (`ws`), **role**, and a **delegation marker** (`delegated`,
/// true when the caller is itself an on-behalf-of/derived principal).
///
/// **This is NOT a token.** It carries no signature the gateway would accept for a *new* call, so a
/// child can never *act as* the caller against a third tool (native-caller-identity scope, the #1
/// risk). A child may only (1) attribute its own row-filter decision to this identity and (2) name
/// `sub` as the `subject` of a reach verb it is *separately* granted to delegate. The projection
/// alone confers nothing — the delegation cap is what authorizes a subject reach.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Caller {
    /// The global identity the host authorized (`user:…` / `key:…` / `agent:…`).
    pub sub: String,
    /// The workspace the call is scoped to — equals the frame's ws and the child token's ws (the
    /// hard wall; a `subject` derived from this can only ever resolve within it).
    pub ws: String,
    /// The caller's role (`super-admin` / `workspace-admin` / `member`), lower-cased on the wire.
    pub role: String,
    /// True when the caller is itself a *derived* (on-behalf-of) principal — an agent acting for a
    /// user, or a re-entrant host-callback chain. A child MAY treat a delegated caller more
    /// conservatively; it is a marker, never additional authority.
    #[serde(default)]
    pub delegated: bool,
}
