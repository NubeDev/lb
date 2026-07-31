//! `ext.publish` — upload a signed extension artifact into this node's catalog (lifecycle-management
//! scope: the browser admin console's "publish an extension" path, over the gateway). Gated
//! `mcp:ext.publish:call`, **workspace-first** (the workspace comes from the authenticated token, never
//! the upload — the hard wall, §7), and **verify-before-store**: the artifact is checked against the
//! workspace's publisher allow-list BEFORE a byte is persisted (authenticity before authority).
//!
//! The signature half of that check is waivable on a development node (`Authenticity`,
//! `LB_EXT_UNTRUSTED_KEY=allow`); the **content-digest half never is**, so even a waived node refuses
//! a corrupt or tampered upload. See `lb_registry::verify` for the contract.
//!
//! It introduces **no new storage** — it reuses the registry service's own cache + catalog seam
//! (`cache_artifact` takes a `VerifiedArtifact`, so an upload that failed verification *cannot* reach
//! the cache; the type system, not call ordering, enforces verify-before-store). Idempotent on the artifact's content
//! digest + `(ext_id, version)`: re-publishing the same signed bytes upserts the same rows (no-op
//! success), exactly like the registry-host `ArtifactStore::publish`.

use lb_auth::Principal;
use lb_ext_loader::Manifest;
use lb_mcp::authorize_tool;
use lb_registry::{verify_artifact_with, Artifact, Authenticity, TrustedKeys, Visibility};
use lb_supervisor::OsLauncher;

use super::error::ExtError;
use super::install_dir::{native_install_dir, write_executable};
use crate::boot::Node;
use crate::install::install_extension;
use crate::native::install_native;
use crate::registry::{cache_artifact, read_cached, record_catalog};

