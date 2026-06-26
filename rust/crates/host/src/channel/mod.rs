//! The channel service — the messaging slice's capability chokepoint (README §6.2, bus +
//! inbox scopes). A channel is a bus subject *and* a durable inbox bucket: posting persists
//! state and moves motion; reading comes from either the durable history or the live stream.
//!
//! Every verb here runs `caps::check` FIRST (capability-first, §3.5) and is workspace-scoped
//! (the hard wall, §7) before any bus or store access — there is no path to a channel that
//! skips authorization. One verb per file (FILE-LAYOUT §3).

mod authorize;
mod error;
mod history;
mod key;
mod post;
mod presence;
mod subscribe;

pub use error::ChannelError;
pub use history::history;
pub use post::post;
pub use presence::{join, watch, ChannelPresence, PresenceFeed};
pub use subscribe::{subscribe_channel, ChannelSub};
