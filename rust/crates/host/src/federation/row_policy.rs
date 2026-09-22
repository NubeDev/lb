//! A datasource's **row policy** (entity-scoped-data scope): whether reads of `source` are narrowed to
//! the caller's entities, from which sources the entity set comes, and the table rules the sidecar
//! rewrites with. One record per source, `datasource_row_policy:{ws}:{source}`, admin-only
//! (`federation.row_policy_set` / `federation.row_policy_get`, both in the admin cap bundle).
//!
//! No record, or `enforce: false`, means the source reads exactly as before for everyone. With
//! `enforce: true` a non-admin caller's reads are narrowed — or, for verbs that cannot be narrowed
//! (sample/profile/mirror/writes), refused.

use lb_auth::Principal;
use lb_store::{read, write, Store, StoreError};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::authorize::authorize;
use super::error::FederationError;
use crate::authz::{entity_scope, EntityScope};
use crate::boot::Node;

pub const TABLE: &str = "datasource_row_policy";

fn default_sources() -> Vec<String> {
    vec!["nav".to_string()]
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RowPolicyRecord {
    pub source: String,
    #[serde(default)]
    pub enforce: bool,
    /// The entity table ids are read from (`site`).
    pub entity_table: String,
    /// Which entity sources are unioned (`nav`, `grant`).
    #[serde(default = "default_sources")]
    pub scope_sources: Vec<String>,
    /// The sidecar's table rules (`{tables, entity_key, extra_functions}`), validated by the sidecar
    /// on every scoped read — a malformed policy refuses restricted reads, it never opens them.
    pub policy: Value,
    #[serde(default)]
    pub ts: u64,
}

pub async fn get(
    store: &Store,
    ws: &str,
    source: &str,
) -> Result<Option<RowPolicyRecord>, StoreError> {
    match read(store, ws, TABLE, source).await? {
        Some(v) => serde_json::from_value(v)
            .map(Some)
            .map_err(|e| StoreError::Decode(e.to_string())),
        None => Ok(None),
    }
}

/// `federation.row_policy_set` — admin-only upsert.
pub async fn row_policy_set(
    node: &Node,
    caller: &Principal,
    ws: &str,
    input: &Value,
    ts: u64,
) -> Result<Value, FederationError> {
    authorize(caller, ws, "federation.row_policy_set")?;
    let mut rec: RowPolicyRecord = serde_json::from_value(input.clone())
        .map_err(|e| FederationError::BadInput(format!("row policy: {e}")))?;
    if rec.entity_table.is_empty() || !rec.policy.is_object() {
        return Err(FederationError::BadInput(
            "row policy needs entity_table and a policy object".into(),
        ));
    }
    rec.ts = ts;
    let value = serde_json::to_value(&rec).map_err(|e| FederationError::BadInput(e.to_string()))?;
    write(&node.store, ws, TABLE, &rec.source, &value).await?;
    Ok(json!({ "ok": true }))
}

/// `federation.row_policy_get` — admin-only read. `null` when the source has no policy.
pub async fn row_policy_get(
    node: &Node,
    caller: &Principal,
    ws: &str,
    source: &str,
) -> Result<Value, FederationError> {
    authorize(caller, ws, "federation.row_policy_get")?;
    Ok(get(&node.store, ws, source)
        .await?
        .map(|r| json!(r))
        .unwrap_or(Value::Null))
}

/// The `row_scope` to attach to `caller`'s read of `source`, or `None` when the read is unrestricted
/// (no enforced policy, or the caller's scope is `All`).
pub async fn row_scope_for(
    node: &Node,
    caller: &Principal,
    ws: &str,
    source: &str,
) -> Result<Option<Value>, FederationError> {
    let Some(rec) = get(&node.store, ws, source).await? else {
        return Ok(None);
    };
    if !rec.enforce {
        return Ok(None);
    }
    match entity_scope(node, caller, ws, &rec.entity_table, &rec.scope_sources).await {
        EntityScope::All => Ok(None),
        EntityScope::Ids(ids) => Ok(Some(json!({ "policy": rec.policy, "ids": ids }))),
    }
}

/// Is `caller` restricted on `source`? For verbs that cannot be narrowed and so must refuse.
pub async fn restricted(
    node: &Node,
    caller: &Principal,
    ws: &str,
    source: &str,
) -> Result<bool, FederationError> {
    Ok(row_scope_for(node, caller, ws, source).await?.is_some())
}

/// Refuse `caller` on `source` when they are restricted — for every federation verb except
/// `federation.query` (which is narrowed instead): schema, sample, profiles and exports would reveal
/// other entities' rows or statistics; mirror would copy them out of scope; writes are never theirs.
pub async fn refuse_if_restricted(
    node: &Node,
    caller: &Principal,
    ws: &str,
    source: &str,
) -> Result<(), FederationError> {
    if restricted(node, caller, ws, source).await? {
        return Err(FederationError::Denied);
    }
    Ok(())
}
