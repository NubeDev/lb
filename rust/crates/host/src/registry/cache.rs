//! The local artifact **cache** — the offline/rollback substrate (registry scope, README §6.4 "caches
//! it locally … once cached, an edge runs offline"). Metadata is a SurrealDB record in the workspace
//! namespace; the payload bytes are a content-addressed file (`blob`, workspace-scoped in its own
//! path) — split so caching a real native sidecar binary never pays JSON's decimal-int-array bloat on
//! `wasm: Vec<u8>` server-side (the same defect the zip-transport upload fix closes on the wire).
//! Structurally workspace-isolated either way (a ws-B relay/install can never read ws-A's cache — the
//! hard wall, §7): the SurrealDB row by namespace, the blob file by its path (`blob`'s own tests pin
//! this), so there is still no SEPARATE datastore to reason about (§3.2) — one workspace-scoped cache,
//! two storage shapes for its two very differently-sized halves.
//!
//! `cache_artifact` accepts **only a [`VerifiedArtifact`]** — the load-bearing seam. Because the sole
//! constructor of that newtype is `lb_registry::verify_artifact`, an unverified artifact *cannot* reach
//! the cache: verify-before-cache is a compile-time guarantee, not a call-ordering convention. So the
//! offline path can never later serve poison (registry scope, the verify-before-cache risk).
//!
//! Keyed by content digest (`cached:{digest_hex}`): the same bytes cache once regardless of how many
//! `(ext_id, version)` point at them, and a cache hit is "I already hold exactly these verified bytes".
//! The blob write shares that idempotency (`blob::write_blob_atomic` no-ops if the path already
//! exists) rather than re-writing bytes already known identical.

use lb_registry::{Artifact, VerifiedArtifact};
use lb_store::{read, write, Store, StoreError};
use serde::{Deserialize, Serialize};

use super::blob::{artifact_blob_path, read_blob, write_blob_atomic};

/// The cache table within a workspace namespace. One place owns the name so every verb agrees.
pub(crate) const TABLE: &str = "registry_cache";

/// The SurrealDB half of a cached artifact — every `Artifact` field except `wasm`, which lives in the
/// blob file at `blob::artifact_blob_path(ws, digest_hex)` instead. Same fields the zip-transport's
/// `CommentMeta` carries, for the same reason: the large, opaque payload doesn't belong serialized
/// into a JSON value at all.
#[derive(Serialize, Deserialize)]
struct CachedMeta {
    ext_id: String,
    version: String,
    manifest_toml: String,
    digest_hex: String,
    publisher_key_id: String,
    signature: Vec<u8>,
}

/// Persist a verified artifact into workspace `ws`'s cache, keyed by its content digest. Idempotent:
/// re-caching the same digest upserts the same metadata row and no-ops the blob write (the bytes are
/// identical by construction). Takes a `VerifiedArtifact` by reference — the type proves it passed
/// `verify_artifact`, so this verb performs no check of its own; it only writes.
pub async fn cache_artifact(
    store: &Store,
    ws: &str,
    verified: &VerifiedArtifact,
) -> Result<(), StoreError> {
    let artifact = verified.artifact();
    write_blob_atomic(
        &artifact_blob_path(ws, &artifact.digest_hex),
        &artifact.wasm,
    )?;
    let meta = CachedMeta {
        ext_id: artifact.ext_id.clone(),
        version: artifact.version.clone(),
        manifest_toml: artifact.manifest_toml.clone(),
        digest_hex: artifact.digest_hex.clone(),
        publisher_key_id: artifact.publisher_key_id.clone(),
        signature: artifact.signature.clone(),
    };
    let value = serde_json::to_value(&meta).map_err(|e| StoreError::Decode(e.to_string()))?;
    write(store, ws, TABLE, &artifact.digest_hex, &value).await
}

