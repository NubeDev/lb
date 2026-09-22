//! **Entity narrowing for insights** (entity-scoped-data scope). When the workspace's enforced row
//! policy names an `insight_tag` (e.g. `site`), a restricted principal sees only insights whose tag
//! value is one of their entities — in lists, counts, single reads, actions, the live stream and
//! subscription deliveries. An insight WITHOUT the tag is hidden from them (fail closed): nothing
//! ties it to an entity they may read.
//!
//! The entity set is the same one federated reads use (`authz::entity_scope`), so a group sees the
//! same sites in its charts and in its insights. The tag is read from the insight's flat `tags` echo
//! everywhere (list predicate, point checks, stream) — one source of truth, never the tag graph.

use std::collections::BTreeSet;

use lb_auth::Principal;
use lb_insights::{Insight, Subscription};
use lb_store::Store;

use super::error::InsightSvcError;
use crate::authz::{entity_scope, EntityScope};
use crate::federation::insight_policy;

/// The insight tag and entity ids `principal` is limited to, or `None` when unrestricted.
pub(crate) async fn entity_limit(
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

/// Is `insight` inside `limit`? `None` = unrestricted.
fn within(limit: &Option<(String, BTreeSet<String>)>, insight: &Insight) -> bool {
    match limit {
        None => true,
        Some((tag, ids)) => insight.tags.get(tag).is_some_and(|v| ids.contains(v)),
    }
}

/// May `principal` see `insight`?
pub(super) async fn visible(
    store: &Store,
    principal: &Principal,
    ws: &str,
    insight: &Insight,
) -> Result<bool, InsightSvcError> {
    Ok(within(&entity_limit(store, principal, ws).await?, insight))
}

/// May `principal` see insight `id`? `true` when unrestricted, when the insight is missing (the
/// verb keeps its own missing-id answer), or when it is in scope; `false` only for a real insight
/// outside the caller's entities.
pub(super) async fn visible_id(
    store: &Store,
    principal: &Principal,
    ws: &str,
    id: &str,
) -> Result<bool, InsightSvcError> {
    let limit = entity_limit(store, principal, ws).await?;
    if limit.is_none() {
        return Ok(true);
    }
    Ok(match lb_insights::get(store, ws, id).await? {
        Some(i) => within(&limit, &i),
        None => true,
    })
}

/// Refuse a single-insight verb on an insight outside `principal`'s entities with EXACTLY the error a
/// missing id gets ("no such insight"), so an out-of-scope insight's existence is not disclosed.
/// For the verbs whose missing-id answer is an ERROR (ack, resolve, assign, comment, delete).
pub(crate) async fn ensure_visible(
    store: &Store,
    principal: &Principal,
    ws: &str,
    id: &str,
) -> Result<(), InsightSvcError> {
    if visible_id(store, principal, ws, id).await? {
        Ok(())
    } else {
        Err(InsightSvcError::BadInput(format!("no such insight: {id}")))
    }
}

/// May a subscription's OWNER see an insight with these `tags`? A subscription fires under its
/// owner's stored principal, so a delivery is a read by the owner and is narrowed like one.
pub(super) async fn sub_owner_may_see(
    store: &Store,
    ws: &str,
    sub: &Subscription,
    tags: &std::collections::BTreeMap<String, String>,
) -> bool {
    let caps: Vec<String> = serde_json::from_value(sub.principal.clone()).unwrap_or_default();
    let owner = Principal::routed(&sub.owner, ws, caps);
    match entity_limit(store, &owner, ws).await {
        Ok(None) => true,
        Ok(Some((tag, ids))) => tags.get(&tag).is_some_and(|v| ids.contains(v)),
        // Fail closed: an unreadable policy delivers nothing out of the ordinary.
        Err(_) => false,
    }
}
