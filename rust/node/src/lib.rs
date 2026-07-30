//! `lb-node` — the node package's **library** target: the supported embed API (node-roles / embed
//! scope). It exposes [`BootConfig`] (struct config filled at the binary boundary) and
//! [`boot_full`] / [`RunningNode`], which perform the whole boot ritual ONCE. The `node` binary
//! (`main.rs`) and any third-party embedder (`NubeIO/rubix-ai`, git-dep on `NubeDev/lb`) both call this
//! seam — the binary's `main.rs` shrinks to `boot_full(BootConfig::from_env()).await` + serve/signal.
//!
//! This package is the sanctioned **thin role-aware layer** (§3.1): no core crate under `rust/crates/*`
//! is role-aware; role selection (gateway / federation / control-engine / external-agent) lives here.
//! Env is a *binary* concern — [`BootConfig::from_env`] is the ONE place `LB_*` boot vars are read, and
//! only binaries call it. Below the seam, everything comes from the struct (the federation /
//! control-engine role mounts still read their own `LB_FEDERATION_*` / `LB_CONTROL_ENGINE_*` env — a
//! documented de-env follow-up; the core ritual is fully struct-config).

// The boot seam.
pub mod builder;
pub mod config;

// The ritual's verbs (folder-of-verbs per FILE-LAYOUT). `pub` so an advanced embedder can compose a
// custom ritual, but the supported entry point is `boot_full`.
pub mod hello_demo;
pub mod mail;
pub mod reactors;
pub mod seed_identity;
pub mod seeds;

// The thin role-aware mounts (§3.1) — the binary's role wiring, reused by the builder.
pub mod agent;
pub mod control_engine;
pub mod external_agent;
pub mod federation;

pub use builder::{boot_full, RunningNode};
pub use config::{
    AgentModelConfig, BootConfig, CredentialMode, GatewayMode, OutboxProviders,
    DEFAULT_MAX_EXTENSION_UPLOAD_BYTES,
};

// Re-exports so a third-party embedder needs only the `lb-node` dep to fill a [`BootConfig`] and drive
// the node — no direct dep on the internal `lb-auth`/`lb-host` crates. `SigningKey` fills
// `BootConfig::signing_key` (custody at the binary boundary); `Node` is what `RunningNode::node` hands
// back for in-process host-verb calls.
pub use lb_auth::SigningKey;
pub use lb_host::Node;
// `BrowserSessionConfig` fills `BootConfig::browser_session`. It lives in `lb-role-gateway`, but an
// embedder must be able to NAME it with only the `lb-node` dep — same reason `SigningKey` and `Node`
// are re-exported above. Without this, opting into the `/api/*` seam forces a direct dep on an
// internal role crate purely to spell one field's type, which is the leak this block exists to close.
/// The optional response-cache config an embedder sets on [`BootConfig::cache`] (response-cache
/// scope). Re-exported so a downstream host names it with only the `lb-node` dep (the
/// `BrowserSessionConfig` precedent). Always available; the LIVE cache is `page-cache`-gated.
pub use lb_host::CacheConfig;
pub use lb_role_gateway::BrowserSessionConfig;

// ---- The embedder seam ------------------------------------------------------------------------
//
// Everything below exists for the same reason `SigningKey`/`Node`/`BrowserSessionConfig` do: a host
// binary that deps ONLY on `lb-node` must be able to NAME the types it needs. Without these, an
// embedder wanting to plug in an outbox target, enqueue an effect, or store an asset had to add
// direct git-deps on `lb-host` and `lb-auth` pinned in lockstep with this crate — which is not a
// supported configuration, it is just a leak that happened to compile.
//
// These re-export the GENERIC seams only. Nothing here names a product, a workspace or an extension.

/// The in-process caller identity every host verb takes. An embedder building an outbox target or
/// calling a verb off the request path needs to construct one.
pub use lb_auth::Principal;
/// The workspace-scoped durable store — `RunningNode::node.store`. Re-exported so an embedder can
/// spell it in its own function signatures rather than only pass it through.
pub use lb_host::Store;

/// Stage an effect on the outbox — what a target calls to enqueue the RESULT of its own work.
pub use lb_host::enqueue_outbox;
/// The workspace asset store — where a host puts bytes it produced (a rendered PDF) so an outbox row
/// can reference them instead of carrying them.
pub use lb_host::{get_asset, put_asset, Asset, AssetError, MAX_ASSET_BYTES};
/// The outbox delivery contract an embedder implements to register a target on
/// [`BootConfig::outbox_providers`]'s `targets` list, plus the effect it is handed and the error it
/// returns. [`DeliveryError`](lb_host::DeliveryError) converts `From<String>`/`From<&str>` into its
/// **retryable** form, so an embedder's target keeps its existing behaviour by changing only the
/// signature; it opts into "park this now, do not retry" with `DeliveryError::permanent`.
pub use lb_host::{DeliveryError, DynTarget, OutboxEffect, Target};

/// Mint a short-lived session token for a real principal — see
/// [`RunningNode::mint_service_session`], which is the ergonomic form and the one to prefer.
pub use lb_role_gateway::{mint_full_session_with_ttl, MintedSession, SESSION_TTL_SECS};

/// `NodeId` — the identity a discovery advertisement carries, shared with fleet-presence's bus
/// roster so a discovered peer correlates with a roster entry once connected.
pub use lb_bus::{NodeId, NodeIdError};
/// The LAN-discovery seam an embedder fills on [`BootConfig::discovery`] — same reason
/// `BrowserSessionConfig` is re-exported: a host that deps only on `lb-node` must be able to NAME
/// the type. [`Advertisement`] is what this node publishes; [`ServiceType`] is the
/// product-supplied DNS-SD type (lb's own default is the generic `_lb._tcp` — no core crate names
/// a product). [`browse`]/[`Browse`]/[`Discovered`]/[`DiscoveredPeer`] are the discovering half,
/// for a host that wants to FIND peers as well as be found.
///
/// This is the **bootstrap** layer only: it yields an endpoint to dial before a bus session
/// exists, then hands off to Zenoh and the fleet-presence liveliness roster, which remains
/// authoritative for workspace presence.
pub use lb_discovery::{
    advertise, browse, Advertised, Advertisement, Browse, Discovered, DiscoveredPeer,
    DiscoveryError, ServiceType, DEFAULT_SERVICE_TYPE,
};
