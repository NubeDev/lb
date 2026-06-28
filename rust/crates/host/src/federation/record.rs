//! The `datasource:{ws}:{name}` store record (datasources scope). The ONLY platform record for a
//! federated source — workspace-walled, in the one datastore. It holds the source **kind**, the
//! **endpoint** (`host:port`, the net:* subject), and the **secret ref** (`secret:federation/{name}`
//! — a POINTER, never the DSN value). The connection string lives in `lb-secrets`, mediated by the
//! host and handed to the sidecar at query time; it never lands in this record, a log, or a result
//! (secret mediation, rule 5/§6.7).

use lb_store::{read, write, Store, StoreError};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A registered datasource. `name` is the workspace-unique alias a caller names; `kind` selects the
/// engine (`postgres`/`timescale`/`sqlite`); `endpoint` is the `host:port` the net:* grant gates;
/// `secret_ref` is the `lb-secrets` path holding the DSN (never the DSN itself).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Datasource {
    pub name: String,
    pub kind: String,
    pub endpoint: String,
    pub secret_ref: String,
    /// A constant discriminator so `datasource.list` can enumerate every source via the store's
    /// field-equality list (there is no "list a whole table" verb; this is the indexable handle).
    #[serde(default = "datasource_tag")]
    pub tag: String,
    /// A soft-delete marker (`datasource.remove`): a removed source reads as absent on resolve/list
    /// (the store has no delete verb; a tombstone keeps the id stable + idempotent).
    #[serde(default)]
    pub removed: bool,
    pub ts: u64,
}

/// The constant `tag` value every datasource record carries (the list discriminator).
pub fn datasource_tag() -> String {
    "datasource".to_string()
}

/// The store table for datasource records.
pub const TABLE: &str = "datasource";

impl Datasource {
    pub fn new(
        name: impl Into<String>,
        kind: impl Into<String>,
        endpoint: impl Into<String>,
        secret_ref: impl Into<String>,
        ts: u64,
    ) -> Self {
        Self {
            name: name.into(),
            kind: kind.into(),
            endpoint: endpoint.into(),
            secret_ref: secret_ref.into(),
            tag: datasource_tag(),
            removed: false,
            ts,
        }
    }
}

/// Persist (upsert) a datasource record in `ws`. Workspace-namespaced by the store (the hard wall).
pub async fn put(store: &Store, ws: &str, ds: &Datasource) -> Result<(), StoreError> {
    let value = serde_json::to_value(ds).map_err(|e| StoreError::Decode(e.to_string()))?;
    write(store, ws, TABLE, &ds.name, &value).await
}

/// Resolve `name` to its datasource record in `ws`. `None` if not registered here OR tombstoned —
/// which is exactly what a cross-tenant name resolves to (a ws-B caller naming a ws-A source finds
/// nothing, the workspace wall made structural at the namespace).
pub async fn resolve(
    store: &Store,
    ws: &str,
    name: &str,
) -> Result<Option<Datasource>, StoreError> {
    let Some(value) = read(store, ws, TABLE, name).await? else {
        return Ok(None);
    };
    let ds: Datasource = decode(value)?;
    if ds.removed {
        return Ok(None);
    }
    Ok(Some(ds))
}

fn decode(value: Value) -> Result<Datasource, StoreError> {
    serde_json::from_value(value).map_err(|e| StoreError::Decode(e.to_string()))
}
