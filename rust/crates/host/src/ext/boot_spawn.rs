//! `spawn_enabled` — the node's boot bring-up of **native (Tier-2)** extensions (lifecycle-management
//! scope). The exact peer of [`load_enabled`](super::load_enabled), which owns the wasm half.
//!
//! `reconcile` decides the *intent* (`enabled ∧ not-running` → start) but returns a plan rather than
//! spawning, because a native respawn needs the `Launcher` + the install dir the node binary owns —
//! which is why this verb is generic over `L: Launcher` exactly as `install_native` is, and why the
//! node passes `OsLauncher` while a test passes its own. Without this, the plan's native actions were
//! computed and dropped: an `enabled: true` native install simply never came back after a restart,
//! and nothing was logged (issue #64).
//!
//! **No new persistence and no new trust.** The durable `Install` record + the digest-keyed artifact
//! cache are already the source of truth for both tiers: `resolve` → `read_cached` yields the
//! `manifest_toml` AND the verified bytes — precisely the pair `install_native` takes. The bytes were
//! verified before they were cached (the `VerifiedArtifact` seam), and the grant is recomputed as
//! `requested ∩ approved` from the durable `Install`, so nothing here can widen privilege. This is
//! the same argument `load_enabled` makes for wasm, and it holds identically for native.
//!
//! Not capability-gated as a *caller* verb: like `reconcile` and `load_enabled`, this is a node-boot
//! operation. See [`boot_caller`] for why the authority it hands `install_native` is sound.

use lb_assets::{list_installs, Tier};
use lb_auth::Principal;
use lb_ext_loader::Manifest;
use lb_supervisor::Launcher;
use serde::{Deserialize, Serialize};

use super::error::ExtError;
use super::reconcile::reconcile;
use crate::boot::Node;
use crate::native::install_native;
use crate::registry::{read_cached, resolve};

/// One native extension this verb respawned (or could not), for the boot log. The peer of
/// [`LoadedExt`](super::LoadedExt) — a symmetric boot log across both tiers.
///
/// `Serialize` because [`ext_start`](super::ext_start) hands this row straight back to an HTTP
/// caller: the operator starting an extension by hand reads the same `spawned` + `reason` vocabulary
/// the boot log prints, rather than a second dialect invented for the route.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpawnedExt {
    pub ext: String,
    pub version: String,
    /// `true` → the child process is running again; `false` → skipped, or could not be respawned.
    pub spawned: bool,
    /// Why, for the boot log: `"spawned"` / `"disabled"` / `"already-running"` /
    /// `"no-catalog-entry (…)"` (the install record and the catalog disagree about `(ext, version)`)
    /// / `"no-cached-bytes"` (the catalog knows it, the cache lost the bytes) / `"not-native"` /
    /// a spawn error. The two empty-lookup cases are kept apart on purpose — they are different
    /// faults with different fixes, and one reason for both sent operators to the wrong one.
    pub reason: String,
}

/// Respawn every **enabled native** install for workspace `ws` from the durable cache, honoring the
/// boot [`reconcile`] plan (disabled / already-running are skipped). Returns one [`SpawnedExt`] per
/// native install for the boot log. Idempotent against the runtime — `reconcile` filters out
/// already-running children, so calling this twice does not double-spawn.
///
/// **Best-effort per extension, like the wasm half:** one extension that cannot be respawned (its
/// artifact was evicted, its binary won't exec) is reported in its own row and does not abort the
/// others or fail the boot. See the module doc on why this does not hard-fail.
pub async fn spawn_enabled<L: Launcher>(
    node: &Node,
    launcher: &L,
    ws: &str,
    ts: u64,
) -> Result<Vec<SpawnedExt>, ExtError> {
    let plan = reconcile(node, ws).await?;
    let installs = list_installs(&node.store, ws).await?;
    let mut out = Vec::new();

    for action in &plan.actions {
        if action.tier != "native" {
            continue; // the wasm half is `load_enabled`'s job, not this verb's.
        }
        if !action.start {
            out.push(SpawnedExt {
                ext: action.ext.clone(),
                version: String::new(),
                spawned: false,
                reason: action.reason.clone(),
            });
            continue;
        }
        let install = installs
            .iter()
            .find(|i| i.ext_id == action.ext && i.tier == Tier::Native);
        out.push(spawn_one(node, launcher, ws, &action.ext, install, ts).await?);
    }
    Ok(out)
}

