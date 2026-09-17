//! Resolving a pinned curated menu ROW — `nav:<navid>/<rowid>` (nav-row-pins scope).
//!
//! A curated row is often not a bare target: a board bound to `site=chullora`, a folder that opens its
//! own overview. Pinning its target would lose the binding, and a folder has no target ref at all — so
//! the pin addresses the ROW, by the stable id `nav.save` gives it, and resolves it the way the menu
//! does:
//!
//!   1. read the nav through the SAME readability gate the pick tiers use — a nav the caller cannot
//!      read (unshared, private to someone else, deleted) strips the pin;
//!   2. find the row by id at any depth, collecting the folder labels above it;
//!   3. run it through the ordinary `resolve_item` pipeline — a board the caller cannot read strips;
//!   4. hide still beats pin — the row's underlying target ref in the hidden-set strips it.
//!
//! Every miss is `Ok(None)` and the stored `nav_pref` is never touched, so a later re-share, re-grant
//! or un-hide restores the pin for free (the `resolve_pins` invariant).

use std::collections::BTreeSet;
use std::sync::Arc;

use lb_auth::Principal;

use super::error::NavError;
use super::model::NavItem;
use super::resolve::{item_ref, label_or, readable_nav, resolve_item};
use super::resolved::ResolvedItem;
use super::row_ids::check_row_id;
use crate::boot::Node;

/// Resolve one `nav:<navid>/<rowid>` pin for `principal`, or `None` when it strips.
pub(super) async fn resolve_row_pin(
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
    pin: &str,
    hidden: &BTreeSet<String>,
) -> Result<Option<ResolvedItem>, NavError> {
    let Some((nav_id, row_id)) = split_row_ref(pin) else {
        return Ok(None);
    };
    let Some(nav) = readable_nav(&node.store, principal, ws, nav_id).await? else {
        return Ok(None);
    };
    let mut trail = Vec::new();
    let Some(row) = find_row(&nav.items, row_id, &mut trail) else {
        return Ok(None); // the author deleted the row
    };
    let Some(mut resolved) = resolve_item(node, principal, ws, row).await? else {
        return Ok(None);
    };

    // The Pinned list is flat. A folder pins AS ITS BOARD; a folder with no readable board — and a
    // tag-/template-group, which resolves to a board-less group — has nowhere to go.
    if resolved.kind == "group" {
        if resolved.dashboard.is_empty() {
            return Ok(None);
        }
        resolved.kind = "dashboard".into();
        resolved.items.clear();
    }
    if hidden.contains(&item_ref(&resolved)) {
        return Ok(None);
    }

    resolved.nav_id = nav.id.clone();
    resolved.trail = trail;
    // A pinned copy is not where the menu lands or what it ends with.
    resolved.home = false;
    resolved.footer = false;
    Ok(Some(resolved))
}

/// `nav:<navid>/<rowid>` → `(navid, rowid)`, or `None` for any other shape (exactly one `/`, both
/// halves non-empty, a legal row id).
fn split_row_ref(pin: &str) -> Option<(&str, &str)> {
    let (nav_id, row_id) = pin.strip_prefix("nav:")?.split_once('/')?;
    if nav_id.is_empty() || check_row_id(row_id).is_err() {
        return None;
    }
    Some((nav_id, row_id))
}

/// Depth-first search for the row with `id`, leaving the labels of the folders above it in `trail`
/// (outermost first).
fn find_row<'a>(items: &'a [NavItem], id: &str, trail: &mut Vec<String>) -> Option<&'a NavItem> {
    for item in items {
        if item.id == id {
            return Some(item);
        }
        if !item.items.is_empty() {
            trail.push(label_or(&item.label, "Group"));
            if let Some(found) = find_row(&item.items, id, trail) {
                return Some(found);
            }
            trail.pop();
        }
    }
    None
}
