//! `pull` — fetch · verify · cache an artifact, serving the cached copy offline (registry scope,
//! README §6.4: "pulls an artifact on demand, verifies its signature, caches it locally … once
//! cached, an edge runs offline"). The load-bearing verb of the slice.
//!
//! The offline-first path: if this workspace already resolved `(ext_id, version)` (a catalog entry
//! holds its digest) AND the bytes for that digest are cached, return them **without calling the
//! `Source`** — that is what lets a node install offline. Only a cache miss hits the source, and a
//! fetched artifact is **verified before it is cached** (the type system enforces this: `cache_artifact`
//! takes a `VerifiedArtifact`, minted only by `verify_artifact`). A tampered/unsigned artifact is
//! refused here — never cached, never returned.

use lb_registry::{verify_artifact, Artifact, TrustedKeys, Visibility};
use lb_store::Store;

use super::cache::{cache_artifact, read_cached};
use super::catalog::{record_catalog, resolve};
use super::error::RegistryServiceError;
use super::source::Source;

/// Pull `ext_id`@`version` into workspace `ws`'s cache and return the verified artifact. `trusted` is
/// the workspace's publisher-key allow-list; `visibility` is recorded with the catalog entry; `ts` is
/// the injected logical timestamp. Cache hit → no `Source` call (offline path); cache miss → fetch,
/// `verify_artifact`, cache, record catalog. Verification failure → [`RegistryServiceError::Unverified`].
///
/// Raw-ish verb at the host layer — the caller (`install_from_registry`) has already passed the gate.
pub async fn pull<S: Source>(
    store: &Store,
    source: &S,
    ws: &str,
    ext_id: &str,
    version: &str,
    trusted: &TrustedKeys,
    visibility: Visibility,
    ts: u64,
) -> Result<Artifact, RegistryServiceError> {
    // OFFLINE PATH: do we already know this version's digest (a prior resolve/pull) and hold its
    // bytes? If so, serve from cache and never touch the source — the edge runs offline (§6.4).
    if let Some(entry) = resolve(store, ws, ext_id, version).await? {
        if let Some(cached) = read_cached(store, ws, &entry.digest_hex).await? {
            return Ok(cached);
        }
    }

    // CACHE MISS: fetch from the (untrusted) source. An offline source errors here → NotAvailable.
    let fetched = source.fetch(ext_id, version).await?;

    // VERIFY BEFORE CACHE: prove the digest + signature against the allow-listed publisher key. On
    // failure nothing is cached and nothing is returned (the verify-before-cache guarantee).
    let verified = verify_artifact(fetched, trusted)?;

    // Cache the verified bytes and record the catalog entry so the next pull can serve offline + the
    // version is selectable for rollback.
    cache_artifact(store, ws, &verified).await?;
    record_catalog(store, ws, verified.artifact(), visibility, ts).await?;

    Ok(verified.into_artifact())
}
