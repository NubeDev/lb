//! The "catalog changed" bus hint (i18n-catalogs scope, pinned key + payload) — published by
//! `message.set_catalog` after the store write so open clients re-fetch `prefs.catalog` and re-render.
//! State stays in the store (§3 rule 3: the store holds state, the bus only moves motion); this is a
//! fire-and-forget nudge, mirroring the shipped "prefs changed" hint.
//!
//!   key:     ws/{ws}/prefs/catalog-changed   (the `ws/{ws}/` prefix is added by `lb_bus`)
//!   payload: { "locale": "es", "catalog_version": "…" }

use lb_bus::{publish, Bus};
use serde_json::json;

/// The workspace-relative subject the hint rides on. `lb_bus` walls it under `ws/{id}/` →
/// `ws/{id}/prefs/catalog-changed`.
pub fn catalog_changed_subject() -> &'static str {
    "prefs/catalog-changed"
}

/// Publish the "catalog changed" hint for `(ws, locale)`. Best-effort: a serialization or bus error
/// is dropped (the durable override record is the source of truth; a late client catches up on its
/// next `prefs.catalog` fetch).
pub async fn publish_catalog_changed(bus: &Bus, ws: &str, locale: &str, catalog_version: &str) {
    let payload = json!({ "locale": locale, "catalog_version": catalog_version });
    let Ok(bytes) = serde_json::to_vec(&payload) else {
        return;
    };
    let _ = publish(bus, ws, catalog_changed_subject(), &bytes).await;
}
