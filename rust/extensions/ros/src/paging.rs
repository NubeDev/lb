//! The keyset `{items, next_cursor}` envelope for the `*.list` verbs (resource-verbs grammar). There
//! is no shared `Page<T>` helper in the platform (the scope notes it is hand-rolled per handler); this
//! is the ros extension's one implementation, kept in one file so every `list` verb pages identically.
//!
//! Keyset, not offset: the cursor is the last id returned, and the next page is "ids strictly greater
//! than the cursor" (stable under concurrent inserts, unlike an offset). Items are sorted by their
//! stable id (the ROS uuid) before the window is taken, so the ordering the cursor walks is total.

use serde::Serialize;
use serde_json::{json, Value};

/// The default page size when the caller does not specify one — bounded so a `list` never returns an
/// unbounded firehose (the config tree is low-cardinality, but the envelope is uniform).
pub const DEFAULT_LIMIT: usize = 100;

/// Take one keyset page from `all` (already the full candidate set for this parent). `cursor` is the
/// id AFTER which to start (exclusive); `limit` caps the page. Returns the page items plus the
/// `next_cursor` (the last id of this page) when a further page exists, or `null` when exhausted.
///
/// `id_of` extracts the stable ordering key (the uuid) from an item. The result envelope is
/// `{items:[…], next_cursor: <id>|null}` — the exact resource-verbs list shape.
pub fn keyset_page<T, F>(mut all: Vec<T>, cursor: Option<&str>, limit: usize, id_of: F) -> Value
where
    T: Serialize,
    F: Fn(&T) -> String,
{
    // Total order on the stable id so the cursor walk is deterministic across calls.
    all.sort_by(|a, b| id_of(a).cmp(&id_of(b)));

    let start = match cursor {
        // First id strictly greater than the cursor (keyset: exclusive of the cursor itself).
        Some(c) => all
            .iter()
            .position(|it| id_of(it).as_str() > c)
            .unwrap_or(all.len()),
        None => 0,
    };

    let limit = limit.max(1);
    let window: Vec<&T> = all.iter().skip(start).take(limit).collect();
    // A next page exists iff there are items beyond this window.
    let has_more = start + window.len() < all.len();
    let next_cursor = if has_more {
        window.last().map(|it| id_of(it))
    } else {
        None
    };

    json!({
        "items": window,
        "next_cursor": next_cursor,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(page: &Value) -> Vec<String> {
        page["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().to_string())
            .collect()
    }

    #[test]
    fn pages_walk_the_whole_set_without_gaps_or_repeats() {
        let all: Vec<String> = vec!["c", "a", "e", "b", "d"]
            .into_iter()
            .map(String::from)
            .collect();

        let p1 = keyset_page(all.clone(), None, 2, |s| s.clone());
        assert_eq!(ids(&p1), vec!["a", "b"]);
        assert_eq!(p1["next_cursor"], "b");

        let p2 = keyset_page(all.clone(), Some("b"), 2, |s| s.clone());
        assert_eq!(ids(&p2), vec!["c", "d"]);
        assert_eq!(p2["next_cursor"], "d");

        let p3 = keyset_page(all, Some("d"), 2, |s| s.clone());
        assert_eq!(ids(&p3), vec!["e"]);
        assert_eq!(p3["next_cursor"], Value::Null);
    }

    #[test]
    fn empty_set_is_an_empty_page_with_no_cursor() {
        let page = keyset_page(Vec::<String>::new(), None, 10, |s| s.clone());
        assert!(ids(&page).is_empty());
        assert_eq!(page["next_cursor"], Value::Null);
    }
}
