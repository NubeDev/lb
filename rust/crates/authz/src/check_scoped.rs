//! [`check_scoped`] and [`scope_filter`] — the entity-scoped read API (entity-scoped-grants
//! scope). Thin reads over [`resolve_caps_scoped`](crate::resolve_scoped): a point check "may this
//! principal reach this record?" and a query-side filter "which rows may it reach?". Both are
//! store reads (not token reads) so they reflect the latest grants — the freshness levers
//! (builtin-role-freshness) apply here too.
//!
//! These are the **host-facing** functions the host service wraps (with principal extraction and
//! capability gating) and that extensions reach via `host.call-tool("authz.check_scoped", …)` /
//! `host.call-tool("authz.scope_filter", …)` — the generic MCP re-entry, no WIT change needed.

use lb_store::{Store, StoreError};

use crate::resolve::BuiltinRoleCaps;
use crate::resolve_scoped::{resolve_caps_scoped, resolve_caps_scoped_with};
use crate::scope::ScopeFilter;

/// May `user` reach record `id` in `table` under cap `cap` in workspace `ws`? True iff the user
/// holds `cap` AND the scope union includes `(table, id)`. A cap held with `All` scope → true for
/// any id. A cap not held → false (not an error — degrade to deny).
pub async fn check_scoped(
    store: &Store,
    ws: &str,
    user: &str,
    cap: &str,
    table: &str,
    id: &str,
) -> Result<bool, StoreError> {
    let scoped = resolve_caps_scoped(store, ws, user).await?;
    Ok(scoped
        .iter()
        .find(|sc| sc.cap == cap)
        .map(|sc| sc.scope.contains(table, id))
        .unwrap_or(false))
}

/// Like [`check_scoped`] but with an injected [`BuiltinRoleCaps`] (builtin-role-freshness scope).
pub async fn check_scoped_with(
    store: &Store,
    ws: &str,
    user: &str,
    cap: &str,
    table: &str,
    id: &str,
    builtins: &dyn BuiltinRoleCaps,
) -> Result<bool, StoreError> {
    let scoped = resolve_caps_scoped_with(store, ws, user, builtins).await?;
    Ok(scoped
        .iter()
        .find(|sc| sc.cap == cap)
        .map(|sc| sc.scope.contains(table, id))
        .unwrap_or(false))
}

/// Which rows in `table` may `user` reach under cap `cap` in workspace `ws`? `All` = every row
/// (the cap is fully reachable); `Ids(ids)` = only those (empty = the cap is held but scoped to
/// zero rows, or scoped to a different table — degrade to empty, not error). A cap not held →
/// `Ids([])`.
pub async fn scope_filter(
    store: &Store,
    ws: &str,
    user: &str,
    cap: &str,
    table: &str,
) -> Result<ScopeFilter, StoreError> {
    let scoped = resolve_caps_scoped(store, ws, user).await?;
    Ok(scoped
        .iter()
        .find(|sc| sc.cap == cap)
        .map(|sc| sc.scope.filter_for(table))
        .unwrap_or(ScopeFilter::Ids(vec![])))
}

/// Like [`scope_filter`] but with an injected [`BuiltinRoleCaps`] (builtin-role-freshness scope).
pub async fn scope_filter_with(
    store: &Store,
    ws: &str,
    user: &str,
    cap: &str,
    table: &str,
    builtins: &dyn BuiltinRoleCaps,
) -> Result<ScopeFilter, StoreError> {
    let scoped = resolve_caps_scoped_with(store, ws, user, builtins).await?;
    Ok(scoped
        .iter()
        .find(|sc| sc.cap == cap)
        .map(|sc| sc.scope.filter_for(table))
        .unwrap_or(ScopeFilter::Ids(vec![])))
}
