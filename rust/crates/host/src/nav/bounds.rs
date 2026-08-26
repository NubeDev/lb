//! Nav record bounds (nav scope, "Resolution cost" / the item-cap open question; nested-nav scope). The
//! host is the authority — it rejects an over-cap `items[]` rather than silently storing it unbounded,
//! so the resolver stays cheap. The builder mirrors these caps for a friendly error, but this is the
//! boundary. Two INDEPENDENT rules, each `BadInput` on breach (nothing persists, no silent
//! flatten/truncate):
//!   - the total node count over EVERY depth (groups included) is capped at [`MAX_ITEMS`];
//!   - `group` nesting depth is capped at [`MAX_GROUP_DEPTH`] (top-level list = depth 1; a `group` at
//!     depth 5 may hold leaves but no further `group`).

use lb_ext_loader::template_refs;

use super::error::NavError;
use super::model::{
    NavItem, MAX_GROUP_DEPTH, MAX_ICON_COLOR_LEN, MAX_ICON_LEN, MAX_ITEMS, MAX_TITLE_TEMPLATE,
};

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
    // At most ONE home across the whole tree (nav-home scope). A menu with two homes has no answer to
    // "where does this person land", and the client would have to invent one — so the ambiguity is
    // refused at the door rather than resolved by a tie-break nobody authored. Counted over every
    // depth, because a marked entry nested in a group is just as much the home as a top-level one.
    let homes = count_homes(items);
    if homes > 1 {
        return Err(NavError::BadInput(format!(
            "nav marks {homes} items as home, at most 1 allowed"
        )));
    }
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

/// Home markers over EVERY depth (nav-home scope). Recurses through `group` children exactly as
/// [`count`] does, so a home nested inside a folder is seen.
fn count_homes(items: &[NavItem]) -> usize {
    items
        .iter()
        .map(|i| usize::from(i.home) + count_homes(&i.items))
        .sum()
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

/// The `title_template` check (nav-context-builtins scope, §G4) — the SAME rule the extension
/// manifest's `validate_nav` applies, reached through the SAME `template_refs` extractor, so an
/// admin authoring in the nav builder and an extension authoring in `extension.toml` get one verdict
/// for one template. Validating only the ext seam would make it the privileged path (rule 10).
///
/// Bounded at [`MAX_TITLE_TEMPLATE`], and rejected when it references a name this item cannot bind:
/// its own `vars` keys, its `template-group` `var`, or a `__`-prefixed built-in the client supplies.
/// The host still expands NOTHING — the string is stored raw and interpolated client-side.
///
/// `label` is deliberately NOT checked here: it is retroactive (a stored label may carry a literal
/// `$` and the grammar has no escape), so a bad reference renders literally, exactly as today.
fn check_title_template(item: &NavItem) -> Result<(), NavError> {
    let Some(tpl) = item.title_template.as_deref() else {
        return Ok(());
    };
    if tpl.is_empty() || tpl.len() > MAX_TITLE_TEMPLATE {
        return Err(NavError::BadInput(format!(
            "nav item title template must be non-empty and ≤{MAX_TITLE_TEMPLATE} chars"
        )));
    }
    let mut bindable: Vec<&str> = item.vars.keys().map(String::as_str).collect();
    if !item.var.is_empty() {
        bindable.push(item.var.as_str());
    }
    if let Some(name) = template_refs::first_unbindable(tpl, &bindable) {
        return Err(NavError::BadInput(format!(
            "nav item title template references `{name}`, which the item cannot bind (not in its \
             `vars`, not its template-group `var`, not a built-in)"
        )));
    }
    Ok(())
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
    if item.icon.len() > MAX_ICON_LEN {
        return Err(NavError::BadInput(format!(
            "nav item icon name exceeds {MAX_ICON_LEN} chars"
        )));
    }
    if item.icon_color.len() > MAX_ICON_COLOR_LEN {
        return Err(NavError::BadInput(format!(
            "nav item icon color exceeds {MAX_ICON_COLOR_LEN} chars"
        )));
    }
    check_title_template(item)?;
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
