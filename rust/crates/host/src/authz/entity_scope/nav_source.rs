//! The `nav` entity source: the entities marked on the menus a principal was HANDED — every
//! team-shared nav they can read and the workspace default (`nav::handed_navs`). A personal pick,
//! the built-in fallback and pins contribute nothing, exactly like record reach's valve 2: a choice
//! a member makes for themself can never widen what they may read — nor, here, zero it.
//!
//! The `nav_pin` source reads the same menus but takes the ids from a pin instead of a marker. Which
//! pin is the policy's `entity_key.key_value`, the tag those ids are matched against, so the data and
//! the menu cannot name different keys.

use std::collections::BTreeSet;

use lb_auth::Principal;
use lb_store::Store;

use super::EntityScope;
use crate::federation::nav_pin_keys;
use crate::nav::{collect_entities, collect_pinned, handed_navs};

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

pub(super) async fn pinned(
    store: &Store,
    principal: &Principal,
    ws: &str,
    table: &str,
) -> EntityScope {
    let mut out = BTreeSet::new();
    let keys = match nav_pin_keys(store, ws, table).await {
        Ok(keys) => keys,
        Err(e) => {
            tracing::warn!(ws, error = ?e, "entity scope: policies unreadable, nav_pin adds nothing");
            return EntityScope::Ids(out);
        }
    };
    match handed_navs(store, principal, ws).await {
        Ok(navs) => {
            for nav in &navs {
                for key in &keys {
                    collect_pinned(&nav.items, key, &mut out);
                }
            }
        }
        Err(e) => {
            tracing::warn!(ws, error = ?e, "entity scope: menus unreadable, nav_pin adds nothing")
        }
    }
    EntityScope::Ids(out)
}
