//! The `nav` entity source: the entities marked on the menu a principal was HANDED — a team share
//! (tier 2) or the workspace default (tier 3). A personal pick (tier 1) and the built-in fallback
//! contribute nothing, exactly like record reach's valve 2: a choice a member makes for themself can
//! never widen what they may read. Pins are personal too, so they never count either.

use lb_auth::Principal;
use std::collections::BTreeSet;

use super::EntityScope;
use crate::nav::{collect_entities, pick_nav, ResolvedSource};
use lb_store::Store;

pub(super) async fn entities(
    store: &Store,
    principal: &Principal,
    ws: &str,
    table: &str,
) -> EntityScope {
    let mut out = BTreeSet::new();
    // The same 4-tier pick the menu resolver makes — only the store is needed, not the board
    // hydration the full resolve does.
    match pick_nav(store, principal, ws).await {
        Ok(Some((nav, source)))
            if matches!(
                source,
                ResolvedSource::Team | ResolvedSource::WorkspaceDefault
            ) =>
        {
            collect_entities(&nav.items, table, &mut out);
        }
        Ok(_) => {}
        Err(e) => {
            tracing::warn!(ws, error = ?e, "entity scope: menu unreadable, nav source adds nothing")
        }
    }
    EntityScope::Ids(out)
}
