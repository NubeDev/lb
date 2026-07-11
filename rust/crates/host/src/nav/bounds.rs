//! Nav record bounds (nav scope, "Resolution cost" / the item-cap open question; nested-nav scope). The
//! host is the authority — it rejects an over-cap `items[]` rather than silently storing it unbounded,
//! so the resolver stays cheap. The builder mirrors these caps for a friendly error, but this is the
//! boundary. Two INDEPENDENT rules, each `BadInput` on breach (nothing persists, no silent
//! flatten/truncate):
//!   - the total node count over EVERY depth (groups included) is capped at [`MAX_ITEMS`];
//!   - `group` nesting depth is capped at [`MAX_GROUP_DEPTH`] (top-level list = depth 1; a `group` at
//!     depth 5 may hold leaves but no further `group`).

use super::error::NavError;
use super::model::{NavItem, MAX_GROUP_DEPTH, MAX_ITEMS};

/// The reserved pick sentinel (no-lockout scope) — a `nav_pref.active` of this value means "force the
/// built-in sidebar; ignore team/default tiers". It is NOT a real nav id, so `nav.save` must reject it
/// (and any `__…__` reserved shape) to keep the pick axis unambiguous.
pub const BUILTIN_PICK: &str = "__builtin__";

/// Reject a nav id that collides with a reserved value (no-lockout scope). A reserved id is the
/// `__…__` shape (currently just [`BUILTIN_PICK`]); a real nav can never BE the built-in sentinel.
pub fn check_id(id: &str) -> Result<(), NavError> {
    if id.starts_with("__") && id.ends_with("__") {
        return Err(NavError::BadInput(format!(
            "nav id `{id}` is reserved (the `__…__` shape is not a valid nav id)"
        )));
    }
    Ok(())
}

/// The item kinds a nav may hold. A `group` nests other items recursively (capped at
/// [`MAX_GROUP_DEPTH`]). `template-group` (reusable-pages scope) is the one-dashboard-many-bindings
/// fan-out — additive, next to `tag-group`.
const KINDS: &[&str] = &[
    "surface",
    "dashboard",
    "ext",
    "tag-group",
    "template-group",
    "group",
];

/// Reject a nav whose `items[]` breaches either INDEPENDENT bound: the total node count over every
/// depth exceeds [`MAX_ITEMS`], `group` nesting exceeds [`MAX_GROUP_DEPTH`] (nested-nav scope), or an
/// item names an unknown kind. Both checks run so a wide-but-shallow tree and a narrow-but-deep tree
/// each fail on their own limit.
pub fn check_items(items: &[NavItem]) -> Result<(), NavError> {
    let total = count(items);
    if total > MAX_ITEMS {
        return Err(NavError::BadInput(format!(
            "nav has {total} items, exceeds cap {MAX_ITEMS}"
        )));
    }
    // The top-level list is depth 1 (nested-nav scope): a `group` here holds children at depth 2.
    for item in items {
        check_item(item, 1)?;
    }
    Ok(())
}

/// Total node count over EVERY depth (groups counted as nodes too — nested-nav scope).
fn count(items: &[NavItem]) -> usize {
    items
        .iter()
        .map(|i| {
            1 + if i.kind == "group" {
                count(&i.items)
            } else {
                0
            }
        })
        .sum()
}

/// Validate one item at `depth` (the top-level list is depth 1). A `group` deeper than
/// [`MAX_GROUP_DEPTH`] is rejected — a `group` at the max depth may still hold leaf kinds, but a
/// further nested `group` (which would land at `depth + 1`) is refused. An unknown kind is rejected.
fn check_item(item: &NavItem, depth: usize) -> Result<(), NavError> {
    if !KINDS.contains(&item.kind.as_str()) {
        return Err(NavError::BadInput(format!(
            "unknown nav item kind: {}",
            item.kind
        )));
    }
    if item.kind == "group" {
        if depth > MAX_GROUP_DEPTH {
            return Err(NavError::BadInput(format!(
                "nav group nesting exceeds cap {MAX_GROUP_DEPTH} (a group may not appear below depth {MAX_GROUP_DEPTH})"
            )));
        }
        for child in &item.items {
            check_item(child, depth + 1)?;
        }
    }
    // A `template-group` (reusable-pages scope) must name the template dashboard, the parameter it
    // binds (`var`), and exactly one option source (tag `facets` OR a `{tool,args}` query) — reject a
    // malformed one at author time rather than emit an empty menu at resolve.
    if item.kind == "template-group" {
        if item.dashboard.is_empty() {
            return Err(NavError::BadInput(
                "template-group needs a `dashboard` (the template)".into(),
            ));
        }
        if item.var.is_empty() {
            return Err(NavError::BadInput(
                "template-group needs a `var` (the template parameter to bind)".into(),
            ));
        }
        let has_facets = !item.facets.is_empty();
        let has_tool = !item.tool.is_empty();
        if has_facets == has_tool {
            return Err(NavError::BadInput(
                "template-group needs exactly one option source: `facets` OR `tool`".into(),
            ));
        }
    }
    Ok(())
}
