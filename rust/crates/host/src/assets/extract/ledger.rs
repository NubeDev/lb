//! The extraction ledger — read/write the provenance records that make re-runs idempotent
//! (doc-extraction scope). Raw store verbs only (no auth — the caller was gated upstream, exactly
//! like `lb_assets`/`lb_jobs` are auth-free store layers). Workspace-namespaced, so a ledger read
//! never crosses the tenancy wall.

use lb_store::{read, write, Store, StoreError};

use super::model::{Extraction, EXTRACTION_TABLE};

/// Read the derivation record for a `(media, extractor family)` lineage, if any. `None` = never
/// derived (or another workspace's — the namespace makes it invisible).
pub async fn get_extraction(
    store: &Store,
    ws: &str,
    media_id: &str,
    extractor_id: &str,
) -> Result<Option<Extraction>, StoreError> {
    let id = Extraction::make_id(media_id, extractor_id);
    match read(store, ws, EXTRACTION_TABLE, &id).await? {
        Some(v) => {
            let rec: Extraction =
                serde_json::from_value(v).map_err(|e| StoreError::Decode(e.to_string()))?;
            Ok(Some(rec))
        }
        None => Ok(None),
    }
}

/// Upsert a derivation record. Idempotent on its stable id — a re-derivation at a new version
/// overwrites the SAME row (so the ledger always reflects the current derivation, and a count of
/// records equals the count of distinct `(media, extractor)` lineages, never a growing log).
pub async fn put_extraction(store: &Store, ws: &str, rec: &Extraction) -> Result<(), StoreError> {
    let value = serde_json::to_value(rec).map_err(|e| StoreError::Decode(e.to_string()))?;
    write(store, ws, EXTRACTION_TABLE, &rec.id, &value).await
}

/// Whether an existing derivation is still valid for this request: same checksum AND (no forced
/// version, or the existing version already meets the forced floor). A checksum change or a version
/// bump past the floor means "re-derive". This is the one idempotency decision, in one place.
pub fn is_fresh(existing: &Extraction, checksum: &str, force_version: Option<u32>) -> bool {
    if existing.media_checksum != checksum {
        return false;
    }
    match force_version {
        Some(floor) => existing.extractor_version >= floor,
        None => true,
    }
}
