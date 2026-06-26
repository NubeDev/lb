//! The local artifact **cache** — the offline/rollback substrate (registry scope, README §6.4 "caches
//! it locally … once cached, an edge runs offline"). SurrealDB records in the workspace namespace, so
//! the cache is structurally workspace-isolated (a ws-B relay/install can never read ws-A's cache —
//! the hard wall, §7) and there is no second datastore (§3.2).
//!
//! `cache_artifact` accepts **only a [`VerifiedArtifact`]** — the load-bearing seam. Because the sole
//! constructor of that newtype is `lb_registry::verify_artifact`, an unverified artifact *cannot* reach
//! the cache: verify-before-cache is a compile-time guarantee, not a call-ordering convention. So the
//! offline path can never later serve poison (registry scope, the verify-before-cache risk).
//!
//! Keyed by content digest (`cached:{digest_hex}`): the same bytes cache once regardless of how many
//! `(ext_id, version)` point at them, and a cache hit is "I already hold exactly these verified bytes".

use lb_registry::{Artifact, VerifiedArtifact};
use lb_store::{read, write, Store, StoreError};

/// The cache table within a workspace namespace. One place owns the name so every verb agrees.
pub(crate) const TABLE: &str = "registry_cache";

/// Persist a verified artifact into workspace `ws`'s cache, keyed by its content digest. Idempotent:
/// re-caching the same digest upserts the same row (the bytes are identical by construction). Takes a
/// `VerifiedArtifact` by reference — the type proves it passed `verify_artifact`, so this verb performs
/// no check of its own; it only writes.
pub async fn cache_artifact(
    store: &Store,
    ws: &str,
    verified: &VerifiedArtifact,
) -> Result<(), StoreError> {
    let artifact = verified.artifact();
    let value = serde_json::to_value(artifact).map_err(|e| StoreError::Decode(e.to_string()))?;
    write(store, ws, TABLE, &artifact.digest_hex, &value).await
}

/// Read a cached artifact by its `digest_hex` from workspace `ws`. `None` if not cached here — which
/// is the signal `pull` uses to decide whether it must hit the `Source` (cache miss) or can serve
/// offline (cache hit). A cached artifact in another workspace is invisible (namespace-scoped read).
pub async fn read_cached(
    store: &Store,
    ws: &str,
    digest_hex: &str,
) -> Result<Option<Artifact>, StoreError> {
    match read(store, ws, TABLE, digest_hex).await? {
        Some(value) => {
            let artifact: Artifact =
                serde_json::from_value(value).map_err(|e| StoreError::Decode(e.to_string()))?;
            Ok(Some(artifact))
        }
        None => Ok(None),
    }
}
