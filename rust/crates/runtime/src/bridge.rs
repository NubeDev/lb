//! The host-callback seam: the narrow trait a guest's `host.call-tool` import dispatches through.
//!
//! **Why a trait and not `Arc<Node>` (host-callback scope, open question 5).** The guest callback
//! must end up at `lb_host::call_tool` — but `lb-runtime` sits BELOW `lb-host` in the dep graph
//! (`runtime` → `mcp` → `host`). Making `runtime` depend on `host` would invert the layering and
//! create a cycle. Instead the host supplies a `dyn HostBridge` trait object: `runtime` defines the
//! seam, `lb-host` implements it over the real `call_tool` chokepoint. The runtime stays oblivious
//! to what's on the other side — it only knows "dispatch this `{name, input}` under the identity I
//! was handed". This keeps the forever-ABI addition from leaking the host's shape into the SDK layer.
//!
//! The identity ([`CallContext`]) is set into the instance's `HostState` for the DURATION of one
//! `tool.call` and CLEARED after (per-call, never instance-sticky). The loaded instance is
//! node-global (one instance serves many workspaces — see
//! `debugging/extensions/loaded-extension-instance-is-node-global.md`), so identity MUST live on the
//! call, not the instance, or it would leak across workspaces.

use std::sync::Arc;

use async_trait::async_trait;

/// The error a host callback can return to the guest. Mirrors the WIT `tool-error` variants so the
/// runtime can map it onto the generated host-side error without `lb-runtime` knowing `lb-mcp`'s
/// concrete error type (kept narrow to avoid the layering inversion).
#[derive(Debug, Clone)]
pub enum BridgeError {
    /// The tool input was malformed for the target tool.
    BadInput(String),
    /// Anything else — denied, not found, depth-exceeded, or a downstream tool error. Collapsed to
    /// the WIT `failed` variant; a guest learns nothing finer (deny stays opaque, mcp scope).
    Failed(String),
}

/// The narrow host seam a guest reaches through `host.call-tool`. `lb-host` implements this over
/// `lb_host::call_tool`, carrying the effective principal + workspace the host set per call.
///
/// One method, one verb: dispatch a single qualified MCP tool call. The implementor owns ALL of
/// authorization (workspace-first, then `mcp:<tool>:call` against `caller ∩ grant`), the workspace
/// (host-set, never guest-supplied), and the depth guard — the runtime just forwards.
#[async_trait]
pub trait HostBridge: Send + Sync {
    /// Dispatch `name` (a qualified `<ext>.<tool>` or host-native verb) with `input_json`, returning
    /// the tool's JSON output. `depth` is the current re-entrancy depth (0 for the outermost guest
    /// call); the implementor enforces the limit.
    async fn call_tool(
        &self,
        name: &str,
        input_json: &str,
        depth: u32,
    ) -> Result<String, BridgeError>;
}

/// The per-call identity + dispatch handle the host injects into `HostState` before running a guest,
/// and clears after. `None` on `HostState` means "no host call in flight" — a guest that imports
/// `call-tool` but is somehow reached without a context set gets a `Failed` (never a panic).
#[derive(Clone)]
pub struct CallContext {
    /// The host seam (carries the effective principal + workspace inside the host).
    pub bridge: Arc<dyn HostBridge>,
    /// The current re-entrancy depth — incremented by the host as each callback dispatches a fresh
    /// guest call, so the host can enforce a fixed limit (host-callback scope, open question 1).
    pub depth: u32,
}
