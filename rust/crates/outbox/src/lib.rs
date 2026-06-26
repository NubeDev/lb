//! The transactional must-deliver **outbox** — the durability backstop for every external effect
//! a workspace produces (README §6.10, outbox scope). The S6 driver.
//!
//! An [`Effect`] is *state*: a SurrealDB record (`outbox:{id}`) within a workspace namespace (the
//! hard wall, §7). No separate queue or datastore (§3.2). The pattern in one line: the domain
//! change AND the effect row are written in **one transaction** (`enqueue`, over `lb_store::write_tx`),
//! so an effect is never orphaned from its change and a change never silently fails to schedule its
//! effect. A relay then delivers `pending` rows **at-least-once with retry**; the receiver dedups on
//! the stable `idempotency_key`, so a re-delivery is a no-op (never lost, never double-sent).
//!
//! This crate is the *store side*: the record + the raw verbs. It holds **no authorization** —
//! exactly like `lb_inbox`/`lb_jobs`/`lb_assets`, these run *after* `caps::check` (the host
//! `workflow` service is the chokepoint, capability-first §3.5). The relay loop itself lives in the
//! host (it needs the `Target` seam + the bus); this crate owns only the durable record + its verbs.
//!
//! Verbs, one per file (FILE-LAYOUT §3):
//! - [`enqueue`] — write a domain change AND its effect in one transaction (the seam).
//! - [`pending`] — scan the workspace's undelivered effects (the relay's durable backstop).
//! - [`mark_delivered`] / [`mark_failed`] — record the outcome of a delivery attempt.

mod enqueue;
mod mark;
mod model;
mod pending;

pub use enqueue::enqueue;
pub use mark::{mark_delivered, mark_failed};
pub use model::{backoff, Effect, EffectStatus, DEFAULT_MAX_ATTEMPTS};
pub use pending::{dead_lettered, due, pending};

/// The outbox table within a workspace namespace. One place owns the name so every verb agrees.
pub(crate) const TABLE: &str = "outbox";
