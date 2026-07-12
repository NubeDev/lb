//! The extension runtime — Tier-1 WASM components on wasmtime (WASI 0.2 / Component Model,
//! README §6.3). This is the host side of the stable WIT boundary (crate-layout scope): it
//! generates host bindings from the *same* `sdk/wit/world.wit` the guest compiles against, so
//! the contract cannot drift.
//!
//! A loaded component is a [`Instance`]; calling a tool on it routes JSON in/out through the
//! WIT `tool.call` export. The host `log` import is provided here. Capability checks happen
//! *before* the runtime is ever reached (in `mcp`) — the runtime is the dispatch mechanism,
//! not an authorization point.
//!
//! Tier-2 native sidecars (§6.3 escape hatch) land at S7; this crate is Tier-1 only for now.

mod bindings;
mod bridge;
mod compat_v0_1;
mod dispatch;
mod engine;
mod instance;

pub use bridge::{BridgeError, CallContext, Caller, HostBridge};
pub use dispatch::LocalDispatch;
pub use engine::{Engine, RuntimeError};
pub use instance::Instance;
