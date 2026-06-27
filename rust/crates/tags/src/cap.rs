//! The per-workspace **tag-node cap** — a required guardrail, not optional (tags scope). Tags are
//! for *dimensions you filter by* (`region`, `kind`, `unit`), never high-cardinality values (a raw
//! reading, a UUID — those belong in the payload). Ingest names cardinality explosion as its #1 risk
//! and uses tags as its ONLY discovery layer over heterogeneous payloads, so the primary consumer's
//! robustness depends on this cap holding.
//!
//! Policy (the resolved lean): **deny** a new tag node once the workspace is at its cap — a hard
//! stop is safer than a warn that lets the store bloat unbounded. Applying an *existing* tag (an
//! edge to a node already present) is always allowed; only creating a NEW distinct node counts.

use lb_store::{Store, StoreError};

use crate::tag::{Tag, TAG_TABLE};

/// The default per-workspace cap on distinct tag nodes. A real node folds this into config; the
/// slice fixes a sane default so the guardrail is always on.
pub const DEFAULT_TAG_NODE_CAP: usize = 10_000;

/// Raised when a new tag node would exceed the workspace cap. The caller maps this to a `Denied`
/// at the host boundary (the cap is policy, not an existence signal).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapExceeded {
    pub cap: usize,
}

/// Check whether creating `tag` in `ws` is allowed under `cap`. Returns `Ok(())` if the tag node
/// already exists (applying an existing node never counts) or the workspace is below the cap;
/// `Err(CapExceeded)` if a NEW node would push the count over the cap. A `cap` of 0 is unbounded.
pub async fn check_cap(
    store: &Store,
    ws: &str,
    tag: &Tag,
    cap: usize,
) -> Result<Result<(), CapExceeded>, StoreError> {
    if cap == 0 {
        return Ok(Ok(()));
    }
    if tag_exists(store, ws, tag).await? {
        return Ok(Ok(())); // re-using an existing node — never counts toward the cap.
    }
    let count = node_count(store, ws).await?;
    if count >= cap {
        return Ok(Err(CapExceeded { cap }));
    }
    Ok(Ok(()))
}

/// Does the `tag:[key,value]` node already exist in `ws`?
async fn tag_exists(store: &Store, ws: &str, tag: &Tag) -> Result<bool, StoreError> {
    let mut resp = store
        .query_ws(
            ws,
            &format!("SELECT count() FROM type::thing('{TAG_TABLE}', [$key, $value]) GROUP ALL"),
            vec![
                ("key".into(), serde_json::Value::String(tag.key.clone())),
                ("value".into(), tag.value.clone()),
            ],
        )
        .await?;
    let n: Option<i64> = resp
        .take("count")
        .map_err(|e| StoreError::Decode(e.to_string()))?;
    Ok(n.unwrap_or(0) > 0)
}

/// Count of distinct tag nodes in `ws` (workspace-partitioned — never another workspace's).
async fn node_count(store: &Store, ws: &str) -> Result<usize, StoreError> {
    let mut resp = store
        .query_ws(
            ws,
            &format!("SELECT count() FROM {TAG_TABLE} GROUP ALL"),
            vec![],
        )
        .await?;
    let n: Option<i64> = resp
        .take("count")
        .map_err(|e| StoreError::Decode(e.to_string()))?;
    Ok(n.unwrap_or(0).max(0) as usize)
}
