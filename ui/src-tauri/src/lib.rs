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

mod commands;
mod state;

pub use commands::{
    agent_invoke, channel_delete, channel_edit, channel_history, channel_post, AgentResult,
};
pub use state::NodeHandle;
