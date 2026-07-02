//! The **`LocalDispatch`** seam — "an extension whose tools run on THIS node", abstracted away
//! from the concrete Tier the extension happens to be (mcp scope, README §6.5).
//!
//! The routing `Registry` (`lb_mcp`) holds one dispatch target per hosted extension. Historically
//! that target was a wasm [`Instance`](crate::Instance) directly, so a native Tier-2 sidecar — which
//! lives in a separate `SidecarMap` — was unreachable through `resolve`/`dispatch`/`serve_call` and
//! thus could not answer a routed cross-node call. This trait removes the Tier from the call path:
//! the registry holds `Arc<Mutex<dyn LocalDispatch>>`, and BOTH a wasm instance (here) and a native
//! sidecar adapter (in `lb_host`) implement it. Tier is a *registration* detail — which impl was
//! registered — never an `if native` branch in dispatch (§3.1).
//!
//! Object-safe (`&mut self`, all args by value/ref, boxed future via `async_trait`) so the registry
//! can store it behind `Arc<Mutex<dyn LocalDispatch>>`. The supertrait is `Send` (NOT `Sync`): a
//! wasm `Instance` owns a wasmtime `Store` that is `Send` but not `Sync`, and a `tokio::sync::Mutex<T>`
//! is already `Sync` whenever `T: Send` — so `Arc<Mutex<dyn LocalDispatch>>` is `Send + Sync` (shared
//! across the call/serve/reload paths) without demanding the target itself be `Sync`.

use crate::bridge::CallContext;
use crate::engine::RuntimeError;
use crate::instance::Instance;

/// A local tool-dispatch target on this node — a wasm instance or a native sidecar. `call_tool`
/// takes the UNQUALIFIED tool name and a JSON input string, returning the JSON output.
///
/// `ws` is the workspace the (already-authorized) call is scoped to. A wasm instance IGNORES it (it
/// is node-global — one instance per ext, per-call identity rides `ctx`). A native adapter USES it to
/// resolve its per-`(ws, ext_id)` sidecar, keeping the workspace wall structural for Tier 2.
///
/// `ctx` is the host-callback context, honored by wasm guests (their `host.call-tool` import runs
/// under it) and ignored by natives (a sidecar has its own `SidecarClient` identity via
/// `LB_EXT_TOKEN`).
#[async_trait::async_trait]
pub trait LocalDispatch: Send {
    async fn call_tool(
        &mut self,
        ws: &str,
        tool: &str,
        input_json: &str,
        ctx: Option<CallContext>,
    ) -> Result<String, RuntimeError>;
}

/// A wasm [`Instance`] is a local dispatch target: it ignores `ws` (node-global) and forwards to the
/// WIT `tool.call` export via [`Instance::call_tool_with`], carrying `ctx` for the guest callback.
#[async_trait::async_trait]
impl LocalDispatch for Instance {
    async fn call_tool(
        &mut self,
        _ws: &str,
        tool: &str,
        input_json: &str,
        ctx: Option<CallContext>,
    ) -> Result<String, RuntimeError> {
        self.call_tool_with(tool, input_json, ctx).await
    }
}
