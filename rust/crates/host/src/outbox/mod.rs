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
//!
//! The **delivery** primitives — the [`Target`] trait and the [`relay_outbox`] at-least-once loop over
//! it — live here too (relocated from the retired `workflow/` service, rules-workflow-convergence
//! scope). They are provider-free (rule 10) and drive the outbox-sink flow node's outbound delivery
//! plus the reminders/approval reactors.

mod email_target;
mod enqueue;
mod enqueue_held;
mod error;
mod relay;
mod relay_ops;
mod relay_reactor;
mod router_target;
mod status;
mod target;

pub use email_target::{
    EmailMeta, EmailProvider, EmailTarget, LoggingEmailProvider, RecordedEmail,
    RecordingEmailProvider,
};
pub use enqueue::enqueue_outbox;
pub use enqueue_held::enqueue_held_outbox;
pub use error::OutboxError;
pub use relay::{relay_outbox, RelayPass};
pub use relay_ops::{outbox_due, outbox_mark_delivered, outbox_mark_failed};
pub use relay_reactor::spawn_relay_reactors;
pub use router_target::{DynTarget, RouterTarget};
pub use status::{outbox_status, OutboxStatus};
pub use target::Target;
