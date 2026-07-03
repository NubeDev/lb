//! The host tags verbs — authorize, then delegate to the raw `lb_tags` graph operations. Each runs
//! the MCP gate first (capability-first, §3.5); `add` also enforces the required per-workspace
//! tag-node cap and maps a cap hit to an opaque service error (tags scope).

use lb_auth::Principal;
use lb_store::Store;
use lb_tags::{
    add as tag_add, facet_values as tag_facet_values, find as tag_find, of as tag_of,
    remove as tag_remove, AddError, Applied, Facet, Provenance, Tag, DEFAULT_TAG_NODE_CAP,
};

use super::authorize::authorize_tags;
use super::error::TagsError;

/// `tags.add` — apply `tag` to `entity` with `provenance`, under the per-workspace tag-node cap.
pub async fn tags_add(
    store: &Store,
    principal: &Principal,
    ws: &str,
    entity: &str,
    tag: &Tag,
    provenance: &Provenance,
) -> Result<(), TagsError> {
    authorize_tags(principal, ws, "tags.add")?;
    match tag_add(store, ws, entity, tag, provenance, DEFAULT_TAG_NODE_CAP).await {
        Ok(()) => Ok(()),
        Err(AddError::CapExceeded(c)) => Err(TagsError::CapExceeded(c.cap)),
        Err(AddError::Store(e)) => Err(TagsError::Store(e)),
    }
}

/// `tags.remove` — drop `entity`'s edges for `key` (and `value` if given).
pub async fn tags_remove(
    store: &Store,
    principal: &Principal,
    ws: &str,
    entity: &str,
    key: &str,
    value: Option<&serde_json::Value>,
) -> Result<(), TagsError> {
    authorize_tags(principal, ws, "tags.remove")?;
    Ok(tag_remove(store, ws, entity, key, value).await?)
}

/// `tags.of` — every tag applied to `entity`, with provenance.
pub async fn tags_of(
    store: &Store,
    principal: &Principal,
    ws: &str,
    entity: &str,
) -> Result<Vec<Applied>, TagsError> {
    authorize_tags(principal, ws, "tags.of")?;
    Ok(tag_of(store, ws, entity).await?)
}

/// `tags.find` — the entity references matching ALL `facets` (exact / key-only / faceted).
pub async fn tags_find(
    store: &Store,
    principal: &Principal,
    ws: &str,
    facets: &[Facet],
) -> Result<Vec<String>, TagsError> {
    authorize_tags(principal, ws, "tags.find")?;
    Ok(tag_find(store, ws, facets).await?)
}

/// The **distinct values** present for tag `key` in `ws` (reusable-pages template-group fan-out).
/// Gated on the SAME `tags.find` cap — enumerating a key's values is the read privilege of finding
/// by it, so no new cap is minted (reusable-pages scope: "no new caps"). A caller lacking `tags.find`
/// is denied, so the template-group entry is stripped without leaking any option value (the lens).
pub async fn tags_facet_values(
    store: &Store,
    principal: &Principal,
    ws: &str,
    key: &str,
) -> Result<Vec<serde_json::Value>, TagsError> {
    authorize_tags(principal, ws, "tags.find")?;
    Ok(tag_facet_values(store, ws, key).await?)
}
