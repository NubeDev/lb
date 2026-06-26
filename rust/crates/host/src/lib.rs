//! The host — the kernel that wires the spine together (core scope, README §4).
//!
//! Boots the embedded store (SurrealDB) and bus (Zenoh peer), builds the runtime engine,
//! loads extensions through the loader + runtime, and registers their tools in the MCP
//! registry. Role selection is config (symmetric nodes, §3.1) — the host itself has no
//! `if cloud`; the `node` binary picks which role crates to mount.
//!
//! What the host exposes is the *spine*: a [`Node`] holding store + bus + the MCP registry,
//! and `load_extension` to bring a component online. Tool calls go through `lb_mcp::call`.

mod boot;
mod channel;
mod load;
mod reload;

pub use boot::{Node, NodeError};
pub use channel::{
    history, join, post, subscribe_channel, watch, ChannelError, ChannelPresence, ChannelSub,
    PresenceFeed,
};
pub use load::{load_extension, LoadError, Loaded};
pub use reload::reload_extension;
