//! The **inbox** service — the capability-gated surface over `lb_inbox` for the real inbox UI
//! (collaboration scope, slice 4). The durable items already exist (`lb_inbox::list`/`resolve`);
//! this service gates them so the UI's `features/inbox/` reads the **real** triage/approval items
//! and the S6 approval gate becomes a real UI action — replacing the workflow fake's simulated inbox.
//!
//! Authorization is the MCP gate (`mcp:inbox.<verb>:call`) through `authorize_tool` (workspace-first
//! §7, then capability §3.5). The raw item/resolution persistence stays in `lb_inbox`; this layer is
//! authorization only. One verb per file (FILE-LAYOUT §3):
//!   - `list_inbox`    — a channel's durable items (`needs:triage`, `needs:approval`, …).
//!   - `record_inbox`  — create an item (author forced to the principal's `sub`; proof-workflow-sim).
//!   - `resolve_inbox` — record a reviewer's approve/reject/defer (the actor is forced to the
//!     principal's `sub`, never caller-supplied — a caller can't forge another reviewer's sign-off).

mod error;
mod list;
mod record;
mod resolve;

pub use error::InboxError;
pub use list::list_inbox;
pub use record::record_inbox;
pub use resolve::resolve_inbox;
