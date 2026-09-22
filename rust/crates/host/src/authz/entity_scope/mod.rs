//! **Entity scope** (entity-scoped-data scope): which ids of an entity table (e.g. `site`) a
//! principal may read, resolved from SERVER state only — never from a URL, a dashboard variable or a
//! tool argument. Enforcement points (federation reads today) call [`entity_scope`] and nothing else,
//! so adding a source never touches them.
//!
//! The scope is the UNION of the enabled sources (`scope_sources` on the datasource's row policy):
//! - `nav` — entities marked on the principal's HANDED menu ([`nav_source`]);
//! - `grant` — the entity-scoped grant `data:<table>:read` ([`grant_source`]).
//!
//! Fail closed: an unknown source, a source error, or no marked entities adds nothing. Only an
//! explicit `All` from a source lifts a principal to everything. Workspace admins are unrestricted.

mod cache;
mod grant_source;
mod nav_source;

use std::collections::BTreeSet;

use lb_auth::Principal;

use crate::boot::Node;
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
    node: &Node,
    principal: &Principal,
    ws: &str,
    table: &str,
    sources: &[String],
) -> EntityScope {
    if is_workspace_admin(principal, ws) {
        return EntityScope::All;
    }
    let key = cache::Key::new(ws, principal.owner_sub(), table, sources);
    if let Some(hit) = cache::get(&key) {
        return hit;
    }
    let mut ids = BTreeSet::new();
    let mut all = false;
    for source in sources {
        let got = match source.as_str() {
            "nav" => nav_source::entities(node, principal, ws, table).await,
            "grant" => grant_source::entities(node, principal, ws, table).await,
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
    let scope = if all {
        EntityScope::All
    } else {
        EntityScope::Ids(ids)
    };
    cache::put(key, scope.clone());
    scope
}
