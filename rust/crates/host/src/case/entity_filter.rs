//! **Entity narrowing for cases** (entity-scoped-data scope). A restricted principal sees only cases
//! at their entities (a case's `site` facet), with the same entity set their charts and insights use
//! (`crate::insight::entity_limit`). A case with no site is hidden from them (fail closed). An
//! out-of-scope case reads EXACTLY like a missing one ("no such case"), so its existence is not
//! disclosed.

use lb_auth::Principal;
use lb_cases::Case;
use lb_store::Store;

use super::error::CaseSvcError;

/// May `principal` see `case`?
pub(super) async fn visible(
    store: &Store,
    principal: &Principal,
    ws: &str,
    case: &Case,
) -> Result<bool, CaseSvcError> {
    let limit = crate::insight::entity_limit(store, principal, ws)
        .await
        .map_err(|e| CaseSvcError::Store(e.to_string()))?;
    Ok(match limit {
        None => true,
        Some((_tag, ids)) => case.site.as_deref().is_some_and(|s| ids.contains(s)),
    })
}

/// Refuse a single-case verb on a case outside `principal`'s entities, as if it did not exist.
pub(super) async fn ensure_visible(
    store: &Store,
    principal: &Principal,
    ws: &str,
    id: &str,
) -> Result<(), CaseSvcError> {
    match lb_cases::get(store, ws, id).await? {
        Some(c) if visible(store, principal, ws, &c).await? => Ok(()),
        Some(_) => Err(CaseSvcError::BadInput(format!("no such case: {id}"))),
        // A genuinely missing case keeps whatever answer the verb itself gives.
        None => Ok(()),
    }
}

/// May `principal` see case `id`? `true` when unrestricted, missing, or in scope.
pub(super) async fn case_visible_id(
    store: &Store,
    principal: &Principal,
    ws: &str,
    id: &str,
) -> Result<bool, CaseSvcError> {
    Ok(match lb_cases::get(store, ws, id).await? {
        Some(c) => visible(store, principal, ws, &c).await?,
        None => true,
    })
}
