//! Boot a node: open the embedded store + bus, build the runtime engine, and hold the MCP
//! registry. This is the assembled spine the rest of the host (and the `node` binary) drive.

use std::sync::{Arc, Mutex};

use lb_auth::SigningKey;
use lb_bus::{Bus, BusError};
use lb_mcp::Registry;
use lb_runtime::{Engine, RuntimeError};
use lb_store::{Store, StoreError};
use thiserror::Error;

use crate::agent::{RuntimeRegistry, UnconfiguredModel};
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
    /// The **agent runtime registry** (external-agent runtime-seam #1) — part of the node spine so
    /// any host service that starts a run reaches ONE source of truth: the routed `serve_agent`, and
    /// the in-channel agent worker (`channel/agent_worker.rs`). Boot installs a default-only registry
    /// (the in-house `default` over the [`UnconfiguredModel`] placeholder); the `node` binary, when
    /// built with the `external-agent` feature, swaps in a registry that also holds the external
    /// `AcpRuntime` entries via [`install_runtimes`](Node::install_runtimes). Held behind a `Mutex`
    /// so the binary can install after boot; readers [`clone`](Node::runtimes) the inner `Arc` out
    /// and never hold the lock across the (long) run.
    runtimes: Mutex<Arc<RuntimeRegistry>>,
    /// The node's **token-signing key** — the one identity root the whole node trusts. It mints and
    /// verifies session tokens (the gateway's `login`/`authenticate` read it) AND the scoped
    /// `LB_EXT_TOKEN` a native sidecar carries when it calls back through `POST /mcp/call`
    /// (native-callback-transport scope). Before this it lived only on the `Gateway`, so a child
    /// token had to be minted with a throwaway key no one could verify (the co-trust hack the
    /// native-tier scope flagged deferred); now the minter (`native/spec.rs`) and the verifier
    /// (gateway `authenticate`) share THIS key, so a child token is a genuine, verifiable JWT.
    /// Behind a `Mutex<Arc>` (like `runtimes`) so a gateway/test can install a shared key after boot
    /// via [`install_key`](Node::install_key) — the key never leaves the node (scope: "Secrets").
    key: Mutex<Arc<SigningKey>>,
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
            runtimes: Mutex::new(Arc::new(default_runtimes())),
            key: Mutex::new(Arc::new(SigningKey::generate())),
            role: Role::Solo,
        })
    }

    /// Boot a node over a CALLER-SUPPLIED `bus` and `role`, with a fresh in-memory store. Same wiring
    /// as [`boot_as`](Node::boot_as) — only the bus is injected, so a test can point-to-point link a
    /// hub and an edge on one explicit bus (the routed-call tests). A config choice (§3.1), not a code
    /// branch. Keeps `Node`'s spine fields encapsulated (the runtime registry is installed here, not
    /// hand-assembled at each call site).
    pub async fn boot_on_bus(bus: Bus, role: Role) -> Result<Self, NodeError> {
        let store = Store::memory().await?;
        let engine = Engine::new()?;
        Ok(Self {
            store,
            bus,
            engine,
            registry: Arc::new(Registry::new()),
            sidecars: Arc::new(SidecarMap::new()),
            apikeys: Arc::new(ApiKeyCache::new()),
            runtimes: Mutex::new(Arc::new(default_runtimes())),
            key: Mutex::new(Arc::new(SigningKey::generate())),
            role,
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
            runtimes: Mutex::new(Arc::new(default_runtimes())),
            key: Mutex::new(Arc::new(SigningKey::generate())),
            role,
        })
    }

    /// The node's agent runtime registry (external-agent #1). Clones the inner `Arc` out under a brief
    /// lock so a caller can drive a (long) run without holding the lock — see [`runtimes`](Node::runtimes-field).
    pub fn runtimes(&self) -> Arc<RuntimeRegistry> {
        self.runtimes.lock().expect("runtimes lock").clone()
    }

    /// Install a runtime registry, replacing the default-only one. Called ONCE by the `node` binary
    /// after boot when the `external-agent` feature is on, to add the external `AcpRuntime` entries.
    /// (A feature-off node never calls this and keeps the default-only registry.)
    pub fn install_runtimes(&self, registry: RuntimeRegistry) {
        *self.runtimes.lock().expect("runtimes lock") = Arc::new(registry);
    }

    /// The node's token-signing key. The gateway mints/verifies session tokens with it, and the
    /// native tier mints a sidecar's `LB_EXT_TOKEN` with it (so the gateway can verify that token on
    /// the callback — native-callback-transport scope). Clones the inner `Arc` out under a brief lock.
    pub fn key(&self) -> Arc<SigningKey> {
        self.key.lock().expect("node key lock").clone()
    }

    /// Install the node's signing key, replacing the one generated at boot. Called ONCE when a
    /// gateway fronts this node with an explicit key (so mint and verify agree on ONE key across the
    /// gateway's `login`/`authenticate` AND the native token minter). Tests use it to front a node
    /// with a known key so a minted child token verifies. The key never leaves the node.
    pub fn install_key(&self, key: SigningKey) {
        *self.key.lock().expect("node key lock") = Arc::new(key);
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

/// The boot-time runtime registry: the in-house `default` only, over the [`UnconfiguredModel`]
/// placeholder (no model provider wired at this layer yet — the `agent_invoke`-needs-a-provider gap).
/// A feature-on `node` binary replaces this with one that also carries the external runtimes via
/// [`Node::install_runtimes`]; the resolve invariant (absent → default) holds either way.
fn default_runtimes() -> RuntimeRegistry {
    RuntimeRegistry::with_default(Arc::new(UnconfiguredModel))
}
