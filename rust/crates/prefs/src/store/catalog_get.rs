//! Read a workspace's sparse override catalog `message_catalog:[ws, locale]` — the flat key→MF1 map,
//! or an empty map if the workspace set no override for this locale (i18n-catalogs scope). Namespace-
//! scoped (the hard wall): a read in ws-B can structurally never see ws-A's override. Raw verb; runs
//! *after* the host capability gate — this file does no authorization.

use std::collections::BTreeMap;

use lb_store::{Store, StoreError};
use serde_json::Value;

use super::catalog_schema::{define_catalog_schema, CATALOG_COLUMNS, CATALOG_TABLE};

/// Load `(ws, locale)`'s override map. Returns `(messages, has_override)` — an empty map with
/// `has_override = false` when no record exists (every key falls through to the builtin).
pub async fn get_catalog_override(
    store: &Store,
    ws: &str,
    locale: &str,
) -> Result<(BTreeMap<String, String>, bool), StoreError> {
    define_catalog_schema(store, ws).await?;
    let mut resp = store
        .query_ws(
            ws,
            &format!(
                "SELECT {CATALOG_COLUMNS} FROM type::thing('{CATALOG_TABLE}', [$ws, $locale])"
            ),
            vec![
                ("ws".into(), Value::String(ws.to_string())),
                ("locale".into(), Value::String(locale.to_string())),
            ],
        )
        .await?;
    let rows: Vec<Value> = resp
        .take(0)
        .map_err(|e| StoreError::Decode(e.to_string()))?;
    decode_first(rows)
}

/// Decode the first row's `messages` object into a flat `String -> String` map. A missing row is
/// `(empty, false)`; a present row with a null/absent `messages` is `(empty, true)` (the record
/// exists but holds no keys).
fn decode_first(rows: Vec<Value>) -> Result<(BTreeMap<String, String>, bool), StoreError> {
    match rows.into_iter().next() {
        None => Ok((BTreeMap::new(), false)),
        Some(row) => {
            let messages = row
                .get("messages")
                .and_then(|m| m.as_object())
                .map(|obj| {
                    obj.iter()
                        .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                        .collect::<BTreeMap<String, String>>()
                })
                .unwrap_or_default();
            Ok((messages, true))
        }
    }
}
