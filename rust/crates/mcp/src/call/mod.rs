//! The tool-call pipeline, one phase per file (mcp scope, FILE-LAYOUT §3).
//!
//! Orchestration only: resolve the name, authorize (the deny gate), dispatch. `authorize`
//! runs before `dispatch` and — critically — a denied caller learns nothing about whether the
//! tool exists (the error carries no existence signal). That ordering is a tested contract.

mod authorize;
mod dispatch;
mod error;
mod resolve;

pub use error::ToolError;

use lb_auth::Principal;
use lb_bus::{Bus, NodeId};
use lb_runtime::CallContext;

use crate::registry::Registry;

/// The MCP authorize gate, exposed for **host-native tools** (e.g. the asset verbs) that are not
/// wasm extensions but must still be reached through the one MCP contract (README §6.5). A host
/// tool runs this first — workspace-first, then the `mcp:<tool>:call` capability — so the MCP
/// surface enforces the same isolation + deny as a routed extension call, *before* delegating to
/// the host verb (which adds its own store-surface capability + membership/grant gate).
pub fn authorize_tool(
    principal: &Principal,
    ws: &str,
    qualified_tool: &str,
) -> Result<(), ToolError> {
    authorize::authorize(principal, ws, qualified_tool)
}

/// Call `<ext>.<tool>` as `principal` with a JSON input string. Returns the JSON output, or a
/// [`ToolError`]. The single public entry to the MCP tool surface.
///
/// `bus` + `ws` carry the routed path: if the extension is hosted on another node, `dispatch`
/// routes the (already-authorized) call over the workspace-scoped queryable. Authorization
/// always runs HERE first, workspace-first — the remote node never sees an unauthorized call.
pub async fn call(
    registry: &Registry,
    bus: &Bus,
    principal: &Principal,
    ws: &str,
    qualified_tool: &str,
    input_json: &str,
) -> Result<String, ToolError> {
    call_with_ctx(
        registry,
        bus,
        principal,
        ws,
        qualified_tool,
        input_json,
        None,
        false,
    )
    .await
}

/// Call `<ext>.<tool>` **on a named node** (routed-node-dispatch, #81). Identical to [`call`] in
/// every respect except that resolve is given an explicit target, so:
///
/// - a multiply-hosted extension resolves unambiguously instead of returning
///   [`ToolError::Ambiguous`];
/// - a node that is not reachable returns [`ToolError::NodeUnreachable`] — a refusal, never a
///   fallback to another host of the same extension (the fallback is the misprovisioning bug).
///
/// **Addressing is not authorization.** A targeted call authorizes exactly as an untargeted one
/// does, `mcp:<ext>.<tool>:call`, with no per-node grant and no new grammar — naming a node cannot
/// widen what a caller may do, and a capless caller is `Denied` before any node is looked at.
pub async fn call_on_node(
    registry: &Registry,
    bus: &Bus,
    principal: &Principal,
    ws: &str,
    qualified_tool: &str,
    input_json: &str,
    node: &NodeId,
) -> Result<String, ToolError> {
    call_inner(
        registry,
        bus,
        principal,
        ws,
        qualified_tool,
        input_json,
        None,
        false,
        Some(node),
    )
    .await
}

/// Like [`call`], but carries an optional host-callback [`CallContext`] for a **local wasm guest**:
/// the host installs it into the instance so the guest's `host.call-tool` import can re-enter the
/// host MCP surface under the guest's delegated authority (host-callback scope). `None` (and any
/// routed/remote target) means the guest gets no callback — a routed guest's identity would have to
/// ride the wire (a separate scope), and a host-native verb carries no guest at all.
pub async fn call_with_ctx(
    registry: &Registry,
    bus: &Bus,
    principal: &Principal,
    ws: &str,
    qualified_tool: &str,
    input_json: &str,
    ctx: Option<CallContext>,
    reentrant: bool,
) -> Result<String, ToolError> {
    call_inner(
        registry,
        bus,
        principal,
        ws,
        qualified_tool,
        input_json,
        ctx,
        reentrant,
        None,
    )
    .await
}

/// The one pipeline every entry point funnels through. `target_node` is the only axis that
/// differs between [`call`]/[`call_with_ctx`] and [`call_on_node`] — keeping ONE body means the
/// authorize→resolve→dispatch ordering (and its deny guarantee) cannot drift between the targeted
/// and untargeted paths.
#[allow(clippy::too_many_arguments)]
async fn call_inner(
    registry: &Registry,
    bus: &Bus,
    principal: &Principal,
    ws: &str,
    qualified_tool: &str,
    input_json: &str,
    ctx: Option<CallContext>,
    reentrant: bool,
    target_node: Option<&NodeId>,
) -> Result<String, ToolError> {
    // 1. authorize FIRST — the DENY gate. Workspace isolation, then the
    //    mcp:<ext>.<tool>:call capability. Running it before resolve guarantees a denied
    //    caller cannot distinguish "not allowed" from "tool doesn't exist": both paths that
    //    a unauthorized caller can reach return `Denied` with no existence signal.
    //
    //    This ordering is LOAD-BEARING for #81 too, and more sharply than before: resolve can now
    //    return `Ambiguous` (which NAMES the nodes hosting an ext) and `NodeUnreachable` (which
    //    confirms whether a named node exists). Running authorize first means an unauthorized
    //    caller reaches neither — it cannot enumerate the fleet, and its `Denied` is identical
    //    whether or not the node it named is real. The targeted path adds NO new grant surface:
    //    the capability checked is the same `mcp:<ext>.<tool>:call` regardless of `target_node`.
    authorize::authorize(principal, ws, qualified_tool)?;

    // 2. resolve the "<ext>.<tool>" name (plus any explicit target) to ONE target — only reached
    //    once authorized. This is where an untargeted call to a multiply-hosted ext is refused
    //    rather than coin-flipped.
    let target = resolve::resolve(registry, qualified_tool, target_node)?;

    // 3. dispatch: call the local instance (with the callback context), or route over the bus to
    //    the hosting node. The seam is identical whether the ext is local or remote — that is the
    //    S3 point.
    dispatch::dispatch(&target, bus, ws, qualified_tool, input_json, ctx, reentrant).await
}
