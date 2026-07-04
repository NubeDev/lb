//! The **generic approval-release reactor** — a rule proposes a gated effect (`inbox.request_approval`
//! stages it `held`), a human disposes (`inbox.resolve`), and this reactor releases the effect to the
//! outbox on `Approved` or discards it on `Rejected` (rules-approvals scope).
//!
//! Domain-free (rule 10): it keys only on the `(resolution, held-effect-id)` pair, deriving the effect
//! id from the item id ([`held_effect_id`]) — it never branches on any extension or effect semantics.
//! It is the sibling the scope chose over generalizing the coding-workflow's `resolve_approval` (Open
//! questions, option b), keeping that PR path untouched.
//!
//! One responsibility per file (FILE-LAYOUT §3): [`id`] derives the effect id, [`react`] is the pass,
//! [`spawn`] is the boot tick.

mod id;
mod react;
mod spawn;

pub use id::held_effect_id;
pub use react::{react_to_approval_releases, ApprovalReleasePass};
pub use spawn::spawn_approval_reactors;
