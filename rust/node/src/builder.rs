//! [`boot_full`] + [`RunningNode`] — the supported embed API's boot seam (node-roles / embed scope).
//!
//! [`boot_full`] performs the whole boot ritual ONCE, from a [`BootConfig`](crate::BootConfig): boot
//! the spine over the config's store/key, apply telemetry, run the `hello` demo (gated), re-load
//! enabled extensions, seed, spawn reactors (gated), then the gateway/roles block. The `node` binary
//! and any third-party embedder both call it — the binary's `main.rs` is `boot_full(from_env()).await`
//! plus `serve()`.
//!
//! **Load-bearing ordering (preserved EXACTLY from the old `main.rs`):** the native sidecar roles
//! (`federation`, `control-engine`) and the in-house `agent` are mounted AFTER the gateway installs its
//! signing key onto the node (`Gateway::new_live` → `node.install_key`). `install_native` mints each
//! sidecar's `LB_EXT_TOKEN` with `node.key()`, and the gateway verifies those callback tokens with its
//! own key — mounting before the gateway installed that key meant every callback 401'd. With the
//! gateway OFF, the node's boot key stays and the roles mount headless (their callbacks degrade
//! cleanly).

use std::sync::Arc;

use lb_host::{load_enabled, AgentServer, Node};
use lb_role_gateway::Gateway;

use crate::config::{BootConfig, GatewayMode};

/// A booted node, ready to serve. Hands back the [`Node`] (store / bus / host verbs — the embedder
/// calls verbs in-process), and — when the gateway is on — the [`Gateway`] value + bind address to
/// serve, plus the live [`AgentServer`] kept alive for the node's lifetime (dropping it stops serving).
///
/// With the gateway OFF, `gateway` is `None` and [`serve`](RunningNode::serve) returns immediately; the
/// embedder drives the node purely in-process. Teardown beyond process exit (reactor cancellation,
/// sidecar shutdown token) is a documented follow-up — `RunningNode` deliberately owns the pieces so a
/// `shutdown()` can be added additively without changing this shape.
pub struct RunningNode {
    /// The booted node: store, bus, registry, host verbs. Call `lb_host::*` verbs in-process against it.
    pub node: Arc<Node>,
    /// The gateway value + bind address, when [`GatewayMode::Addr`] was configured. `None` = headless.
    pub gateway: Option<(Gateway, std::net::SocketAddr)>,
    /// The served in-house agent, kept alive here (dropping it stops serving routed `agent.invoke`).
    pub agent_server: Option<AgentServer>,
}

impl RunningNode {
    /// Serve the gateway, blocking until it stops (never, in normal operation). A no-op that returns
    /// `Ok(())` immediately when the gateway is off — the embedder is driving the node in-process. The
    /// `agent_server` is held for the duration so routed invocations keep serving.
    pub async fn serve(self) -> anyhow::Result<()> {
        let RunningNode {
            gateway,
            agent_server,
            ..
        } = self;
        if let Some((gw, addr)) = gateway {
            println!("gateway: serving on http://{addr}");
            lb_role_gateway::serve(gw, addr).await?;
        }
        // Hold the agent server alive until serve returns (or forever, headless would drop here).
        drop(agent_server);
        Ok(())
    }
}

/// Perform the full boot ritual from `cfg` and return a [`RunningNode`]. This is the ONE copy of the
/// ritual; the binary and every embedder call it. No env is read here below the seam — everything
/// comes from `cfg` (the exceptions are the role mounts `federation`/`control_engine`, which still read
/// their own `LB_FEDERATION_*` / `LB_CONTROL_ENGINE_*` env; that de-env'ing is an explicit documented
/// follow-up — the core ritual store/key/workspace/seeds/reactors/gateway is fully struct-config).
pub async fn boot_full(cfg: BootConfig) -> anyhow::Result<RunningNode> {
    // Boot the spine over the config's store. `Arc` so the reactors + role mounts share it.
    let node = Arc::new(Node::boot_with_store(open_store(&cfg).await?).await?);

    // TELEMETRY sink selection: choose the tracing layers by config, right after boot so every
    // subsequent instrumented call is captured. Shares the node's OWN store + bus handles.
    lb_telemetry::sink_layers(node.store.clone(), node.bus.clone(), cfg.telemetry.clone());

    // S1 hello demo bring-up (gated): load the `hello` extension and call `hello.echo` once. The binary
    // runs it; an embedder wants it off.
    if cfg.hello_demo {
        if let Err(e) = crate::hello_demo::run(&node).await {
            eprintln!("boot: hello demo failed: {e}");
        }
    }

    // BOOT BRING-UP: re-load every previously-published-and-enabled wasm extension for the configured
    // workspace from the durable cache, so an upload survives a restart. A no-op on a fresh store.
    let ws = cfg.workspace.clone();
    match load_enabled(&node, &ws).await {
        Ok(loaded_exts) => {
            for e in loaded_exts.iter().filter(|e| e.loaded) {
                println!(
                    "boot-loaded extension: {}@{} ({})",
                    e.ext, e.version, e.reason
                );
            }
        }
        Err(e) => eprintln!("boot extension load for ws={ws} failed: {e}"),
    }

    // SEEDS: dev identity, core skills, agent definitions, personas, active-persona migration, default
    // core-skill grants. All idempotent + best-effort.
    crate::seeds::run(&node, &cfg).await;

    // REACTORS (gated): flow / agent / approval / insight-digest scans + the one-shot insight-ts heal.
    if cfg.reactors {
        crate::reactors::spawn(&node, &ws).await;
    }

    // GATEWAY + ROLES block. Mount the native roles + agent AFTER the gateway installs its signing key
    // (the load-bearing ordering — see the module doc). With the gateway off, the node's boot key stays
    // and the roles mount headless.
    let gateway = match &cfg.gateway {
        GatewayMode::Addr(addr) => {
            // A LIVE clock: `Gateway::new_live` reads wall time per request and installs its key onto
            // the node. Do NOT call `Gateway::boot()` here — that would open a second store handle.
            let gw = Gateway::new_live(node.clone(), cfg.signing_key.clone());
            Some((gw, *addr))
        }
        GatewayMode::Off => None,
    };

    // Now there is ONE signing identity (the gateway installed it, or the node's boot key stands).
    // Mount the native sidecar roles + the in-house agent HERE so a served run's tool callbacks verify.
    crate::federation::mount(node.clone()).await;
    crate::control_engine::mount(node.clone()).await;
    let agent_server =
        crate::agent::mount(node.clone(), &cfg.agent_model, cfg.agent_caps.clone()).await;

    Ok(RunningNode {
        node,
        gateway,
        agent_server,
    })
}

/// Open the store the boot config selects: `store_path: Some(non-empty)` ⇒ a durable on-disk store;
/// `None`/empty ⇒ an ephemeral `mem://` store. This is the ONE place the store path (today's
/// `LB_STORE_PATH`, filled into `cfg` at the binary boundary) turns into a `Store` — no library code
/// below reads the env. Mirrors `Node::open_store`'s config-not-role selection, but sourced from the
/// struct so an embedder controls it directly.
async fn open_store(cfg: &BootConfig) -> anyhow::Result<lb_store::Store> {
    let store = match cfg.store_path.as_deref() {
        Some(path) if !path.is_empty() => lb_store::Store::open(path).await?,
        _ => lb_store::Store::memory().await?,
    };
    Ok(store)
}
