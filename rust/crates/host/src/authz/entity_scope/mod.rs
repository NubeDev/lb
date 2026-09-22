//! **Entity scope** (entity-scoped-data scope): which ids of an entity table (e.g. `site`) a
//! principal may read, resolved from SERVER state only — never from a URL, a dashboard variable or a
//! tool argument. Enforcement points (federation reads, insights, cases) call [`entity_scope`] and
//! nothing else, so adding a source never touches them.
//!
//! The scope is the UNION of the enabled sources (`scope_sources` on the datasource's row policy):
//! - `nav` — entities marked on the menus the principal was HANDED ([`nav_source`]);
//! - `grant` — the entity-scoped grant `data:<table>:read` ([`grant_source`]).
//!
//! Fail closed: an unknown source, a source error, or no marked entities adds nothing. Only an
//! explicit `All` from a source lifts a principal to everything.
//!
//! **Whose scope.** A derived principal (an extension backend or the agent acting for a member,
//! `Principal::derive`) reads with its OWNER's scope: the owner's menus, grants and admin standing
//! decide, since the read is on their behalf. The owner's caps are resolved live from the store for
//! that check. Workspace admins are unrestricted.

mod cache;
mod grant_source;
mod nav_source;

use std::collections::BTreeSet;

use lb_auth::Principal;
use lb_store::Store;

use crate::authz::resolve_caps_live;
use crate::nav::is_workspace_admin;

/// What a principal may read of one entity table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntityScope {
    /// Unrestricted.
    All,
    /// Exactly these ids (possibly none).
    Ids(BTreeSet<String>),
}

/// Resolve `principal`'s scope over `table` in `ws` from the enabled `sources`.
pub async fn entity_scope(
    store: &Store,
    principal: &Principal,
    ws: &str,
    table: &str,
    sources: &[String],
) -> EntityScope {
    let owner = principal.owner_sub().to_string();
    let key = cache::Key::new(ws, &owner, table, sources);
    if let Some(hit) = cache::get(&key) {
        return hit;
    }
    // The principal whose menus, grants and standing decide: the caller, or — for a derived
    // (on-behalf-of) actor — its owner, rebuilt with the owner's live caps.
    let acting: Principal = if owner == principal.sub() {
        principal.clone()
    } else {
        let bare = owner.strip_prefix("user:").unwrap_or(&owner);
        let caps = resolve_caps_live(store, ws, bare).await.unwrap_or_default();
        Principal::routed(owner.as_str(), ws, caps)
    };
    let scope = if is_workspace_admin(&acting, ws) {
        EntityScope::All
    } else {
        let mut ids = BTreeSet::new();
        let mut all = false;
        for source in sources {
            let got = match source.as_str() {
                "nav" => nav_source::entities(store, &acting, ws, table).await,
                "grant" => grant_source::entities(store, &acting, ws, table).await,
                other => {
                    tracing::warn!(
                        ws,
                        source = other,
                        "entity scope: unknown source ignored (adds nothing)"
                    );
                    EntityScope::Ids(BTreeSet::new())
                }
            };
            match got {
                EntityScope::All => all = true,
                EntityScope::Ids(more) => ids.extend(more),
            }
        }
        if all {
            EntityScope::All
        } else {
            EntityScope::Ids(ids)
        }
    };
    cache::put(key, scope.clone());
    scope
}

/// Forget every cached scope in `ws`. Called by every write that can change who reaches what: a
/// menu save/share/unshare/delete or default change, a team membership change, a grant change, a
/// row-policy change.
pub fn invalidate_entity_scope(ws: &str) {
    cache::invalidate_ws(ws);
}

/// The source names a row policy may enable.
pub const ENTITY_SCOPE_SOURCES: &[&str] = &["nav", "grant"];
