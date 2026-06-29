//! The `query:{ws}:{id}` store record (query scope). The ONLY platform record for a saved query —
//! workspace-walled, in the one datastore (rule 2). It holds the authoring `lang` (`prql`|`raw`), the
//! `text`, the `target` (`"platform"` | `"datasource:<name>"`), and the declared `params` (the `$var`
//! names bound at run). `id` is the kebab-case slug unique per workspace; `name` is the editable
//! display label (mirrors the rules `id` + `name` pattern). Soft-delete + `ts` like every saved record.

use lb_store::{read, write, Store, StoreError};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A saved query. `id` is the workspace-unique slug; `name` is the display label; `lang` is `prql` or
/// `raw`; `text` is the PRQL (or raw SQL/SurrealQL); `target` is `platform` or `datasource:<name>`;
/// `params` are the declared `$var` names bound at run time.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SavedQuery {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub lang: String,
    pub text: String,
    pub target: String,
    #[serde(default)]
    pub params: Vec<String>,
    /// A constant discriminator so `query.list` can enumerate via the store's field-equality list.
    #[serde(default = "query_tag")]
    pub tag: String,
    /// Soft-delete tombstone (`query.delete`): a removed query reads as absent on resolve/list.
    #[serde(default)]
    pub removed: bool,
    pub ts: u64,
}

/// The constant `tag` value every saved-query record carries (the list discriminator).
pub fn query_tag() -> String {
    "query".to_string()
}

/// The store table for saved-query records.
pub const TABLE: &str = "query";

impl SavedQuery {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        description: impl Into<String>,
        lang: impl Into<String>,
        text: impl Into<String>,
        target: impl Into<String>,
        params: Vec<String>,
        ts: u64,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            description: description.into(),
            lang: lang.into(),
            text: text.into(),
            target: target.into(),
            params,
            tag: query_tag(),
            removed: false,
            ts,
        }
    }
}

/// Persist (upsert) a saved query in `ws`. Workspace-namespaced by the store (the hard wall).
pub async fn put(store: &Store, ws: &str, q: &SavedQuery) -> Result<(), StoreError> {
    let value = serde_json::to_value(q).map_err(|e| StoreError::Decode(e.to_string()))?;
    write(store, ws, TABLE, &q.id, &value).await
}

/// Resolve `id` to its saved-query record in `ws`. `None` if absent OR tombstoned — exactly what a
/// cross-tenant id resolves to (a ws-B caller naming a ws-A query finds nothing; the wall is
/// structural at the namespace).
pub async fn resolve(store: &Store, ws: &str, id: &str) -> Result<Option<SavedQuery>, StoreError> {
    let Some(value) = read(store, ws, TABLE, id).await? else {
        return Ok(None);
    };
    let q: SavedQuery = decode(value)?;
    if q.removed {
        return Ok(None);
    }
    Ok(Some(q))
}

fn decode(value: Value) -> Result<SavedQuery, StoreError> {
    serde_json::from_value(value).map_err(|e| StoreError::Decode(e.to_string()))
}
