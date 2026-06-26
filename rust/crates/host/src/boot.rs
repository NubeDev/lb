//! Boot a node: open the embedded store + bus, build the runtime engine, and hold the MCP
//! registry. This is the assembled spine the rest of the host (and the `node` binary) drive.

use std::sync::Arc;

use lb_bus::{Bus, BusError};
use lb_mcp::Registry;
use lb_runtime::{Engine, RuntimeError};
use lb_store::{Store, StoreError};
use thiserror::Error;

use crate::native::SidecarMap;
use crate::role::Role;

#[derive(Debug, Error)]
pub enum NodeError {
    #[error("store boot failed: {0}")]
    Store(#[from] StoreError),
    #[error("bus boot failed: {0}")]
    Bus(#[from] BusError),
    #[error("runtime boot failed: {0}")]
    Runtime(#[from] RuntimeError),
}

/// A booted node: the embedded store + bus + runtime engine + the registry of hosted tools,
/// plus its configured [`Role`]. One per process; the `node` binary owns it for the process
/// lifetime. The role is *config* the wiring layers read — core paths never branch on it (§3.1).
pub struct Node {
    pub store: Store,
    pub bus: Bus,
    pub engine: Engine,
    /// The MCP registry, shared (`Arc`) so the local call path, the routed serve loop, and
    /// `reload` all see one source of truth. Interior-mutable (an `RwLock` inside), so loading
    /// or reloading needs only `&Node`.
    pub registry: Arc<Registry>,
    /// The live native Tier-2 sidecars on this node (native-tier scope), keyed `(ws, ext_id)`.
    /// Runtime-only — the PID is motion; the durable truth is the `Install` + `native_status`
    /// records. Shared (`Arc`) like `registry` so the native service drives it with `&Node`.
    pub sidecars: Arc<SidecarMap>,
    pub role: Role,
}

impl Node {
    /// Boot a **solo** node with an in-memory store and an embedded Zenoh peer (S1 posture).
    pub async fn boot() -> Result<Self, NodeError> {
        Self::boot_as(Role::Solo).await
    }

    /// Boot a node in `role`. Same code, same crates — the role only selects what the wiring
    /// layers mount (sync relay, gateway) and the data-authority axis (README §6.8). Every role
    /// opens the same store + Zenoh peer; the second node in S3 is just a second `boot_as`.
    pub async fn boot_as(role: Role) -> Result<Self, NodeError> {
        let store = Store::memory().await?;
        let bus = Bus::peer().await?;
        let engine = Engine::new()?;
        Ok(Self {
            store,
            bus,
            engine,
            registry: Arc::new(Registry::new()),
            sidecars: Arc::new(SidecarMap::new()),
            role,
        })
    }
}
