//! The `nav` entity source: the entities marked on the menus a principal was HANDED — every
//! team-shared nav they can read and the workspace default (`nav::handed_navs`). A personal pick,
//! the built-in fallback and pins contribute nothing, exactly like record reach's valve 2: a choice
//! a member makes for themself can never widen what they may read — nor, here, zero it.

use std::collections::BTreeSet;

use lb_auth::Principal;
use lb_store::Store;

use super::EntityScope;
use crate::nav::{collect_entities, handed_navs};

pub(super) async fn entities(
    store: &Store,
    principal: &Principal,
    ws: &str,
    table: &str,
) -> EntityScope {
    let mut out = BTreeSet::new();
    match handed_navs(store, principal, ws).await {
        Ok(navs) => {
            for nav in &navs {
                collect_entities(&nav.items, table, &mut out);
            }
        }
        Err(e) => {
            tracing::warn!(ws, error = ?e, "entity scope: menus unreadable, nav source adds nothing")
        }
    }
    EntityScope::Ids(out)
}
