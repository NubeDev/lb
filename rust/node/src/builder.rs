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
//!
//! **Boot bring-up is per-tier, and the two halves sit in different places for that same reason:**
//! wasm (`load_enabled`) loads components into the runtime and is key-independent, so it runs early;
//! native (`spawn_enabled`) spawns children whose tokens are minted with the node's key, so it runs
//! down in the roles block, AFTER the gateway installed that key. Both consume the one `reconcile`
//! plan. Calling only the wasm half is what left every published native extension dead after a
//! restart, silently (issue #64).

use std::sync::Arc;

use lb_host::{load_enabled, AgentServer, Node};
use lb_role_gateway::Gateway;

use crate::config::{BootConfig, CredentialMode, GatewayMode};

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
        crate::reactors::spawn(&node, &ws, &cfg.outbox_providers).await;
    }

    // GATEWAY + ROLES block. Mount the native roles + agent AFTER the gateway installs its signing key
    // (the load-bearing ordering — see the module doc). With the gateway off, the node's boot key stays
    // and the roles mount headless.
    let gateway = match &cfg.gateway {
        GatewayMode::Addr(addr) => {
            // A LIVE clock: `Gateway::new_live` reads wall time per request and installs its key onto
            // the node. Do NOT call `Gateway::boot()` here — that would open a second store handle.
            let mut gw = Gateway::new_live(node.clone(), cfg.signing_key.clone());
            // Select the credential check `POST /login` runs (embedder-credential-mode scope).
            // `new_live` hardwired `DevTrustAny` (password-less); apply the config's choice through
            // the existing `with_credential_check` builder so an embedded node can enforce real
            // passwords (`PasswordHash` → argon2, wrong/absent secret 401s). This is the ONLY place
            // an embedded node's login check is selected; the mode came down from `BootConfig`
            // (from_env at the binary, or the embedder's explicit choice), never re-read from env.
            let check: Arc<dyn lb_role_gateway::CredentialCheck> = match cfg.credential_mode {
                CredentialMode::DevTrustAny => Arc::new(lb_role_gateway::DevTrustAny),
                CredentialMode::PasswordHash => Arc::new(lb_role_gateway::PasswordHash),
            };
            gw = gw.with_credential_check(check);
            // Pin the `POST /extensions` upload ceiling from config (extension-upload-limit fix). The
            // route-scoped body limit is sized from this; the binary fills it via `from_env`
            // (`LB_MAX_EXTENSION_UPLOAD_BYTES`, default 384 MiB), an embedder sets it directly.
            gw = gw.with_max_extension_upload_bytes(cfg.max_extension_upload_bytes);
            // Relocate the extension-UI serve dir when the embedder set one (`Some` ⇒ pin it via the
            // builder); `None` leaves the gateway's own `LB_EXT_UI_DIR`/"extensions-ui" default in place,
            // so the standalone binary is untouched (ext-UI-dir embed seam).
            if let Some(dir) = cfg.ext_ui_dir.as_deref().filter(|d| !d.is_empty()) {
                gw = gw.with_ext_ui_dir(dir);
            }
            // Serve a static web app at `/` when the embedder set a static root (static-root scope);
            // `None`/empty leaves the router with no fallback (unmatched paths 404, unchanged).
            if let Some(dir) = cfg.static_root.as_deref().filter(|d| !d.is_empty()) {
                gw = gw.with_static_root(dir);
            }
            Some((gw, *addr))
        }
        GatewayMode::Off => None,
    };

    // Now there is ONE signing identity (the gateway installed it, or the node's boot key stands).
    // Mount the native sidecar roles + the in-house agent HERE so a served run's tool callbacks verify.
    crate::federation::mount(node.clone()).await;
    crate::control_engine::mount(node.clone()).await;

    // BOOT BRING-UP, native half: respawn every previously-published-and-enabled NATIVE extension, so
    // a published sidecar survives a restart exactly as a wasm one does (issue #64 — the reconcile
    // plan's native actions were computed and then dropped, leaving the child dead and the boot log
    // silent). The node owns the `Launcher`, which is why this half lives here and not in the verb.
    //
    // Placed HERE, after the gateway block, for the SAME load-bearing reason the role mounts are (see
    // the module doc): `install_native` mints each child's `LB_EXT_TOKEN` with `node.key()`, and the
    // gateway verifies those callback tokens with its own key. Respawning up beside `load_enabled`
    // would mint every sidecar's token with the pre-gateway key and 401 each callback.
    spawn_native_enabled(&node, &ws).await;
    let agent_server =
        crate::agent::mount(node.clone(), &cfg.agent_model, cfg.agent_caps.clone()).await;

    Ok(RunningNode {
        node,
        gateway,
        agent_server,
    })
}

/// Respawn the workspace's enabled native extensions and LOG every one — including the ones that did
/// not come back.
///
/// **Log-and-continue, deliberately** (the open question issue #64 raised). Hard-failing boot on an
/// extension that cannot respawn would turn one broken extension into a node that will not start —
/// and the recovery path for a bad extension (publish a fix, disable it) runs *through the node it
/// just killed, over the gateway that never came up*. That trades a degraded node for an unbootable
/// one and can strand a fleet on an unattended box, which is strictly worse than the failure it
/// guards. Best-effort also matches every neighbouring boot step (`load_enabled`, the seeds, the role
/// mounts).
///
/// What made #64 expensive was never the continuing — it was the SILENCE. So the cure is here: an
/// extension that should be running and is not says so on stderr, every boot, by name and reason. An
/// operator who wants "no degraded boots" can build that on this output; an operator whose node is
/// one broken extension away from unreachable cannot un-build a panic.
async fn spawn_native_enabled(node: &lb_host::Node, ws: &str) {
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    match lb_host::spawn_enabled(node, &lb_supervisor::OsLauncher, ws, ts).await {
        Ok(spawned) => {
            for e in &spawned {
                if e.spawned {
                    println!("boot-spawned native extension: {}@{}", e.ext, e.version);
                } else if e.reason != "disabled" && e.reason != "already-running" {
                    // The load-bearing line: an ENABLED native extension boot did not bring back.
                    //
                    // It states what this verb KNOWS (the durable intent said run; boot did not
                    // start it, for this reason) and NOT "it is not running" — an embedder may mount
                    // its own sidecars directly after `boot_full` returns, so a later mount can make
                    // this extension live and that is not a fault. Over-claiming here would send an
                    // operator hunting a healthy extension, which is the opposite of the point.
                    eprintln!(
                        "boot: native extension {}@{} not started by boot bring-up ({}) — it is \
                         installed and enabled; if nothing else starts it, it is not running",
                        e.ext, e.version, e.reason
                    );
                }
            }
        }
        Err(e) => eprintln!("boot native extension respawn for ws={ws} failed: {e}"),
    }
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
