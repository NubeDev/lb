//! `tags.of(entity)` — every tag applied to one entity, with its provenance. A graph traversal over
//! the entity's outgoing `tagged` edges (tags scope) — no scan. Returns one row per edge, so a tag
//! asserted by two sources appears twice (each attribution preserved).
//!
//! Namespace-scoped (the hard wall). Raw verb — run after `caps::check`.

use lb_store::{Store, StoreError};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::edge::{Source, TAGGED_TABLE};
use crate::entity::entity_parts;

/// One tag application on an entity: the typed tag plus its provenance.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Applied {
    pub key: String,
    pub value: Value,
    pub at: u64,
    pub by: String,
    pub source: Source,
    pub confidence: f64,
    /// SurrealDB hands a NULL/NONE column to serde as a unit, which plain `Option<u64>` rejects.
    #[serde(default, deserialize_with = "lb_store::null_as_none")]
    pub expires: Option<u64>,
}

// Delegated, NOT `#[derive(SurrealValue)]`. `tag.add` binds its columns as JSON, so an absent
// `expires` is stored as SQL NULL -- and SurrealDB separates NULL from NONE, so the derive's
// `Option<u64>` refused it ("Expected number, got null"). serde treats null as `None`, which is what
// wrote the row in the first place.
lb_store::surreal_value_via_serde!(Applied);

/// Every tag applied to `entity` in `ws`, one row per `(tag, source)` edge.
pub async fn of(store: &Store, ws: &str, entity: &str) -> Result<Vec<Applied>, StoreError> {
    let mut resp = store
        .query_ws(
            ws,
            // tkey/tval are the edge's denormalized tag key/value (a RELATION drops fields literally
            // named key/value — debugging/tags/relation-drops-key-value-fields.md); alias them back.
            // The entity link is built two-arg (dotted ids — debugging/tags/dotted-entity-id-needs-two-arg.md).
            &format!(
                "SELECT tkey AS key, tval AS value, at, by, source, confidence, expires \
                 FROM {TAGGED_TABLE} WHERE in = type::record($etb, $eid)"
            ),
            {
                let (etb, eid) = entity_parts(entity);
                vec![
                    ("etb".into(), Value::String(etb.to_string())),
                    ("eid".into(), Value::String(eid.to_string())),
                ]
            },
        )
        .await?;
    let rows: Vec<Applied> = resp
        .take(0)
        .map_err(|e| StoreError::Decode(e.to_string()))?;
    Ok(rows)
}
