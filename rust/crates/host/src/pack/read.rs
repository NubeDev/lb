//! `pack.list` / `pack.get` — the first-class receipt reads (pack-core-scope §Goals).
//!
//! Member-read: receipts are operator documentation ("what turned this workspace into this
//! product"), and hiding them from the people the vocabulary teaches would defeat their purpose.
//! Applying stays the admin act.
//!
//! These replace the downstream `store.query`-on-`pack_receipts` convention with a real, caps-walled
//! read surface — an embedder's Packs pages call these, not the store.

use lb_auth::Principal;
use lb_store::Store;
use serde_json::{json, Value};

use super::authorize::authorize_pack;
use super::error::PackError;
use super::store::{read_receipt, scan_receipts};

/// The roster: every pack applied in `ws`. The manifest is STRIPPED — a list read should not carry
/// every pack's full vocabulary; `pack.get` is where that lives.
pub async fn pack_list(store: &Store, principal: &Principal, ws: &str) -> Result<Value, PackError> {
    authorize_pack(principal, ws, "pack.list")?;

    let receipts = scan_receipts(store, ws)
        .await
        .map_err(|e| PackError::Internal(format!("reading receipts: {e}")))?;

    Ok(json!({
        "packs": receipts.iter().map(|r| json!({
            "pack": r.pack,
            "title": r.title,
            "version": r.version,
            "manifest_checksum": r.manifest_checksum,
            "applied_ts": r.applied_ts,
            "complete": r.is_complete(),
            "objects": r.objects.len(),
        })).collect::<Vec<_>>(),
    }))
}

/// One pack's full receipt — the manifest as applied (so a reader renders entities and the insight
/// grammar without re-sending the bundle) plus every object's id, checksum, and outcome.
pub async fn pack_get(
    store: &Store,
    principal: &Principal,
    ws: &str,
    pack: &str,
) -> Result<Value, PackError> {
    authorize_pack(principal, ws, "pack.get")?;

    let receipt = read_receipt(store, ws, pack)
        .await
        .map_err(|e| PackError::Internal(format!("reading receipt: {e}")))?
        .ok_or(PackError::NotFound)?;

    serde_json::to_value(&receipt).map_err(|e| PackError::Internal(e.to_string()))
}
