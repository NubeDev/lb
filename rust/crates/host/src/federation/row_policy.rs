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
use crate::authz::{entity_scope, EntityScope, ENTITY_SCOPE_SOURCES};
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
    /// The insight tag that carries the entity id (`site`). When set on an ENFORCED policy, insights
    /// and cases are narrowed to the caller's entities too (at most one enforced policy per workspace
    /// should set it; the first by source name wins).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub insight_tag: Option<String>,
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

/// Every ENFORCED policy in `ws`, by source name — read through a short per-workspace cache (the
/// insight and case verbs consult it on every call) that `row_policy_set` clears.
pub async fn enforced_policies(
    store: &Store,
    ws: &str,
) -> Result<Vec<RowPolicyRecord>, StoreError> {
    if let Some(hit) = policy_cache::get(store.instance_id(), ws) {
        return Ok(hit);
    }
    let mut found: Vec<RowPolicyRecord> = lb_store::scan_all(store, ws, TABLE)
        .await?
        .into_iter()
        .filter_map(|row| match row.data {
            Value::Object(mut o) => o.remove("data"),
            _ => None,
        })
        .filter_map(|v| serde_json::from_value::<RowPolicyRecord>(v).ok())
        .filter(|r| r.enforce)
        .collect();
    found.sort_by(|a, b| a.source.cmp(&b.source));
    policy_cache::put(store.instance_id(), ws, found.clone());
    Ok(found)
}

/// The per-workspace cache behind [`enforced_policies`]: same TTL as the entity-scope cache, so a
/// policy flip and a scope change become visible together.
mod policy_cache {
    use std::collections::HashMap;
    use std::sync::{Mutex, OnceLock};
    use std::time::{Duration, Instant};

    use super::RowPolicyRecord;

    const TTL: Duration = Duration::from_secs(30);

    /// Keyed on the store instance too (`Store::instance_id`): the cache is process-wide, and two
    /// nodes in one process must never see each other's policies.
    type Map = HashMap<(usize, String), (Instant, Vec<RowPolicyRecord>)>;

    fn map() -> &'static Mutex<Map> {
        static MAP: OnceLock<Mutex<Map>> = OnceLock::new();
        MAP.get_or_init(|| Mutex::new(HashMap::new()))
    }

    pub(super) fn get(store: usize, ws: &str) -> Option<Vec<RowPolicyRecord>> {
        let m = map().lock().ok()?;
        m.get(&(store, ws.to_string()))
            .filter(|(at, _)| at.elapsed() < TTL)
            .map(|(_, v)| v.clone())
    }

    pub(super) fn put(store: usize, ws: &str, v: Vec<RowPolicyRecord>) {
        if let Ok(mut m) = map().lock() {
            m.insert((store, ws.to_string()), (Instant::now(), v));
        }
    }

    pub(super) fn invalidate(ws: &str) {
        if let Ok(mut m) = map().lock() {
            m.retain(|(_, w), _| w != ws);
        }
    }
}

/// The enforced policy in `ws` that narrows insights (`insight_tag` set), if any.
pub async fn insight_policy(
    store: &Store,
    ws: &str,
) -> Result<Option<RowPolicyRecord>, StoreError> {
    Ok(enforced_policies(store, ws)
        .await?
        .into_iter()
        .find(|r| r.insight_tag.as_deref().is_some_and(|t| !t.is_empty())))
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
    if rec.source.is_empty() || !plain_ident(&rec.entity_table) {
        return Err(FederationError::BadInput(
            "row policy needs a source and a plain entity_table".into(),
        ));
    }
    if rec.scope_sources.is_empty()
        || rec
            .scope_sources
            .iter()
            .any(|s| !ENTITY_SCOPE_SOURCES.contains(&s.as_str()))
    {
        return Err(FederationError::BadInput(format!(
            "row policy: scope_sources must be a non-empty subset of {ENTITY_SCOPE_SOURCES:?}"
        )));
    }
    if let Some(tag) = &rec.insight_tag {
        if !plain_ident(tag) {
            return Err(FederationError::BadInput(
                "row policy: insight_tag must be a plain identifier".into(),
            ));
        }
    }
    let shape_ok = rec.policy.get("tables").is_some_and(Value::is_object)
        && rec.policy.get("entity_key").is_some_and(Value::is_object);
    if !shape_ok {
        return Err(FederationError::BadInput(
            "row policy: policy needs `tables` and `entity_key` objects".into(),
        ));
    }
    rec.ts = ts;
    let value = serde_json::to_value(&rec).map_err(|e| FederationError::BadInput(e.to_string()))?;
    write(&node.store, ws, TABLE, &rec.source, &value).await?;
    policy_cache::invalidate(ws);
    crate::authz::invalidate_entity_scope(ws);
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
    match entity_scope(
        &node.store,
        caller,
        ws,
        &rec.entity_table,
        &rec.scope_sources,
    )
    .await
    {
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

/// A plain lowercase identifier — what a policy may name as a table, column or tag key.
fn plain_ident(s: &str) -> bool {
    let mut chars = s.chars();
    matches!(chars.next(), Some(c) if c == '_' || c.is_ascii_lowercase())
        && chars.all(|c| c == '_' || c.is_ascii_lowercase() || c.is_ascii_digit())
}
