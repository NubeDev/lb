//! The host-callback bridge: the `lb-runtime` [`HostBridge`] seam implemented over the host's
//! `call_tool` chokepoint (host-callback scope).
//!
//! When the host runs a wasm guest under a caller's identity, it installs a [`Bridge`] into the
//! instance's `HostState`. The guest's `host.call-tool` import dispatches through it — re-entering
//! the SAME `lb_host::call_tool` the page bridge and the gateway reach, so a guest-initiated call is
//! authorized and workspace-checked identically to any other. The bridge carries:
//!   - an `Arc<Node>` (to reach the store/registry/bus the dispatch needs),
//!   - the guest's **effective principal** = `caller ∩ install-grant` (set by the caller, never
//!     widened — see [`super::tool_call`]),
//!   - the workspace (host-set, never guest-supplied),
//! so the callback acts on behalf of the caller AND within the install grant, in the caller's ws.
//!
//! **Borrow discipline (the re-entrancy hazard).** The callback dispatches through `call_tool`,
//! which resolves a FRESH target (instance/route) and locks it — it never re-borrows the in-flight
//! `&mut Instance` whose guest is mid-call. A guest that calls its OWN tool recurses through a fresh
//! lock acquisition; the depth guard (a fixed limit) bounds it before a stack blow-up or a deadlock.

use std::sync::Arc;

use async_trait::async_trait;
use lb_auth::Principal;
use lb_mcp::ToolError;
use lb_runtime::{BridgeError, HostBridge};

use crate::boot::Node;
use crate::tool_call::call_tool_at_depth;

/// The re-entrancy depth limit (host-callback scope, open question 1 → a small fixed constant). A
/// guest→host→guest chain deeper than this returns `tool-error::failed("call depth exceeded")`
/// rather than recursing into a stack overflow or a lock deadlock.
pub const MAX_CALL_DEPTH: u32 = 8;

/// The live host-callback handle installed into a guest's `HostState` for the duration of one call.
pub struct Bridge {
    node: Arc<Node>,
    /// The effective principal = caller ∩ install-grant. Authorized against on every callback.
    principal: Principal,
    ws: String,
}

impl Bridge {
    pub fn new(node: Arc<Node>, principal: Principal, ws: impl Into<String>) -> Self {
        Self {
            node,
            principal,
            ws: ws.into(),
        }
    }
}

#[async_trait]
impl HostBridge for Bridge {
    async fn call_tool(
        &self,
        name: &str,
        input_json: &str,
        depth: u32,
    ) -> Result<String, BridgeError> {
        // The guest is one hop deeper than the call that installed this bridge. Guard BEFORE
        // dispatching so a recursive guest can never blow the stack or deadlock on its own lock.
        let next_depth = depth + 1;
        if next_depth > MAX_CALL_DEPTH {
            return Err(BridgeError::Failed("call depth exceeded".into()));
        }
        // Re-enter the one chokepoint: authorize (workspace-first, then `mcp:<tool>:call` against
        // the effective principal) then dispatch. A host-native verb runs over the store; an
        // `<ext>.<tool>` resolves a fresh instance/route (never the in-flight one).
        call_tool_at_depth(
            &self.node,
            &self.principal,
            &self.ws,
            name,
            input_json,
            next_depth,
        )
        .await
        .map_err(map_tool_err)
    }
}

/// Collapse a host `ToolError` to the guest-visible [`BridgeError`]. Deny/not-found stay opaque
/// (`Failed`) — a guest learns nothing finer than the page bridge would; only `BadInput` is
/// distinguished so a guest can tell "I sent bad JSON" from "the host said no".
fn map_tool_err(e: ToolError) -> BridgeError {
    match e {
        ToolError::BadInput(m) => BridgeError::BadInput(m),
        ToolError::Denied => BridgeError::Failed("denied".into()),
        ToolError::NotFound => BridgeError::Failed("no such tool".into()),
        ToolError::Extension(m) => BridgeError::Failed(m),
        // Routing failures (routed-node-dispatch #81) reach the guest as opaque `Failed`, like
        // deny/not-found. A guest re-entering the host does not address nodes — it has no target
        // parameter and no node identity — so these are host-infrastructure facts it can neither
        // act on nor legitimately learn. Their `Display` carries the detail for host-side logs.
        e @ (ToolError::Ambiguous { .. }
        | ToolError::NodeUnreachable { .. }
        | ToolError::NodeTooOld { .. }) => BridgeError::Failed(e.to_string()),
    }
}
