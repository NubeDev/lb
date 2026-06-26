//! The **catalog entry** — an artifact's metadata, resolvable without moving its bytes (registry
//! scope, README §6.4 "a catalog plus signed, versioned artifacts").
//!
//! `list_catalog`/`resolve` return these so authorization and rollback selection happen *before* any
//! transfer — the same "declared, not discovered" discipline the manifest's tool list uses. Mirrors
//! the `Visibility` the manifest already carries (public = global catalog; private = one workspace).

use serde::{Deserialize, Serialize};

use super::artifact::Artifact;

/// Where a catalog entry is visible. Independent of trust — "public" means discoverable across
/// workspaces, never "more privileged" (README §6.4, §11.5). Matches the manifest's `Visibility`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Visibility {
    Public,
    Private,
}

/// The metadata subset of an artifact — enough to list, authorize, and select a version for
/// rollback, with no bytes attached. Addressed by `(ext_id, version)`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CatalogEntry {
    pub ext_id: String,
    pub version: String,
    pub digest_hex: String,
    pub publisher_key_id: String,
    pub visibility: Visibility,
    /// Caller-injected logical timestamp (no wall-clock — testing §3).
    pub ts: u64,
}

impl CatalogEntry {
    /// Project an [`Artifact`] to its catalog metadata at logical time `ts` with `visibility`.
    pub fn of(artifact: &Artifact, visibility: Visibility, ts: u64) -> Self {
        Self {
            ext_id: artifact.ext_id.clone(),
            version: artifact.version.clone(),
            digest_hex: artifact.digest_hex.clone(),
            publisher_key_id: artifact.publisher_key_id.clone(),
            visibility,
            ts,
        }
    }
}
