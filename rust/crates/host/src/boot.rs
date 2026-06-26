//! Boot a node: open the embedded store + bus, build the runtime engine, and hold the MCP
//! registry. This is the assembled spine the rest of the host (and the `node` binary) drive.

use lb_bus::{Bus, BusError};
use lb_mcp::Registry;
use lb_runtime::{Engine, RuntimeError};
use lb_store::{Store, StoreError};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum NodeError {
    #[error("store boot failed: {0}")]
    Store(#[from] StoreError),
    #[error("bus boot failed: {0}")]
    Bus(#[from] BusError),
    #[error("runtime boot failed: {0}")]
    Runtime(#[from] RuntimeError),
}

/// A booted node: the embedded store + bus + runtime engine + the registry of hosted tools.
/// One per process; the `node` binary owns it for the process lifetime.
pub struct Node {
    pub store: Store,
    pub bus: Bus,
    pub engine: Engine,
    pub registry: Registry,
}

impl Node {
    /// Boot a solo node with an in-memory store and an embedded Zenoh peer (S1). Engine and
    /// store backends are config later (symmetric nodes); S1 uses the minimal profile.
    pub async fn boot() -> Result<Self, NodeError> {
        let store = Store::memory().await?;
        let bus = Bus::peer().await?;
        let engine = Engine::new()?;
        Ok(Self {
            store,
            bus,
            engine,
            registry: Registry::new(),
        })
    }
}
