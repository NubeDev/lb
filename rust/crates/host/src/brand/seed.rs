//! `seed_default_brand` — the idempotent boot seeder (reports scope, "Brand profiles": "the default
//! `brand` seed" so pickers are never empty). If the workspace has no brand yet, write ONE neutral
//! default `brand:default`. Idempotent: a second call, or a call in a workspace that already has any
//! brand, is a no-op.
//!
//! NOTE: a richer seed would derive the initial name/logo from the workspace `ui_branding` blob
//! (workspace *identity*). That is deliberately deferred here to keep this module store-only and
//! dependency-light — a follow-up wires the `ui_branding`-derived seed.

use lb_store::{Store, StoreError};

use super::model::{Brand, SCHEMA_VERSION};
use super::store::{scan_brands, write_brand};

/// The id of the seeded default brand — the picker's non-empty fallback.
pub const DEFAULT_BRAND_ID: &str = "default";

/// Seed a neutral default brand into `ws` if none exists. No-op when any brand is already present
/// (including a tombstoned one — a workspace that deliberately deleted every brand is not re-seeded).
pub async fn seed_default_brand(store: &Store, ws: &str, now: u64) -> Result<(), StoreError> {
    let existing = scan_brands(store, ws).await?;
    if !existing.is_empty() {
        return Ok(());
    }
    let brand = Brand {
        id: DEFAULT_BRAND_ID.to_string(),
        name: "Default".to_string(),
        // A seed has no human owner — the system owns it. Owner-only update still lets a member
        // create their own brands; this one is a workspace-shared read-only default in practice.
        owner: "system".to_string(),
        schema_version: SCHEMA_VERSION,
        updated_ts: now,
        ..Brand::default()
    };
    write_brand(store, ws, &brand).await
}
