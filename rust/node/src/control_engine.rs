//! The `control-engine` (CE bridge) role wiring — env-gated, mounted from `main.rs`. The same thin
//! role-aware layer §3.1 permits in the *binary* (like `federation.rs`/`github.rs`): no core crate is
//! role-aware; the decision to install the native CE sidecar (and pre-approve its `net:*` endpoint)
//! lives here, keyed off config (env), never an `if cloud`.
//!
//! Driven by one env var:
//!   - `LB_CONTROL_ENGINE_BASE` — the CE `ce-rest` `host:port` the admin approves the sidecar to
//!     connect to (`net:tcp:host:port:connect`). Setting it installs + supervises the `control-engine`
//!     sidecar in `LB_WORKSPACE` with the manifest's requested grant intersected with that approval.
//!
//! Optionally, one appliance is pre-registered so the wiresheet page shows a working entry on first
//! boot (the local CE), via the extension's own `control-engine.appliance.add` verb:
//!   - `LB_CONTROL_ENGINE_APPLIANCE` — the appliance id to seed (default `local`).
//!
//! The sidecar binary is resolved from the workspace target dir (where `cargo run` builds it); the
//! manifest is the extension's own `extension.toml`. `now` enters here, at the binary boundary, as
//! wall-clock seconds (the no-wall-clock rule keeps time out of the *core crates*).

use std::path::PathBuf;
use std::sync::Arc;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{install_native, Node};
use lb_supervisor::OsLauncher;

/// Wall-clock seconds since the Unix epoch — the install's `now` at the binary boundary.
fn unix_seconds() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// The admin service principal the CE install acts as in `ws` — holds exactly the native install gate
/// plus the registry write caps the appliance seed needs. (A real login→token→principal session
/// replaces this demo identity later, like the gateway's dev login.)
fn admin_principal(ws: &str) -> Principal {
    let key = SigningKey::generate();
    let claims = Claims {
        sub: "ext:control-engine-bootstrap".into(),
        ws: ws.into(),
        role: Role::Member,
        caps: vec![
            "mcp:native.install:call".into(),
            // The appliance-seed uses the registry add verb (which itself gates on the store write).
            "mcp:control-engine.appliance.add:call".into(),
            "store:ce_appliance:write".into(),
        ],
        iat: 0,
        exp: u64::MAX,
    };
    verify(&key, &mint(&key, &claims), 1).expect("freshly minted token verifies")
}

/// The control-engine extension manifest (compiled in so the binary needs no file at this path at run
/// time — it is the same source the E2E tests install from).
const MANIFEST: &str = include_str!("../../extensions/control-engine/extension.toml");

/// Resolve the directory holding the built `control-engine` binary (the workspace target dir).
/// `cargo run` builds debug; a release run uses release. Overridable with `LB_CONTROL_ENGINE_DIR`.
fn control_engine_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("LB_CONTROL_ENGINE_DIR") {
        return PathBuf::from(dir);
    }
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    // node/ is a workspace member; the shared target/ is one level up.
    let profile = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };
    manifest_dir.join("..").join("target").join(profile)
}

/// Mount the control-engine role on `node` per the environment. Installs + supervises the
/// `control-engine` sidecar with the admin-approved `net:tcp` grant for the configured CE, then
/// (optionally) pre-registers one appliance so the wiresheet page works on first boot. A no-op if
/// `LB_CONTROL_ENGINE_BASE` is unset.
pub async fn mount(node: Arc<Node>) {
    let Ok(base) = std::env::var("LB_CONTROL_ENGINE_BASE") else {
        return; // The CE bridge role is not configured — no control-engine sidecar.
    };
    let base = base.trim().to_string();
    if base.is_empty() {
        return;
    }

    let ws = std::env::var("LB_WORKSPACE").unwrap_or_else(|_| "acme".into());
    let admin = admin_principal(&ws);
    let now = unix_seconds();

    // The admin-approved grant: the manifest requests `net:tcp:127.0.0.1:7979:connect` (the canonical
    // local CE) plus the store/graph/watch verb caps. We approve exactly the configured CE endpoint's
    // connect (the per-endpoint wall) and let the manifest's own verb-cap requests through unchanged
    // by ALSO approving them — `install_native` computes `requested ∩ approved`, so we mirror the
    // manifest's request set here (this dev bootstrap approves everything the ext asks for).
    let approved = approved_grant(&base);

    let dir = control_engine_dir();
    let dir_str = dir.to_string_lossy().into_owned();
    let bin = dir.join("control-engine");
    if !bin.exists() {
        eprintln!(
            "control-engine: sidecar binary not found at {} — build it with \
             `cargo build -p control-engine` (skipping install)",
            bin.display()
        );
        return;
    }

    match install_native(
        &node, &OsLauncher, &admin, &ws, MANIFEST, &dir_str, &approved, now,
    )
    .await
    {
        Ok(s) => println!(
            "control-engine: installed sidecar in '{ws}' (tools={:?}, granted={:?}, CE base={base})",
            s.tools, s.granted_caps
        ),
        Err(e) => {
            eprintln!("control-engine: sidecar install failed: {e}");
            return;
        }
    }

    seed_appliance(&node, &ws, &base, now).await;
}

