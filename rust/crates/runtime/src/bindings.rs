//! Host-side bindings generated from the stable WIT (`sdk/wit/world.wit`), plus the host
//! state that satisfies the world's imports.
//!
//! `bindgen!` reads the *same* WIT the guest uses, so the host and guest sides of the ABI are
//! generated from one source — they cannot drift (crate-layout scope, the SDK/WIT decision).

use wasmtime::component::ResourceTable;
use wasmtime_wasi::{WasiCtx, WasiCtxBuilder, WasiCtxView, WasiView};

wasmtime::component::bindgen!({
    path: "../../sdk/wit",
    world: "extension",
    exports: { default: async },
});

/// Per-instance host state. Holds the WASI context (the component is a WASI 0.2 command) and
/// a sink for the guest's `log` import. Tool calls capture logs for the host to surface.
pub struct HostState {
    wasi: WasiCtx,
    table: ResourceTable,
    pub logs: Vec<String>,
}

impl HostState {
    pub fn new() -> Self {
        Self {
            wasi: WasiCtxBuilder::new().build(),
            table: ResourceTable::new(),
            logs: Vec::new(),
        }
    }
}

impl Default for HostState {
    fn default() -> Self {
        Self::new()
    }
}

impl WasiView for HostState {
    fn ctx(&mut self) -> WasiCtxView<'_> {
        WasiCtxView {
            ctx: &mut self.wasi,
            table: &mut self.table,
        }
    }
}

// The world's `import host;` — the host functions a guest may call. Each would be
// capability-gated host-side before doing anything real; `log` is harmless and just captured.
impl lazybones::ext::host::Host for HostState {
    fn log(&mut self, message: String) {
        self.logs.push(message);
    }
}