/// Reject an artifact whose `(ext_id, version)` metadata contradicts the manifest it carries.
///
/// These two are keyed apart and independently controlled: the **catalog** is addressed by
/// `Artifact.ext_id`/`Artifact.version` (`CatalogEntry::of`), while the **install record** is
/// addressed by `Manifest.id`/`Manifest.version` (`Install::new`). Crucially the content digest
/// commits to exactly `(manifest_toml, wasm)` — it does **not** cover `ext_id`/`version` — so
/// `verify_artifact` cannot catch a disagreement: those two fields are unsigned, and nothing
/// downstream reconciled them.
///
/// The consequence was a silent strand. Publish reads the manifest and succeeds; boot bring-up
/// reads the version from the *install* record and resolves the *catalog* by it, so a mismatch means
/// `resolve` finds nothing and the extension never comes back — reported as `no-cached-artifact`,
/// which sends an operator hunting an evicted cache that was never the problem. Same shape for a
/// mismatched `ext_id`. Both tiers resolve this way, so both were affected.
///
/// So the rule is enforced at the door: an artifact whose metadata contradicts its own signed
/// manifest is not a coherent artifact, and there is no sound way to pick a winner between them.
/// Fail-closed at publish, where the operator is present to read the error.
///
/// Validating the unsigned copies against the signed manifest — rather than extending the digest to
/// cover them — is deliberate: the manifest is *already* signed, so this closes the gap with no new
/// trust, while changing the digest would be a signing-format break that invalidates every existing
/// signed artifact for no extra safety.
fn coherent(artifact: &Artifact, manifest: &Manifest) -> Result<(), ExtError> {
    if artifact.ext_id != manifest.id {
        return Err(ExtError::Manifest(format!(
            "artifact metadata disagrees with its manifest: ext_id {:?} but the manifest declares \
             id {:?} — the catalog would be keyed by one and the install record by the other",
            artifact.ext_id, manifest.id
        )));
    }
    if artifact.version != manifest.version {
        return Err(ExtError::Manifest(format!(
            "artifact metadata disagrees with its manifest: {} version {:?} but the manifest \
             declares {:?} — the catalog would be keyed by one and the install record by the other",
            manifest.id, artifact.version, manifest.version
        )));
    }
    Ok(())
}

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
/// `authenticity` selects whether the publisher-signature half of that gate is enforced.
/// [`Authenticity::Required`] is production and the default everywhere; a node configured with
/// `LB_EXT_UNTRUSTED_KEY=allow` passes [`Authenticity::WaivedUntrustedKey`], which accepts a
/// **foreign-key** upload but still rejects a **tampered** one — the digest check is not waivable.
/// Every waived publish emits a `warn!` naming the artifact and the unchecked key.
///
/// The grant set is the manifest's **requested** caps: the `ext.publish` caller IS the workspace admin
/// approving the install (the admin-console action), so `admin_approved = requested` here. The grant
/// is still computed as `requested ∩ admin_approved` in `install_extension`, so the trust model is
/// unchanged — a real review step narrows `admin_approved` later without touching this seam.
#[allow(clippy::too_many_arguments)]
pub async fn ext_publish(
    node: &Node,
    caller: &Principal,
    ws: &str,
    artifact: Artifact,
    trusted: &TrustedKeys,
    authenticity: Authenticity,
    visibility: Visibility,
    ts: u64,
) -> Result<(), ExtError> {
    // Gate 1: the MCP surface — workspace-first, then mcp:ext.publish:call. Opaque on denial.
    authorize_tool(caller, ws, "ext.publish").map_err(|_| ExtError::Denied)?;

    // Gate 2 (independent): integrity, and — unless the operator waived it — authenticity. Verify
    // the digest and (normally) the signature against the workspace's publisher allow-list. On any
    // failure nothing is stored: the verify-before-store guarantee. The digest half runs either way,
    // so a corrupt upload is rejected even on a node with the escape hatch on.
    let verified =
        verify_artifact_with(artifact, trusted, authenticity).map_err(|_| ExtError::Unverified)?;

    // Loud on every waived artifact, not just once at boot. A boot warning scrolls out of a journal
    // within hours; this is what an operator debugging "why did this install?" a week later finds,
    // and it names exactly which artifact and which unchecked key got in.
    if verified.authenticity().is_waived() {
        tracing::warn!(
            ws = %ws,
            ext_id = %verified.artifact().ext_id,
            version = %verified.artifact().version,
            publisher_key_id = %verified.artifact().publisher_key_id,
            "PUBLISHER TRUST GATE WAIVED: accepted an extension without checking its publisher \
             signature (LB_EXT_UNTRUSTED_KEY=allow). Content integrity was still verified. This \
             node will install code from any publisher — development use only."
        );
    }

    // Gate 3: COHERENCE — the artifact's `(ext_id, version)` must agree with the manifest they
    // carry. Both are parsed BEFORE anything is stored, so an incoherent upload leaves no trace
    // (the same verify-before-store guarantee gate 2 gives, extended to this check).
    let manifest = Manifest::parse(&verified.artifact().manifest_toml)
        .map_err(|e| ExtError::Manifest(e.to_string()))?;
    coherent(verified.artifact(), &manifest)?;

    // Persist the VERIFIED bytes through the existing registry cache + catalog seam (no new store).
    // `cache_artifact` accepts only a `VerifiedArtifact`, so the unverified bytes never had a path here.
    // Cache-hit guard: the cache is keyed by content digest, so a hit means the store already
    // holds exactly these bytes — re-writing them would append the FULL multi-MB payload to the
    // append-only commit log on every re-publish (the log grows unbounded and boot replays it).
    // `pull` has always had this guard; publish gets the same one. Catalog/install still run —
    // only the byte payload write is skipped.
    if read_cached(&node.store, ws, &verified.artifact().digest_hex)
        .await?
        .is_none()
    {
        cache_artifact(&node.store, ws, &verified).await?;
    }
    record_catalog(&node.store, ws, verified.artifact(), visibility, ts).await?;

    // Bring it online: persist the durable install grant, then load the component into the runtime.
    // The publisher (this caller) is the approver, so admin_approved = the manifest's requested caps.
    let artifact = verified.artifact();
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
