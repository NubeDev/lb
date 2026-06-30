//! The Lazybones desktop shell — command layer (FILE-LAYOUT). The node runs in-process here;
//! the window (the `desktop` bin) attaches to it and the UI calls the commands below over IPC.
//!
//! The command functions are kept here as a library so they can be unit-tested WITHOUT the
//! webkit window toolchain (`cargo test -p lazybones-shell`). The `desktop` feature + the bin
//! add the Tauri `#[command]` wrappers and the window.

mod commands;
mod state;

pub use commands::{channel_delete, channel_edit, channel_history, channel_post};
pub use state::NodeHandle;
