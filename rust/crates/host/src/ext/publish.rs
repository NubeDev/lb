//! `ext.publish` — upload a signed extension artifact into this node's catalog (lifecycle-management
//! scope: the browser admin console's "publish an extension" path, over the gateway). Gated
//! `mcp:ext.publish:call`, **workspace-first** (the workspace comes from the authenticated token, never
//! the upload — the hard wall, §7), and **verify-before-store**: the artifact is checked against the
//! workspace's publisher allow-list BEFORE a byte is persisted (authenticity before authority).
//!
//! It introduces **no new storage** — it reuses the registry service's own cache + catalog seam
//! (`cache_artifact` takes a `VerifiedArtifact`, so an unverified upload *cannot* reach the cache; the
//! type system, not call ordering, enforces verify-before-store). Idempotent on the artifact's content
//! digest + `(ext_id, version)`: re-publishing the same signed bytes upserts the same rows (no-op
//! success), exactly like the registry-host `ArtifactStore::publish`.

use lb_auth::Principal;
use lb_ext_loader::Manifest;
use lb_mcp::authorize_tool;
use lb_registry::{verify_artifact, Artifact, TrustedKeys, Visibility};
use lb_supervisor::OsLauncher;

use super::error::ExtError;
use super::install_dir::{native_install_dir, write_executable};
use crate::boot::Node;
use crate::install::install_extension;
use crate::native::install_native;
use crate::registry::{cache_artifact, record_catalog};

/// Publish `artifact` into workspace `ws`'s catalog for `caller`, then **install + load it live**:
/// gate, **verify against `trusted` BEFORE storing**, cache the verified bytes + record the catalog
/// entry with `visibility`, then run the S4 install (persist the durable `Install` grant record and
/// `load_extension` the component into the running runtime) so an uploaded extension is *reachable
/// immediately*, not merely cataloged (lifecycle-management scope: "publish then install" — the gap
/// where publish previously stopped at the catalog and nothing brought the component online).
///
/// Idempotent — re-publishing the same bytes upserts every record and reloads the component. A
/// tampered/unsigned/foreign-key upload is rejected and **nothing is stored** ([`ExtError::Unverified`]).
///
/// The grant set is the manifest's **requested** caps: the `ext.publish` caller IS the workspace admin
/// approving the install (the admin-console action), so `admin_approved = requested` here. The grant
/// is still computed as `requested ∩ admin_approved` in `install_extension`, so the trust model is
/// unchanged — a real review step narrows `admin_approved` later without touching this seam.
pub async fn ext_publish(
    node: &Node,
    caller: &Principal,
    ws: &str,
    artifact: Artifact,
    trusted: &TrustedKeys,
    visibility: Visibility,
    ts: u64,
) -> Result<(), ExtError> {
    // Gate 1: the MCP surface — workspace-first, then mcp:ext.publish:call. Opaque on denial.
    authorize_tool(caller, ws, "ext.publish").map_err(|_| ExtError::Denied)?;

    // Gate 2 (independent): authenticity. Verify the digest + signature against the workspace's
    // publisher allow-list. On any failure nothing is stored — the verify-before-store guarantee.
    let verified = verify_artifact(artifact, trusted).map_err(|_| ExtError::Unverified)?;

    // Persist the VERIFIED bytes through the existing registry cache + catalog seam (no new store).
    // `cache_artifact` accepts only a `VerifiedArtifact`, so the unverified bytes never had a path here.
    cache_artifact(&node.store, ws, &verified).await?;
    record_catalog(&node.store, ws, verified.artifact(), visibility, ts).await?;

    // Bring it online: persist the durable install grant, then load the component into the runtime.
    // The publisher (this caller) is the approver, so admin_approved = the manifest's requested caps.
    let artifact = verified.artifact();
    let manifest =
        Manifest::parse(&artifact.manifest_toml).map_err(|e| ExtError::Manifest(e.to_string()))?;
    if manifest.tier == "native" {
        let install_dir = native_install_dir(ws, &manifest.id);
        let exec = manifest
            .native
            .as_ref()
            .map(|n| n.exec.as_str())
            .ok_or_else(|| ExtError::Manifest("native manifest missing exec".into()))?;
        write_executable(&install_dir, exec, &artifact.wasm)?;
        install_native(
            node,
            &OsLauncher,
            caller,
            ws,
            &artifact.manifest_toml,
            install_dir.to_string_lossy().as_ref(),
            &manifest.requested_caps,
            ts,
        )
        .await
        .map_err(|e| ExtError::Native(e.to_string()))?;
    } else {
        install_extension(
            node,
            ws,
            &artifact.manifest_toml,
            &artifact.wasm,
            &manifest.requested_caps,
            ts,
        )
        .await
        .map_err(|e| ExtError::Manifest(e.to_string()))?;
    }
    Ok(())
}