/// Bring ONE native install online from the durable cache: resolve its catalog entry → digest →
/// cached bytes → land the binary → `install_native`. Returns the [`SpawnedExt`] row describing what
/// happened (never an `Err` for a per-extension fault — those are `reason`s, so one extension cannot
/// abort a boot or a caller's request; only a store failure propagates).
///
/// Shared by boot bring-up ([`spawn_enabled`]) and the on-demand [`ext_start`] verb, so the two can
/// never drift: "start this extension" means exactly what boot does, whoever asks.
pub(super) async fn spawn_one<L: Launcher>(
    node: &Node,
    launcher: &L,
    ws: &str,
    ext: &str,
    install: Option<&lb_assets::Install>,
    ts: u64,
) -> Result<SpawnedExt, ExtError> {
    let version = install.map(|i| i.version.clone()).unwrap_or_default();
    let row = |spawned: bool, reason: String| SpawnedExt {
        ext: ext.to_string(),
        version: version.clone(),
        spawned,
        reason,
    };

    // Resolve the catalog entry → digest → the cached, previously-verified artifact. The two ways
    // this can come up empty are DIFFERENT faults with different fixes, so they get different
    // reasons: a missing catalog entry means the install record and the catalog disagree about
    // `(ext, version)` (publish now rejects that at the door, but a store written before that gate
    // existed can still carry it) — pointing at an evicted cache would send an operator to the wrong
    // place entirely. A missing *cached row* is the real eviction case.
    let Some(entry) = resolve(&node.store, ws, ext, &version).await? else {
        return Ok(row(
            false,
            format!("no-catalog-entry (looked for {ext}@{version})"),
        ));
    };
    let Some(artifact) = read_cached(&node.store, ws, &entry.digest_hex).await? else {
        return Ok(row(false, "no-cached-bytes".into()));
    };

    Ok(
        match respawn(node, launcher, ws, &artifact, install, ts).await {
            Ok(()) => row(true, "spawned".into()),
            Err(reason) => row(false, reason),
        },
    )
}

/// Land the cached binary on disk and hand it to `install_native` (records + spawn + supervise +
/// MCP registration). Errors come back as the boot-log `reason` string — one extension's failure is
/// never the boot's.
async fn respawn<L: Launcher>(
    node: &Node,
    launcher: &L,
    ws: &str,
    artifact: &lb_registry::Artifact,
    install: Option<&lb_assets::Install>,
    ts: u64,
) -> Result<(), String> {
    let manifest =
        Manifest::parse(&artifact.manifest_toml).map_err(|e| format!("manifest: {e}"))?;
    let exec = manifest
        .native
        .as_ref()
        .map(|n| n.exec.as_str())
        .ok_or_else(|| "not-native".to_string())?;

    // The SAME deterministic install dir `ext.publish` wrote to — derived from `(ws, ext)`, so boot
    // re-derives it with no new persistence. Re-landing the cached bytes (rather than trusting the
    // file to still be there) makes this self-healing: a wiped install dir is repaired at boot.
    let install_dir = crate::ext::native_install_dir(ws, &manifest.id);
    crate::ext::write_executable(&install_dir, exec, &artifact.wasm)
        .map_err(|e| format!("write-binary: {e}"))?;

    // The approved set from the DURABLE install record — never the manifest's `requested`. A restart
    // must not re-approve what an admin narrowed: `install_native` recomputes `requested ∩ approved`,
    // so passing the stored grant reproduces exactly the privilege the install already had. Falling
    // back to an EMPTY set when the record is missing is the fail-closed direction (it would grant
    // nothing) rather than the fail-open one (`requested`).
    let approved: Vec<String> = install.map(|i| i.granted.clone()).unwrap_or_default();

    install_native(
        node,
        launcher,
        &boot_caller(ws),
        ws,
        &artifact.manifest_toml,
        install_dir.to_string_lossy().as_ref(),
        &approved,
        ts,
    )
    .await
    .map_err(|e| format!("spawn: {e}"))?;
    Ok(())
}

/// The identity boot re-applies durable intent under.
///
/// `install_native` is a *caller* verb: it gates on `mcp:native.install:call` because a human/agent
/// asking to spawn a process must hold that grant. **Boot is not a caller.** No one is asking; the
/// node is re-applying an install some admin already approved and persisted, exactly as the wasm
/// half's `load_extension` does — which takes no principal at all, because at boot there is nobody to
/// authenticate. The asymmetry is only that the native path's spawn lives behind the same gate its
/// interactive path uses.
///
/// So rather than widen that gate, or thread a caller into a boot path (which would invite passing an
/// *untrusted* one), boot names itself: a `node:boot` sub holding EXACTLY the one cap it needs, scoped
/// to the one workspace it is reconciling. It is minted in-process and never leaves it — no token is
/// signed, nothing is persisted, and it is unreachable from any request path. It cannot be used to
/// widen an install either: the grant handed to `install_native` comes from the durable record, not
/// from this principal.
///
/// The authority this represents is real but bounded, and it is authority the node already has: it
/// owns the process table. A node that may not spawn its own approved extensions cannot boot them at
/// all — which is the bug this file fixes.
fn boot_caller(ws: &str) -> Principal {
    Principal::for_key("node:boot", ws, vec!["mcp:native.install:call".to_string()])
}
