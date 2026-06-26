//! `tags.of(entity)` — every tag applied to one entity, with its provenance. A graph traversal over
//! the entity's outgoing `tagged` edges (tags scope) — no scan. Returns one row per edge, so a tag
//! asserted by two sources appears twice (each attribution preserved).
//!
//! Namespace-scoped (the hard wall). Raw verb — run after `caps::check`.

use lb_store::{Store, StoreError};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::edge::{Source, TAGGED_TABLE};

/// One tag application on an entity: the typed tag plus its provenance.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Applied {
    pub key: String,
    pub value: Value,
    pub at: u64,
    pub by: String,
    pub source: Source,
    pub confidence: f64,
    pub expires: Option<u64>,
}

/// Every tag applied to `entity` in `ws`, one row per `(tag, source)` edge.
pub async fn of(store: &Store, ws: &str, entity: &str) -> Result<Vec<Applied>, StoreError> {
    let mut resp = store
        .query_ws(
            ws,
            // tkey/tval are the edge's denormalized tag key/value (a RELATION drops fields literally
            // named key/value — debugging/tags/relation-drops-key-value-fields.md); alias them back.
            &format!(
                "SELECT tkey AS key, tval AS value, at, by, source, confidence, expires \
                 FROM {TAGGED_TABLE} WHERE in = type::thing($entity)"
            ),
            vec![("entity".into(), Value::String(entity.to_string()))],
        )
        .await?;
    let rows: Vec<Applied> = resp.take(0).map_err(|e| StoreError::Decode(e.to_string()))?;
    Ok(rows)
}
