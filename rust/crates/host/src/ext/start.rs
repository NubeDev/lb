//! `ext.start` — start an **installed, enabled, stopped** extension *now*, without bouncing the node
//! (lifecycle-management scope).
//!
//! The gap this closes: the lifecycle had no way to start a stopped extension. `ext.enable` flips the
//! durable intent but spawns nothing ("the boot reconciler / next start brings it up" — a *start* that
//! had no verb). `native.restart` and `native.reset` both need a live-or-dead **handle** in the
//! `SidecarMap` and return `NotRunning` when there is none — and `reset`'s own doc pointed the
//! operator at "`ext.enable`/install to start a stopped extension", which did not do that. So every
//! documented recovery path led somewhere that could not start anything, and **republishing the
//! artifact was the only way back** — which is why a boot gap (issue #64) could only be worked around
//! by re-uploading a binary that was already on disk.
//!
//! `enable`/`start` stay distinct, deliberately — the same split `disable`/`stop` already has:
//! `enable` is durable *intent* ("may run, and auto-start on boot"), `start` is the *act* ("run it
//! now"). Conflating them would make `enable` a spawn and lose the ability to mark an extension
//! runnable without running it yet.
//!
//! It reuses boot's exact path (`spawn_one`) rather than a parallel one: "start this extension" means
//! precisely what the node does for it at boot, whoever asks. No new persistence and no new trust —
//! the artifact is the same verified, cached one, and the grant comes from the durable `Install`.
//!
//! Reached over the gateway (`POST /extensions/{ext}/start`), not the `ext.*` MCP bridge — that
//! bridge ([`call_ext_tool`](super::call_ext_tool)) has no caller and `"ext."` is absent from
//! `HOST_NATIVE_PREFIXES`, so the whole `ext.*` MCP surface is currently unreachable. Adding `start`
//! there would only imply a reachability that does not exist; wiring that surface up is its own
//! change, for every verb at once.

use lb_assets::{read_install, Tier};
use lb_auth::Principal;
use lb_mcp::authorize_tool;
use lb_supervisor::Launcher;

use super::boot_spawn::{spawn_one, SpawnedExt};
use super::error::ExtError;
use crate::boot::Node;

/// Start `ext_id` in workspace `ws` as `caller`. Returns the [`SpawnedExt`] row describing the
/// outcome — `spawned: true` on success, otherwise a `reason` naming the fault (the same vocabulary
/// the boot log uses, so an operator reads one language in both places).
///
/// Gated `mcp:ext.start:call`, workspace-first (the caller's token carries the ws — a start can never
/// reach across the wall). Authorization runs FIRST: a caller without the grant is refused before any
/// record is read or any process is spawned.
///
/// Refuses to start a **disabled** extension: `enabled: false` is durable intent that says "do not run
/// this", and a start that overrode it would resurrect exactly what `disable` exists to prevent —
/// silently, until the next boot honored the flag again and it vanished. Enable it first; the two-step
/// is the point.
///
/// Idempotent: starting an already-running extension is a no-op success (`already-running`), not an
/// error and not a second child.
///
/// An extension that is not installed here is a `not-installed` row rather than an error, for the
/// same reason `ext.enable`/`ext.disable` are idempotent no-ops on an absent id: the caller learns
/// the outcome, and a denied caller still cannot probe existence (the gate ran first, opaquely).
///
/// **Native tier only.** A wasm "start" is a component load with no process, and its boot peer is
/// `load_enabled`; a wasm row here is `not-native` rather than a lie about having started something.
pub async fn ext_start<L: Launcher>(
    node: &Node,
    launcher: &L,
    caller: &Principal,
    ws: &str,
    ext_id: &str,
    ts: u64,
) -> Result<SpawnedExt, ExtError> {
    authorize_tool(caller, ws, "ext.start").map_err(|_| ExtError::Denied)?;

    let Some(install) = read_install(&node.store, ws, ext_id).await? else {
        return Ok(row(ext_id, "", "not-installed"));
    };
    if install.tier != Tier::Native {
        return Ok(row(ext_id, &install.version, "not-native"));
    }
    if !install.enabled {
        return Ok(row(ext_id, &install.version, "disabled"));
    }
    if node.sidecars.is_running(ws, ext_id) {
        return Ok(row(ext_id, &install.version, "already-running"));
    }
    spawn_one(node, launcher, ws, ext_id, Some(&install), ts).await
}

fn row(ext: &str, version: &str, reason: &str) -> SpawnedExt {
    SpawnedExt {
        ext: ext.to_string(),
        version: version.to_string(),
        spawned: false,
        reason: reason.to_string(),
    }
}
