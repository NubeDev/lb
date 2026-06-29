//! The channel service error. Like the MCP `Denied`, a `Denied` here carries no detail about
//! which gate failed — a caller without access learns nothing about the channel (§3.5).

use lb_bus::BusError;
use lb_store::StoreError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ChannelError {
    /// Authorization failed (workspace isolation or missing capability). Opaque by design.
    #[error("denied")]
    Denied,
    /// The caller is authorized and is the owner, but no item lives at the requested id in this
    /// workspace. Surfaced (not collapsed to `Denied`) only AFTER the ownership check passes, so a
    /// non-author caller still learns nothing — their miss resolves to `Denied` first.
    #[error("not found")]
    NotFound,
    /// The durable store rejected the operation.
    #[error("store error: {0}")]
    Store(#[from] StoreError),
    /// The bus rejected the publish/subscribe.
    #[error("bus error: {0}")]
    Bus(#[from] BusError),
}
