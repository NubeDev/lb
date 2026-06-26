//! The wasmtime engine + linker: load a component from bytes into an [`Instance`].
//!
//! One [`Engine`] is shared across the host (it caches compiled code). Per-workspace resource
//! caps (wasmtime fuel/epoch, §11.4) are a knob configured here at S1 but not tuned until a
//! real workload exists (core scope open Q).

use thiserror::Error;
use wasmtime::component::{Component, HasSelf, Linker};
use wasmtime::{Config, Engine as WtEngine, Store as WtStore};

use crate::bindings::{Extension, HostState};
use crate::instance::Instance;

#[derive(Debug, Error)]
pub enum RuntimeError {
    #[error("failed to compile component: {0}")]
    Compile(String),
    #[error("failed to instantiate component: {0}")]
    Instantiate(String),
    #[error("tool call trapped or failed: {0}")]
    Call(String),
    #[error("tool returned an error: {0}")]
    Tool(String),
}

/// The shared wasmtime engine + a linker pre-wired with WASI and the host imports.
pub struct Engine {
    engine: WtEngine,
}

impl Engine {
    /// Build an async-enabled component engine.
    pub fn new() -> Result<Self, RuntimeError> {
        let mut config = Config::new();
        config.wasm_component_model(true);
        config.async_support(true);
        let engine = WtEngine::new(&config).map_err(|e| RuntimeError::Compile(e.to_string()))?;
        Ok(Self { engine })
    }

    /// Instantiate a component from its bytes. The returned [`Instance`] can answer tool calls.
    pub async fn load(&self, bytes: &[u8]) -> Result<Instance, RuntimeError> {
        let component = Component::new(&self.engine, bytes)
            .map_err(|e| RuntimeError::Compile(e.to_string()))?;

        let mut linker: Linker<HostState> = Linker::new(&self.engine);
        wasmtime_wasi::p2::add_to_linker_async(&mut linker)
            .map_err(|e| RuntimeError::Instantiate(e.to_string()))?;
        Extension::add_to_linker::<_, HasSelf<_>>(&mut linker, |s| s)
            .map_err(|e| RuntimeError::Instantiate(e.to_string()))?;

        let mut store = WtStore::new(&self.engine, HostState::new());
        let bindings = Extension::instantiate_async(&mut store, &component, &linker)
            .await
            .map_err(|e| RuntimeError::Instantiate(e.to_string()))?;

        Ok(Instance::new(store, bindings))
    }
}
