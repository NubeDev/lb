//! `tags.remove(entity, key, value?)` — remove a tag application (an edge), not the shared node.
//! Removing `value?`-omitted drops every edge from `entity` for that `key` (all values); with a
//! value, drops the specific `(entity, key, value)` edges across all sources (tags scope).
//!
//! The shared `tag` node is left in place — other entities may still point at it, and the cap counts
//! nodes, not edges. Namespace-scoped. Raw verb — run after `caps::check`.

use lb_store::{Store, StoreError};
use serde_json::Value;

use crate::edge::TAGGED_TABLE;

/// Remove `entity`'s edges for `key` (and `value` if given) in `ws`. Drops all source variants of
/// the matched edges. The tag node is untouched.
pub async fn remove(
    store: &Store,
    ws: &str,
    entity: &str,
    key: &str,
    value: Option<&Value>,
) -> Result<(), StoreError> {
    // Filter on the edge's denormalized tkey/tval (the RELATION drops literal key/value fields —
    // debugging/tags/relation-drops-key-value-fields.md).
    let mut where_clause = String::from("in = type::thing($entity) AND tkey = $key");
    let mut bindings: Vec<(String, Value)> = vec![
        ("entity".into(), Value::String(entity.to_string())),
        ("key".into(), Value::String(key.to_string())),
    ];
    if let Some(v) = value {
        where_clause.push_str(" AND tval = $value");
        bindings.push(("value".into(), v.clone()));
    }
    store
        .query_ws(
            ws,
            &format!("DELETE {TAGGED_TABLE} WHERE {where_clause}"),
            bindings,
        )
        .await?;
    Ok(())
}
