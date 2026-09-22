//! The `grant` entity source: an explicit list via the existing entity-scoped grant,
//! `grants.assign {subject, cap: "data:<table>:read", scope: {kind:"ids", table, ids}}` on a user or a
//! team. No such grant adds nothing; a grant with `Scope::All` unrestricts.

use std::collections::BTreeSet;

use lb_auth::Principal;
use lb_authz::{scope_filter_with, ScopeFilter};
use lb_store::Store;

use super::EntityScope;
use crate::authz::LiveBuiltinRoleCaps;

pub(super) async fn entities(
    store: &Store,
    principal: &Principal,
    ws: &str,
    table: &str,
) -> EntityScope {
    // Grants are stored under the BARE user name (see `reminder/fire.rs`). Only a `user:` subject
    // has team edges and user grants; a key or any other subject kind adds nothing here.
    let Some(bare) = principal.sub().strip_prefix("user:") else {
        return EntityScope::Ids(BTreeSet::new());
    };
    // Read LIVE from the grant store (not the token's minted caps), so a new or revoked grant takes
    // effect within the scope cache window. No such grant → `Ids([])`.
    let cap = format!("data:{table}:read");
    match scope_filter_with(store, ws, bare, &cap, table, &LiveBuiltinRoleCaps).await {
        Ok(ScopeFilter::All) => EntityScope::All,
        Ok(ScopeFilter::Ids(ids)) => EntityScope::Ids(ids.into_iter().collect()),
        Err(e) => {
            tracing::warn!(ws, error = ?e, "entity scope: grant read failed, grant source adds nothing");
            EntityScope::Ids(BTreeSet::new())
        }
    }
}
