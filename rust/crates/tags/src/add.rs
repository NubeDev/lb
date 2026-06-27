//! `tags.add(entity, key, value, meta?)` — apply a tag. The CORE verb (tags scope):
//!   1. enforce the per-workspace tag-node cap (a new node past the cap is denied);
//!   2. UPSERT the shared `tag:[key,value]` node (deterministic, deduplicated — constructed);
//!   3. RELATE `entity -> tagged -> tag` with a **deterministic edge id keyed on
//!      `(entity, tag, source)`**, so a same-source re-tag upserts in place (idempotent,
//!      `by`/`confidence`/`expires` mutable) while a DIFFERENT source coexists as a distinct edge.
//!
//! All three statements run in ONE transaction so the node + edge land together. Namespace-scoped
//! (the hard wall). Raw verb — run after `caps::check`.

use lb_store::{Store, StoreError};
use serde_json::{json, Value};

use crate::cap::{check_cap, CapExceeded};
use crate::edge::{Provenance, TAGGED_TABLE};
use crate::entity::entity_parts;
use crate::tag::{Tag, TAG_TABLE};

/// The error of an add: a store failure, or the workspace tag-node cap was hit.
#[derive(Debug)]
pub enum AddError {
    Store(StoreError),
    CapExceeded(CapExceeded),
}

impl From<StoreError> for AddError {
    fn from(e: StoreError) -> Self {
        AddError::Store(e)
    }
}

/// Apply `tag` to `entity` (a `table:id` record reference, e.g. `series:cpu`) in `ws` with the given
/// `provenance`, under `cap` (max distinct tag nodes). Idempotent on `(entity, tag, source)`.
pub async fn add(
    store: &Store,
    ws: &str,
    entity: &str,
    tag: &Tag,
    provenance: &Provenance,
    cap: usize,
) -> Result<(), AddError> {
    if let Err(exceeded) = check_cap(store, ws, tag, cap).await? {
        return Err(AddError::CapExceeded(exceeded));
    }

    // Deterministic edge id = [entity, key, value, source] → same-source re-tag upserts; different
    // source is a distinct edge. The entity is a record reference passed as `type::thing`.
    //
    // The tag's key/value are denormalized onto the edge as `tkey`/`tval` (NOT `key`/`value`):
    // a RELATION row carrying `in`/`out` silently drops user fields named `key`/`value`
    // (debugging/tags/relation-drops-key-value-fields.md) — so they are renamed to persist + be
    // filterable without a node hop. The shared `tag` NODE still uses `key`/`value` (no in/out there).
    let sql = format!(
        "BEGIN TRANSACTION;
         UPSERT type::thing('{TAG_TABLE}', [$key, $value]) SET key = $key, value = $value;
         UPSERT type::thing('{TAGGED_TABLE}', [$entity, $key, $value, $source]) SET
            in = type::thing($etb, $eid),
            out = type::thing('{TAG_TABLE}', [$key, $value]),
            ent = $entity, tkey = $key, tval = $value,
            at = $at, by = $by, source = $source, confidence = $confidence, expires = $expires;
         COMMIT TRANSACTION;"
    );
    let (etb, eid) = entity_parts(entity);
    store
        .query_ws(
            ws,
            &sql,
            vec![
                ("entity".into(), Value::String(entity.to_string())),
                ("etb".into(), Value::String(etb.to_string())),
                ("eid".into(), Value::String(eid.to_string())),
                ("key".into(), Value::String(tag.key.clone())),
                ("value".into(), tag.value.clone()),
                ("source".into(), json!(provenance.source.as_str())),
                ("at".into(), json!(provenance.at)),
                ("by".into(), Value::String(provenance.by.clone())),
                ("confidence".into(), json!(provenance.confidence)),
                ("expires".into(), json!(provenance.expires)),
            ],
        )
        .await?;
    Ok(())
}
