//! The **standalone full-stack boot** (`full` cargo feature): mount the SSE/HTTP gateway
//! in-process on a loopback port + run the boot seeders + spawn the background reactors, so
//! the packaged shell is a 100% standalone node — login, MCP, SSE, the agent catalog, flows,
//! insights, all of it — with no external node to talk to.
//!
//! This mirrors `rust/node/src/main.rs`'s gateway branch (`seed_dev_identity` + the catalog
//! seeders + `Gateway::new_live` + `serve` + the four reactors), minus the native sidecars
//! that need their own binaries (federation / control-engine — env-gated in `make dev`, not
//! shipped in the desktop binary). The window (`desktop.rs`) attaches to the SAME node; the
//! webview talks to the loopback gateway over HTTP exactly as the browser does against
//! `make dev`. See `docs/scope/desktop/desktop-standalone-backend-scope.md`.
//!
//! One responsibility (FILE-LAYOUT §9): "boot the standalone backend onto a node". The node,
//! the gateway, and the window wiring all live elsewhere; this file only composes them.

use std::net::SocketAddr;
use std::sync::Arc;

use lb_auth::SigningKey;
use lb_authz as raw;
use lb_host::{self, Node};
use lb_role_gateway::{serve_listener, Gateway};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;

/// The loopback address the in-process gateway binds when the shell is built `full`.
///
/// **Fixed, not runtime-negotiated**, because the UI's `VITE_GATEWAY_URL` is baked at build
/// time — the webview fetches this exact origin. Distinct from the dev `8080` so a developer
/// running `make dev` alongside the desktop binary never collides. Override with
/// `LB_DESKTOP_GATEWAY_ADDR` (the override is only useful with a matching UI rebuild).
pub const LOOPBACK_ADDR: &str = "127.0.0.1:8800";

