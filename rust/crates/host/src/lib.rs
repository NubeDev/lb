//! The host — the kernel that wires the spine together (core scope, README §4).
//!
//! Boots the embedded store (SurrealDB) and bus (Zenoh peer), builds the runtime engine,
//! loads extensions through the loader + runtime, and registers their tools in the MCP
//! registry. Role selection is config (symmetric nodes, §3.1) — the host itself has no
//! `if cloud`; the `node` binary picks which role crates to mount.
//!
//! What the host exposes is the *spine*: a [`Node`] holding store + bus + the MCP registry,
//! and `load_extension` to bring a component online. Tool calls go through `lb_mcp::call`.

mod assets;
mod boot;
mod channel;
mod install;
mod installed;
mod load;
mod reload;
mod remote;
mod role;
mod serve;
mod sync;

pub use assets::{
    add_member, call_asset_tool, get_doc, grant_skill, link_doc, list_docs, load_skill, put_doc,
    put_skill, revoke_skill, share_doc, AssetError,
};
pub use boot::{Node, NodeError};
pub use channel::{
    history, join, post, subscribe_channel, watch, ChannelError, ChannelPresence, ChannelSub,
    PresenceFeed,
};
pub use install::install_extension;
pub use installed::installed;
pub use load::{load_extension, LoadError, Loaded};
pub use reload::reload_extension;
pub use remote::register_remote_extension;
pub use role::Role;
pub use serve::{serve_ext, ToolServer};
pub use sync::{replay_history, sync_channel, ChannelSync};