/// Read a cached artifact by its `digest_hex` from workspace `ws`. `None` if not cached here — which
/// is the signal `pull` uses to decide whether it must hit the `Source` (cache miss) or can serve
/// offline (cache hit). A cached artifact in another workspace is invisible (namespace-scoped
/// metadata read AND workspace-scoped blob path — see `blob`'s own isolation tests).
pub async fn read_cached(
    store: &Store,
    ws: &str,
    digest_hex: &str,
) -> Result<Option<Artifact>, StoreError> {
    match read(store, ws, TABLE, digest_hex).await? {
        Some(value) => {
            let meta: CachedMeta =
                serde_json::from_value(value).map_err(|e| StoreError::Decode(e.to_string()))?;
            let wasm = read_blob(&artifact_blob_path(ws, digest_hex))?;
            Ok(Some(Artifact {
                ext_id: meta.ext_id,
                version: meta.version,
                manifest_toml: meta.manifest_toml,
                wasm,
                digest_hex: meta.digest_hex,
                publisher_key_id: meta.publisher_key_id,
                signature: meta.signature,
            }))
        }
        None => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use ed25519_dalek::Signer;
    use lb_registry::{
        digest, digest_hex, verify_artifact, Artifact as RawArtifact, PublisherKey, TrustedKeys,
    };
    use lb_store::Store;

    use super::*;

    fn sample_verified(wasm: Vec<u8>) -> VerifiedArtifact {
        let manifest = "id = \"x\"\nversion = \"0.1.0\"\n".to_string();
        let d = digest(&manifest, &wasm);
        let sk = ed25519_dalek::SigningKey::from_bytes(&[9u8; 32]);
        let artifact = RawArtifact {
            ext_id: "x".into(),
            version: "0.1.0".into(),
            manifest_toml: manifest,
            wasm,
            digest_hex: digest_hex(&d),
            publisher_key_id: "k".into(),
            signature: sk.sign(&d).to_bytes().to_vec(),
        };
        let pk = PublisherKey::from_bytes(&sk.verifying_key().to_bytes()).unwrap();
        let trusted = TrustedKeys::from([("k".to_string(), pk)]);
        verify_artifact(artifact, &trusted).expect("verifies")
    }

    #[tokio::test]
    async fn cache_then_read_round_trips_the_whole_artifact() {
        // The shared, once-only LB_DIR (see `blob::TEST_LB_DIR`'s doc comment for why this must
        // never be a per-test `set_var`/`remove_var`). This test's digest is unique to its own
        // content, so it cannot collide with another test's blob under the same fixed root.
        std::sync::LazyLock::force(&super::super::blob::TEST_LB_DIR);
        let store = Store::memory().await.unwrap();
        let verified = sample_verified(b"payload bytes".to_vec());
        let digest_hex = verified.artifact().digest_hex.clone();
        let expected = verified.artifact().clone();

        cache_artifact(&store, "nube", &verified).await.unwrap();
        let read_back = read_cached(&store, "nube", &digest_hex)
            .await
            .unwrap()
            .expect("cached");
        assert_eq!(read_back, expected);
    }

    #[tokio::test]
    async fn a_miss_is_none_not_an_error() {
        // No blob is ever written on a cache miss (the metadata read short-circuits first), so this
        // test never touches the filesystem — no LB_DIR setup needed.
        let store = Store::memory().await.unwrap();
        assert!(read_cached(&store, "nube", &"a".repeat(64))
            .await
            .unwrap()
            .is_none());
    }

    #[tokio::test]
    async fn the_surrealdb_row_never_grows_with_payload_size() {
        // The exact defect being fixed: the metadata row must stay small regardless of `wasm`'s
        // size, proving the payload never gets JSON-serialized into the store.
        std::sync::LazyLock::force(&super::super::blob::TEST_LB_DIR);
        let store = Store::memory().await.unwrap();
        let verified = sample_verified(vec![0xABu8; 2 * 1024 * 1024]);
        let digest_hex = verified.artifact().digest_hex.clone();
        cache_artifact(&store, "nube", &verified).await.unwrap();

        let raw_row = read(&store, "nube", TABLE, &digest_hex)
            .await
            .unwrap()
            .expect("row exists");
        let row_bytes = serde_json::to_vec(&raw_row).unwrap().len();
        assert!(
            row_bytes < 4096,
            "the metadata row must stay well under 4 KiB regardless of a 2 MiB payload, got {row_bytes}"
        );
    }
}
