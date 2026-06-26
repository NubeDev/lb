//! SPIKE-GATED ADD-ON — value **full-text** over tag values (`DEFINE ANALYZER` + `DEFINE INDEX …
//! SEARCH BM25`). The store spike marked `SEARCH` **available** on SurrealKV, so this ships (tags
//! scope). A ✗ would have deferred it; the core graph + exact/facet are unaffected either way.
//!
//! `define_text_index` is idempotent (run once per workspace at first use); `find_text` matches the
//! tokenized/fuzzy value. Namespace-scoped. Raw verbs — run after `caps::check`.

use lb_store::{Store, StoreError};
use serde_json::Value;

use crate::tag::TAG_TABLE;

/// Define the analyzer + BM25 full-text index on `tag.value` in `ws`. Idempotent (`IF NOT EXISTS`),
/// so a node calls it once before the first `find_text`.
pub async fn define_text_index(store: &Store, ws: &str) -> Result<(), StoreError> {
    store
        .query_ws(
            ws,
            &format!(
                "DEFINE ANALYZER IF NOT EXISTS tag_simple TOKENIZERS blank FILTERS lowercase;
                 DEFINE INDEX IF NOT EXISTS tag_value_text ON TABLE {TAG_TABLE} \
                    FIELDS value SEARCH ANALYZER tag_simple BM25;"
            ),
            vec![],
        )
        .await?;
    Ok(())
}

/// Full-text search tag values in `ws` for `text` via the BM25 index. Returns the matching
/// `[key, value]` pairs (the tag nodes). Requires `define_text_index` to have run.
pub async fn find_text(store: &Store, ws: &str, text: &str) -> Result<Vec<(String, Value)>, StoreError> {
    let mut resp = store
        .query_ws(
            ws,
            &format!("SELECT key, value FROM {TAG_TABLE} WHERE value @@ $text"),
            vec![("text".into(), Value::String(text.to_string()))],
        )
        .await?;
    let rows: Vec<TextRow> = resp.take(0).map_err(|e| StoreError::Decode(e.to_string()))?;
    Ok(rows.into_iter().map(|r| (r.key, r.value)).collect())
}

#[derive(serde::Deserialize)]
struct TextRow {
    key: String,
    value: Value,
}
