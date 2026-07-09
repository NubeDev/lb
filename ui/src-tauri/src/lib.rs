//! The Lazybones desktop shell — command layer (FILE-LAYOUT). The node runs in-process here;
//! the window (the `desktop` bin) attaches to it and the UI calls the commands below over IPC.
//!
//! The command functions are kept here as a library so they can be unit-tested WITHOUT the
//! webkit window toolchain (`cargo test -p lazybones-shell`). The `desktop` feature + the bin
//! add the Tauri `#[command]` wrappers and the window.

// The standalone full-stack boot (mounts the in-process gateway + boot seeders). Only
// compiled under the `full` feature; the thin shell + the headless command layer skip it.
#[cfg(feature = "full")]
pub mod full;

// The bundled federation datasources sidecar mount (desktop-federation-bundle scope). `full`-only:
// the thin shell ships no sidecar. Public so `boot_full` + the loopback test can drive it.
#[cfg(feature = "full")]
pub mod federation;

// The per-user persistent store path resolver (desktop-persistent-store scope). `full`-only: the
// windowed boot fills `LB_STORE_PATH` so a restart keeps the user's work. Public so `run` calls it.
#[cfg(feature = "full")]
pub mod store;

mod commands;
mod state;

pub use commands::{
    agent_invoke, channel_delete, channel_edit, channel_history, channel_post, AgentResult,
};
pub use state::NodeHandle;
