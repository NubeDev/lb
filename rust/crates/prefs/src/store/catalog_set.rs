//! Upsert a workspace's sparse override catalog `message_catalog:[ws, locale]` from a patch — a flat
//! `key -> MF1` map merged onto the stored `messages` object (i18n-catalogs scope: "MERGE upsert; LWW
//! per message-key"). The merge is **per message-key**: a patch touching `alert.x` leaves a
//! previously-stored `alert.y` intact, so two offline edits to *different* keys both survive; a
//! same-key conflict is last-writer-wins.
//!
//! The composite id `message_catalog:[ws, locale]` is deterministic, so an offline edit replays
//! idempotently as the same per-key merge — no duplicate record. Namespace-scoped. Raw verb: the
//! host capability gate (`message.set_catalog` = admin) runs first; this file does no authorization.

use std::collections::BTreeMap;

use lb_store::{Store, StoreError};
use serde_json::{Map, Value};

use super::catalog_get::get_catalog_override;
use super::catalog_schema::{define_catalog_schema, CATALOG_TABLE};

/// Merge `patch` (flat key→MF1) into `(ws, locale)`'s override record, creating it if absent. Present
/// keys overwrite (same-key LWW); untouched keys stay. Returns the full merged map (what the caller
/// echoes / the "catalog changed" hint refers to).
pub async fn set_catalog_override(
    store: &Store,
    ws: &str,
    locale: &str,
    patch: &BTreeMap<String, String>,
) -> Result<BTreeMap<String, String>, StoreError> {
    define_catalog_schema(store, ws).await?;

    // Read-merge-write the flat map so the merge is PER KEY (SurrealDB record MERGE would replace the
    // whole `messages` object, losing keys the patch didn't mention). The composite id keeps this
    // idempotent on replay; a same-key conflict resolves last-writer-wins by construction.
    let (mut merged, _existed) = get_catalog_override(store, ws, locale).await?;
    for (k, v) in patch {
        merged.insert(k.clone(), v.clone());
    }

    let messages: Map<String, Value> = merged
        .iter()
        .map(|(k, v)| (k.clone(), Value::String(v.clone())))
        .collect();
    let mut record = Map::new();
    record.insert("ws".into(), Value::String(ws.to_string()));
    record.insert("locale".into(), Value::String(locale.to_string()));
    record.insert("messages".into(), Value::Object(messages));

    store
        .query_ws(
            ws,
            &format!("UPSERT type::thing('{CATALOG_TABLE}', [$ws, $locale]) MERGE $patch"),
            vec![
                ("ws".into(), Value::String(ws.to_string())),
                ("locale".into(), Value::String(locale.to_string())),
                ("patch".into(), Value::Object(record)),
            ],
        )
        .await?;
    Ok(merged)
}
