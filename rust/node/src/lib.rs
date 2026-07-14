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
