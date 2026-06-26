//! The **catalog** read/record verbs — artifact metadata resolvable without moving bytes (registry
//! scope, README §6.4). A `CatalogEntry` is recorded in the workspace namespace whenever a verified
//! artifact is cached, so `list_catalog`/`resolve` answer "what versions can this workspace install"
//! without a `Source` round-trip — the same "declared, not discovered" discipline the manifest's tool
//! list uses, and the basis for offline rollback selection.
//!
//! S7-first decision (registry scope open question "public catalog storage"): catalog entries are
//! **workspace-namespaced**. A private artifact's entry is structurally invisible to another workspace
//! (the hard wall, §7) — exactly the isolation the mandatory test demands. A shared `public` namespace
//! resolved read-only is the deferred follow-up; recording per-workspace now keeps the isolation
//! guarantee airtight and the visibility union a later additive change, not a re-cut.

use lb_registry::{Artifact, CatalogEntry, Visibility};
use lb_store::{list, write, Store, StoreError};

/// The catalog table within a workspace namespace.
pub(crate) const TABLE: &str = "registry_catalog";

/// Record (upsert) the catalog entry for a verified `artifact` at logical time `ts`, with
/// `visibility`. Addressed by `{ext_id}:{version}` so re-recording the same version is idempotent and
/// distinct versions of one extension coexist (the precondition for rollback).
pub async fn record_catalog(
    store: &Store,
    ws: &str,
    artifact: &Artifact,
    visibility: Visibility,
    ts: u64,
) -> Result<(), StoreError> {
    let entry = CatalogEntry::of(artifact, visibility, ts);
    let value = serde_json::to_value(&entry).map_err(|e| StoreError::Decode(e.to_string()))?;
    let id = format!("{}:{}", entry.ext_id, entry.version);
    write(store, ws, TABLE, &id, &value).await
}

/// List every catalog entry for `ext_id` visible to workspace `ws` (its private entries; the public
/// union is the deferred follow-up). Empty if none — never another workspace's rows. The caller picks
/// a version to install/roll-back to from this.
pub async fn list_catalog(
    store: &Store,
    ws: &str,
    ext_id: &str,
) -> Result<Vec<CatalogEntry>, StoreError> {
    let rows = list(store, ws, TABLE, "ext_id", ext_id).await?;
    rows.into_iter()
        .map(|v| serde_json::from_value(v).map_err(|e| StoreError::Decode(e.to_string())))
        .collect()
}

/// Resolve a single `(ext_id, version)` catalog entry in workspace `ws`, or `None`. Used to find an
/// artifact's digest before deciding whether the bytes are already cached (the offline check).
pub async fn resolve(
    store: &Store,
    ws: &str,
    ext_id: &str,
    version: &str,
) -> Result<Option<CatalogEntry>, StoreError> {
    Ok(list_catalog(store, ws, ext_id)
        .await?
        .into_iter()
        .find(|e| e.version == version))
}
