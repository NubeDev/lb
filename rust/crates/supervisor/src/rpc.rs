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

/// The `params` shape for a [`Method::Call`]: which tool and its opaque-JSON input.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CallParams {
    pub tool: String,
    pub input: String,
}
