//! The channel service — the messaging slice's capability chokepoint (README §6.2, bus +
//! inbox scopes). A channel is a bus subject *and* a durable inbox bucket: posting persists
//! state and moves motion; reading comes from either the durable history or the live stream.
//!
//! Every verb here runs `caps::check` FIRST (capability-first, §3.5) and is workspace-scoped
//! (the hard wall, §7) before any bus or store access — there is no path to a channel that
//! skips authorization. One verb per file (FILE-LAYOUT §3).

mod agent_worker;
mod authorize;
mod chart;
mod chart_pref;
mod chart_pref_tool;
mod delete;
mod edit;
mod error;
mod history;
mod key;
mod payload;
mod post;
mod presence;
mod query_worker;
mod subscribe;

// The chart picker + kind-tagged payload helpers are crate-internal: the inline query worker
// (`query_worker.rs`) and `post.rs` are the only consumers today. Exposed `pub(crate)` (not `pub`)
// so they don't leak from the crate API until a host caller actually needs them — keeping the lib's
// public surface honest (no dead `pub use`).
#[allow(unused_imports)]
pub(crate) use chart::{pick_chart, ChartKind, ChartSeries, ChartSpec};
pub use chart_pref_tool::call_channel_chart_pref_tool;
pub use delete::{delete, watch_deletions, DeletionFeed};
pub use edit::edit;
pub use error::ChannelError;
pub use history::history;
#[allow(unused_imports)]
pub(crate) use payload::{
    encode_payload, error_body, parse_payload, result_body, ItemPayload, QueryErrorPayload,
    QueryPayload, QueryResultPayload,
};
pub use post::post;
pub use presence::{join, watch, ChannelPresence, PresenceFeed};
pub use subscribe::{subscribe_channel, ChannelSub};

// Re-export the bus-key helpers crate-internally so the sync layer publishes/subscribes on the
// EXACT same keys `post`/`subscribe_channel` use — they cannot drift (one owner, `key.rs`).
pub(crate) use key::{msg_key as msg_key_for, sub_key as sub_key_for};

// The channel capability gate, crate-internal, so the asset service can reuse it for the
// doc→channel link path (a doc linked into a channel inherits the channel's `sub` audience).
// One owner of "may this principal read this channel?" — no second copy to drift.
pub(crate) use authorize::authorize as authorize_channel;
