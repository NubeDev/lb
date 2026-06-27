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
use lb_mcp::authorize_tool;
use lb_registry::{verify_artifact, Artifact, TrustedKeys, Visibility};

use super::error::ExtError;
use crate::boot::Node;
use crate::registry::{cache_artifact, record_catalog};

/// Publish `artifact` into workspace `ws`'s catalog for `caller`: gate, then **verify against
/// `trusted` BEFORE storing**, then cache the verified bytes + record the catalog entry with
/// `visibility` at logical time `ts`. Idempotent — re-publishing the same bytes upserts. A
/// tampered/unsigned/foreign-key upload is rejected and **nothing is stored** ([`ExtError::Unverified`]).
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
    Ok(())
}
