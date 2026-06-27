//! A live extension instance: call a tool on it through the WIT `tool.call` export.
//!
//! The instance owns its wasmtime store (and thus its `HostState`). Tool input/output are
//! JSON strings (the stable ABI keeps richer schemas host-side, mcp scope).

use crate::bindings::{Extension, HostState};
use crate::bridge::CallContext;
use crate::compat_v0_1::ExtensionV1;
use crate::engine::RuntimeError;
use wasmtime::Store as WtStore;

/// Which WIT-world generation a loaded guest speaks. The `tool.call` export is byte-identical across
/// 0.1.0 and 0.2.0 — only the imported `host` interface grew — so the only difference here is which
/// generated `call_call` we dispatch through. A 0.1.0 guest never imports `host.call-tool`, so its
/// `CallContext` (if any) is simply unused.
enum Bindings {
    V2(Extension),
    V1(ExtensionV1),
}

/// A loaded, instantiated component ready to answer tool calls.
pub struct Instance {
    store: WtStore<HostState>,
    bindings: Bindings,
}

impl Instance {
    pub(crate) fn new(store: WtStore<HostState>, bindings: Extension) -> Self {
        Self {
            store,
            bindings: Bindings::V2(bindings),
        }
    }

    /// Construct from a legacy `@0.1.0` guest (the ABI back-compat path).
    pub(crate) fn new_v1(store: WtStore<HostState>, bindings: ExtensionV1) -> Self {
        Self {
            store,
            bindings: Bindings::V1(bindings),
        }
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
        let result = match &self.bindings {
            Bindings::V2(b) => b
                .lazybones_ext_tool()
                .call_call(&mut self.store, name, input_json)
                .await
                .map_err(|e| RuntimeError::Call(e.to_string()))
                .map(|r| r.map_err(|e| format!("{e:?}"))),
            Bindings::V1(b) => b
                .lazybones_ext_tool()
                .call_call(&mut self.store, name, input_json)
                .await
                .map_err(|e| RuntimeError::Call(e.to_string()))
                .map(|r| r.map_err(|e| format!("{e:?}"))),
        };
        // Clear identity regardless of how the call ended — no bleed into the next call.
        self.store.data_mut().call_ctx = None;

        result?.map_err(RuntimeError::Tool)
    }

    /// Drain the guest's captured `log` messages (for the host to surface/audit).
    pub fn take_logs(&mut self) -> Vec<String> {
        std::mem::take(&mut self.store.data_mut().logs)
    }
}
