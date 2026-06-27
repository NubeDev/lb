//! SPIKE-GATED ADD-ON — **vector** semantic search over a tag's embedding (`DEFINE INDEX … HNSW`).
//! The store spike marked `HNSW` **available** on SurrealKV, so this ships (tags scope). Embeddings
//! are **caller-supplied** (no model in core); `lb-tags` stores and indexes them.
//!
//! **Dimension is pinned at index-definition time and a mismatched-dim write is REJECTED**, never
//! stored — two callers (or two model versions) supplying different-dim vectors to one workspace's
//! vector tag would otherwise corrupt the index (a correctness bug, not a tuning concern). The
//! dimension is declared per vector-tag `key` (the resolved lean) so different embedding spaces don't
//! collide. Namespace-scoped. Raw verbs — run after `caps::check`.

use lb_store::{Store, StoreError};
use serde_json::{json, Value};

/// A workspace-scoped table holding one embedding per `(key, id)` — kept separate from the scalar
/// `tag` node table so most tags stay cheap and only opted-in keys carry a vector.
pub const VECTOR_TABLE: &str = "tag_vector";

/// Raised when a vector's length does not match the dimension pinned for its key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DimMismatch {
    pub expected: usize,
    pub got: usize,
}

/// Define the HNSW index for vector tags of dimension `dim` in `ws`. Idempotent. The index pins the
/// dimension; callers must supply exactly-`dim`-length embeddings thereafter.
pub async fn define_vector_index(store: &Store, ws: &str, dim: usize) -> Result<(), StoreError> {
    store
        .query_ws(
            ws,
            &format!(
                "DEFINE INDEX IF NOT EXISTS tag_vec_hnsw ON TABLE {VECTOR_TABLE} \
                 FIELDS embedding HNSW DIMENSION {dim} DIST COSINE;"
            ),
            vec![],
        )
        .await?;
    Ok(())
}

/// Store an embedding for vector-tag `(key, id)` in `ws`, after checking it matches `dim`. A
/// mismatched length is rejected (`Err(Ok)`-style: `Ok(Err(DimMismatch))`), never written.
pub async fn put_vector(
    store: &Store,
    ws: &str,
    key: &str,
    id: &str,
    embedding: &[f64],
    dim: usize,
) -> Result<Result<(), DimMismatch>, StoreError> {
    if embedding.len() != dim {
        return Ok(Err(DimMismatch {
            expected: dim,
            got: embedding.len(),
        }));
    }
    store
        .query_ws(
            ws,
            &format!(
                "UPSERT type::thing('{VECTOR_TABLE}', [$key, $id]) \
                 SET key = $key, vid = $id, embedding = $emb"
            ),
            vec![
                ("key".into(), Value::String(key.to_string())),
                ("id".into(), Value::String(id.to_string())),
                ("emb".into(), json!(embedding)),
            ],
        )
        .await?;
    Ok(Ok(()))
}

/// Nearest-neighbour search: the `k` vector-tag ids in `ws` closest to `query` under the HNSW index.
pub async fn find_similar(
    store: &Store,
    ws: &str,
    query: &[f64],
    k: usize,
) -> Result<Vec<String>, StoreError> {
    // HNSW uses the two-arg knn operator `<|K,EF|>` (the single-arg form returns nothing on this
    // engine — debugging/tags/hnsw-knn-needs-ef-arg.md); EF is the search breadth. Results come back
    // ordered by ascending distance, so no ORDER BY is needed. We return the caller's logical `vid`.
    let ef = (k * 4).max(40);
    let mut resp = store
        .query_ws(
            ws,
            &format!("SELECT vid FROM {VECTOR_TABLE} WHERE embedding <|{k},{ef}|> $q"),
            vec![("q".into(), json!(query))],
        )
        .await?;
    let rows: Vec<IdRow> = resp
        .take(0)
        .map_err(|e| StoreError::Decode(e.to_string()))?;
    Ok(rows.into_iter().map(|r| r.vid).collect())
}

#[derive(serde::Deserialize)]
struct IdRow {
    vid: String,
}
