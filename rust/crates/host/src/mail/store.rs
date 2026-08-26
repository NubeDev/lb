//! Durable persistence for [`MailSource`] records — the raw verbs, run **after** the gate.
//!
//! Thin on purpose: the workspace wall is enforced by `lb_store` selecting the namespace from `ws`,
//! so a list in workspace B physically cannot return workspace A's sources. Nothing here
//! authorizes; that is [`authorize_mail_source`](super::authorize::authorize_mail_source)'s job at
//! the verb boundary.

use lb_store::{delete, read, scan_all, write, Store, StoreError};
use serde_json::Value;

use super::source::{MailSource, MAIL_SOURCE_TABLE};

/// Upsert `source` into workspace `ws`. Idempotent on `source.id`.
pub async fn save_source(store: &Store, ws: &str, source: &MailSource) -> Result<(), StoreError> {
    let value = serde_json::to_value(source).map_err(|e| StoreError::Decode(e.to_string()))?;
    write(store, ws, MAIL_SOURCE_TABLE, &source.id, &value).await
}

/// Read one source, or `None`.
pub async fn read_source(
    store: &Store,
    ws: &str,
    id: &str,
) -> Result<Option<MailSource>, StoreError> {
    let Some(value) = read(store, ws, MAIL_SOURCE_TABLE, id).await? else {
        return Ok(None);
    };
    decode_source(value).map(Some)
}

/// Every source in `ws`, sorted by id so a roster is stable between calls.
pub async fn list_sources(store: &Store, ws: &str) -> Result<Vec<MailSource>, StoreError> {
    let rows = scan_all(store, ws, MAIL_SOURCE_TABLE).await?;
    let mut sources: Vec<MailSource> = rows
        .into_iter()
        // `scan` returns the WHOLE record — the store's `{ data, rev }` envelope (`lb_store::record`)
        // — not the unwrapped host value `read` hands back. Unwrapping here is what the envelope
        // requires; reading `row.data` as the source itself silently produced an EMPTY roster while
        // `read_source` on the same record worked, which is exactly the kind of divergence a
        // `filter_map(..ok())` hides. The fallback keeps a hand-written row readable.
        .filter_map(|row| {
            let value = match row.data.get("data") {
                Some(inner) => inner.clone(),
                None => row.data,
            };
            decode_source(value).ok()
        })
        .collect();
    sources.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(sources)
}

/// Remove a source. The import ledger is deliberately NOT cascaded — see
/// [`super::remove::mail_source_delete`].
pub async fn delete_source(store: &Store, ws: &str, id: &str) -> Result<(), StoreError> {
    delete(store, ws, MAIL_SOURCE_TABLE, id).await
}

fn decode_source(value: Value) -> Result<MailSource, StoreError> {
    serde_json::from_value(value).map_err(|e| StoreError::Decode(e.to_string()))
}
