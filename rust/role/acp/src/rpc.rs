//! The JSON-RPC 2.0 envelope the ACP adapter speaks over stdio (agent-run scope Part 4). ACP is a
//! JSON-RPC protocol; this is the minimal, version-pinned wire shape — requests (id + method +
//! params), responses (id + result | error), and notifications (method + params, no id, no reply).
//!
//! Kept deliberately small: the adapter is a **thin encoder** (the scope's "how thin can the adapter
//! be"), so we model only the frames the ACP v1 lifecycle uses, not a general JSON-RPC library. The
//! stable internal contract is the [`RunEvent`](lb_run_events) vocabulary + the durable transcript;
//! this file is just its outermost wire framing, version-pinned so a protocol drift is a localized
//! edit here, never a kernel change.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A JSON-RPC request from the editor (client) to the adapter. `id` is echoed in the response so the
/// client can correlate; a frame *without* an `id` is a [`Notification`] (no reply expected).
#[derive(Debug, Clone, Deserialize)]
pub struct Request {
    #[allow(dead_code)]
    pub jsonrpc: Option<String>,
    /// Present for a request (must be echoed); absent for a notification.
    pub id: Option<Value>,
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

/// A successful JSON-RPC response. `id` echoes the request's id.
#[derive(Debug, Clone, Serialize)]
pub struct Response {
    pub jsonrpc: &'static str,
    pub id: Value,
    pub result: Value,
}

impl Response {
    pub fn ok(id: Value, result: Value) -> Self {
        Self {
            jsonrpc: "2.0",
            id,
            result,
        }
    }
}

/// A JSON-RPC error response.
#[derive(Debug, Clone, Serialize)]
pub struct ErrorResponse {
    pub jsonrpc: &'static str,
    pub id: Value,
    pub error: ErrorBody,
}

#[derive(Debug, Clone, Serialize)]
pub struct ErrorBody {
    pub code: i64,
    pub message: String,
}

impl ErrorResponse {
    pub fn new(id: Value, code: i64, message: impl Into<String>) -> Self {
        Self {
            jsonrpc: "2.0",
            id,
            error: ErrorBody {
                code,
                message: message.into(),
            },
        }
    }
}

/// A JSON-RPC notification from the adapter to the editor (no id, no reply) — how streamed
/// `session/update`s are pushed.
#[derive(Debug, Clone, Serialize)]
pub struct Notification {
    pub jsonrpc: &'static str,
    pub method: &'static str,
    pub params: Value,
}

impl Notification {
    pub fn new(method: &'static str, params: Value) -> Self {
        Self {
            jsonrpc: "2.0",
            method,
            params,
        }
    }
}

/// JSON-RPC error codes the adapter returns. The standard reserved range plus an ACP-app code for
/// "client-provided MCP servers / cwd are not supported in v1" (a clean, explicit refusal rather than
/// a silent drop — scope Non-goals / Resolved decisions).
pub mod codes {
    pub const INVALID_PARAMS: i64 = -32602;
    pub const METHOD_NOT_FOUND: i64 = -32601;
    /// Authentication failed (no/invalid session token) — the trusted-session wall (Part 4).
    pub const UNAUTHENTICATED: i64 = -32001;
    /// A capability/grant denied the operation (opaque — same as a forged call).
    pub const DENIED: i64 = -32002;
    /// Client-provided `mcpServers`/`cwd` on `session/new` — unsupported in v1, rejected cleanly.
    pub const UNSUPPORTED_CLIENT_SERVERS: i64 = -32010;
}
