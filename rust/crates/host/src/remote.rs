//! Register an extension as **remotely hosted on a named node** — the calling-node side of the
//! routed MCP call.
//!
//! A node learns that some extension lives on another node (today: passed in by the wiring layer;
//! the discovery flow that populates this from live liveliness is fleet-presence's ext-hosting
//! announce at `ws/{id}/nodes/{node}/ext/{ext}` — see the routed-node-dispatch scope's discovery
//! risk). Once registered, a `lb_mcp::call` to that extension's tools resolves locally and routes
//! over the bus to the hosting node — callers and `authorize` unchanged (mcp scope). No instance
//! is created here; only the routing entry.
//!
//! **Registering a second node for the same extension is the fleet case**, and is exactly what
//! makes an untargeted call to that extension `Ambiguous` rather than a silent coin flip
//! (routed-node-dispatch, #81). That is why the node id is required rather than optional: without
//! it a second host was not representable, so the ambiguity was invisible.

use lb_bus::NodeId;

use crate::boot::Node;

/// Tell `node_handle` that extension `ext_id` (declaring `tools`) is hosted on node `host`. Calls
/// to its tools will route over the bus to that node. The grant/authorize path is unchanged —
/// authorization still runs on THIS node, workspace-first, before any routing.
///
/// Registering the same `host` again replaces its entry (a re-announce); registering a *different*
/// host adds a second candidate, making untargeted calls to `ext_id` ambiguous.
pub fn register_remote_extension(node_handle: &Node, ext_id: &str, host: NodeId, tools: &[String]) {
    node_handle
        .registry
        .register_remote(ext_id, host, tools.to_vec());
}

/// Forget the remote target for `ext_id` on `host` — the reaction to a hosting announce being
/// retracted (the node dropped, or stopped hosting the ext). An ext that drops back to a single
/// host stops refusing untargeted calls, with no restart needed.
pub fn forget_remote_extension(node_handle: &Node, ext_id: &str, host: &NodeId) {
    node_handle.registry.forget_remote(ext_id, host);
}
