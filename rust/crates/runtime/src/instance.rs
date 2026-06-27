//! A live extension instance: call a tool on it through the WIT `tool.call` export.
//!
//! The instance owns its wasmtime store (and thus its `HostState`). Tool input/output are
//! JSON strings (the stable ABI keeps richer schemas host-side, mcp scope).

use crate::bindings::{Extension, HostState};
use crate::bridge::CallContext;
use crate::engine::RuntimeError;
use wasmtime::Store as WtStore;

/// A loaded, instantiated component ready to answer tool calls.
pub struct Instance {
    store: WtStore<HostState>,
    bindings: Extension,
}

impl Instance {
    pub(crate) fn new(store: WtStore<HostState>, bindings: Extension) -> Self {
        Self { store, bindings }
    }

    /// Invoke `name` with a JSON input string; return the JSON output string. Maps the WIT
    /// `tool-error` variant and any wasm trap onto [`RuntimeError`].
    ///
    /// No host-callback identity: the guest's `host.call-tool` import is unavailable (fails closed).
    /// Used by callers that don't (yet) carry a principal.
    pub async fn call_tool(
        &mut self,
        name: &str,
        input_json: &str,
    ) -> Result<String, RuntimeError> {
        self.call_tool_with(name, input_json, None).await
    }

    /// Invoke `name`, optionally carrying a [`CallContext`] so the guest's `host.call-tool` import
    /// can dispatch host MCP tools under the host-set identity. The context is installed into
    /// `HostState` BEFORE the guest runs and CLEARED after — per-call, never instance-sticky (the
    /// instance is node-global, so a sticky identity would leak across workspaces).
    pub async fn call_tool_with(
        &mut self,
        name: &str,
        input_json: &str,
        ctx: Option<CallContext>,
    ) -> Result<String, RuntimeError> {
        self.store.data_mut().call_ctx = ctx;
        let result = self
            .bindings
            .lazybones_ext_tool()
            .call_call(&mut self.store, name, input_json)
            .await
            .map_err(|e| RuntimeError::Call(e.to_string()));
        // Clear identity regardless of how the call ended — no bleed into the next call.
        self.store.data_mut().call_ctx = None;

        result?.map_err(|tool_err| RuntimeError::Tool(format!("{tool_err:?}")))
    }

    /// Drain the guest's captured `log` messages (for the host to surface/audit).
    pub fn take_logs(&mut self) -> Vec<String> {
        std::mem::take(&mut self.store.data_mut().logs)
    }
}
