//! The **outbox status** service — the read-only delivery view (collaboration scope, slice 4).
//!
//! The outbox is must-deliver *infrastructure*, not a user-edited object (the scope's rejected
//! alternative: "give the outbox a CRUD UI" — no). So this surface is **read-only**: it returns the
//! effects grouped by where they are in their lifecycle (pending / delivered / dead-lettered) so the
//! UI can show "pending → delivered (→ dead-letter)" for a real effect. `mark`/delete stay the relay's
//! job (the `workflow` service), never a user verb — but `enqueue_outbox` (proof-workflow-sim scope)
//! lets a granted bridged caller STAGE a pending effect, so a guest can PRODUCE outbox motion (the
//! delivery itself remains the relay's). A staged effect is `Pending`; the relay decides delivery.
//!
//! Authorization is `mcp:outbox.status:call` (workspace-first §7) — workspace-scoped read for any
//! grantee (the open question's lean: "workspace-scoped read, capability-gated like any verb"). One
//! verb (FILE-LAYOUT §3).

mod enqueue;
mod error;
mod status;

pub use enqueue::enqueue_outbox;
pub use error::OutboxError;
pub use status::{outbox_status, OutboxStatus};
