//! Stable row ids for a nav's items (nav-row-pins scope) — what a `nav:<navid>/<rowid>` pin addresses.
//!
//! `nav.save` runs [`assign_row_ids`] over the incoming tree before it writes:
//!   - a VALID id is kept, so a row the builder loaded and saved back keeps its pins;
//!   - a MISSING id is minted, so every new row is pinnable from its first save;
//!   - a DUPLICATE id is re-minted on every occurrence after the first (the builder's "duplicate row"
//!     copies the id — the original keeps its pins, the copy is a new row);
//!   - a MALFORMED id (wrong charset, over [`MAX_ROW_ID_LEN`]) is `BadInput`, never silently replaced —
//!     a caller that sends one has a bug worth hearing about.

use std::collections::HashSet;

use rand::Rng;

use super::error::NavError;
use super::model::{NavItem, MAX_ROW_ID_LEN};

/// Keep, mint, or re-mint every row id in `items` (all depths), in document order.
pub fn assign_row_ids(items: &mut [NavItem]) -> Result<(), NavError> {
    let mut seen = HashSet::new();
    walk(items, &mut seen)
}

fn walk(items: &mut [NavItem], seen: &mut HashSet<String>) -> Result<(), NavError> {
    for item in items {
        if !item.id.is_empty() {
            check_row_id(&item.id)?;
        }
        if item.id.is_empty() || !seen.insert(item.id.clone()) {
            item.id = mint_unique(seen);
        }
        walk(&mut item.items, seen)?;
    }
    Ok(())
}

/// Is `id` a legal row id? Also the parser's check on the `<rowid>` half of a pin ref.
pub fn check_row_id(id: &str) -> Result<(), NavError> {
    let ok = !id.is_empty()
        && id.len() <= MAX_ROW_ID_LEN
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-');
    if ok {
        Ok(())
    } else {
        Err(NavError::BadInput(format!(
            "nav row id {id:?} must be 1-{MAX_ROW_ID_LEN} chars of [A-Za-z0-9_-]"
        )))
    }
}

/// A fresh id not already in `seen` (and recorded there). 12 hex chars from the thread RNG — ids are
/// unique within ONE nav, where a collision is already vanishingly rare; the loop makes it impossible.
fn mint_unique(seen: &mut HashSet<String>) -> String {
    loop {
        let id = format!(
            "r{:012x}",
            rand::thread_rng().gen::<u64>() & 0xffff_ffff_ffff
        );
        if seen.insert(id.clone()) {
            return id;
        }
    }
}