/// The dev-bootstrap approved grant: the CE endpoint connect + the manifest's own verb/store caps.
/// `install_native` intersects this with the manifest `request`, so listing them here approves the
/// full requested surface for the demo node (a real admin approval flow narrows this per deployment).
fn approved_grant(base: &str) -> Vec<String> {
    let mut approved = vec![
        // Per-endpoint network wall for the configured CE (and the canonical local one the manifest
        // requests, so the default 127.0.0.1:7979 request survives the intersection).
        connect_cap(base),
        connect_cap("127.0.0.1:7979"),
        // Registry store verbs + per-table grant.
        "mcp:store.write:call".into(),
        "mcp:store.query:call".into(),
        "mcp:store.delete:call".into(),
        "store:ce_appliance:read".into(),
        "store:ce_appliance:write".into(),
        // The live-COV write path + the series SSE the watch stream half opens.
        "mcp:ingest.write:call".into(),
        "mcp:series.watch:call".into(),
    ];
    // The read + graph-write + watch + registry verbs (self-checked in the sidecar against this grant;
    // also what the S7 page's `[ui] scope` narrows against, so the whole page surface is granted here).
    for verb in [
        "control-engine.tree",
        "control-engine.schema",
        "control-engine.appliance.add",
        "control-engine.appliance.list",
        "control-engine.appliance.remove",
        "control-engine.add-node",
        "control-engine.patch",
        "control-engine.set-override",
        "control-engine.clear-override",
        "control-engine.add-edge",
        "control-engine.remove-node",
        "control-engine.call-action",
        "control-engine.watch",
    ] {
        approved.push(format!("mcp:{verb}:call"));
    }
    approved
}

/// `net:tcp:host:port:connect` for a `host:port` base (an absent port defaults to the canonical 7979).
fn connect_cap(base: &str) -> String {
    let b = base
        .strip_prefix("http://")
        .or_else(|| base.strip_prefix("https://"))
        .unwrap_or(base)
        .trim_end_matches('/');
    let (host, port) = match b.rsplit_once(':') {
        Some((h, p)) if p.parse::<u16>().is_ok() => (h, p),
        _ => (b, "7979"),
    };
    format!("net:tcp:{host}:{port}:connect")
}

/// Pre-register one appliance via the extension's own `control-engine.appliance.add` verb, so the
/// wiresheet page's picker shows a working entry on first boot. Best-effort: a failure is logged, not
/// fatal (the page still works once an appliance is added through the UI).
async fn seed_appliance(node: &Arc<Node>, ws: &str, base: &str, _now: u64) {
    use lb_mcp::call;

    let id = std::env::var("LB_CONTROL_ENGINE_APPLIANCE").unwrap_or_else(|_| "local".into());
    let admin = admin_principal(ws);
    let args = serde_json::json!({
        "id": id,
        "name": id,
        "mode": "local",
        "node": "local",
        "base": base,
    })
    .to_string();

    match call(
        &node.registry,
        &node.bus,
        &admin,
        ws,
        "control-engine.appliance.add",
        &args,
    )
    .await
    {
        Ok(_) => println!("control-engine: seeded appliance '{id}' → {base}"),
        Err(e) => eprintln!("control-engine: appliance seed skipped ({e})"),
    }
}
