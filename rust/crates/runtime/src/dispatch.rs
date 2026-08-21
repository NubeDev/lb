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
/// An owned, `Send` future the host can await after releasing a lock — what
/// [`LocalDispatch::call_tool_detached`] hands back.
pub type BoxFuture<'a, T> = std::pin::Pin<Box<dyn std::future::Future<Output = T> + Send + 'a>>;

#[async_trait::async_trait]
pub trait LocalDispatch: Send {
    /// Can this target serve several calls at once?
    ///
    /// **Default `false`** — the conservative answer, and the right one for a wasm [`Instance`],
    /// which genuinely cannot run two calls concurrently. The host therefore holds the
    /// per-instance mutex across the whole call for it, exactly as before.
    ///
    /// A **native sidecar** overrides this to `true`: it is a separate process whose wire
    /// multiplexes (`lb-ext-native` spawns each `call`, correlating replies by `id`), so the
    /// exclusive lock buys it nothing and costs it everything — it caps a whole extension at
    /// concurrency 1 node-wide. Measured on a live node before this existed: a 20 ms LOCAL store
    /// read took 8.96 s while one slow verb was in flight.
    ///
    /// This is deliberately a property the TARGET declares rather than a Tier branch at the call
    /// site, so the registry and `dispatch` stay Tier-agnostic (§3.1) — the one trait keeps
    /// reaching either kind, and only the honest answer to "can you multiplex?" differs.
    fn is_multiplexing(&self) -> bool {
        false
    }

    /// A **detached** call for a target that answered `true` to
    /// [`is_multiplexing`](Self::is_multiplexing): build the whole round-trip as an owned future
    /// while the host still holds the per-instance mutex, so the host can then **drop the guard**
    /// and await it unlocked.
    ///
    /// This shape (rather than a plain `&self` method) is deliberate: a wasm [`Instance`] is not
    /// `Sync` — it owns wasmtime state that cannot be shared across threads — so a `&self` future
    /// would force a `Sync` bound the wasm tier genuinely cannot satisfy. Returning an owned
    /// `'static` future keeps the bound local to the impls that opt in.
    ///
    /// **Default:** `None` — "I do not offer a detached path". `dispatch` only consults this when
    /// `is_multiplexing` is `true`, and falls back to the exclusive [`call_tool`](Self::call_tool)
    /// if an impl nonetheless returns `None`, so the two methods cannot disagree into a broken call.
    fn call_tool_detached(
        &self,
        _ws: &str,
        _tool: &str,
        _input_json: &str,
        _ctx: Option<CallContext>,
    ) -> Option<BoxFuture<'static, Result<String, RuntimeError>>> {
        None
    }

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
