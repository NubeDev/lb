//! Boot a node: open the embedded store + bus, build the runtime engine, and hold the MCP
//! registry. This is the assembled spine the rest of the host (and the `node` binary) drive.

use std::sync::Arc;

use lb_bus::{Bus, BusError};
use lb_mcp::Registry;
use lb_runtime::{Engine, RuntimeError};
use lb_store::{Store, StoreError};
use thiserror::Error;

use crate::apikey::ApiKeyCache;
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
    /// The API-key verification cache (api-keys scope) — a small hash→`Principal` cache the auth
    /// path reads and `revoke`/`rotate` bust. Shared so the gateway auth path and the management
    /// verbs see one cache; a revoke bites on this node's next request (instant local revoke).
    pub apikeys: Arc<ApiKeyCache>,
    pub role: Role,
}

impl Node {
    /// Boot a **solo** node with an in-memory store and an embedded Zenoh peer (S1 posture).
    pub async fn boot() -> Result<Self, NodeError> {
        Self::boot_as(Role::Solo).await
    }

    /// Boot a solo node over a CALLER-SUPPLIED store (a persistent one for a restart test, or a
    /// shared in-memory namespace). Same wiring as [`boot`] — only the store differs, which is a
    /// config choice (§3.1), never a code branch. Used to prove durable resume across a "restart"
    /// (re-open the same on-disk store, build a fresh node over it).
    pub async fn boot_with_store(store: Store) -> Result<Self, NodeError> {
        let bus = Bus::peer().await?;
        let engine = Engine::new()?;
        Ok(Self {
            store,
            bus,
            engine,
            registry: Arc::new(Registry::new()),
            sidecars: Arc::new(SidecarMap::new()),
            apikeys: Arc::new(ApiKeyCache::new()),
            role: Role::Solo,
        })
    }

    /// Boot a node in `role`. Same code, same crates — the role only selects what the wiring
    /// layers mount (sync relay, gateway) and the data-authority axis (README §6.8). Every role
    /// opens the same store + Zenoh peer; the second node in S3 is just a second `boot_as`.
    pub async fn boot_as(role: Role) -> Result<Self, NodeError> {
        let store = Self::open_store().await?;
        let bus = Bus::peer().await?;
        let engine = Engine::new()?;
        Ok(Self {
            store,
            bus,
            engine,
            registry: Arc::new(Registry::new()),
            sidecars: Arc::new(SidecarMap::new()),
            apikeys: Arc::new(ApiKeyCache::new()),
            role,
        })
    }

    /// Select the store engine by **config, not role** (symmetric nodes, §3.1): `LB_STORE_PATH`
    /// set → a persistent on-disk store (`Store::open`, durable across restart); unset → an
    /// ephemeral in-memory store (`Store::memory`, dev/test). This is the thin boot-wiring layer
    /// §3.1 permits to read config — no core path branches on it, and there is no `if cloud`.
    async fn open_store() -> Result<Store, StoreError> {
        match std::env::var("LB_STORE_PATH") {
            Ok(path) if !path.is_empty() => Store::open(&path).await,
            _ => Store::memory().await,
        }
    }
}
