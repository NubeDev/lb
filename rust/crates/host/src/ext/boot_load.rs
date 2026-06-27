//! `load_enabled` — the node's boot bring-up of **wasm** extensions (lifecycle-management scope).
//!
//! `reconcile` decides the *intent* (`enabled ∧ not-running` → start) but returns a plan, not a
//! running component, because the native respawn needs the `Launcher` the node owns. For **wasm**
//! there is no OS process to launch — "start" *is* loading the component into the runtime — and the
//! verified bytes already live in the registry **cache** (digest-keyed) from publish/install. So this
//! verb closes the wasm half of reconcile end to end: for every wasm action the plan marked `start`,
//! resolve its catalog entry → read the cached bytes → `load_extension` back into the live runtime.
//!
//! This is what makes a published extension **survive a restart**: the durable `Install` record +
//! the digest-keyed cache are the source of truth; on boot we re-load from them. Native sidecars are
//! left to the node's `Launcher` path (the plan's native actions) — this verb owns wasm only.
//!
//! No new persistence and no new trust: the bytes were verified before they were cached
//! (verify-before-cache, the `VerifiedArtifact` seam), and `load_extension` re-computes the grant from
//! the durable `Install`'s approved set, so nothing here can widen privilege.

use lb_assets::{list_installs, Tier};

use super::error::ExtError;
use super::reconcile::reconcile;
use crate::boot::Node;
use crate::load::load_extension;
use crate::registry::{read_cached, resolve};

/// One extension this verb brought online (or could not), for the boot log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadedExt {
    pub ext: String,
    pub version: String,
    /// `true` → the component is now loaded into the runtime; `false` → skipped or could not load.
    pub loaded: bool,
    /// Why, for the boot log: `"loaded"` / `"disabled"` / `"already-running"` / `"no-cached-bytes"`.
    pub reason: String,
}

/// Re-load every **enabled wasm** install for workspace `ws` from the durable cache into the live
/// runtime, honoring the boot `reconcile` plan (disabled/already-running are skipped). Returns one
/// [`LoadedExt`] per wasm install for the boot log. Idempotent against the runtime — reconcile filters
/// out already-loaded extensions, so calling this twice does not double-load.
///
/// Not capability-gated: a node-boot operation, not a caller verb (like `reconcile` itself).
pub async fn load_enabled(node: &Node, ws: &str) -> Result<Vec<LoadedExt>, ExtError> {
    let plan = reconcile(node, ws).await?;
    let installs = list_installs(&node.store, ws).await?;
    let mut out = Vec::new();

    for action in &plan.actions {
        if action.tier != "wasm" {
            continue; // native respawn is the node Launcher's job, not this verb's.
        }
        if !action.start {
            out.push(LoadedExt {
                ext: action.ext.clone(),
                version: String::new(),
                loaded: false,
                reason: action.reason.clone(),
            });
            continue;
        }

        // Find the installed version, then resolve its catalog entry → digest → cached bytes.
        let install = installs
            .iter()
            .find(|i| i.ext_id == action.ext && i.tier == Tier::Wasm);
        let version = install.map(|i| i.version.clone()).unwrap_or_default();

        let bytes = match resolve(&node.store, ws, &action.ext, &version).await? {
            Some(entry) => read_cached(&node.store, ws, &entry.digest_hex).await?,
            None => None,
        };

        match bytes {
            Some(artifact) => {
                let approved = install.map(|i| i.granted.clone()).unwrap_or_default();
                load_extension(node, &artifact.manifest_toml, &artifact.wasm, &approved)
                    .await
                    .map_err(|e| ExtError::Manifest(e.to_string()))?;
                out.push(LoadedExt {
                    ext: action.ext.clone(),
                    version,
                    loaded: true,
                    reason: "loaded".into(),
                });
            }
            None => out.push(LoadedExt {
                ext: action.ext.clone(),
                version,
                loaded: false,
                reason: "no-cached-bytes".into(),
            }),
        }
    }
    Ok(out)
}
