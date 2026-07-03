//! Nav record bounds (nav scope, "Resolution cost" / the item-cap open question). The host is the
//! authority — it rejects an over-cap `items[]` rather than silently storing it unbounded, so the
//! resolver stays cheap. The builder mirrors these caps for a friendly error, but this is the
//! boundary. Two rules: the total item count (top-level + nested) is capped at [`MAX_ITEMS`], and
//! nesting is exactly one level (a `group`'s children must not themselves be `group`s).

use super::error::NavError;
use super::model::{NavItem, MAX_ITEMS};

/// The item kinds a nav may hold. A `group` nests one level of the non-group kinds.
const KINDS: &[&str] = &["surface", "dashboard", "ext", "tag-group", "group"];

/// Reject a nav whose `items[]` exceeds the caps: total count over [`MAX_ITEMS`], an unknown kind, or
/// a `group` nested inside a `group` (only one nesting level — nav scope, "No deep nesting").
pub fn check_items(items: &[NavItem]) -> Result<(), NavError> {
    let total = count(items);
    if total > MAX_ITEMS {
        return Err(NavError::BadInput(format!(
            "nav has {total} items, exceeds cap {MAX_ITEMS}"
        )));
    }
    for item in items {
        check_item(item, false)?;
    }
    Ok(())
}

/// Total item count including one level of nesting.
fn count(items: &[NavItem]) -> usize {
    items
        .iter()
        .map(|i| 1 + if i.kind == "group" { i.items.len() } else { 0 })
        .sum()
}

/// Validate one item (`nested` = it is a `group`'s child). A nested `group` is rejected (one level
/// only); an unknown kind is rejected.
fn check_item(item: &NavItem, nested: bool) -> Result<(), NavError> {
    if !KINDS.contains(&item.kind.as_str()) {
        return Err(NavError::BadInput(format!(
            "unknown nav item kind: {}",
            item.kind
        )));
    }
    if item.kind == "group" {
        if nested {
            return Err(NavError::BadInput(
                "nav groups may not nest (one level only)".into(),
            ));
        }
        for child in &item.items {
            check_item(child, true)?;
        }
    }
    Ok(())
}
