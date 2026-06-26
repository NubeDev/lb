//! `install_native` — the native tier's install/start verb: persist the durable records, then spawn
//! and supervise the child (native-tier scope). The peer of `install_extension` (wasm) and
//! `install_from_registry` (the signed pull in front). It composes, it does not re-invent:
//!   1. the **capability gate** (`authorize_native`) — workspace-first, `mcp:native.install:call`;
//!   2. the **S4 durable install** — persist `requested ∩ admin_approved` as the `Install` record
//!      (the same grant computation the wasm tier uses; nothing requested is live unless approved);
//!   3. the **supervisor** — build the spec (injecting the scoped identity), spawn the child, and
//!      keep the live handle in the runtime `SidecarMap` (never the store — the PID is motion);
//!   4. the **status projection** — record `native_status = {Started, restart_count: 0}` so a
//!      restart re-derives from durable state (no durable state lost, §3.4).
//!
//! Two independent gates hold: the capability gate here, and (when the binary came from the signed
//! registry) the signature gate in `pull` — installing a native extension does not bypass either.

use lb_assets::{record_install, Install};
use lb_ext_loader::{grant, Manifest};
use lb_supervisor::{Launcher, Sidecar};

use super::error::NativeServiceError;
use super::spec::{build_spec, native_of, tool_names};
use super::status::{record_status, NativeStatus};
use crate::boot::Node;

/// What a native install produced — the granted caps and the child's declared tool names (for the
/// caller to surface/audit), mirroring the wasm `Loaded`.
#[derive(Debug, Clone)]
pub struct Supervised {
    pub granted_caps: Vec<String>,
    pub tools: Vec<String>,
    pub version: String,
}

/// Install (or restart-into) `manifest_toml`'s native extension in workspace `ws` for `caller`,
/// spawning the child via `launcher`. `install_dir` resolves the binary path; `admin_approved` is
/// the approved cap set; `ts` is the injected logical timestamp. Idempotent on `ext_id`: a second
/// install stops the running child first (an upgrade/re-install in place), then spawns the new one.
///
/// Authorization (`mcp:native.install:call`, workspace-first) runs FIRST — a caller without the
/// grant is refused before any record is written or any process is spawned (the deny path).
pub async fn install_native<L: Launcher>(
    node: &Node,
    launcher: &L,
    caller: &lb_auth::Principal,
    ws: &str,
    manifest_toml: &str,
    install_dir: &str,
    admin_approved: &[String],
    ts: u64,
) -> Result<Supervised, NativeServiceError> {
    super::authorize::authorize_native(caller, ws, "install")?;

    let manifest =
        Manifest::parse(manifest_toml).map_err(|e| NativeServiceError::NotNative(e.to_string()))?;
    let native = native_of(&manifest)
        .ok_or_else(|| NativeServiceError::NotNative(format!("{} is not native", manifest.id)))?;

    let granted = grant(&manifest, admin_approved);
    let tools = tool_names(&manifest);

    // STATE first: the durable approved-grant record (the same S4 verb, now for native tier).
    let install = Install::new(
        manifest.id.clone(),
        manifest.version.clone(),
        granted.clone(),
        ts,
    );
    record_install(&node.store, ws, &install).await?;

    // If a sidecar for this id is already running here, stop it before swapping (re-install in
    // place — the durable id/records stay stable, only the process is replaced).
    stop_if_running(node, ws, &manifest.id).await;

    // Spawn the child with its scoped identity, and hold the live handle in the runtime map.
    let spec = build_spec(native, install_dir, ws, &manifest.id, &granted);
    let sidecar = Sidecar::spawn(spec, launcher).await?;
    node.sidecars.insert(ws, &manifest.id, sidecar);

    // Durable status: Started, restart_count 0 — what a boot reconciler (follow-up) re-derives from.
    record_status(
        &node.store,
        ws,
        &NativeStatus::new(&manifest.id, &manifest.version, ts),
    )
    .await?;

    Ok(Supervised {
        granted_caps: granted,
        tools,
        version: manifest.version,
    })
}

/// Stop a running sidecar for `(ws, ext_id)` if present (a cooperative shutdown). Used by a
/// re-install to replace the child in place. No-op if nothing is running here.
pub(crate) async fn stop_if_running(node: &Node, ws: &str, ext_id: &str) {
    if let Some(handle) = node.sidecars.remove(ws, ext_id) {
        handle.lock().await.shutdown().await;
    }
}
