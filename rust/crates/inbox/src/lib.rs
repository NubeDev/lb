//! The generic inbox — one normalized item shape, persisted as state (README §6.10,
//! inbox-outbox scope).
//!
//! Every source (a chat message, a job result, a system notice) collapses to one [`Item`]
//! addressed by `(channel, id)` within a workspace. Items are *state*: they live in the
//! store (`lb_store`), behind the workspace wall, so a channel view / unread count / triage
//! flow is uniform across sources. Motion (the live "it appeared" push) is the bus's job
//! (§3.3) — the inbox is the durable backstop, so history survives a restart.
//!
//! Verbs (one per file): [`record`] persists an item; [`list`] reads a channel's items.
//! Authorization is *not* here — these are raw verbs run after `caps::check`; the host's
//! channel service is the capability chokepoint (capability-first, §3.5).

mod approved;
mod delete;
mod get;
mod item;
mod list;
mod record;
mod rejected;
mod resolution;

pub use approved::approved;
pub use delete::delete;
pub use get::get;
pub use item::Item;
pub use list::list;
pub use record::{record, record_id, TABLE};
pub use rejected::rejected;
pub use resolution::{resolution, resolve, Decision, Resolution, RESOLUTION_TABLE};
