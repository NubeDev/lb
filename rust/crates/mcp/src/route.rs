//! The cross-node tool-call wire: the bus key a routed call rides on, and the request/reply
//! envelope. One place owns the convention so the calling node's `query` and the serving
//! node's queryable cannot drift (mirrors the channel `key.rs` discipline).
//!
//! - bus key (workspace-relative): `mcp/{ext}/call` — workspace-prefixed by `lb_bus` to
//!   `ws/{id}/mcp/{ext}/call`. Ext-specific so ONLY the node hosting `{ext}` answers, and the
//!   `ws/{id}/` prefix means a call authorized for workspace B can never reach a queryable
//!   serving workspace A (the workspace wall on the routed path, §7).
//! - request: `{ tool: "<ext>.<tool>", input: "<json>" }`.
//! - reply:   `Ok(output_json)` or `Err(message)` — the serving node's local dispatch result.

use serde::{Deserialize, Serialize};

/// The workspace-relative bus key for routing calls to extension `ext`. Only that extension's
/// hosting node declares a queryable here, so the call lands on exactly one node.
pub fn call_key(ext: &str) -> String {
    format!("mcp/{ext}/call")
}

/// The routed call request: the qualified tool name and the JSON input, as bytes on the bus.
#[derive(Serialize, Deserialize)]
pub struct CallRequest {
    pub tool: String,
    pub input: String,
}

/// The routed call reply: the serving node's local dispatch outcome, serialized back.
#[derive(Serialize, Deserialize)]
pub enum CallReply {
    Ok(String),
    Err(String),
}
