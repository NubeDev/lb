//! The `node` binary entry point. Selects roles by config (S1: solo) and drives the host.
//!
//! In S1 it boots a solo node, loads the `hello` extension, and calls `hello.echo` once to
//! prove the spine is live end to end. Real role selection + config + the SSE gateway arrive
//! at S3; the UI at S2. Kept to one verb (FILE-LAYOUT): everything substantive is in `lb-host`.

use std::path::PathBuf;
use std::sync::Arc;

use lb_auth::{mint, Claims, Role, SigningKey};
use lb_host::{load_enabled, load_extension, Node};

mod agent;
mod control_engine;
mod external_agent;
mod federation;
mod github;

/// Seed the dev `user` as a `workspace-admin` member of `ws`: create the global identity (idempotent),
/// write the membership row (idempotent), and grant the built-in `member` + `workspace-admin` roles
/// (idempotent). Operator provisioning at boot — the login gate still enforces membership; this just
/// guarantees the dev user IS a member so a fresh OR previously-seeded store logs in cleanly.
async fn seed_dev_identity(node: &Node, ws: &str, user: &str) -> anyhow::Result<()> {
    use lb_authz as raw;
    let store = &node.store;
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    raw::identity_create(store, user, None, ts).await?;
    raw::membership_add_raw(store, ws, user, ts).await?;
    if let Some(name) = user.strip_prefix("user:") {
        let subject = lb_authz::Subject::User(name.to_string());
        raw::grant_assign(store, ws, &subject, "role:member").await?;
        raw::grant_assign(store, ws, &subject, "role:workspace-admin").await?;
    }
    println!("boot seed: {user} is a workspace-admin member of {ws}");
    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Boot the spine (solo node). `Arc` so the env-gated background roles (webhook + workflow driver)
    // can share it with the demo path below.
    let node = Arc::new(Node::boot().await?);

    // Telemetry sink selection (observability scope, symmetric nodes rule #1): choose the
    // `tracing-subscriber` layers by config (`LB_TELEMETRY_SINK`), right after boot so every
    // subsequent instrumented call is captured. The capped SurrealDB ring is the in-product
    // recent-history sink; it shares the node's OWN store + bus handles (the ring lives in the one
    // datastore, rule #2; the tail rides the ws-walled bus) — never a second store handle.
    let sink = lb_telemetry::SinkConfig::from_env();
    lb_telemetry::sink_layers(node.store.clone(), node.bus.clone(), sink);

    // The in-house agent runtime registry + external entries are installed by `agent::mount` below,
    // AFTER the gateway installs its signing key (the federation/control-engine ordering) so a served
    // run's tool callbacks verify. The in-house `default` binds the node's configured model (or the
    // honest `UnconfiguredModel`); the `external-agent` feature adds the external `AcpRuntime` entries.

    // Locate the built hello component. Override with HELLO_WASM; default to the cargo target.
    let wasm_path = std::env::var("HELLO_WASM")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../extensions/hello/target/wasm32-wasip2/release/hello_ext.wasm")
        });
    let manifest = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../extensions/hello/extension.toml"),
    )?;
    let wasm = std::fs::read(&wasm_path)
        .map_err(|e| anyhow::anyhow!("read {}: {e}", wasm_path.display()))?;

    let loaded = load_extension(&node, &manifest, &wasm, &[]).await?;

    // BOOT BRING-UP (lifecycle-management): re-load every previously-published-and-enabled wasm
    // extension for the configured workspace from the durable cache, so an upload survives a restart
    // (the durable Install record + the digest-keyed verified cache are the source of truth). A no-op
    // on a fresh store. The workspace comes from `LB_WORKSPACE` (the dev launch sets it; default "acme").
    let ws = std::env::var("LB_WORKSPACE").unwrap_or_else(|_| "acme".into());
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

    // global-identity seed: ensure the configured dev identity is a `workspace-admin` member of the
    // configured workspace (provisioning + joining — exactly what the scope says provisioning is, done
    // by the operator at boot, NOT a login bypass). The login gate still enforces membership; this just
    // guarantees the dev user IS a member so `make dev` works against a fresh OR a previously-seeded
    // store. Idempotent (upserts). `LB_SEED_USER` defaults to `user:ada`; clear it to skip.
    let seed_user = std::env::var("LB_SEED_USER").unwrap_or_else(|_| "user:ada".into());
    if let Err(e) = seed_dev_identity(&node, &ws, &seed_user).await {
        eprintln!("boot seed for ws={ws} user={seed_user} failed: {e}");
    }

    // CORE-SKILL SEED (core-skills scope): write the embedded `docs/skills/*/SKILL.md` corpus into
    // the reserved system namespace as immutable `skill:core.<name>@<node-version>` records. Idempotent
    // — an already-seeded version is a no-op, so this runs every boot; a node upgrade (a new
    // CARGO_PKG_VERSION) seeds the new versions and leaves the old for rollback. The boot seeder is the
    // ONLY writer of that namespace. `env!("CARGO_PKG_VERSION")` is the node build version.
    let node_version = env!("CARGO_PKG_VERSION");
    let boot_ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    match lb_host::seed_core_skills(&node.store, node_version, boot_ts).await {
        Ok(ids) => println!(
            "boot: seeded {} core skills @{node_version} ({:?})",
            ids.len(),
            ids
        ),
        Err(e) => eprintln!("boot: core-skill seed failed: {e}"),
    }

    // DEFAULT CORE-SKILL GRANTS for the boot workspace (core-skills scope): `workspace_create` applies
    // the default set on a genuinely-new workspace, but the dev boot workspace is seeded directly (not
    // through that verb), so grant the resolved set here too. The set is node config —
    // `LB_DEFAULT_CORE_SKILLS` (comma-separated ids; empty ⇒ none) overrides the compiled-in read-only
    // defaults. Best-effort + idempotent (each is a revocable `grant:skill/{id}` edge).
    let default_skills = lb_host::resolve_default_core_skills(
        std::env::var("LB_DEFAULT_CORE_SKILLS").ok().as_deref(),
    );
    lb_host::grant_default_core_skills(&node.store, &ws, &default_skills).await;
    if !default_skills.is_empty() {
        println!("boot: default core-skill grants for ws={ws}: {default_skills:?}");
    }

    // FLOW REACTOR TICK: drive the cron/reconcile scans on a cadence so a `mode:"cron"` trigger
    // actually fires on a running node (the scans were previously only invoked from tests — a flow
    // armed in the UI never fired). One detached owner per node, scanning the configured workspace.
    // A few-second period catches a minute-granularity cron promptly; each tick is a cheap ws scan.
    lb_host::spawn_flow_reactors(
        node.clone(),
        vec![ws.clone()],
        lb_host::Role::Solo,
        std::time::Duration::from_secs(5),
    );

    // CHANNEL AGENT REACTOR TICK (run-lifecycle #5): drain durable `channel-agent-run` enqueue jobs
    // that `channel::post` writes when a member asks an agent in a channel, and drive each run OFF the
    // POST connection — so an in-channel agent run survives the tab closing and (being durable +
    // idempotent) a node restart. One detached owner per node, scanning the configured workspace on a
    // few-second cadence (a freshly-posted request starts its run promptly; each tick is a cheap scan).
    lb_host::spawn_agent_reactors(
        node.clone(),
        vec![ws.clone()],
        std::time::Duration::from_secs(2),
    );

    // ROLE SELECTION (config, §3.1): mount the github-workflow ingress + background driver if the
    // environment configures them. A no-op otherwise — the binary stays the solo demo below.
    github::mount(node.clone()).await;
    // NOTE: native sidecar roles (federation, control-engine) are mounted AFTER the gateway installs
    // its signing key onto the node (below), NOT here. `install_native` mints each sidecar's
    // `LB_EXT_TOKEN` with `node.key()`, and the gateway VERIFIES those callback tokens with its own
    // key — which it installs onto the node in `Gateway::new_live`. Mounting before that ran meant the
    // token was minted with the node's throwaway boot key and every sidecar callback 401'd
    // (native-callback-transport scope: one signing identity per node). See the gateway block below.
    println!(
        "loaded hello: tools={:?} granted_caps={:?}",
        loaded.tools, loaded.granted_caps
    );

    // Mint a member token for workspace "acme" that may call hello.echo.
    let key = SigningKey::generate();
    let claims = Claims {
        sub: "user:demo".into(),
        ws: "acme".into(),
        role: Role::Member,
        caps: vec!["mcp:hello.echo:call".into()],
        iat: 0,
        exp: u64::MAX,
    };
    let token = mint(&key, &claims);
    let principal = lb_auth::verify(&key, &token, 1).expect("freshly minted token verifies");

    // Call the tool through the MCP pipeline (resolve → authorize → dispatch).
    let out = lb_mcp::call(
        &node.registry,
        &node.bus,
        &principal,
        "acme",
        "hello.echo",
        r#"{"msg":"hi"}"#,
    )
    .await?;
    println!("hello.echo -> {out}");

    // ROLE SELECTION (config, not a code branch in core crates): if `LB_GATEWAY_ADDR` is set,
    // mount the SSE/HTTP gateway so a browser can reach a real node (S3). Otherwise the binary
    // is the solo demo above. This is the thin wiring layer §3.1 permits to be role-aware.
    if let Ok(addr) = std::env::var("LB_GATEWAY_ADDR") {
        let addr: std::net::SocketAddr = addr
            .parse()
            .map_err(|e| anyhow::anyhow!("bad LB_GATEWAY_ADDR: {e}"))?;
        // The gateway fronts THIS node. Do not call `Gateway::boot()` here: that would open a second
        // embedded store handle, and with `LB_STORE_PATH` set both handles would point at the same
        // SurrealKV directory.
        // A LIVE clock (not a value frozen here at boot): `Gateway::new_live` reads wall time per
        // request, so token iat/exp and any derived ts advance. `Gateway::new(.., now)` is the
        // fixed-clock TEST seam only.
        let gw = lb_role_gateway::Gateway::new_live(node.clone(), SigningKey::generate());
        // The gateway just installed its signing key onto the node (`Gateway::new_live` →
        // `node.install_key`), so NOW there is ONE signing identity. Mount the native sidecar roles
        // here — `install_native` mints each child's `LB_EXT_TOKEN` with that shared `node.key()`, so
        // the gateway verifies its callbacks (no 401). Env-gated, no-ops when unconfigured.
        federation::mount(node.clone()).await;
        control_engine::mount(node.clone()).await;
        // The in-house agent (default-agent-wiring): install the runtime registry (in-house default
        // over the configured model + external entries when the feature is on) and serve routed
        // `agent.invoke`. Mounted HERE, after the gateway key install, so a served run's tool callbacks
        // verify — the same ordering federation/control-engine use. `_agent_server` is held to the end
        // of `main` (dropping it stops serving); `serve(..)` below never returns in normal operation.
        let _agent_server = agent::mount(node.clone()).await;
        println!("gateway: serving on http://{addr}");
        lb_role_gateway::serve(gw, addr).await?;
    } else {
        // No gateway (edge/solo posture): still mount the native roles for a headless node. Their
        // callbacks need a gateway, so they degrade cleanly (a sidecar with no callback address).
        federation::mount(node.clone()).await;
        control_engine::mount(node.clone()).await;
        // Install the in-house agent registry even without a gateway, so the in-channel `/agent` path
        // drives the configured model on a solo node (a solo node just has no remote callers to serve).
        let _agent_server = agent::mount(node.clone()).await;
    }

    Ok(())
}
