//! **Entity narrowing for insights** (entity-scoped-data scope). When the workspace's enforced row
//! policy names an `insight_tag` (e.g. `site`), a restricted principal sees only insights whose tag
//! value is one of their entities — in lists, counts, and single reads. An insight WITHOUT the tag is
//! hidden from them (fail closed): nothing ties it to an entity they may read.
//!
//! The entity set is the same one federated reads use (`authz::entity_scope`), so a group sees the
//! same sites in its charts and in its insights.

use std::collections::{BTreeSet, HashSet};

use lb_auth::Principal;
use lb_insights::Insight;
use lb_store::Store;
use lb_tags::Facet;

use super::error::InsightSvcError;
use crate::authz::{entity_scope, EntityScope};
use crate::federation::insight_policy;

/// The insight tag and entity ids `principal` is limited to, or `None` when unrestricted.
pub(super) async fn entity_limit(
    store: &Store,
    principal: &Principal,
    ws: &str,
) -> Result<Option<(String, BTreeSet<String>)>, InsightSvcError> {
    let Some(policy) = insight_policy(store, ws)
        .await
        .map_err(|e| InsightSvcError::Store(e.to_string()))?
    else {
        return Ok(None);
    };
    let tag = policy.insight_tag.unwrap_or_default();
    match entity_scope(
        store,
        principal,
        ws,
        &policy.entity_table,
        &policy.scope_sources,
    )
    .await
    {
        EntityScope::All => Ok(None),
        EntityScope::Ids(ids) => Ok(Some((tag, ids))),
    }
}

/// The insight ids `principal` may see, or `None` when unrestricted. Resolved through the tag graph,
/// one exact facet per entity.
pub(super) async fn allowed_ids(
    store: &Store,
    principal: &Principal,
    ws: &str,
) -> Result<Option<HashSet<String>>, InsightSvcError> {
    let Some((tag, ids)) = entity_limit(store, principal, ws).await? else {
        return Ok(None);
    };
    let mut out = HashSet::new();
    for id in ids {
        let facets = [Facet::exact(tag.clone(), serde_json::Value::String(id))];
        let found = lb_tags::find(store, ws, &facets)
            .await
            .map_err(|e| InsightSvcError::Store(e.to_string()))?;
        out.extend(
            found
                .into_iter()
                .map(|e| e.strip_prefix("insight:").map(str::to_string).unwrap_or(e)),
        );
    }
    Ok(Some(out))
}

/// May `principal` see `insight`? For single-record verbs (get, and every write that addresses one).
pub(super) async fn visible(
    store: &Store,
    principal: &Principal,
    ws: &str,
    insight: &Insight,
) -> Result<bool, InsightSvcError> {
    Ok(match entity_limit(store, principal, ws).await? {
        None => true,
        Some((tag, ids)) => insight.tags.get(&tag).is_some_and(|v| ids.contains(v)),
    })
}

/// Refuse a single-insight verb on an insight outside `principal`'s entities with EXACTLY the error a
/// missing id gets ("no such insight"), so an out-of-scope insight's existence is not disclosed.
pub(super) async fn ensure_visible(
    store: &Store,
    principal: &Principal,
    ws: &str,
    id: &str,
) -> Result<(), InsightSvcError> {
    if entity_limit(store, principal, ws).await?.is_none() {
        return Ok(());
    }
    match lb_insights::get(store, ws, id).await? {
        Some(i) if visible(store, principal, ws, &i).await? => Ok(()),
        _ => Err(InsightSvcError::BadInput(format!("no such insight: {id}"))),
    }
}
