//! The **undo** service — the capability-gated surface over `lb_undo` for the UI's undo/redo
//! affordance (`docs/scope/undo/undo-scope.md`).
//!
//! Authorization is the MCP gate (`mcp:undo:call`, `mcp:redo:call`, `mcp:history.list:call`,
//! plus the no-escalation check on the original tool's cap and `mcp:undo.any:call` for another
//! actor). The raw journal/conditional-restore mechanism stays in `lb_undo`; this layer is
//! authorization only. One verb per file (FILE-LAYOUT §3):
//!   - `undo`                  — reverse the newest undoable step.
//!   - `redo`                  — re-apply the newest redoable step.
//!   - `history_list`          — the stack, for a UI affordance.
//!   - `history_compensations` — what a non-undoable step offers instead.

mod error;
mod history;
mod redo;
#[allow(clippy::module_inception)]
mod undo;

pub use error::UndoSvcError;
pub use history::{history_compensations, history_list};
pub use redo::redo;
pub use undo::undo;
