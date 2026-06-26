//! Register an extension as **remotely hosted** — the calling-node side of the routed MCP call.
//!
//! A node learns that some extension lives on another node (today: passed in by the wiring
//! layer; a discovery/registry flow lands at S4/S7). Once registered, a `lb_mcp::call` to that
//! extension's tools resolves locally and routes over the bus to the hosting node — callers and
//! `authorize` unchanged (mcp scope). No instance is created here; only the routing entry.

use crate::boot::Node;

/// Tell `node` that extension `ext_id` (declaring `tools`) is hosted on another node. Calls to
/// its tools will route over the bus. The grant/authorize path is unchanged — authorization
/// still runs on THIS node, workspace-first, before any routing.
pub fn register_remote_extension(node: &Node, ext_id: &str, tools: &[String]) {
    node.registry.register_remote(ext_id, tools.to_vec());
}