/// Seed the dev `user` as a `workspace-admin` member of `ws`: create the global identity,
/// write the membership row, and grant the built-in `member` + `workspace-admin` roles.
/// Idempotent (upserts). Operator provisioning at boot — the login gate still enforces
/// membership; this just guarantees the dev user IS a member so a fresh store logs in
/// cleanly. Mirrors `rust/node/src/main.rs:22` verbatim (the same contract, one place each).
async fn seed_dev_identity(node: &Node, ws: &str, user: &str) -> Result<(), String> {
    let store = &node.store;
    let ts = now_secs();
    // Seed the built-in `member`/`workspace-admin` role RECORDS so the role grants below resolve
    // to caps (login-hardening scope). Without this, `role:workspace-admin` is assigned but the
    // role has no cap bundle, so the seeded user logs in with (almost) no reach — the "missing
    // access to everything" symptom. Idempotent; mirrors `rust/node/src/main.rs:31` verbatim.
    lb_host::ensure_builtin_authz_roles(store, ws)
        .await
        .map_err(|e| e.to_string())?;
    raw::identity_create(store, user, None, ts)
        .await
        .map_err(|e| e.to_string())?;
    raw::membership_add_raw(store, ws, user, ts)
        .await
        .map_err(|e| e.to_string())?;
    if let Some(name) = user.strip_prefix("user:") {
        let subject = raw::Subject::User(name.to_string());
        raw::grant_assign(store, ws, &subject, "role:member")
            .await
            .map_err(|e| e.to_string())?;
        raw::grant_assign(store, ws, &subject, "role:workspace-admin")
            .await
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Boot the standalone backend onto `node`: run the boot seeders (so login + the agent /
/// persona / skill catalogs work out of the box), mount the SSE/HTTP gateway in-process on
/// `addr`, spawn the four background reactors, and return the gateway serve task + the
/// **actually bound** address (so a caller that passed `127.0.0.1:0` learns the chosen port).
///
/// Every seeder step is idempotent or best-effort-with-a-warning (mirroring `node/main.rs`):
/// a seeder failure never blocks the gateway — it prints and continues, so the app still
/// opens and the operator sees what's missing. The bind, by contrast, is a hard error: if
/// the loopback port is taken the gateway cannot serve, so surface that to the caller.
pub async fn boot_full(
    node: Arc<Node>,
    ws: &str,
    addr: SocketAddr,
) -> std::io::Result<(JoinHandle<()>, SocketAddr)> {
    let seed_user = std::env::var("LB_SEED_USER").unwrap_or_else(|_| "user:ada".into());
    if let Err(e) = seed_dev_identity(&node, ws, &seed_user).await {
        eprintln!("full: boot seed for ws={ws} user={seed_user} failed: {e}");
    }

    // Catalog seeders: core skills, agent definitions, personas, the legacy-persona
    // migration, and the default core-skill grants. Each is idempotent (LWW upsert) and the
    // ONLY writer of its namespace — running every boot is correct (mirrors `node/main.rs`).
    let node_version = env!("CARGO_PKG_VERSION");
    match lb_host::seed_core_skills(&node.store, node_version, now_secs()).await {
        Ok(ids) => println!("full: seeded {} core skills @{}", ids.len(), node_version),
        Err(e) => eprintln!("full: core-skill seed failed: {e}"),
    }
    if let Err(e) = lb_host::seed_agent_definitions(&node.store).await {
        eprintln!("full: agent-definition seed failed: {e}");
    }
    if let Err(e) = lb_host::seed_personas(&node.store).await {
        eprintln!("full: persona seed failed: {e}");
    }
    if let Err(e) = lb_host::migrate_active_persona(&node.store).await {
        eprintln!("full: active_persona migration failed: {e}");
    }
    let default_skills = lb_host::resolve_default_core_skills(
        std::env::var("LB_DEFAULT_CORE_SKILLS").ok().as_deref(),
    );
    lb_host::grant_default_core_skills(&node.store, ws, &default_skills).await;

    // The four background reactors that make durable features actually fire on a running
    // node: flow cron/reconcile, channel-agent runs, approval release, insight digests. One
    // detached owner each, scanning the configured workspace on a few-second cadence.
    lb_host::spawn_flow_reactors(
        node.clone(),
        vec![ws.to_string()],
        lb_host::Role::Solo,
        std::time::Duration::from_secs(5),
    );
    lb_host::spawn_agent_reactors(
        node.clone(),
        vec![ws.to_string()],
        std::time::Duration::from_secs(2),
    );
    lb_host::spawn_approval_reactors(
        node.clone(),
        vec![ws.to_string()],
        std::time::Duration::from_secs(2),
    );
    lb_host::spawn_insight_digest_reactors(
        node.clone(),
        vec![ws.to_string()],
        std::time::Duration::from_secs(30),
    );

    // Mount the gateway in-process. `Gateway::new_live` installs the signing key onto the
    // node (one signing identity), so login mints + every route verifies with the same key.
    // The caller-owned listener lets the desktop bind a fixed port (its UI is baked to match)
    // while tests bind `127.0.0.1:0` for a collision-free port. CORS is already permissive
    // (`CorsLayer::permissive`), so the webview origin reaches the loopback origin cleanly.
    // Keep a node handle for the federation mount below — it must run AFTER the key install.
    let node_for_fed = node.clone();
    let gw = Gateway::new_live(node, SigningKey::generate());

    // Bring up the bundled federation datasources sidecar (desktop-federation-bundle scope): a
    // packaged `.exe` can register AND query the shipped sqlite demo out of the box. AFTER the
    // gateway installed the signing key above (so the child token verifies), matching `node/main.rs`
    // ordering. Best-effort + loud like the other seeders — never blocks the gateway.
    crate::federation::mount_federation(node_for_fed, ws, now_secs()).await;

    let listener = TcpListener::bind(addr).await?;
    let bound = listener.local_addr()?;
    println!("full: loopback gateway on http://{bound} (login as {seed_user} / {ws})");
    let handle = tokio::spawn(async move {
        if let Err(e) = serve_listener(gw, listener).await {
            eprintln!("full: loopback gateway stopped: {e}");
        }
    });
    Ok((handle, bound))
}

/// Resolve the loopback bind address from `LB_DESKTOP_GATEWAY_ADDR` (override) or the fixed
/// default. Surfaced so `desktop.rs` logs the exact address it will bind before booting.
pub fn resolve_addr() -> SocketAddr {
    std::env::var("LB_DESKTOP_GATEWAY_ADDR")
        .unwrap_or_else(|_| LOOPBACK_ADDR.into())
        .parse()
        .unwrap_or_else(|e| {
            eprintln!("full: bad LB_DESKTOP_GATEWAY_ADDR ({e}); falling back to {LOOPBACK_ADDR}");
            LOOPBACK_ADDR
                .parse()
                .expect("LOOPBACK_ADDR is a valid SocketAddr")
        })
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loopback_addr_is_valid_and_distinct_from_dev() {
        let addr: SocketAddr = LOOPBACK_ADDR.parse().expect("LOOPBACK_ADDR parses");
        assert_eq!(addr.ip().is_loopback(), true);
        // Distinct from the dev gateway port (8080) so `make dev` + the desktop binary
        // never collide on the same host.
        assert_ne!(addr.port(), 8080);
    }

    #[test]
    fn resolve_addr_falls_back_on_a_bad_override() {
        // A malformed override must not panic — it falls back to the fixed default.
        std::env::set_var("LB_DESKTOP_GATEWAY_ADDR", "not a socket addr");
        let addr = resolve_addr();
        std::env::remove_var("LB_DESKTOP_GATEWAY_ADDR");
        assert_eq!(addr.port(), 8800);
    }
}
