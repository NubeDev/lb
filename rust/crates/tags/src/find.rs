//! `tags.find(query)` — discovery over the tag graph. The CORE query modes (tags scope), each a
//! SurrealDB built-in, no scan:
//!   - **exact** `key:value` — entities with an edge to `tag:[key,value]`;
//!   - **key-only** ("has any `region`") — entities with any edge whose `key` matches;
//!   - **faceted intersection** ("eu-west AND telemetry") — entities with an edge to EVERY facet.
//!
//! Faceted intersection is a per-query graph traversal (combinatorial — NOT materializable; the
//! materialized `tag_counts` view is per-dimension only, never intersection). Returns the matching
//! entity references (`table:id` strings). Namespace-scoped. Raw verb — run after `caps::check`.

use lb_store::{Store, StoreError};
use serde_json::Value;

use crate::edge::TAGGED_TABLE;

/// One facet of a query: an exact `key=value`, or key-only when `value` is `None`.
#[derive(Debug, Clone, PartialEq)]
pub struct Facet {
    pub key: String,
    pub value: Option<Value>,
}

impl Facet {
    pub fn exact(key: impl Into<String>, value: Value) -> Self {
        Self {
            key: key.into(),
            value: Some(value),
        }
    }
    pub fn key_only(key: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            value: None,
        }
    }
}

/// Find the entity references in `ws` that match ALL `facets` (intersection). An empty `facets`
/// returns nothing (a query must constrain something). Each facet is exact (`key=value`) or
/// key-only (`key` present, any value).
pub async fn find(store: &Store, ws: &str, facets: &[Facet]) -> Result<Vec<String>, StoreError> {
    if facets.is_empty() {
        return Ok(Vec::new());
    }

    // Build the per-facet OR predicate, then require the entity to match all N facets:
    // GROUP BY the entity, count how many DISTINCT facets it matched, keep those that matched all.
    let mut preds = Vec::new();
    let mut bindings: Vec<(String, Value)> = Vec::new();
    for (i, f) in facets.iter().enumerate() {
        let kb = format!("k{i}");
        // Filter on the edge's denormalized tkey/tval (a RELATION drops literal key/value fields —
        // debugging/tags/relation-drops-key-value-fields.md).
        match &f.value {
            Some(v) => {
                let vb = format!("v{i}");
                preds.push(format!("(tkey = ${kb} AND tval = ${vb})"));
                bindings.push((kb, Value::String(f.key.clone())));
                bindings.push((vb, v.clone()));
            }
            None => {
                preds.push(format!("(tkey = ${kb})"));
                bindings.push((kb, Value::String(f.key.clone())));
            }
        }
    }
    let predicate = preds.join(" OR ");
    let n = facets.len();

    // `ent` is the raw entity string the caller passed at add time — returned verbatim for an exact
    // round-trip (`<string>in` would backtick-escape a dotted id like `series:`node.cpu_temp``). We
    // count DISTINCT (tkey,tval) matches per entity so duplicate-source edges don't over-count, then
    // keep entities that matched all N facets (the intersection).
    let sql = format!(
        "SELECT ent AS entity, count(array::distinct([tkey, tval])) AS m \
         FROM {TAGGED_TABLE} WHERE {predicate} \
         GROUP BY entity"
    );
    let mut resp = store.query_ws(ws, &sql, bindings).await?;
    let rows: Vec<MatchRow> = resp
        .take(0)
        .map_err(|e| StoreError::Decode(e.to_string()))?;
    Ok(rows
        .into_iter()
        .filter(|r| r.m as usize >= n)
        .map(|r| r.entity)
        .collect())
}

#[derive(serde::Deserialize)]
struct MatchRow {
    entity: String,
    m: i64,
}
