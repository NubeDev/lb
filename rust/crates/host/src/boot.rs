//! Boot a node: open the embedded store + bus, build the runtime engine, and hold the MCP
//! registry. This is the assembled spine the rest of the host (and the `node` binary) drive.

use std::sync::{Arc, Mutex};

use dashmap::DashMap;
use lb_auth::SigningKey;
use lb_bus::{Bus, BusError};
use lb_mcp::Registry;
use lb_runtime::{Engine, RuntimeError};
use lb_store::{Store, StoreError};
use thiserror::Error;

use crate::agent::{ErasedModel, ModelBuilder, RuntimeRegistry, UnconfiguredModel};
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
    /// The live **rule-run registry** (long-running-rules-scope) — `(ws, run_id)` → the run's
    /// cooperative [`RunControl`](lb_rules::RunControl). Runtime-only motion (the durable truth is
    /// the `job:{id}` record), shared like `sidecars` so the `rules.runs.*` control verbs and the
    /// worker see one source of truth.
    pub rule_runs: Arc<crate::rules::RuleRunMap>,
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
    /// The **per-workspace model cache** (active-agent-wiring scope, Slice 2): `(ws, endpoint-hash) →`
    /// the built [`ErasedModel`], so rules/loop don't rebuild an `AiGateway` per call. Lock-free reads
    /// (the hot path). Invalidated per-ws on `agent.config.set` ([`invalidate_workspace_model`]) so a
    /// rotated key / changed pick can't answer with the stale model. `ws` is part of the key → the wall
    /// holds (ws-B never reads ws-A's entry). Runtime-only — the durable truth is `agent.config`.
    workspace_models: DashMap<(String, u64), Arc<dyn ErasedModel>>,
    /// The installed **model-builder seam** (active-agent-wiring scope, Slice 2). `lb-host` never names
    /// a concrete provider (rule 1 — no build-dep on a role crate); the `node` binary installs a builder
    /// that turns a resolved endpoint + key into an `AiGateway<Provider>`. `None` until installed (a
    /// minimal/test node with no builder resolves the node-fallback/unconfigured path — honest).
    model_builder: Mutex<Option<Arc<dyn ModelBuilder>>>,
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
    /// Where a native sidecar POSTs its host callbacks (`{url}/mcp/call`) — this node's own gateway
    /// address, installed once at boot by the layer that knows it ([`install_gateway_url`]). `None`
    /// on a headless node (no gateway to call back to), and the child is spawned without a callback
    /// address exactly as before.
    ///
    /// It lives on the `Node` for the same reason the signing `key` does: it is one fact about THIS
    /// node that the spawn path needs long after the boot layer that knew it has returned, and both
    /// the gateway and the native token minter must agree on it. Reading it from a process-global
    /// `LB_GATEWAY_URL` instead made the child's callback address depend on whether some *other*
    /// component happened to have set that var before the spawn ran — which broke the moment a second
    /// spawn path (boot bring-up) ran earlier than the one that set it.
    gateway_url: Mutex<Option<String>>,
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
            rule_runs: Arc::new(crate::rules::RuleRunMap::default()),
            apikeys: Arc::new(ApiKeyCache::new()),
            runtimes: Mutex::new(Arc::new(default_runtimes())),
            workspace_models: DashMap::new(),
            model_builder: Mutex::new(None),
            key: Mutex::new(Arc::new(SigningKey::generate())),
            gateway_url: Mutex::new(None),
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
            rule_runs: Arc::new(crate::rules::RuleRunMap::default()),
            apikeys: Arc::new(ApiKeyCache::new()),
            runtimes: Mutex::new(Arc::new(default_runtimes())),
            workspace_models: DashMap::new(),
            model_builder: Mutex::new(None),
            key: Mutex::new(Arc::new(SigningKey::generate())),
            gateway_url: Mutex::new(None),
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
            rule_runs: Arc::new(crate::rules::RuleRunMap::default()),
            apikeys: Arc::new(ApiKeyCache::new()),
            runtimes: Mutex::new(Arc::new(default_runtimes())),
            workspace_models: DashMap::new(),
            model_builder: Mutex::new(None),
            key: Mutex::new(Arc::new(SigningKey::generate())),
            gateway_url: Mutex::new(None),
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

    /// A cached per-workspace model, if one is memoized under `key` (active-agent-wiring #2). Lock-free
    /// read — the hot path for rules/loop. The key is `(ws, endpoint-hash)`; see `resolve_model.rs`.
    pub fn workspace_model_cached(&self, key: &(String, u64)) -> Option<Arc<dyn ErasedModel>> {
        self.workspace_models.get(key).map(|m| m.clone())
    }

    /// Memoize `model` under `key` for future resolves (active-agent-wiring #2). Called by
    /// `resolve_workspace_model` after it builds a fresh adapter.
    pub fn workspace_model_insert(&self, key: (String, u64), model: Arc<dyn ErasedModel>) {
        self.workspace_models.insert(key, model);
    }

    /// Invalidate **every** cached model for `ws` (active-agent-wiring #2). Called by `agent.config.set`
    /// so a rotated key / changed pick can never answer with a stale model. Drops all endpoint-hash
    /// variants for the workspace (a re-pick may change the endpoint; all prior entries are now stale).
    pub fn invalidate_workspace_model(&self, ws: &str) {
        self.workspace_models
            .retain(|(cached_ws, _), _| cached_ws != ws);
    }

    /// The installed model-builder seam (active-agent-wiring #2), if the binary installed one. Clones the
    /// inner `Arc` out under a brief lock. `None` on a node with no builder (minimal/test).
    pub fn model_builder(&self) -> Option<Arc<dyn ModelBuilder>> {
        self.model_builder
            .lock()
            .expect("model builder lock")
            .clone()
    }

    /// Install the model-builder seam (active-agent-wiring #2). Called ONCE by the `node` binary with a
    /// builder that constructs `AiGateway<Provider>` — host never names the provider (rule 1). Idempotent
    /// replace; a re-install invalidates nothing (the cache keys by endpoint, and picks bust per-ws).
    pub fn install_model_builder(&self, builder: Arc<dyn ModelBuilder>) {
        *self.model_builder.lock().expect("model builder lock") = Some(builder);
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

    /// Install this node's own gateway URL — where a native sidecar POSTs its host callbacks. Called
    /// ONCE at boot by the layer that knows the address, beside [`install_key`](Node::install_key)
    /// (same lifecycle, same reason: one fact the spawn path needs after that layer has returned).
    ///
    /// Leave it unset on a headless node: children then spawn with no callback address, and their
    /// callback client fails cleanly — the pre-existing behaviour for a sidecar that never calls back.
    pub fn install_gateway_url(&self, url: impl Into<String>) {
        *self.gateway_url.lock().expect("node gateway url lock") = Some(url.into());
    }

    /// This node's gateway URL for child callbacks, if one was installed.
    pub fn gateway_url(&self) -> Option<String> {
        self.gateway_url
            .lock()
            .expect("node gateway url lock")
            .clone()
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
