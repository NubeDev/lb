//! The IPC command verbs the desktop UI calls (FILE-LAYOUT §3: one verb per file). Each
//! mirrors a Rust channel verb and the TS api client name one-to-one — `channel_post`,
//! `channel_history`, `channel_edit`, `channel_delete` — so a verb has the same name in the host,
//! the shell command, and the client. These are plain async functions (headlessly testable); the
//! Tauri `#[command]` glue that exposes them lives in the desktop bin behind the `desktop` feature.

mod delete;
mod edit;
mod history;
mod post;

pub use delete::channel_delete;
pub use edit::channel_edit;
pub use history::channel_history;
pub use post::channel_post;
