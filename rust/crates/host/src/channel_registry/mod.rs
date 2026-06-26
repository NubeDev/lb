//! The **channel registry** — the thin per-`(ws, channel)` record that makes channels *listable*
//! (collaboration scope, slice 2). Channels are otherwise implicit: today a channel exists only by
//! being posted to (a bus subject). A UI needs to *list* channels, so this adds a small durable
//! record — written on an explicit [`channel_create`] AND on the first [`register_on_post`] — and a
//! [`channel_list`] verb. **Additive only:** posting and history are unchanged; the registry never
//! gates them, it just records that a channel exists so it can be enumerated.
//!
//! Authorization reuses the **channel capability gate** (`bus:chan/{cid}:pub` to create, `:sub` to
//! list) — the same chokepoint `post`/`history` pass (capability-first §3.5, workspace-first §7).
//! No new capability grammar: creating a channel is exactly "may I post here", listing is "may I
//! read here". One verb per file (FILE-LAYOUT §3).

mod create;
mod list;
mod model;
mod register;

pub use create::channel_create;
pub use list::channel_list;
pub use model::ChannelRecord;
pub use register::register_on_post;
